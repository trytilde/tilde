//! The one place list prices become cost. Rates come from `inference_prices` (maintained by
//! `inference::prices`), keyed by Tilde provider id and model; the same arithmetic prices an
//! inference request captured at the gateway and a model span captured in a trace.
use crate::database::{DbResult, GenericClient};
use opentelemetry_proto::tonic::{
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::ResourceSpans,
};
use std::collections::HashMap;

/// Micro-dollars per million tokens, or per unit for models billed without tokens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rate {
    pub provider_id: String,
    pub input_per_m_micros: Option<i64>,
    pub output_per_m_micros: Option<i64>,
    pub cached_input_per_m_micros: Option<i64>,
    pub cache_write_per_m_micros: Option<i64>,
    pub image_micros: Option<i64>,
    pub character_per_m_micros: Option<i64>,
    pub second_micros: Option<i64>,
}
/// What a request or span consumed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Usage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub images: Option<i64>,
    pub characters: Option<i64>,
    pub seconds: Option<i64>,
}
impl Rate {
    /// Base tier only: the response does not say which tier served it. Cached reads are a subset
    /// of input for OpenAI-style usage and additive for Anthropic and Bedrock, where cache writes
    /// are reported separately and billed at a premium. Missing cache rates fall back to the
    /// input rate. Models billed per image, character or second use that unit when the sheet
    /// prices it; otherwise the tokens the response reported.
    pub fn cost_micros(&self, usage: Usage) -> Option<i64> {
        let per_unit = |count: Option<i64>, rate: Option<i64>| Some(count?.max(0) * rate?);
        let per_million = |count: Option<i64>, rate: Option<i64>| {
            Some((count?.max(0) as f64 * rate? as f64 / 1_000_000.).round() as i64)
        };
        if let Some(cost) = per_unit(usage.images, self.image_micros)
            .or_else(|| per_million(usage.characters, self.character_per_m_micros))
            .or_else(|| per_unit(usage.seconds, self.second_micros))
        {
            return Some(cost);
        }
        let input = usage.input_tokens?.max(0);
        let cached = usage.cached_input_tokens.unwrap_or(0).max(0);
        let written = usage.cache_write_tokens.unwrap_or(0).max(0);
        let output = usage.output_tokens.unwrap_or(0).max(0);
        let additive_cache = matches!(self.provider_id.as_str(), "anthropic" | "bedrock");
        let uncached = if additive_cache {
            input
        } else {
            (input - cached).max(0)
        };
        let micros = uncached as f64 * self.input_per_m_micros.unwrap_or(0) as f64
            + cached as f64
                * self
                    .cached_input_per_m_micros
                    .or(self.input_per_m_micros)
                    .unwrap_or(0) as f64
            + written as f64
                * self
                    .cache_write_per_m_micros
                    .or(self.input_per_m_micros)
                    .unwrap_or(0) as f64
            + output as f64 * self.output_per_m_micros.unwrap_or(0) as f64;
        Some((micros / 1_000_000.).round() as i64)
    }
}
fn rate_of(r: crate::inference::db::PriceRow) -> Rate {
    Rate {
        provider_id: r.provider_id,
        input_per_m_micros: r.input_per_m_micros,
        output_per_m_micros: r.output_per_m_micros,
        cached_input_per_m_micros: r.cached_input_per_m_micros,
        cache_write_per_m_micros: r.cache_write_per_m_micros,
        image_micros: r.image_micros,
        character_per_m_micros: r.character_per_m_micros,
        second_micros: r.second_micros,
    }
}
/// The rate for a model. Without a provider, whichever provider lists the model.
pub async fn rate(
    db: &impl GenericClient,
    provider: Option<&str>,
    model: &str,
) -> DbResult<Option<Rate>> {
    let model = model.trim();
    if model.is_empty() {
        return Ok(None);
    }
    Ok(match provider.filter(|p| !p.is_empty()) {
        Some(provider) => crate::inference::db::price_get_opt(db, provider, model)
            .await?
            .map(rate_of),
        None => crate::inference::db::price_find_opt(db, model)
            .await?
            .map(rate_of),
    })
}
/// The Tilde provider id for the provider an instrumented span names: OpenTelemetry's
/// `gen_ai.provider.name`/`gen_ai.system`, or the AI SDK's `ai.model.provider` (`openai.chat`).
pub fn provider_id(reported: &str) -> Option<&'static str> {
    let name = reported
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    Some(match name.as_str() {
        "openai" => "openai",
        "anthropic" => "anthropic",
        "azure" | "azure_openai" | "az" => "azure_openai",
        "google" | "gemini" | "google_ai" | "gcp" => "google_ai",
        "aws" | "amazon-bedrock" | "bedrock" | "amazon" => "bedrock",
        "cohere" => "cohere",
        "voyage" => "voyage",
        "cerebras" => "cerebras",
        "baseten" => "baseten",
        "vercel" | "vercel_ai_gateway" => "vercel_ai_gateway",
        _ => return None,
    })
}
fn text(attributes: &[KeyValue], keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        attributes
            .iter()
            .find(|a| a.key == *key)
            .and_then(|a| a.value.as_ref())
            .and_then(|v| match &v.value {
                Some(Value::StringValue(s)) if !s.is_empty() => Some(s.clone()),
                Some(Value::IntValue(n)) => Some(n.to_string()),
                Some(Value::DoubleValue(n)) => Some(n.to_string()),
                _ => None,
            })
    })
}
fn count(attributes: &[KeyValue], keys: &[&str]) -> Option<i64> {
    text(attributes, keys).and_then(|v| v.parse::<f64>().ok().map(|n| n.round() as i64))
}
/// Stamp `tilde.observation.cost_usd` on every model span the price table covers, before
/// the batch is queued. Prices are read once per distinct model in the batch.
pub async fn price_spans(db: &impl GenericClient, resources: &mut [ResourceSpans]) -> DbResult<()> {
    let mut rates: HashMap<(Option<String>, String), Option<Rate>> = HashMap::new();
    for span in resources
        .iter_mut()
        .flat_map(|r| &mut r.scope_spans)
        .flat_map(|s| &mut s.spans)
    {
        let attributes = &span.attributes;
        if text(attributes, &["tilde.observation.type"]).as_deref() != Some("generation")
            || text(attributes, &["tilde.observation.cost_usd"]).is_some()
        {
            continue;
        }
        let Some(model) = text(attributes, &["tilde.observation.model"]) else {
            continue;
        };
        let provider = text(
            attributes,
            &["gen_ai.provider.name", "gen_ai.system", "ai.model.provider"],
        )
        .and_then(|p| provider_id(&p))
        .map(str::to_owned);
        let key = (provider, model);
        if !rates.contains_key(&key) {
            let found = rate(db, key.0.as_deref(), &key.1).await?;
            rates.insert(key.clone(), found);
        }
        let Some(rate) = rates[&key].as_ref() else {
            continue;
        };
        let usage = Usage {
            input_tokens: count(attributes, &["gen_ai.usage.input_tokens"]),
            output_tokens: count(attributes, &["gen_ai.usage.output_tokens"]),
            cached_input_tokens: count(
                attributes,
                &[
                    "gen_ai.usage.cache_read.input_tokens",
                    "gen_ai.usage.cached_input_tokens",
                    "ai.usage.cachedInputTokens",
                ],
            ),
            cache_write_tokens: count(attributes, &["gen_ai.usage.cache_creation.input_tokens"]),
            ..Default::default()
        };
        if let Some(micros) = rate.cost_micros(usage) {
            for (key, value) in [
                (
                    "tilde.observation.cost_usd",
                    format!("{:.6}", micros as f64 / 1e6),
                ),
                ("tilde.observation.provider", rate.provider_id.clone()),
            ] {
                span.attributes.retain(|a| a.key != key);
                span.attributes.push(KeyValue {
                    key: key.into(),
                    value: Some(AnyValue {
                        value: Some(Value::StringValue(value)),
                    }),
                    ..Default::default()
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_reads_are_a_subset_for_openai_and_additive_for_anthropic() {
        let rate = |provider: &str| Rate {
            provider_id: provider.into(),
            input_per_m_micros: Some(1_000_000),
            output_per_m_micros: Some(2_000_000),
            cached_input_per_m_micros: Some(100_000),
            cache_write_per_m_micros: None,
            ..Default::default()
        };
        let usage = Usage {
            input_tokens: Some(1_000_000),
            output_tokens: Some(1_000_000),
            cached_input_tokens: Some(500_000),
            ..Default::default()
        };
        assert_eq!(
            rate("openai").cost_micros(usage),
            Some(500_000 + 50_000 + 2_000_000)
        );
        assert_eq!(
            rate("anthropic").cost_micros(usage),
            Some(1_000_000 + 50_000 + 2_000_000)
        );
        // Per-unit billing wins over tokens when the sheet prices the unit.
        let image = Rate {
            image_micros: Some(40_000),
            ..rate("openai")
        };
        assert_eq!(
            image.cost_micros(Usage {
                images: Some(3),
                ..usage
            }),
            Some(120_000)
        );
        assert_eq!(rate("openai").cost_micros(Usage::default()), None);
        assert_eq!(provider_id("openai.chat"), Some("openai"));
        assert_eq!(provider_id("amazon-bedrock"), Some("bedrock"));
        assert_eq!(provider_id("acme"), None);
    }
}
