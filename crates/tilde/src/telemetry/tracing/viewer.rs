//! Read-only trace views over the ClickHouse span store.
//!
//! Every query carries a server-authored agent predicate, and every optional filter is a
//! static parameter test in `queries/_traces`, so nothing from a request is interpolated into
//! SQL. A whole trace, which may include gateway and cooperating-agent spans, is returned
//! only after proving one of its spans belongs to the caller's agent.
use super::store::Store;
use crate::database::Pool;
use crate::proto::tilde::management::v1 as pb;
use crate::services::tilde::management::v1::TracingService;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;
pub mod filter;
mod metrics;

const PAGE: usize = 50;
const MAX_RANGE_DAYS: i64 = 90;

#[derive(Clone)]
pub struct Reader {
    pool: Pool,
    store: Store,
    objects: Option<crate::agent::avatar::ObjectStore>,
    admission: Arc<tokio::sync::Semaphore>,
}
/// Keyset position plus the window and filter it belongs to, so every page of a listing sees
/// the same range even though "now" moves, and a cursor cannot be replayed against other filters.
#[derive(Serialize, Deserialize)]
struct Cursor {
    time: i64,
    trace: String,
    span: String,
    from: i64,
    to: i64,
    filter: String,
}
/// A validated filter as ClickHouse parameters and rendered clauses, with its resolved range.
pub(super) struct Query {
    pub parameters: Vec<(String, String)>,
    pub clauses: String,
    pub from: i64,
    pub to: i64,
    identity: String,
}
fn parameter(name: &str, value: impl Into<String>) -> (String, String) {
    (format!("param_{name}"), value.into())
}
fn nanos(value: &str, error: &'static str) -> Result<i64, ConnectError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .and_then(|time| time.timestamp_nanos_opt())
        .ok_or_else(|| ConnectError::invalid_argument(error))
}
fn rfc3339(nanos: i64) -> String {
    chrono::DateTime::from_timestamp_nanos(nanos).to_rfc3339()
}
fn failure(error: crate::error::Error) -> ConnectError {
    match error {
        crate::error::Error::Invalid(message) if message.contains("limit") => {
            ConnectError::resource_exhausted("Trace query is too large; narrow the date range")
        }
        _ => ConnectError::unavailable("Trace storage is unavailable"),
    }
}

impl Reader {
    pub fn new(
        pool: Pool,
        store: Store,
        objects: Option<crate::agent::avatar::ObjectStore>,
    ) -> Self {
        Self {
            pool,
            store,
            objects,
            admission: Arc::new(tokio::sync::Semaphore::new(8)),
        }
    }
    async fn agent(&self, value: &str) -> Result<String, ConnectError> {
        let id = Uuid::parse_str(value)
            .map_err(|_| ConnectError::invalid_argument("Agent ID must be a UUID"))?;
        if !crate::telemetry::tracing::db::agent_exists_one(&self.pool.get().await?, id)
            .await
            .map_err(|_| ConnectError::unavailable("Agent registry unavailable"))?
            .exists
        {
            return Err(ConnectError::not_found("Agent not found"));
        }
        Ok(id.to_string())
    }
    pub(super) async fn run(
        &self,
        sql: &str,
        parameters: &[(String, String)],
    ) -> Result<Vec<Value>, ConnectError> {
        let _permit = self.admission.clone().try_acquire_owned().map_err(|_| {
            ConnectError::resource_exhausted("Too many trace queries; retry shortly")
        })?;
        let response = self.store.query(sql, parameters).await.map_err(failure)?;
        Ok(response["data"].as_array().cloned().unwrap_or_default())
    }
    /// Validate a filter into parameters. A cursor's window stands in for an omitted bound, so
    /// "now" stays frozen across pages, but an explicit bound must agree with it: a cursor
    /// reused under a changed date range is a mismatch, not a silent read of the old range.
    pub(super) fn query(
        agent: &str,
        filter: &pb::ObservationFilter,
        window: Option<(i64, i64)>,
    ) -> Result<Query, ConnectError> {
        let explicit_to = (!filter.to_time.is_empty())
            .then(|| nanos(&filter.to_time, "Invalid end time"))
            .transpose()?;
        let explicit_from = (!filter.from_time.is_empty())
            .then(|| nanos(&filter.from_time, "Invalid start time"))
            .transpose()?;
        let (from, to) = match window {
            Some((from, to)) => {
                if explicit_from.is_some_and(|value| value != from)
                    || explicit_to.is_some_and(|value| value != to)
                {
                    return Err(ConnectError::invalid_argument(
                        "Cursor does not match the filters",
                    ));
                }
                (from, to)
            }
            None => {
                let to = explicit_to.unwrap_or_else(|| {
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(i64::MAX)
                });
                (explicit_from.unwrap_or(to - 86_400_000_000_000), to)
            }
        };
        if from < 0 || from >= to {
            return Err(ConnectError::invalid_argument("Start must precede end"));
        }
        if to - from > MAX_RANGE_DAYS * 86_400_000_000_000 {
            return Err(ConnectError::invalid_argument(
                "Use a time range of at most 90 days",
            ));
        }
        let rendered = filter::render(&filter.conditions)?;
        let mut parameters = vec![
            parameter("agent", agent),
            parameter("from", from.to_string()),
            parameter("to", to.to_string()),
        ];
        parameters.extend(rendered.parameters);
        let mut identity = Sha256::new();
        identity.update(agent);
        identity.update([0]);
        identity.update(&rendered.identity);
        Ok(Query {
            parameters,
            clauses: rendered.clauses,
            from,
            to,
            identity: hex::encode(identity.finalize()),
        })
    }
    /// A ten-minute URL for one of the agent's media objects or full payloads.
    pub async fn object_url(&self, agent: &str, key: &str) -> Result<String, ConnectError> {
        let agent = self.agent(agent).await?;
        // Keys name their owner; nothing outside the agent's media and payloads resolves.
        if !super::media::owned_by(key, &agent) {
            return Err(ConnectError::not_found("Object not found"));
        }
        let objects = self
            .objects
            .as_ref()
            .ok_or_else(|| ConnectError::failed_precondition("Trace storage is not configured"))?;
        objects
            .signed_url(key, 600)
            .await
            .map_err(|_| ConnectError::unavailable("Object storage is unavailable"))
    }
    /// One page of an agent's observations, or with `trace_id` of one whole trace.
    pub async fn list(
        &self,
        agent: &str,
        filter: pb::ObservationFilter,
        cursor: &str,
        trace_id: Option<&str>,
    ) -> Result<pb::ListObservationsResponse, ConnectError> {
        let agent = self.agent(agent).await?;
        let mismatch = || ConnectError::invalid_argument("Cursor does not match the filters");
        let position: Option<Cursor> = if cursor.is_empty() {
            None
        } else {
            if cursor.len() > 2048 {
                return Err(ConnectError::invalid_argument("Invalid cursor"));
            }
            Some(
                URL_SAFE_NO_PAD
                    .decode(cursor)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                    .ok_or_else(|| ConnectError::invalid_argument("Invalid cursor"))?,
            )
        };
        let (sql, mut query) = match trace_id {
            None => {
                let query =
                    Self::query(&agent, &filter, position.as_ref().map(|c| (c.from, c.to)))?;
                (
                    include_str!("../../../../../queries/_traces/list.sql")
                        .replace("%%CONDITIONS%%", &query.clauses),
                    query,
                )
            }
            Some(trace) => {
                if trace.len() > 64 || !trace.bytes().all(|c| c.is_ascii_hexdigit()) {
                    return Err(ConnectError::invalid_argument("Invalid trace ID"));
                }
                // A trace can contain gateway parents and other agents. Prove membership with
                // an agent-scoped lookup before returning that trace's full context.
                let proof = self
                    .run(
                        include_str!("../../../../../queries/_traces/membership.sql"),
                        &[parameter("agent", agent.clone()), parameter("trace", trace)],
                    )
                    .await?;
                let bound = |key: &str| {
                    proof
                        .first()
                        .and_then(|row| row[key].as_str())
                        .and_then(|v| v.parse::<i64>().ok())
                };
                let spans = proof
                    .first()
                    .and_then(|row| count(&row["spans"]))
                    .unwrap_or(0);
                let (Some(first), Some(last), true) =
                    (bound("first_ns"), bound("last_ns"), spans > 0)
                else {
                    return Err(ConnectError::not_found(
                        "Trace does not belong to this agent",
                    ));
                };
                // Other participants' spans start within the trace's lifetime; a day either
                // side keeps the unscoped read bounded to a few partitions.
                let day = 86_400_000_000_000;
                let window = position
                    .as_ref()
                    .map(|c| (c.from, c.to))
                    .unwrap_or((first.saturating_sub(day).max(0), last.saturating_add(day)));
                let mut query =
                    Self::query(&agent, &pb::ObservationFilter::default(), Some(window))?;
                query.parameters.push(parameter("trace", trace));
                query.identity = hex::encode(Sha256::digest(format!("{agent}\0trace\0{trace}")));
                (
                    include_str!("../../../../../queries/_traces/trace.sql").to_owned(),
                    query,
                )
            }
        };
        if position
            .as_ref()
            .is_some_and(|c| c.filter != query.identity)
        {
            return Err(mismatch());
        }
        let (time, trace, span) = position
            .map(|c| (c.time, c.trace, c.span))
            .unwrap_or_default();
        query.parameters.extend([
            parameter("cursor_time", time.to_string()),
            parameter("cursor_trace", trace),
            parameter("cursor_span", span),
        ]);
        let mut rows = self.run(&sql, &query.parameters).await?;
        let more = rows.len() > PAGE;
        rows.truncate(PAGE);
        let next_cursor = match rows.last() {
            Some(last) if more => URL_SAFE_NO_PAD.encode(
                serde_json::to_vec(&Cursor {
                    time: text(last, "start_ns").parse().unwrap_or_default(),
                    trace: text(last, "trace_id"),
                    span: text(last, "id"),
                    from: query.from,
                    to: query.to,
                    filter: query.identity.clone(),
                })
                .map_err(|_| ConnectError::internal("Unable to encode cursor"))?,
            ),
            _ => String::new(),
        };
        let observations: Vec<_> = rows.iter().map(observation).collect();
        Ok(pb::ListObservationsResponse {
            sessions: session_summaries(&observations),
            partial: !next_cursor.is_empty(),
            observations,
            next_cursor,
            ..Default::default()
        })
    }
}

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}
/// ClickHouse renders 64-bit integers as JSON strings.
fn count(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|v| v.parse().ok()))
}
fn number(row: &Value, key: &str) -> Option<f64> {
    row.get(key)
        .and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|v| v.parse().ok()))
        })
        .filter(|n| n.is_finite())
}
fn observation(row: &Value) -> pb::Observation {
    let start: i64 = text(row, "start_ns").parse().unwrap_or_default();
    let duration: i64 = text(row, "duration_ns").parse().unwrap_or_default();
    let started = rfc3339(start);
    let (input_tokens, output_tokens) = (number(row, "input_tokens"), number(row, "output_tokens"));
    let first_token = chrono::DateTime::parse_from_rfc3339(&text(row, "completion_start_time"))
        .ok()
        .and_then(|at| at.timestamp_nanos_opt())
        .filter(|at| *at >= start)
        .map(|at| (at - start) as f64 / 1e9);
    let mut attributes = Vec::new();
    for (prefix, key) in [("", "attributes"), ("resource.", "resource_attributes")] {
        if let Some(map) = row.get(key).and_then(Value::as_object) {
            attributes.extend(map.iter().map(|(name, value)| {
                pb::TraceAttribute {
                    key: format!("{prefix}{name}"),
                    value: value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string()),
                    ..Default::default()
                }
            }));
        }
    }
    pb::Observation {
        id: text(row, "id"),
        trace_id: text(row, "trace_id"),
        parent_id: text(row, "parent_id"),
        name: text(row, "name"),
        r#type: text(row, "type"),
        level: text(row, "level"),
        status_message: text(row, "status_message"),
        end_time: rfc3339(start.saturating_add(duration)),
        // Spans are immutable here; the start time is a stable version for client merging.
        updated_time: started.clone(),
        start_time: started,
        input: text(row, "input"),
        output: text(row, "output"),
        model: text(row, "model"),
        latency_seconds: Some(duration as f64 / 1e9),
        time_to_first_token_seconds: first_token,
        total_tokens: match (input_tokens, output_tokens) {
            (None, None) => None,
            (input, output) => Some(input.unwrap_or_default() + output.unwrap_or_default()),
        },
        input_tokens,
        output_tokens,
        cost_usd: number(row, "cost_usd"),
        session_id: text(row, "session_id"),
        invocation_id: text(row, "invocation_id"),
        agent_id: text(row, "agent_id"),
        attributes,
        ..Default::default()
    }
}

/// Aggregate the returned page. Counts are page-local; the browser re-aggregates across the
/// pages it has loaded.
fn session_summaries(observations: &[pb::Observation]) -> Vec<pb::TraceSessionSummary> {
    let mut indices = BTreeMap::<&str, usize>::new();
    let mut sessions = Vec::<pb::TraceSessionSummary>::new();
    for row in observations {
        if row.session_id.is_empty() {
            continue;
        }
        let index = indices.entry(&row.session_id).or_insert_with(|| {
            let index = sessions.len();
            sessions.push(pb::TraceSessionSummary {
                id: row.session_id.clone(),
                ..Default::default()
            });
            index
        });
        let session = &mut sessions[*index];
        apply_session_metadata(session, row);
        if row.level == "ERROR" {
            session.error_count += 1;
        }
        if let Some(tokens) = row.total_tokens {
            session.total_tokens = Some(session.total_tokens.unwrap_or_default() + tokens);
        }
        if let Some(cost) = row.cost_usd {
            session.cost_usd = Some(session.cost_usd.unwrap_or_default() + cost);
        }
    }
    sessions
}
// Compare capture times, not counts: a later trace can legitimately report fewer
// retained messages after deletion. Copy a whole snapshot from one observation.
fn apply_session_metadata(session: &mut pb::TraceSessionSummary, row: &pb::Observation) {
    let value = |key: &str| {
        row.attributes
            .iter()
            .find(|a| a.key == format!("tilde.session.{key}"))
            .map(|a| {
                serde_json::from_str::<Value>(&a.value)
                    .unwrap_or_else(|_| Value::String(a.value.clone()))
            })
    };
    let text = |key: &str| {
        value(key)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    };
    let at = text("metadata_time");
    let Ok(incoming) = chrono::DateTime::parse_from_rfc3339(&at) else {
        return;
    };
    if chrono::DateTime::parse_from_rfc3339(&session.metadata_time)
        .is_ok_and(|current| current >= incoming)
    {
        return;
    }
    session.metadata_time = at;
    session.provider_id = text("provider_id");
    session.provider_name = text("provider_name");
    session.provider_icon_url = text("provider_icon_url");
    session.last_turn_time = text("last_turn_time");
    session.message_count = value("message_count").and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    });
    session.identity_ids = value("identity_ids")
        .map(|v| strings(&v))
        .unwrap_or_default()
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok().map(|id| id.to_string()))
        .collect();
}
/// An OTLP array attribute reaches the attribute map as JSON text: either a plain array, or
/// the exporter's rendering of the OTLP value (`{"values":[{"stringValue":"…"}]}`).
fn strings(value: &Value) -> Vec<String> {
    match value {
        Value::String(text) => vec![text.clone()],
        Value::Array(items) => items.iter().flat_map(strings).collect(),
        Value::Object(fields) => fields.values().flat_map(strings).collect(),
        _ => Vec::new(),
    }
}
async fn resolve_identities(
    pool: &Pool,
    agent: Uuid,
    sessions: &mut [pb::TraceSessionSummary],
) -> Result<(), ConnectError> {
    let ids: Vec<_> = sessions
        .iter()
        .filter_map(|s| Uuid::parse_str(&s.id).ok())
        .collect();
    let identities: Vec<_> = sessions
        .iter()
        .flat_map(|s| &s.identity_ids)
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();
    let labels = if ids.is_empty() || identities.is_empty() {
        Vec::new()
    } else {
        super::db::session_identity_labels_all(&pool.get().await?, agent, &ids, &identities).await?
    };
    let labels: BTreeMap<_, _> = labels
        .into_iter()
        .map(|row| ((row.session_id, row.identity_id), row.value))
        .collect();
    for session in sessions {
        session.identities = session
            .identity_ids
            .iter()
            .map(|value| {
                Uuid::parse_str(&session.id)
                    .ok()
                    .zip(Uuid::parse_str(value).ok())
                    .and_then(|key| labels.get(&key))
                    .cloned()
                    .unwrap_or_else(|| value.clone())
            })
            .collect();
    }
    Ok(())
}

pub fn router(reader: Reader) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(reader)),
        1024 * 1024,
    )
}
impl TracingService for Reader {
    async fn get_trace_object_url<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, pb::GetTraceObjectUrlRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::GetTraceObjectUrlResponse> + Send + use<'a>>
    {
        Response::ok(pb::GetTraceObjectUrlResponse {
            url: self.object_url(r.agent_id, r.key).await?,
            expires_in: 600,
            ..Default::default()
        })
    }
    async fn list_observations<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, pb::ListObservationsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::ListObservationsResponse> + Send + use<'a>>
    {
        let r = r.to_owned_message();
        let mut page = self
            .list(
                &r.agent_id,
                r.filter.into_option().unwrap_or_default(),
                &r.cursor,
                None,
            )
            .await?;
        if r.include_session_details {
            resolve_identities(
                &self.pool,
                Uuid::parse_str(&r.agent_id).expect("validated agent"),
                &mut page.sessions,
            )
            .await?;
        }
        Response::ok(page)
    }

    async fn get_observation_metrics<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, pb::GetObservationMetricsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::GetObservationMetricsResponse> + Send + use<'a>>
    {
        let r = r.to_owned_message();
        Response::ok(
            self.metrics(&r.agent_id, r.filter.into_option().unwrap_or_default())
                .await?,
        )
    }
    async fn get_trace<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, pb::GetTraceRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::GetTraceResponse> + Send + use<'a>> {
        if r.trace_id.is_empty() {
            return Err(ConnectError::invalid_argument("Trace ID is required"));
        }
        let page = self
            .list(
                r.agent_id,
                pb::ObservationFilter::default(),
                r.cursor,
                Some(r.trace_id),
            )
            .await?;
        Response::ok(pb::GetTraceResponse {
            observations: page.observations,
            next_cursor: page.next_cursor,
            partial: page.partial,
            ..Default::default()
        })
    }
    async fn get_session<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, pb::GetSessionRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::GetSessionResponse> + Send + use<'a>> {
        let r = r.to_owned_message();
        if r.session_id.is_empty() {
            return Err(ConnectError::invalid_argument("Session ID is required"));
        }
        let mut filter = r.filter.into_option().unwrap_or_default();
        filter.conditions.push(pb::FilterCondition {
            column: "session_id".into(),
            operator: "=".into(),
            values: vec![r.session_id],
            ..Default::default()
        });
        let page = self.list(&r.agent_id, filter, &r.cursor, None).await?;
        Response::ok(pb::GetSessionResponse {
            observations: page.observations,
            next_cursor: page.next_cursor,
            partial: page.partial,
            ..Default::default()
        })
    }
}
