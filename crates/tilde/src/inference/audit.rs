//! Accounting for forwarded inference calls, entirely off the request path. The handler hands
//! over a [`Raw`] capture (shared frame buffers, no copies) and returns; a worker parses model
//! and token usage per wire format and writes batches. On the gateway the sink is Postgres; on a
//! sidecar it is the outbox, and the gateway stores the arriving frames. Best effort: a full
//! queue drops the record with a warning rather than slowing an inference call.
use super::{Kind, Provider};
use crate::chat::Scope;
use crate::prompts::matching;
use crate::proto::tilde::agent_event_ingress::v1 as wire;
use axum::body::Bytes;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{
    sync::Weak,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

const BATCH: usize = 128;
const FLUSH_INTERVAL: Duration = Duration::from_millis(50);
/// Records kept while Postgres is unavailable before the oldest are dropped.
const RETAIN: usize = 4096;

pub struct Raw {
    pub scope: Scope,
    pub connection: Uuid,
    pub provider: Provider,
    pub kind: Kind,
    pub path: String,
    /// `name@hash` stamps from the SDK's `x-tilde-prompt` header: dynamic prompt versions the
    /// call used, which the request text cannot show.
    pub prompts: Vec<(String, Vec<u8>)>,
    pub status: u16,
    pub started: Instant,
    pub first_byte: Option<Duration>,
    pub request: Bytes,
    pub content_type: Option<String>,
    pub encoded: bool,
    pub response: Vec<Bytes>,
    pub response_bytes: u64,
    pub truncated: bool,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    Parsed,
    Missing,
    Truncated,
    Binary,
    Error,
}
impl Usage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parsed => "parsed",
            Self::Missing => "missing",
            Self::Truncated => "truncated",
            Self::Binary => "binary",
            Self::Error => "error",
        }
    }
    fn parse(value: &str) -> Self {
        match value {
            "parsed" => Self::Parsed,
            "truncated" => Self::Truncated,
            "binary" => Self::Binary,
            "error" => Self::Error,
            _ => Self::Missing,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Record {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub agent_id: Uuid,
    pub connection_id: Uuid,
    pub invocation_id: Uuid,
    pub thread_id: Uuid,
    pub run_id: Uuid,
    pub participant_id: Uuid,
    pub provider_id: String,
    pub kind: String,
    pub path: String,
    pub model: Option<String>,
    pub status: i32,
    pub latency_ms: i32,
    pub first_byte_ms: Option<i32>,
    pub request_bytes: i64,
    pub response_bytes: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    /// Images generated or characters synthesised, for kinds billed without tokens.
    pub units: Option<i64>,
    pub usage: Usage,
    /// Stamped prompt versions as (name, hash); the gateway resolves them to versions.
    pub prompts: Vec<(String, Vec<u8>)>,
}
const MAX_STAMPS: usize = 64;
fn prompt_stamp(value: &str) -> Option<(String, Vec<u8>)> {
    let (name, hash) = value.trim().rsplit_once('@')?;
    let hash = hex::decode(hash).ok().filter(|h| h.len() == 32)?;
    (!name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/')))
    .then(|| (name.to_owned(), hash))
}
/// Parse the SDK's `name@hash, name@hash` stamps; malformed entries are ignored rather than
/// refused, since the header never reaches the provider and a bad stamp should not fail a call.
pub fn prompt_stamps(value: &str) -> Vec<(String, Vec<u8>)> {
    let mut stamps: Vec<(String, Vec<u8>)> = Vec::new();
    for stamp in value.split(',').filter_map(prompt_stamp) {
        if stamps.len() < MAX_STAMPS && !stamps.contains(&stamp) {
            stamps.push(stamp);
        }
    }
    stamps
}
impl Record {
    fn from_raw(raw: Raw) -> Self {
        let latency = raw.started.elapsed();
        let model = model_of(raw.provider, &raw.path, &raw.request);
        let units = units_of(raw.kind, &raw.request);
        let (tokens, usage) = usage_of(&raw, units.is_some());
        let units = units.or(tokens.seconds.filter(|_| raw.kind == Kind::Transcription));
        Self {
            id: Uuid::new_v4(),
            created_at: Utc::now() - chrono::Duration::from_std(latency).unwrap_or_default(),
            agent_id: raw.scope.agent_id,
            connection_id: raw.connection,
            invocation_id: raw.scope.id,
            thread_id: raw.scope.thread_id,
            run_id: raw.scope.run_id,
            participant_id: raw.scope.participant_id,
            provider_id: raw.provider.id().into(),
            kind: raw.kind.as_str().into(),
            path: raw.path,
            model,
            status: i32::from(raw.status),
            latency_ms: latency.as_millis().min(i32::MAX as u128) as i32,
            first_byte_ms: raw
                .first_byte
                .map(|d| d.as_millis().min(i32::MAX as u128) as i32),
            request_bytes: raw.request.len() as i64,
            response_bytes: raw.response_bytes as i64,
            input_tokens: tokens.input,
            output_tokens: tokens.output,
            cached_input_tokens: tokens.cached,
            cache_write_tokens: tokens.cache_write,
            units,
            usage,
            prompts: raw.prompts,
        }
    }
    pub fn wire(&self) -> wire::InferenceUsage {
        wire::InferenceUsage {
            id: self.id.to_string(),
            created_at: self.created_at.timestamp_millis(),
            invocation_id: self.invocation_id.to_string(),
            thread_id: self.thread_id.to_string(),
            run_id: self.run_id.to_string(),
            participant_id: self.participant_id.to_string(),
            connection_id: self.connection_id.to_string(),
            provider_id: self.provider_id.clone(),
            kind: self.kind.clone(),
            path: self.path.clone(),
            model: self.model.clone().unwrap_or_default(),
            status: self.status,
            latency_ms: self.latency_ms,
            first_byte_ms: self.first_byte_ms,
            request_bytes: self.request_bytes,
            response_bytes: self.response_bytes,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cached_input_tokens: self.cached_input_tokens,
            cache_write_tokens: self.cache_write_tokens,
            units: self.units,
            usage: self.usage.as_str().into(),
            prompts: self
                .prompts
                .iter()
                .map(|(name, hash)| format!("{name}@{}", hex::encode(hash)))
                .collect(),
            ..Default::default()
        }
    }
    /// A sidecar's record. The agent comes from the deployment token, never the frame, and the
    /// caller checks the connection is assigned to that agent. A replica holds the provider
    /// credential and could misreport counts; ranges are bounded so a bad frame cannot corrupt
    /// sums or settle a budget with negative spend.
    pub fn from_wire(agent: Uuid, w: wire::InferenceUsage) -> Result<Self, crate::error::Error> {
        let invalid = || crate::error::Error::Invalid("Invalid inference record".into());
        let id = |v: &str| Uuid::parse_str(v).map_err(|_| invalid());
        const MAX_TOKENS: i64 = 100_000_000;
        const MAX_BYTES: i64 = 1 << 40;
        let tokens = |v: Option<i64>| match v {
            Some(n) if !(0..=MAX_TOKENS).contains(&n) => Err(invalid()),
            other => Ok(other),
        };
        if w.provider_id.len() > 64
            || w.kind.len() > 32
            || w.path.len() > 2048
            || w.model.len() > 256
            || w.usage.len() > 16
            || !(0..=999).contains(&w.status)
            || w.latency_ms < 0
            || w.first_byte_ms.is_some_and(|ms| ms < 0)
            || !(0..=MAX_BYTES).contains(&w.request_bytes)
            || !(0..=MAX_BYTES).contains(&w.response_bytes)
            || Provider::parse(&w.provider_id).is_none()
        {
            return Err(invalid());
        }
        Ok(Self {
            id: id(&w.id)?,
            created_at: DateTime::from_timestamp_millis(w.created_at).unwrap_or_else(Utc::now),
            agent_id: agent,
            connection_id: id(&w.connection_id)?,
            invocation_id: id(&w.invocation_id)?,
            thread_id: id(&w.thread_id)?,
            run_id: id(&w.run_id)?,
            participant_id: id(&w.participant_id)?,
            provider_id: w.provider_id,
            kind: w.kind,
            path: w.path,
            model: (!w.model.is_empty()).then_some(w.model),
            status: w.status,
            latency_ms: w.latency_ms,
            first_byte_ms: w.first_byte_ms,
            request_bytes: w.request_bytes,
            response_bytes: w.response_bytes,
            input_tokens: tokens(w.input_tokens)?,
            output_tokens: tokens(w.output_tokens)?,
            cached_input_tokens: tokens(w.cached_input_tokens)?,
            cache_write_tokens: tokens(w.cache_write_tokens)?,
            units: tokens(w.units)?,
            usage: Usage::parse(&w.usage),
            prompts: prompt_stamps(&w.prompts.join(",")),
        })
    }
}

/// Google and Bedrock name the model in the path; everyone else in the JSON body.
fn model_of(provider: Provider, path: &str, request: &[u8]) -> Option<String> {
    let from_path = match provider {
        Provider::GoogleAi => path
            .split('/')
            .find_map(|segment| segment.split_once(':').map(|(model, _)| model)),
        Provider::Bedrock => {
            let mut segments = path.split('/');
            segments
                .find(|s| *s == "model")
                .and_then(|_| segments.next())
        }
        _ => None,
    };
    if let Some(model) = from_path {
        return Some(model.to_owned());
    }
    #[derive(serde::Deserialize)]
    struct Model {
        model: Option<String>,
    }
    serde_json::from_slice::<Model>(request)
        .ok()
        .and_then(|m| m.model)
        .filter(|m| !m.is_empty() && m.len() <= 256)
}

#[derive(Default, Clone, Copy)]
struct Tokens {
    input: Option<i64>,
    output: Option<i64>,
    cached: Option<i64>,
    cache_write: Option<i64>,
    /// Audio seconds a transcription response reports (`usage.seconds` or `duration`).
    seconds: Option<i64>,
}
impl Tokens {
    fn found(&self) -> bool {
        self.input.is_some()
            || self.output.is_some()
            || self.cached.is_some()
            || self.cache_write.is_some()
            || self.seconds.is_some()
    }
    /// Streams report usage cumulatively or split across events; the maximum per field is the total.
    fn merge(&mut self, other: Tokens) {
        for (mine, theirs) in [
            (&mut self.input, other.input),
            (&mut self.output, other.output),
            (&mut self.cached, other.cached),
            (&mut self.cache_write, other.cache_write),
            (&mut self.seconds, other.seconds),
        ] {
            *mine = match (*mine, theirs) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
        }
    }
}
/// Billing units that live in the request, not the response: images requested (`n`, default 1)
/// and characters of speech input. Transcription seconds come from the response instead.
fn units_of(kind: Kind, request: &[u8]) -> Option<i64> {
    let body: Value = serde_json::from_slice(request).ok()?;
    match kind {
        Kind::Image => Some(body["n"].as_i64().unwrap_or(1).clamp(0, 100)),
        Kind::Speech => Some(body["input"].as_str()?.chars().count() as i64),
        _ => None,
    }
}
fn usage_of(raw: &Raw, priced_by_units: bool) -> (Tokens, Usage) {
    if raw.status >= 400 {
        return (Tokens::default(), Usage::Error);
    }

    let content_type = raw.content_type.as_deref().unwrap_or("");
    if raw.encoded
        || content_type.starts_with("audio/")
        || content_type.starts_with("application/octet-stream")
    {
        return (Tokens::default(), Usage::Binary);
    }
    let mut body = Vec::with_capacity(raw.response.iter().map(Bytes::len).sum());
    for frame in &raw.response {
        body.extend_from_slice(frame);
    }
    let mut tokens = Tokens::default();
    if content_type.starts_with("application/vnd.amazon.eventstream") {
        for payload in eventstream_payloads(&body) {
            tokens.merge(tokens_in(&payload));
        }
    } else if content_type.starts_with("text/event-stream") {
        for line in body.split(|b| *b == b'\n') {
            let Some(data) = line.strip_prefix(b"data:") else {
                continue;
            };
            if let Ok(event) = serde_json::from_slice::<Value>(data) {
                tokens.merge(tokens_in(&event));
            }
        }
    } else if let Ok(value) = serde_json::from_slice::<Value>(&body) {
        tokens.merge(tokens_in(&value));
    }
    // Token-priced image models report usage in the body; per-image models leave it out and are
    // priced from the request's unit count instead. Either one is a complete record.
    let usage = if raw.truncated {
        Usage::Truncated
    } else if tokens.found() || priced_by_units {
        Usage::Parsed
    } else {
        Usage::Missing
    };
    (tokens, usage)
}
/// Usage objects across OpenAI (chat and responses), Anthropic, Google, Bedrock, Cohere and Voyage.
/// JSON payloads of an Amazon event stream (`application/vnd.amazon.eventstream`): a sequence
/// of `total_len | headers_len | prelude_crc | headers | payload | message_crc` messages. Bedrock
/// ConverseStream sends a `metadata` event carrying `usage`; InvokeModelWithResponseStream wraps
/// each chunk as `{"bytes": <base64 JSON>}` whose final chunk carries
/// `amazon-bedrock-invocationMetrics`. CRCs are not verified: the capture is for accounting only.
fn eventstream_payloads(body: &[u8]) -> Vec<Value> {
    use base64::Engine;
    let mut payloads = Vec::new();
    let mut offset = 0;
    while body.len() - offset >= 16 {
        let total = u32::from_be_bytes(body[offset..offset + 4].try_into().unwrap()) as usize;
        let headers = u32::from_be_bytes(body[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if total < 16 || headers + 16 > total || offset + total > body.len() {
            break;
        }
        let payload = &body[offset + 12 + headers..offset + total - 4];
        if let Ok(value) = serde_json::from_slice::<Value>(payload) {
            if let Some(inner) = value["bytes"]
                .as_str()
                .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok())
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            {
                payloads.push(inner);
            } else {
                payloads.push(value);
            }
        }
        offset += total;
    }
    payloads
}
/// Parse an Amazon event stream capture into `(input, output, cached)`; a test seam.
pub fn eventstream_usage_for_test(body: &[u8]) -> (Option<i64>, Option<i64>, Option<i64>) {
    let mut tokens = Tokens::default();
    for payload in eventstream_payloads(body) {
        tokens.merge(tokens_in(&payload));
    }
    (tokens.input, tokens.output, tokens.cached)
}
/// Audio seconds a transcription JSON body reports; a test seam.
pub fn seconds_for_test(body: &[u8]) -> Option<i64> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| tokens_in(&v).seconds)
}
fn tokens_in(value: &Value) -> Tokens {
    let candidates = [
        &value["usage"],
        &value["usageMetadata"],
        &value["response"]["usage"],
        &value["message"]["usage"],
        &value["meta"]["billed_units"],
        &value["amazon-bedrock-invocationMetrics"],
    ];
    let count = |usage: &Value, keys: &[&str]| keys.iter().find_map(|key| usage[*key].as_i64());
    let mut tokens = Tokens {
        seconds: value["duration"].as_f64().map(|s| s.ceil() as i64),
        ..Tokens::default()
    };
    for usage in candidates.into_iter().filter(|u| u.is_object()) {
        let mut found = Tokens {
            seconds: None,
            input: count(
                usage,
                &[
                    "input_tokens",
                    "prompt_tokens",
                    "promptTokenCount",
                    "inputTokens",
                    "inputTokenCount",
                ],
            ),
            output: count(
                usage,
                &[
                    "output_tokens",
                    "completion_tokens",
                    "candidatesTokenCount",
                    "outputTokens",
                    "outputTokenCount",
                ],
            ),
            cached: count(
                usage,
                &[
                    "cache_read_input_tokens",
                    "cachedContentTokenCount",
                    "cacheReadInputTokens",
                ],
            )
            .or_else(|| usage["prompt_tokens_details"]["cached_tokens"].as_i64())
            .or_else(|| usage["input_tokens_details"]["cached_tokens"].as_i64()),
            cache_write: count(
                usage,
                &["cache_creation_input_tokens", "cacheWriteInputTokens"],
            ),
        };
        if let (Some(output), Some(thoughts)) = (found.output, usage["thoughtsTokenCount"].as_i64())
        {
            found.output = Some(output + thoughts);
        }
        if !found.found() {
            // Embedding and rerank responses bill a single total.
            found.input = count(usage, &["total_tokens", "totalTokenCount"]);
        }
        // Whisper-style transcriptions report audio seconds rather than tokens.
        found.seconds = usage["seconds"]
            .as_f64()
            .or_else(|| value["duration"].as_f64())
            .map(|s| s.ceil() as i64);
        tokens.merge(found);
    }
    tokens
}

/// A deployment's text-matchable prompt versions (see `Prompts::for_deployment`), sent to its
/// sidecars so they can match request bodies without shipping them.
pub fn prompt_patterns(
    versions: Vec<(Uuid, String, crate::prompts::PromptVersion)>,
) -> Vec<wire::PromptPattern> {
    versions
        .into_iter()
        .filter_map(|(_, name, v)| {
            let sections: Vec<(String, String)> = v
                .sections
                .into_iter()
                .map(|s| (s.name, s.content))
                .collect();
            Some(wire::PromptPattern {
                segments: matching::segments(v.format, &v.template, &sections)?,
                name,
                hash: v.hash,
                ..Default::default()
            })
        })
        .collect()
}
/// Add each version whose text the request carries to the record's stamps.
fn stamp_matches(record: &mut Record, body: &[u8], patterns: &[wire::PromptPattern]) {
    if patterns.is_empty() {
        return;
    }
    let texts = matching::request_texts(body);
    for pattern in patterns {
        let stamp = (pattern.name.clone(), pattern.hash.clone());
        if record.prompts.len() < MAX_STAMPS
            && !record.prompts.contains(&stamp)
            && texts
                .iter()
                .any(|t| matching::found_in(&pattern.segments, t))
        {
            record.prompts.push(stamp);
        }
    }
}

/// Where parsed records go.
pub enum Sink {
    Postgres(crate::database::Pool),
    Sidecar(Weak<crate::deployment::runtime::Runtime>),
}
enum Command {
    Raw(Box<Raw>),
    Flush(oneshot::Sender<()>),
}
#[derive(Clone)]
pub struct Audit {
    sender: mpsc::Sender<Command>,
}
impl Audit {
    pub fn start(sink: Sink) -> Self {
        let (sender, receiver) = mpsc::channel(4096);
        tokio::spawn(run(sink, receiver));
        Self { sender }
    }
    pub(crate) fn record(&self, raw: Raw) {
        if self.sender.try_send(Command::Raw(Box::new(raw))).is_err() {
            tracing::warn!("Inference audit queue full; dropping usage record");
        }
    }
    /// Barrier for tests and shutdown: everything received so far has been written or shipped.
    pub async fn flush(&self) -> Result<(), crate::error::Error> {
        let (sent, received) = oneshot::channel();
        tokio::time::timeout(Duration::from_secs(5), async {
            self.sender.send(Command::Flush(sent)).await.map_err(|_| {
                crate::error::Error::Invalid("Inference audit worker stopped".into())
            })?;
            received
                .await
                .map_err(|_| crate::error::Error::Invalid("Inference audit worker stopped".into()))
        })
        .await
        .map_err(|_| crate::error::Error::Invalid("Inference audit flush timed out".into()))?
    }
}
async fn run(sink: Sink, mut receiver: mpsc::Receiver<Command>) {
    // Each record keeps its request body until the flush that matches prompts against it.
    let mut pending: Vec<(Record, Bytes)> = Vec::new();
    let mut linker = match &sink {
        Sink::Postgres(pool) => Some(matching::Linker::new(crate::prompts::Prompts::new(
            pool.clone(),
        ))),
        Sink::Sidecar(_) => None,
    };
    let mut tick = tokio::time::interval(FLUSH_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let barrier = tokio::select! {
            command = receiver.recv() => match command {
                Some(Command::Raw(raw)) => {
                    let body = raw.request.clone();
                    pending.push((Record::from_raw(*raw), body));
                    if pending.len() < BATCH { continue; }
                    None
                }
                Some(Command::Flush(done)) => Some(done),
                None => { flush(&sink, linker.as_mut(), &mut pending).await; break; }
            },
            _ = tick.tick() => None,
        };
        if !pending.is_empty() {
            flush(&sink, linker.as_mut(), &mut pending).await;
        }
        if let Some(done) = barrier {
            let _ = done.send(());
        }
    }
}
/// Link each stored call to the prompt versions it used. A failure loses only the links.
pub(crate) async fn link_prompts(
    db: &impl crate::database::GenericClient,
    linker: &mut matching::Linker,
    records: &[(&Record, Option<&[u8]>)],
) {
    let calls: Vec<matching::Call<'_>> = records
        .iter()
        .map(|(r, body)| matching::Call {
            request: r.id,
            agent: r.agent_id,
            invocation: r.invocation_id,
            stamps: &r.prompts,
            body: *body,
        })
        .collect();
    if let Err(error) = linker.link(db, &calls).await {
        tracing::warn!(%error, "Inference calls not linked to prompt versions");
    }
}
async fn flush(
    sink: &Sink,
    linker: Option<&mut matching::Linker>,
    pending: &mut Vec<(Record, Bytes)>,
) {
    match sink {
        Sink::Postgres(pool) => {
            let result = async {
                let db = pool.get().await?;
                let records: Vec<&Record> = pending.iter().map(|(r, _)| r).collect();
                super::db::requests_insert_execute(&db, &records).await?;
                if let Some(linker) = linker {
                    let with_bodies: Vec<(&Record, Option<&[u8]>)> = pending
                        .iter()
                        .map(|(r, body)| (r, Some(body.as_ref())))
                        .collect();
                    link_prompts(&db, linker, &with_bodies).await;
                }
                Ok::<_, crate::database::DbError>(())
            }
            .await;
            match result {
                Ok(()) => pending.clear(),
                Err(error) => {
                    tracing::warn!(%error, "Inference usage batch not written; retaining");
                    if pending.len() > RETAIN {
                        let excess = pending.len() - RETAIN;
                        pending.drain(..excess);
                    }
                }
            }
        }
        Sink::Sidecar(runtime) => {
            if let Some(runtime) = runtime.upgrade() {
                // Bodies never leave the replica: static prompts are matched here against the
                // deployment's patterns from the Watch snapshot and shipped as stamps.
                let patterns = runtime.state.prompts.get().map(Vec::as_slice);
                for (mut record, body) in pending.drain(..) {
                    stamp_matches(&mut record, &body, patterns.unwrap_or_default());
                    runtime
                        .push_ephemeral(wire::upstream::Frame::Inference(Box::new(record.wire())));
                }
            } else {
                pending.clear();
            }
        }
    }
}
