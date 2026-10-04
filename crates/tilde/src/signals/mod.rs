//! Signals: typed events a connection's provider emits from its verified webhook deliveries,
//! ported from trytilde/api's signal providers. A source lists its signal types and template
//! variables and normalizes an authenticated delivery into signals; routines render their
//! templates against a signal's context (its data plus `provider_delivery_id`, `signal_type`
//! and `summary`).
//!
//! Chat adapters authenticate the webhooks of channel providers. Providers without a chat
//! adapter (Sentry, Firecrawl) are authenticated by [`verify`].
mod agentmail;
mod firecrawl;
mod github;
mod linq;
mod sentry;
mod slack;
mod whatsapp;

use crate::chat::providers::Access;
use crate::chat::tools::ToolResult;
use connectrpc::ConnectError;
use http::HeaderMap;
use serde_json::{Map, Value};

/// One normalized event. `data` is what templates and the agent see.
pub struct Signal {
    pub event_id: String,
    pub signal_type: String,
    pub summary: String,
    pub data: Value,
}
pub struct SignalType {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The thread title a new routine on this signal starts with.
    pub title: String,
}
pub struct Variable {
    pub key: &'static str,
    pub description: &'static str,
    pub example: &'static str,
}

pub trait Source: Sync {
    fn types(&self) -> Vec<SignalType>;
    /// Provider-specific template variables; [`COMMON`] ones are offered for every signal.
    fn variables(&self) -> &'static [Variable];
    /// Normalizes a delivery the ingress has already authenticated. A delivery may carry events
    /// of other accounts sharing the webhook (another phone number, line or inbox); only the
    /// connection's own become signals.
    fn signals(&self, access: &Access, headers: &HeaderMap, payload: &Value) -> Vec<Signal>;
}

pub const COMMON: &[Variable] = &[
    Variable {
        key: "provider_delivery_id",
        description: "The provider's unique ID for this delivery.",
        example: "7c8f4c1a-2b19-4a8b-9c31-0b9422f44c2d",
    },
    Variable {
        key: "signal_type",
        description: "Normalized Tilde signal type.",
        example: "github.issue.opened",
    },
    Variable {
        key: "summary",
        description: "Human-readable summary of the event.",
        example: "GitHub issue opened: acme/app#12 - Login fails",
    },
];

pub fn source(provider: &str, typ: &str) -> Option<&'static dyn Source> {
    match (provider, typ) {
        ("github", "github_app") => Some(&github::Github),
        ("slack", "slack_app") => Some(&slack::Slack),
        ("agentmail", "inbox") => Some(&agentmail::Agentmail),
        ("linq", "account") => Some(&linq::Linq),
        ("whatsapp", "meta") | ("telnyx", "whatsapp") => Some(&whatsapp::Whatsapp),
        ("sentry", "token") => Some(&sentry::Sentry),
        ("firecrawl", "api") => Some(&firecrawl::Firecrawl),
        _ => None,
    }
}

/// Authenticates a delivery for a signal-only provider, one no chat adapter verifies.
pub fn verify(provider: &str, access: &Access, headers: &HeaderMap, body: &[u8]) -> ToolResult<()> {
    use crate::chat::providers::ingress::{header, hmac_verify};
    let (name, key, prefix) = match provider {
        // A Sentry integration signs the exact body with its client secret.
        "sentry" => ("sentry-hook-signature", "client_secret", ""),
        "firecrawl" => ("x-firecrawl-signature", "webhook_secret", "sha256="),
        _ => return Err(ConnectError::not_found("Unknown webhook provider")),
    };
    let signature = header(headers, name)?
        .strip_prefix(prefix)
        .and_then(|hex| hex::decode(hex).ok())
        .ok_or_else(|| ConnectError::unauthenticated("Invalid signature"))?;
    // An absent secret means the connection was set up for tools only.
    let secret = access
        .secret(key)
        .map_err(|_| ConnectError::unauthenticated("Signals are not configured"))?;
    hmac_verify(secret.as_bytes(), body, &signature)
}

/// The values a template may name: the signal's data with the common fields beside it.
pub fn context(signal: &Signal) -> Value {
    let mut context = match &signal.data {
        Value::Object(map) => map.clone(),
        other => Map::from_iter([("data".to_owned(), other.clone())]),
    };
    context.insert(
        "provider_delivery_id".into(),
        signal.event_id.clone().into(),
    );
    context.insert("signal_type".into(), signal.signal_type.clone().into());
    context.insert("summary".into(), signal.summary.clone().into());
    Value::Object(context)
}

/// Replaces each `{{ a.b.c }}` with that path in `context`; strings render bare, other values
/// as JSON, and a missing path as nothing.
pub fn render(template: &str, context: &Value) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open..].find("}}").map(|close| open + close) else {
            break;
        };
        out.push_str(&rest[..open]);
        let key = rest[open + 2..close].trim();
        let pointer = format!("/{}", key.replace('.', "/"));
        match context.pointer(&pointer) {
            Some(Value::String(value)) => out.push_str(value),
            Some(Value::Null) | None => {}
            Some(value) => out.push_str(&value.to_string()),
        }
        rest = &rest[close + 2..];
    }
    out.push_str(rest);
    out
}

fn str_at<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer).and_then(Value::as_str)
}
/// A stable delivery ID for payloads that carry none, so redeliveries still dedupe.
fn hashed(prefix: &str, value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(value.to_string());
    format!("{prefix}-{}", hex::encode(&digest[..16]))
}
fn readable(event: &str) -> String {
    event.replace(['.', '_'], " ")
}
