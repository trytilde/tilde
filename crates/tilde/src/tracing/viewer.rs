//! Management-only Langfuse reads. Filters always include authenticated installation/agent scope.
use super::Destination;
use crate::{proto::tilde::management::v1 as pb, services::tilde::management::v1::TracingService};
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;

type Cache = BTreeMap<String, (Instant, Value)>;
#[derive(Clone)]
pub struct Reader {
    pool: PgPool,
    destination: Option<Destination>,
    cache: Arc<Mutex<Cache>>,
    project: Arc<Mutex<Option<String>>>,
    next_allowed: Arc<Mutex<Option<Instant>>>,
}
impl Reader {
    pub fn new(pool: PgPool, destination: Option<Destination>) -> Self {
        Self {
            pool,
            destination,
            cache: Arc::new(Mutex::new(BTreeMap::new())),
            project: Arc::new(Mutex::new(None)),
            next_allowed: Arc::new(Mutex::new(None)),
        }
    }
    async fn agent(&self, value: &str) -> Result<String, ConnectError> {
        let id = Uuid::parse_str(value)
            .map_err(|_| ConnectError::invalid_argument("Agent ID must be a UUID"))?;
        if !sqlx::query_file!("../../queries/tracing/agent_exists.sql", id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| ConnectError::unavailable("Agent registry unavailable"))?
            .exists
        {
            return Err(ConnectError::not_found("Agent not found"));
        }
        Ok(id.to_string())
    }
    async fn fetch(
        &self,
        path: &str,
        query: &[(String, String)],
        refresh: bool,
    ) -> Result<Value, ConnectError> {
        let destination = self
            .destination
            .as_ref()
            .ok_or_else(|| ConnectError::failed_precondition("Tracing is not configured"))?;
        let base = destination
            .base
            .as_ref()
            .ok_or_else(|| ConnectError::failed_precondition("Langfuse is not configured"))?;
        let mut url = url::Url::parse(&format!(
            "{}/{}",
            base.as_str().trim_end_matches('/'),
            path.trim_start_matches('/')
        ))
        .map_err(|_| ConnectError::internal("Invalid Langfuse URL"))?;
        url.query_pairs_mut()
            .extend_pairs(query.iter().map(|(k, v)| (k, v)));
        // The lock coalesces concurrent identical reads; bounded cache entries expire quickly.
        let mut cache = self.cache.lock().await;
        if !refresh
            && let Some((time, value)) = cache.get(url.as_str())
            && time.elapsed() < Duration::from_secs(10)
        {
            return Ok(value.clone());
        }
        if self
            .next_allowed
            .lock()
            .await
            .is_some_and(|deadline| Instant::now() < deadline)
        {
            return Err(ConnectError::resource_exhausted(
                "Langfuse rate limit reached; retry later",
            ));
        }
        let mut response = destination
            .client
            .get(url.clone())
            .headers(destination.headers())
            .send()
            .await
            .map_err(|_| ConnectError::unavailable("Langfuse is unavailable"))?;
        if response.status().as_u16() == 429 {
            let seconds = response
                .headers()
                .get(http::header::RETRY_AFTER)
                .and_then(|h| h.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60)
                .clamp(1, 3600);
            *self.next_allowed.lock().await = Some(Instant::now() + Duration::from_secs(seconds));
        }
        if !response.status().is_success() {
            return Err(match response.status().as_u16() {
                401 | 403 => ConnectError::unavailable("Langfuse credentials were rejected"),
                429 => {
                    ConnectError::resource_exhausted("Langfuse rate limit reached; retry shortly")
                }
                400 => ConnectError::invalid_argument("Langfuse rejected the filter or cursor"),
                _ => ConnectError::unavailable("Langfuse is unavailable"),
            });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ConnectError::unavailable("Langfuse response failed"))?
        {
            if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
                return Err(ConnectError::resource_exhausted(
                    "Langfuse response is too large; narrow the date range",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| ConnectError::unavailable("Invalid Langfuse response"))?;
        cache.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(10));
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(url.to_string(), (Instant::now(), value.clone()));
        Ok(value)
    }
    async fn project_id(&self) -> Result<String, ConnectError> {
        if let Some(id) = self.project.lock().await.clone() {
            return Ok(id);
        }
        let value = self.fetch("api/public/projects", &[], false).await?;
        let id = value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(|v| v.get("id"))
            .and_then(Value::as_str)
            .ok_or_else(|| ConnectError::unavailable("Langfuse project was not found"))?
            .to_owned();
        *self.project.lock().await = Some(id.clone());
        Ok(id)
    }
    fn link(&self, project: &str, kind: &str, id: &str, observation: Option<&str>) -> String {
        let Some(base) = self
            .destination
            .as_ref()
            .and_then(|d| d.public_base.as_ref())
        else {
            return String::new();
        };
        if project.is_empty() {
            return String::new();
        }
        let mut url = base.clone();
        if let Ok(mut segments) = url.path_segments_mut() {
            segments.pop_if_empty().push("project").push(project);
            if !kind.is_empty() {
                segments.push(kind).push(id);
            }
        }
        if let Some(id) = observation {
            url.query_pairs_mut().append_pair("observation", id);
        }
        url.into()
    }
    pub async fn status(&self, agent: &str) -> Result<pb::GetTracingStatusResponse, ConnectError> {
        self.agent(agent).await?;
        if self.destination.is_none() {
            return Ok(pb::GetTracingStatusResponse {
                state: pb::TracingState::Disabled.into(),
                message: "Tracing is not configured for this installation".into(),
                ..Default::default()
            });
        }
        // Health checks are cached briefly, while known links survive an outage.
        match self.fetch("api/public/projects", &[], false).await {
            Ok(value) => {
                let project = value["data"]
                    .as_array()
                    .and_then(|rows| rows.first())
                    .and_then(|p| p["id"].as_str())
                    .unwrap_or("")
                    .to_owned();
                if project.is_empty() {
                    return Ok(pb::GetTracingStatusResponse {
                        state: pb::TracingState::Unavailable.into(),
                        message: "Langfuse project is unavailable".into(),
                        ..Default::default()
                    });
                }
                *self.project.lock().await = Some(project.clone());
                Ok(pb::GetTracingStatusResponse {
                    state: pb::TracingState::Ready.into(),
                    project_url: self.link(&project, "", "", None),
                    ..Default::default()
                })
            }
            Err(_) => {
                let project = self.project.lock().await.clone().unwrap_or_default();
                Ok(pb::GetTracingStatusResponse {
                    state: pb::TracingState::Unavailable.into(),
                    message: "Langfuse is unavailable; check connectivity and project credentials"
                        .into(),
                    project_url: self.link(&project, "", "", None),
                    ..Default::default()
                })
            }
        }
    }
    pub async fn list(
        &self,
        agent: &str,
        filter: pb::ObservationFilter,
        cursor: &str,
        trace_id: Option<&str>,
        refresh: bool,
    ) -> Result<pb::ListObservationsResponse, ConnectError> {
        let agent = self.agent(agent).await?;
        if cursor.len() > 8192 {
            return Err(ConnectError::invalid_argument("Invalid cursor"));
        }
        let project = self.project_id().await?;
        let mut conditions = vec![
            json!({"type":"stringObject","column":"metadata","key":"tilde_agent_id","operator":"=","value":agent}),
        ];
        if trace_id.is_none() || !filter.from_time.is_empty() {
            let end = if filter.to_time.is_empty() {
                chrono::Utc::now()
            } else {
                chrono::DateTime::parse_from_rfc3339(&filter.to_time)
                    .map_err(|_| ConnectError::invalid_argument("Invalid end time"))?
                    .with_timezone(&chrono::Utc)
            };
            let start = if filter.from_time.is_empty() {
                end - chrono::Duration::hours(24)
            } else {
                chrono::DateTime::parse_from_rfc3339(&filter.from_time)
                    .map_err(|_| ConnectError::invalid_argument("Invalid start time"))?
                    .with_timezone(&chrono::Utc)
            };
            if start >= end {
                return Err(ConnectError::invalid_argument("Start must precede end"));
            }
            conditions.push(json!({"type":"datetime","column":"startTime","operator":">=","value":start.to_rfc3339()}));
            conditions.push(json!({"type":"datetime","column":"startTime","operator":"<","value":end.to_rfc3339()}));
        }
        for (column, value, operator) in [
            ("name", filter.name.as_str(), "contains"),
            ("type", filter.r#type.as_str(), "="),
            ("level", filter.level.as_str(), "="),
            ("model", filter.model.as_str(), "="),
            ("sessionId", filter.session_id.as_str(), "="),
            ("input", filter.input_search.as_str(), "matches"),
            ("output", filter.output_search.as_str(), "matches"),
            ("traceId", trace_id.unwrap_or(""), "="),
        ] {
            if value.len() > 4096 {
                return Err(ConnectError::invalid_argument("Filter value is too long"));
            }
            if !value.is_empty() {
                conditions.push(
                    json!({"type":"string","column":column,"operator":operator,"value":value}),
                );
            }
        }
        if !filter.invocation_id.is_empty() {
            Uuid::parse_str(&filter.invocation_id)
                .map_err(|_| ConnectError::invalid_argument("Invalid invocation ID"))?;
            conditions.push(json!({"type":"stringObject","column":"metadata","key":"tilde_invocation_id","operator":"=","value":filter.invocation_id}));
        }
        if let Some(trace) = trace_id {
            // A trace can contain gateway parents and multiple agents. Prove membership
            // with an agent-filtered lookup before returning that trace's full context.
            let proof = self
                .fetch(
                    "api/public/v2/observations",
                    &[
                        (
                            "filter".into(),
                            Value::Array(conditions.clone()).to_string(),
                        ),
                        ("fields".into(), "core,metadata".into()),
                        ("limit".into(), "1".into()),
                    ],
                    refresh,
                )
                .await?;
            if !proof["data"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    string(&row["metadata"], "tilde_agent_id") == agent
                        && string(row, "traceId") == trace
                })
            }) {
                return Err(ConnectError::not_found(
                    "Trace does not belong to this agent",
                ));
            }
            conditions.remove(0);
        }
        let mut query = vec![
            (
                "fields".into(),
                "core,basic,time,io,metadata,model,usage,metrics,trace_context".into(),
            ),
            (
                "expandMetadata".into(),
                "attributes,resourceAttributes".into(),
            ),
            ("limit".into(), "50".into()),
            ("filter".into(), Value::Array(conditions).to_string()),
        ];
        if !cursor.is_empty() {
            query.push(("cursor".into(), cursor.to_owned()));
        }
        let value = self
            .fetch("api/public/v2/observations", &query, refresh)
            .await?;
        let mut observations = vec![];
        let mut seen: BTreeMap<(String, String), (usize, String)> = BTreeMap::new();
        for row in value["data"]
            .as_array()
            .ok_or_else(|| ConnectError::unavailable("Invalid observations response"))?
        {
            // Defense in depth if the upstream filter behavior changes.
            if match trace_id {
                Some(trace) => string(row, "traceId") != trace,
                None => string(&row["metadata"], "tilde_agent_id") != agent,
            } {
                continue;
            }
            let id = string(row, "id");
            let trace = string(row, "traceId");
            let session = string(row, "sessionId");
            let attributes = row["metadata"]
                .as_object()
                .map(|m| {
                    m.iter()
                        .map(|(key, value)| pb::TraceAttribute {
                            key: key.clone(),
                            value: if let Some(s) = value.as_str() {
                                s.to_owned()
                            } else {
                                value.to_string()
                            },
                            ..Default::default()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let observation = pb::Observation {
                id: id.clone(),
                trace_id: trace.clone(),
                parent_id: string(row, "parentObservationId"),
                name: string(row, "name"),
                r#type: string(row, "type"),
                level: string(row, "level"),
                status_message: string(row, "statusMessage"),
                start_time: string(row, "startTime"),
                end_time: string(row, "endTime"),
                input: io(row, "input"),
                output: io(row, "output"),
                model: string(row, "model"),
                latency_seconds: number(row, "latency"),
                time_to_first_token_seconds: number(row, "timeToFirstToken"),
                input_tokens: number(row, "inputUsage"),
                output_tokens: number(row, "outputUsage"),
                total_tokens: number(row, "totalUsage"),
                cost_usd: number(row, "totalCost"),
                session_id: session.clone(),
                user_id: string(row, "userId"),
                environment: string(row, "environment"),
                tags: row["tags"]
                    .as_array()
                    .map(|v| {
                        v.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
                attributes,
                observation_url: self.link(&project, "traces", &trace, Some(&id)),
                trace_url: self.link(&project, "traces", &trace, None),
                session_url: if session.is_empty() {
                    String::new()
                } else {
                    self.link(&project, "sessions", &session, None)
                },
                invocation_id: string(&row["metadata"], "tilde_invocation_id"),
                updated_time: string(row, "updatedAt"),
                ..Default::default()
            };
            let key = (trace, id);
            let version = string(row, "updatedAt");
            if let Some((index, previous)) = seen.get_mut(&key) {
                if version > *previous {
                    observations[*index] = observation;
                    *previous = version;
                }
            } else {
                seen.insert(key, (observations.len(), version));
                observations.push(observation);
            }
        }
        let next_cursor = string(&value["meta"], "cursor");
        Ok(pb::ListObservationsResponse {
            observations,
            partial: !next_cursor.is_empty(),
            next_cursor,
            ..Default::default()
        })
    }
}
fn string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}
fn io(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}
fn number(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|v| v.parse().ok()))
        })
        .filter(|n| n.is_finite())
}
pub fn router(reader: Reader) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(reader)),
        1024 * 1024,
    )
}
impl TracingService for Reader {
    async fn get_tracing_status<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, pb::GetTracingStatusRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::GetTracingStatusResponse> + Send + use<'a>>
    {
        Response::ok(self.status(r.agent_id).await?)
    }
    async fn list_observations<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, pb::ListObservationsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::ListObservationsResponse> + Send + use<'a>>
    {
        let r = r.to_owned_message();
        Response::ok(
            self.list(
                &r.agent_id,
                r.filter.into_option().unwrap_or_default(),
                &r.cursor,
                None,
                r.refresh,
            )
            .await?,
        )
    }
    async fn get_trace<'a>(
        &'a self,
        _: RequestContext,
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
                r.refresh,
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
        _: RequestContext,
        r: ServiceRequest<'_, pb::GetSessionRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::GetSessionResponse> + Send + use<'a>> {
        let r = r.to_owned_message();
        if r.session_id.is_empty() {
            return Err(ConnectError::invalid_argument("Session ID is required"));
        }
        let mut filter = r.filter.into_option().unwrap_or_default();
        filter.session_id = r.session_id;
        let page = self
            .list(&r.agent_id, filter, &r.cursor, None, r.refresh)
            .await?;
        Response::ok(pb::GetSessionResponse {
            observations: page.observations,
            next_cursor: page.next_cursor,
            partial: page.partial,
            ..Default::default()
        })
    }
}
