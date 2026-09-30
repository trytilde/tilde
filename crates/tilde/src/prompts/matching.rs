//! Which prompt versions an inference call used, decided by the audit worker after the call.
//! The candidates are the versions the invocation's deployment shipped (immutable, so cached
//! per deployment). Static text is found in the request body: a plain prompt as a substring, a
//! mustache or braces template when each of its literal segments appears in order in one
//! message. Dynamic prompts exist only as function source, so they are reached by the
//! `x-tilde-prompt: name@hash, …` stamps the SDK sends instead.
use super::{Format, Prompts, db};
use crate::{database::GenericClient, error::Error};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// Shorter static text matches too much ordinary conversation to identify a prompt.
const MIN_STATIC_BYTES: usize = 16;
const CACHED_DEPLOYMENTS: usize = 256;

/// One inference call to link. `body` is absent for sidecar records, which carry only stamps.
pub struct Call<'a> {
    pub request: Uuid,
    pub agent: Uuid,
    pub invocation: Uuid,
    pub stamps: &'a [(String, Vec<u8>)],
    pub body: Option<&'a [u8]>,
}
/// A version that can be found by text: its literal segments in order.
struct Candidate {
    version: Uuid,
    segments: Vec<String>,
}
pub struct Linker {
    prompts: Prompts,
    deployments: HashMap<Uuid, Arc<Vec<Candidate>>>,
}

/// Literal text of a template in order, or None when it cannot be recognised by text.
pub(crate) fn segments(
    format: Format,
    template: &str,
    sections: &[(String, String)],
) -> Option<Vec<String>> {
    let parts: Vec<String> = match format {
        Format::Dynamic => return None,
        Format::Plain => vec![template.to_owned()],
        Format::Braces => super::brace_parts(template)
            .into_iter()
            .filter_map(Result::ok)
            .collect(),
        Format::Mustache => {
            // Sections are inlined first (they cannot nest), then every tag splits the text.
            let mut inlined = String::new();
            for (literal, tag) in tags(template) {
                inlined.push_str(literal);
                let partial = tag.strip_prefix('>').map(str::trim);
                match sections.iter().find(|(n, _)| Some(n.as_str()) == partial) {
                    Some((_, content)) => inlined.push_str(content),
                    None if !tag.is_empty() => inlined.push_str(&format!("{{{{{tag}}}}}")),
                    None => {}
                }
            }
            tags(&inlined)
                .into_iter()
                .map(|(literal, _)| literal.to_owned())
                .collect()
        }
    };
    // Renderers and frameworks trim around the text; interior whitespace is kept.
    let parts: Vec<String> = parts
        .into_iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    (parts.iter().map(String::len).sum::<usize>() >= MIN_STATIC_BYTES).then_some(parts)
}
/// Mustache text as (literal, trimmed tag) pairs; the last pair has an empty tag.
fn tags(text: &str) -> Vec<(&str, &str)> {
    let mut pairs = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let Some(end) = rest[start..].find("}}") else {
            break;
        };
        pairs.push((&rest[..start], rest[start + 2..start + end].trim()));
        rest = &rest[start + end + 2..];
    }
    pairs.push((rest, ""));
    pairs
}
pub(crate) fn found_in(segments: &[String], text: &str) -> bool {
    let mut rest = text;
    for segment in segments {
        match rest.find(segment.as_str()) {
            Some(at) => rest = &rest[at + segment.len()..],
            None => return false,
        }
    }
    true
}
/// Instruction and user text of a chat request in every format the gateway forwards as chat:
/// `system`, `instructions`, and `messages`/`input` items whose role is system, developer or
/// user. That covers OpenAI chat and Responses, Anthropic Messages (top-level system), Bedrock
/// Converse (`system` and content as `[{text}]`) and Cohere v2 chat. Google's
/// `generateContent` puts instructions in `systemInstruction.parts` and turns in
/// `contents[].parts`, with `model` as the assistant role.
pub fn request_texts(body: &[u8]) -> Vec<String> {
    fn parts(value: &Value, texts: &mut Vec<String>) {
        match value {
            Value::String(text) => texts.push(text.clone()),
            Value::Array(items) => {
                for item in items {
                    match item {
                        Value::String(text) => texts.push(text.clone()),
                        Value::Object(part) => {
                            if let Some(Value::String(text)) = part.get("text") {
                                texts.push(text.clone());
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    fn items(value: &Value, texts: &mut Vec<String>) {
        match value {
            Value::String(text) => texts.push(text.clone()),
            Value::Array(items) => {
                for item in items {
                    let role = item["role"].as_str().unwrap_or("user");
                    if matches!(role, "system" | "developer" | "user") {
                        parts(item.get("content").unwrap_or(&item["parts"]), texts);
                    }
                }
            }
            _ => {}
        }
    }
    let Ok(body) = serde_json::from_slice::<Value>(body) else {
        return vec![];
    };
    let mut texts = Vec::new();
    parts(&body["system"], &mut texts);
    parts(&body["instructions"], &mut texts);
    items(&body["messages"], &mut texts);
    items(&body["input"], &mut texts);
    let google = body
        .get("systemInstruction")
        .or(body.get("system_instruction"));
    if let Some(instruction) = google {
        parts(&instruction["parts"], &mut texts);
    }
    items(&body["contents"], &mut texts);
    texts
}

impl Linker {
    pub fn new(prompts: Prompts) -> Self {
        Self {
            prompts,
            deployments: HashMap::new(),
        }
    }
    async fn candidates(&mut self, deployment: Uuid) -> Result<Arc<Vec<Candidate>>, Error> {
        if let Some(cached) = self.deployments.get(&deployment) {
            return Ok(cached.clone());
        }
        let candidates: Vec<Candidate> = self
            .prompts
            .for_deployment(deployment)
            .await?
            .into_iter()
            .filter_map(|(_, _, v)| {
                let sections: Vec<(String, String)> = v
                    .sections
                    .into_iter()
                    .map(|s| (s.name, s.content))
                    .collect();
                Some(Candidate {
                    version: v.id,
                    segments: segments(v.format, &v.template, &sections)?,
                })
            })
            .collect();
        if self.deployments.len() >= CACHED_DEPLOYMENTS {
            self.deployments.clear();
        }
        let candidates = Arc::new(candidates);
        self.deployments.insert(deployment, candidates.clone());
        Ok(candidates)
    }
    /// Record every version each call used. The request rows must already exist.
    pub async fn link(&mut self, db: &impl GenericClient, calls: &[Call<'_>]) -> Result<(), Error> {
        let invocations: Vec<Uuid> = calls
            .iter()
            .filter(|c| c.body.is_some())
            .map(|c| c.invocation)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let deployments: HashMap<Uuid, Uuid> = if invocations.is_empty() {
            HashMap::new()
        } else {
            db::invocation_deployments_all(db, &invocations)
                .await?
                .into_iter()
                .collect()
        };
        let mut links: HashSet<(Uuid, Uuid)> = HashSet::new();
        for call in calls {
            if let (Some(body), Some(deployment)) = (call.body, deployments.get(&call.invocation)) {
                let candidates = self.candidates(*deployment).await?;
                if !candidates.is_empty() {
                    let texts = request_texts(body);
                    for candidate in candidates.iter() {
                        if texts.iter().any(|t| found_in(&candidate.segments, t)) {
                            links.insert((call.request, candidate.version));
                        }
                    }
                }
            }
            for (name, hash) in call.stamps {
                if let Some(version) = db::version_by_stamp_opt(db, call.agent, name, hash).await? {
                    links.insert((call.request, version));
                }
            }
        }
        if !links.is_empty() {
            let (requests, versions): (Vec<Uuid>, Vec<Uuid>) = links.into_iter().unzip();
            db::request_links_insert_execute(db, &requests, &versions).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_match_their_rendered_text_in_order() {
        let braces = segments(
            Format::Braces,
            "You research {topic} for {audience}. Use {{json}}.",
            &[],
        )
        .unwrap();
        assert_eq!(braces, ["You research", "for", ". Use {json}."]);
        assert!(found_in(
            &braces,
            "You research tides for sailors. Use {json}."
        ));
        assert!(!found_in(&braces, "for sailors. You research tides"));
        let mustache = segments(
            Format::Mustache,
            "Hello {{name}}\n{{> rules}}",
            &[("rules".into(), "Always answer briefly.".into())],
        )
        .unwrap();
        assert!(found_in(&mustache, "Hello Ada\nAlways answer briefly."));
        assert!(segments(Format::Braces, "{a} {b}", &[]).is_none());
        assert!(segments(Format::Plain, "Be brief.", &[]).is_none());
        assert!(segments(Format::Dynamic, "() => 'You are a helpful agent'", &[]).is_none());
    }
    #[test]
    fn request_text_comes_from_instructions_and_user_turns() {
        let chat = serde_json::json!({"messages":[
            {"role":"system","content":"S"},
            {"role":"assistant","content":"A"},
            {"role":"user","content":[{"type":"text","text":"U"}]}
        ]});
        assert_eq!(request_texts(chat.to_string().as_bytes()), ["S", "U"]);
        let responses = serde_json::json!({"instructions":"I","input":[{"role":"developer","content":[{"type":"input_text","text":"D"}]}]});
        assert_eq!(request_texts(responses.to_string().as_bytes()), ["I", "D"]);
        let anthropic = serde_json::json!({"system":[{"type":"text","text":"S"}],"messages":[{"role":"user","content":"U"}]});
        assert_eq!(request_texts(anthropic.to_string().as_bytes()), ["S", "U"]);
        let gemini = serde_json::json!({
            "systemInstruction":{"parts":[{"text":"S"}]},
            "contents":[
                {"role":"user","parts":[{"text":"U"}]},
                {"role":"model","parts":[{"text":"M"}]},
                {"parts":[{"text":"V"}]}
            ]
        });
        assert_eq!(
            request_texts(gemini.to_string().as_bytes()),
            ["S", "U", "V"]
        );
        let gemini_rest = serde_json::json!({"system_instruction":{"parts":[{"text":"S"}]},"contents":[{"role":"user","parts":[{"text":"U"}]}]});
        assert_eq!(
            request_texts(gemini_rest.to_string().as_bytes()),
            ["S", "U"]
        );
        let converse = serde_json::json!({
            "system":[{"text":"S"}],
            "messages":[
                {"role":"user","content":[{"text":"U"}]},
                {"role":"assistant","content":[{"text":"A"}]}
            ]
        });
        assert_eq!(request_texts(converse.to_string().as_bytes()), ["S", "U"]);
        let cohere = serde_json::json!({"messages":[
            {"role":"system","content":"S"},
            {"role":"user","content":[{"type":"text","text":"U"}]},
            {"role":"assistant","content":"A"}
        ]});
        assert_eq!(request_texts(cohere.to_string().as_bytes()), ["S", "U"]);
    }
}
