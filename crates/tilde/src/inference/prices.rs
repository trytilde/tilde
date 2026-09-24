//! Token prices in micro-dollars per million tokens. No provider publishes list prices as an
//! API, so the source is LiteLLM's community-maintained sheet. A trimmed copy is vendored for
//! the first start and for air-gapped installs (`scripts/update-inference-prices.py`); a daily
//! worker then refreshes from the live sheet so price changes land without a release. Both
//! paths upsert rows with source `litellm`; rows an operator edited (`manual`) are untouched.
//! Costs are computed by Postgres at insert time from this table, so a re-price is one UPDATE.
use crate::{database::Pool, error::Error};
use serde::Deserialize;
use std::time::Duration;

pub const DEFAULT_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
const REFRESH: Duration = Duration::from_secs(24 * 60 * 60);
/// Tilde provider id, the LiteLLM provider labels whose rows apply, and the key prefix to strip.
const PROVIDERS: &[(&str, &[&str], &str)] = &[
    ("openai", &["openai"], ""),
    ("anthropic", &["anthropic"], ""),
    ("azure_openai", &["azure"], "azure/"),
    ("azure_ai", &["azure_ai"], "azure_ai/"),
    ("google_ai", &["gemini"], "gemini/"),
    (
        "bedrock",
        &["bedrock", "bedrock_converse", "bedrock_mantle"],
        "bedrock/",
    ),
    ("baseten", &["baseten"], "baseten/"),
    ("cerebras", &["cerebras"], "cerebras/"),
    (
        "vercel_ai_gateway",
        &["vercel_ai_gateway"],
        "vercel_ai_gateway/",
    ),
    ("cohere", &["cohere", "cohere_chat"], "cohere/"),
    ("voyage", &["voyage"], "voyage/"),
];

#[derive(Deserialize)]
pub struct Row {
    pub provider_id: String,
    pub model: String,
    pub input_per_m_micros: Option<i64>,
    pub output_per_m_micros: Option<i64>,
    pub cached_input_per_m_micros: Option<i64>,
    pub cache_write_per_m_micros: Option<i64>,
    /// Micro-dollars per generated image.
    pub image_micros: Option<i64>,
    /// Micro-dollars per million input characters (speech synthesis).
    pub character_per_m_micros: Option<i64>,
    /// Micro-dollars per second of audio (transcription).
    pub second_micros: Option<i64>,
}
#[derive(Deserialize)]
struct Entry {
    litellm_provider: Option<String>,
    input_cost_per_token: Option<f64>,
    output_cost_per_token: Option<f64>,
    cache_read_input_token_cost: Option<f64>,
    cache_creation_input_token_cost: Option<f64>,
    output_cost_per_image: Option<f64>,
    input_cost_per_character: Option<f64>,
    input_cost_per_second: Option<f64>,
}
fn per_million_micros(per_token: Option<f64>) -> Option<i64> {
    per_token.map(|p| (p * 1_000_000.0 * 1_000_000.0).round() as i64)
}
/// The same trimming `scripts/update-inference-prices.py` applies to the vendored copy.
pub fn from_litellm(json: &str) -> Result<Vec<Row>, Error> {
    let table: std::collections::BTreeMap<String, serde_json::Value> =
        serde_json::from_str(json)
            .map_err(|_| Error::Invalid("Price sheet is not a JSON object".into()))?;
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (key, value) in table {
        let Ok(entry) = serde_json::from_value::<Entry>(value) else {
            continue;
        };
        // Any billable unit qualifies: tokens, images, characters or audio seconds.
        if entry.input_cost_per_token.is_none()
            && entry.output_cost_per_image.is_none()
            && entry.input_cost_per_character.is_none()
            && entry.input_cost_per_second.is_none()
        {
            continue;
        }
        let Some(label) = entry.litellm_provider.as_deref() else {
            continue;
        };
        for (provider, labels, prefix) in PROVIDERS {
            if !labels.contains(&label) {
                continue;
            }
            let model = key.strip_prefix(prefix).unwrap_or(&key);
            if (model.contains('/') && *provider != "bedrock")
                || !seen.insert((*provider, model.to_owned()))
            {
                continue;
            }
            rows.push(Row {
                provider_id: (*provider).into(),
                model: model.into(),
                input_per_m_micros: per_million_micros(entry.input_cost_per_token),
                output_per_m_micros: per_million_micros(entry.output_cost_per_token),
                cached_input_per_m_micros: per_million_micros(entry.cache_read_input_token_cost),
                cache_write_per_m_micros: per_million_micros(entry.cache_creation_input_token_cost),
                image_micros: entry
                    .output_cost_per_image
                    .map(|p| (p * 1_000_000.0).round() as i64),
                character_per_m_micros: per_million_micros(entry.input_cost_per_character),
                second_micros: entry
                    .input_cost_per_second
                    .map(|p| (p * 1_000_000.0).round() as i64),
            });
        }
    }
    Ok(rows)
}
async fn upsert(pool: &Pool, rows: &[Row]) -> Result<usize, Error> {
    let providers: Vec<&str> = rows.iter().map(|r| r.provider_id.as_str()).collect();
    let models: Vec<&str> = rows.iter().map(|r| r.model.as_str()).collect();
    let input: Vec<i64> = rows
        .iter()
        .map(|r| r.input_per_m_micros.unwrap_or(-1))
        .collect();
    let output: Vec<i64> = rows
        .iter()
        .map(|r| r.output_per_m_micros.unwrap_or(-1))
        .collect();
    let cached: Vec<i64> = rows
        .iter()
        .map(|r| r.cached_input_per_m_micros.unwrap_or(-1))
        .collect();
    let writes: Vec<i64> = rows
        .iter()
        .map(|r| r.cache_write_per_m_micros.unwrap_or(-1))
        .collect();
    let images: Vec<i64> = rows.iter().map(|r| r.image_micros.unwrap_or(-1)).collect();
    let characters: Vec<i64> = rows
        .iter()
        .map(|r| r.character_per_m_micros.unwrap_or(-1))
        .collect();
    let seconds: Vec<i64> = rows.iter().map(|r| r.second_micros.unwrap_or(-1)).collect();
    super::db::prices_upsert_execute(
        &pool.get().await?,
        &providers,
        &models,
        &input,
        &output,
        &cached,
        &writes,
        &images,
        &characters,
        &seconds,
    )
    .await?;
    Ok(rows.len())
}
/// Upsert the vendored sheet. Idempotent; a few hundred rows in one statement.
pub async fn reconcile(pool: &Pool) -> Result<usize, Error> {
    let rows: Vec<Row> = serde_json::from_str(include_str!("prices.json"))
        .map_err(|_| Error::Invalid("Vendored price sheet is invalid".into()))?;
    upsert(pool, &rows).await
}
/// Fetch the live sheet and upsert it. Failures leave the current rows in place.
pub async fn refresh(pool: &Pool, url: &str) -> Result<usize, Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("tilde-inference")
        .build()
        .map_err(|_| Error::Invalid("Unable to build the price sheet client".into()))?;
    let body = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|_| Error::Invalid("Price sheet download failed".into()))?
        .text()
        .await
        .map_err(|_| Error::Invalid("Price sheet download failed".into()))?;
    if body.len() > 32 * 1024 * 1024 {
        return Err(Error::Invalid("Price sheet is unexpectedly large".into()));
    }
    let rows = from_litellm(&body)?;
    if rows.len() < 100 {
        return Err(Error::Invalid(
            "Price sheet has too few rows to trust".into(),
        ));
    }
    upsert(pool, &rows).await
}
/// Daily refresh from the live sheet; an empty URL disables it.
pub async fn worker(pool: Pool, url: String, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    if url.is_empty() {
        return;
    }
    let mut tick = tokio::time::interval(REFRESH);
    loop {
        tokio::select! {
            _ = tick.tick() => match refresh(&pool, &url).await {
                Ok(count) => tracing::info!(count, "Inference prices refreshed"),
                Err(error) => tracing::warn!(%error, "Inference price refresh failed; keeping current prices"),
            },
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { break; }
            }
        }
    }
}
