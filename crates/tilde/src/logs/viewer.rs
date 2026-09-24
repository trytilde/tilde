//! Bounded read-only queries. The agent predicate is supplied by the server on every page.
use super::clickhouse::Store;
use crate::{proto::tilde::management::v1 as pb, services::tilde::management::v1::LogsService};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
#[derive(Clone)]
pub struct Reader {
    pool: crate::database::Pool,
    store: Option<Store>,
    forwarding: bool,
    admission: Arc<tokio::sync::Semaphore>,
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    time: i64,
    id: String,
    from: i64,
    to: i64,
    filter: String,
}
impl Reader {
    pub fn new(pool: crate::database::Pool, store: Option<Store>, forwarding: bool) -> Self {
        Self {
            pool,
            store,
            forwarding,
            admission: Arc::new(tokio::sync::Semaphore::new(8)),
        }
    }
    async fn agent(&self, agent: &str) -> Result<String, ConnectError> {
        let id = uuid::Uuid::parse_str(agent)
            .map_err(|_| ConnectError::invalid_argument("Agent ID must be a UUID"))?;
        if !crate::telemetry::db::agent_exists_one(&self.pool.get().await?, id)
            .await
            .map_err(|_| ConnectError::unavailable("Agent registry unavailable"))?
            .exists
        {
            return Err(ConnectError::not_found("Agent not found"));
        }
        Ok(id.to_string())
    }
    pub async fn metrics(
        &self,
        r: pb::GetLogMetricsRequest,
    ) -> Result<pb::GetLogMetricsResponse, ConnectError> {
        let _permit =
            self.admission.clone().try_acquire_owned().map_err(|_| {
                ConnectError::resource_exhausted("Too many log queries; retry shortly")
            })?;
        let agent = self.agent(&r.agent_id).await?;
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| ConnectError::failed_precondition("Log storage is disabled"))?;
        let filter = r.filter.into_option().unwrap_or_default();
        validate_filter(&filter)?;
        let (from, to) = time_range(&filter, None)?;
        // HyperDX convertDateRangeToGranularityString: target 60 buckets with
        // human-friendly intervals. The 30-day API bound needs at most 12 hours.
        // https://github.com/hyperdxio/hyperdx/blob/c98be91f618fca6931ae0aea75553ea8b6573551/packages/common-utils/src/core/utils.ts
        let target = ((to - from) / 1_000_000_000 + 59) / 60;
        let interval = [15i64, 30, 60, 300, 900, 1800, 3600, 7200, 21600, 43200]
            .into_iter()
            .find(|seconds| *seconds >= target)
            .unwrap_or(43200);
        let step = interval * 1_000_000_000;
        let mut params = filter_parameters(agent, &filter, from, to);
        params.push(("param_bucket_seconds".into(), interval.to_string()));
        let response = store
            .query(
                include_str!("../../../../queries/_logs/metrics.sql"),
                &params,
            )
            .await
            .map_err(|_| {
                ConnectError::unavailable(
                    "Log activity query failed; narrow the time range or retry",
                )
            })?;
        let rows = response["data"]
            .as_array()
            .ok_or_else(|| ConnectError::unavailable("Invalid log activity response"))?;
        let mut counts = std::collections::BTreeMap::new();
        for row in rows {
            let time = text(row, "bucket_ns")
                .parse::<i64>()
                .map_err(|_| ConnectError::unavailable("Invalid log activity timestamp"))?;
            let value = |key: &str| {
                row[key]
                    .as_u64()
                    .or_else(|| text(row, key).parse().ok())
                    .ok_or_else(|| ConnectError::unavailable("Invalid log activity count"))
            };
            let count = value("log_count")?;
            let entry = counts.entry(time).or_insert((0u64, 0u64));
            entry.0 += count;
            if value("severity_number")? >= 17 {
                entry.1 += count;
            }
        }
        let mut buckets = Vec::new();
        let mut time = from.div_euclid(step) * step;
        while time < to {
            let end = time.saturating_add(step);
            let (log_count, error_count) = counts.get(&time).copied().unwrap_or_default();
            buckets.push(pb::LogMetricBucket {
                start_time: chrono::DateTime::from_timestamp_nanos(time.max(from)).to_rfc3339(),
                end_time: chrono::DateTime::from_timestamp_nanos(end.min(to)).to_rfc3339(),
                log_count,
                error_count,
                ..Default::default()
            });
            time = end;
        }
        Ok(pb::GetLogMetricsResponse {
            buckets,
            ..Default::default()
        })
    }
    pub async fn list(&self, r: pb::ListLogsRequest) -> Result<pb::ListLogsResponse, ConnectError> {
        let _permit =
            self.admission.clone().try_acquire_owned().map_err(|_| {
                ConnectError::resource_exhausted("Too many log queries; retry shortly")
            })?;
        let agent = self.agent(&r.agent_id).await?;
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| ConnectError::failed_precondition("Log storage is disabled"))?;
        let filter = r.filter.into_option().unwrap_or_default();
        validate_filter(&filter)?;
        use sha2::{Digest, Sha256};
        let identity = hex::encode(Sha256::digest(format!(
            "{agent}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            filter.search,
            filter.service,
            filter.invocation_id,
            filter.trace_id,
            filter.minimum_severity,
            filter.from_time,
            filter.to_time,
            filter.thread_id,
            filter.run_id,
            filter.scope
        )));
        let cursor = if r.cursor.is_empty() {
            None
        } else {
            if r.cursor.len() > 2048 {
                return Err(ConnectError::invalid_argument("Invalid cursor"));
            }
            Some(
                serde_json::from_slice::<Cursor>(
                    &URL_SAFE_NO_PAD
                        .decode(&r.cursor)
                        .map_err(|_| ConnectError::invalid_argument("Invalid cursor"))?,
                )
                .map_err(|_| ConnectError::invalid_argument("Invalid cursor"))?,
            )
        };
        if cursor.as_ref().is_some_and(|c| {
            c.filter != identity || c.id.len() != 64 || !c.id.bytes().all(|b| b.is_ascii_hexdigit())
        }) {
            return Err(ConnectError::invalid_argument(
                "Cursor does not match the filters",
            ));
        }
        let (from, to) = time_range(&filter, cursor.as_ref())?;
        let mut params = filter_parameters(agent, &filter, from, to);
        params.push((
            "param_cursor_id".into(),
            cursor.as_ref().map(|c| c.id.clone()).unwrap_or_default(),
        ));
        params.push((
            "param_cursor_time".into(),
            cursor.as_ref().map(|c| c.time).unwrap_or(0).to_string(),
        ));
        let response = store
            .query(include_str!("../../../../queries/_logs/list.sql"), &params)
            .await
            .map_err(|_| {
                ConnectError::unavailable("Log query failed; narrow the time range or retry")
            })?;
        let rows = response["data"]
            .as_array()
            .ok_or_else(|| ConnectError::unavailable("Invalid log query response"))?;
        let partial = rows.len() > 100;
        let mut records = Vec::new();
        let mut last = None;
        for row in rows.iter().take(100) {
            let ns = text(row, "timestamp_ns")
                .parse::<i64>()
                .map_err(|_| ConnectError::unavailable("Invalid log timestamp"))?;
            let id = text(row, "id");
            last = Some(Cursor {
                time: ns,
                id: id.clone(),
                from,
                to,
                filter: identity.clone(),
            });
            records.push(pb::LogRecord {
                id,
                timestamp: chrono::DateTime::from_timestamp_nanos(ns)
                    .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                severity: text(row, "severity"),
                severity_number: row["severity_number"].as_u64().unwrap_or(0) as u32,
                body: text(row, "body"),
                service: text(row, "service"),
                trace_id: text(row, "trace_id"),
                span_id: text(row, "span_id"),
                invocation_id: text(row, "invocation_id"),
                thread_id: text(row, "thread_id"),
                run_id: text(row, "run_id"),
                scope: text(row, "scope"),
                deployment_id: text(row, "deployment_id"),
                scope_name: text(row, "scope_name"),
                attributes: attrs(&row["attributes"]),
                resource_attributes: attrs(&row["resource_attributes"]),
                scope_attributes: attrs(&row["scope_attributes"]),
                ..Default::default()
            });
        }
        let next_cursor = if partial {
            last.map(|c| {
                URL_SAFE_NO_PAD.encode(serde_json::to_vec(&c).expect("cursor serialization"))
            })
            .unwrap_or_default()
        } else {
            String::new()
        };
        Ok(pb::ListLogsResponse {
            records,
            next_cursor,
            partial,
            ..Default::default()
        })
    }
}
fn validate_filter(filter: &pb::LogFilter) -> Result<(), ConnectError> {
    if !matches!(filter.scope.as_str(), "" | "deployment" | "invocation") {
        return Err(ConnectError::invalid_argument(
            "Scope must be deployment or invocation",
        ));
    }
    for value in [
        &filter.search,
        &filter.service,
        &filter.invocation_id,
        &filter.trace_id,
        &filter.thread_id,
        &filter.run_id,
    ] {
        if value.len() > 1024 {
            return Err(ConnectError::invalid_argument("Filter is too long"));
        }
    }
    for (label, value) in [
        ("invocation", &filter.invocation_id),
        ("session", &filter.thread_id),
        ("run", &filter.run_id),
    ] {
        if !value.is_empty() {
            uuid::Uuid::parse_str(value)
                .map_err(|_| ConnectError::invalid_argument(format!("Invalid {label} ID")))?;
        }
    }
    if !filter.trace_id.is_empty()
        && (filter.trace_id.len() != 32 || !filter.trace_id.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(ConnectError::invalid_argument("Invalid trace ID"));
    }
    if filter.minimum_severity > 24 {
        return Err(ConnectError::invalid_argument("Invalid severity"));
    }
    Ok(())
}
fn time_range(filter: &pb::LogFilter, cursor: Option<&Cursor>) -> Result<(i64, i64), ConnectError> {
    let parse = |s: &str| -> Result<i64, ConnectError> {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .and_then(|d| d.timestamp_nanos_opt())
            .ok_or_else(|| ConnectError::invalid_argument("Invalid log time bound"))
    };
    let to = if let Some(c) = &cursor {
        c.to
    } else if filter.to_time.is_empty() {
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    } else {
        parse(&filter.to_time)?
    };
    let from = if let Some(c) = &cursor {
        c.from
    } else if filter.from_time.is_empty() {
        to.saturating_sub(86400 * 1_000_000_000)
    } else {
        parse(&filter.from_time)?
    };
    if from < 0 || from >= to || to.saturating_sub(from) > 30 * 86400 * 1_000_000_000 {
        return Err(ConnectError::invalid_argument(
            "Use a time range of at most 30 days",
        ));
    }
    Ok((from, to))
}
fn filter_parameters(
    agent: String,
    filter: &pb::LogFilter,
    from: i64,
    to: i64,
) -> Vec<(String, String)> {
    vec![
        ("agent", agent),
        ("from", from.to_string()),
        ("to", to.to_string()),
        ("search", filter.search.clone()),
        // HyperDX's literal ILIKE fallback escapes wildcard metacharacters.
        (
            "search_pattern",
            format!(
                "%{}%",
                filter
                    .search
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            ),
        ),
        ("service", filter.service.clone()),
        ("trace", filter.trace_id.to_lowercase()),
        ("invocation", filter.invocation_id.clone()),
        ("thread", filter.thread_id.clone()),
        ("run", filter.run_id.clone()),
        ("scope", filter.scope.clone()),
        ("severity", filter.minimum_severity.to_string()),
    ]
    .into_iter()
    .map(|(key, value)| (format!("param_{key}"), value))
    .collect()
}
fn text(row: &Value, key: &str) -> String {
    row[key].as_str().unwrap_or_default().to_owned()
}
fn attrs(value: &Value) -> Vec<pb::LogAttribute> {
    value
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(k, v)| pb::LogAttribute {
                    key: k.clone(),
                    value: v
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string()),
                    ..Default::default()
                })
                .collect()
        })
        .unwrap_or_default()
}
pub fn router(reader: Reader) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(reader)),
        1024 * 1024,
    )
}
impl LogsService for Reader {
    async fn get_logs_status<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, pb::GetLogsStatusRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::GetLogsStatusResponse> + Send + use<'a>> {
        crate::iam::authz::require(
            &ctx,
            crate::iam::authz::Resource::agent(crate::iam::parse_id(r.agent_id)?),
            crate::iam::authz::Action::View,
        )
        .await?;
        self.agent(r.agent_id).await?;
        let (state, message) = match &self.store {
            None => (
                pb::LogsState::Disabled,
                if self.forwarding {
                    "Local log storage is disabled; external forwarding is enabled"
                } else {
                    "Logs are not configured for this installation"
                },
            ),
            Some(s) => {
                if s.ready().await.is_ok() {
                    (pb::LogsState::Ready, "")
                } else {
                    (
                        pb::LogsState::Unavailable,
                        "ClickHouse logs unavailable; accepted logs remain queued within delivery limits",
                    )
                }
            }
        };
        Response::ok(pb::GetLogsStatusResponse {
            state: state.into(),
            message: message.into(),
            forwarding_enabled: self.forwarding,
            ..Default::default()
        })
    }
    async fn get_log_metrics<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, pb::GetLogMetricsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::GetLogMetricsResponse> + Send + use<'a>> {
        crate::iam::authz::require(
            &ctx,
            crate::iam::authz::Resource::agent(crate::iam::parse_id(r.agent_id)?),
            crate::iam::authz::Action::View,
        )
        .await?;
        Response::ok(self.metrics(r.to_owned_message()).await?)
    }
    async fn list_logs<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, pb::ListLogsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<pb::ListLogsResponse> + Send + use<'a>> {
        crate::iam::authz::require(
            &ctx,
            crate::iam::authz::Resource::agent(crate::iam::parse_id(r.agent_id)?),
            crate::iam::authz::Action::View,
        )
        .await?;
        Response::ok(self.list(r.to_owned_message()).await?)
    }
}
