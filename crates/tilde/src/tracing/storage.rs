use super::INVOCATION_HEADER;
use crate::error::Error;
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::ResourceSpans,
};
use prost::Message as _;
use rotel::{
    bounded_channel::BoundedReceiver,
    topology::payload::{Ack, ExporterError, Message, RequestContext},
};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

pub(super) async fn run(
    pool: PgPool,
    mut receiver: BoundedReceiver<Vec<Message<ResourceSpans>>>,
    forward: bool,
) {
    while let Some(mut batch) = receiver.next().await {
        // Scope comes only from middleware, never from untrusted OTLP attributes.
        let mut valid = Vec::new();
        for mut message in batch.drain(..) {
            match validate(&pool, &mut message).await {
                Ok(()) => valid.push(message),
                Err(Error::Denied) => {
                    if let Some(meta) = &message.metadata {
                        meta.reject(400).await;
                    }
                }
                Err(_) => {
                    if let Some(meta) = &message.metadata {
                        let _ = meta
                            .nack(ExporterError::ExportFailed {
                                error_code: 503,
                                error_message: "Trace scope unavailable".into(),
                            })
                            .await;
                    }
                }
            }
        }
        if valid.is_empty() {
            continue;
        }
        let mut retry = 1;
        loop {
            let result =
                tokio::time::timeout(Duration::from_secs(10), persist(&pool, &valid, forward))
                    .await;
            if matches!(result, Ok(Ok(()))) {
                for message in &valid {
                    if let Some(meta) = &message.metadata {
                        let _ = meta.ack().await;
                    }
                }
                break;
            }
            // Retain the bounded batch across database outages. Notify HTTP
            // callers to retry, but do not discard platform spans on failure.
            for message in &valid {
                if let Some(meta) = &message.metadata {
                    let _ = meta
                        .nack(ExporterError::ExportFailed {
                            error_code: 503,
                            error_message: "Trace persistence unavailable".into(),
                        })
                        .await;
                }
            }
            ::tracing::warn!(
                "Trace persistence unavailable; retaining batch and applying backpressure"
            );
            tokio::time::sleep(Duration::from_secs(retry)).await;
            retry = (retry * 2).min(30);
        }
    }
}
pub(super) fn validate_request(message: &Message<ResourceSpans>) -> Result<(), u16> {
    let Some(RequestContext::Http(headers)) = &message.request_context else {
        return Err(400);
    };
    let parent = headers.get("x-tilde-trace-parent").ok_or(400u16)?;
    let parts: Vec<_> = parent.split('-').collect();
    if parts.len() != 4 {
        return Err(400);
    }
    for resource in &message.payload {
        for scope in &resource.scope_spans {
            for span in &scope.spans {
                if span.trace_id.len() != 16
                    || span.span_id.len() != 8
                    || span.span_id.iter().all(|b| *b == 0)
                    || !matches!(span.parent_span_id.len(), 0 | 8)
                    || span.end_time_unix_nano < span.start_time_unix_nano
                    || span.end_time_unix_nano > i64::MAX as u64
                    || parts[1] != hex::encode(&span.trace_id)
                    || parts[2] == hex::encode(&span.span_id)
                {
                    return Err(400);
                }
            }
        }
    }
    Ok(())
}
async fn validate(pool: &PgPool, message: &mut Message<ResourceSpans>) -> Result<(), Error> {
    let invocation = match &message.request_context {
        Some(RequestContext::Http(headers)) => Some(
            headers
                .get(INVOCATION_HEADER)
                .ok_or(Error::Denied)?
                .parse::<Uuid>()
                .map_err(|_| Error::Denied)?,
        ),
        None => None,
        _ => return Err(Error::Denied),
    };
    let scope = if let Some(invocation) = invocation {
        Some(
            sqlx::query_file!("../../queries/tracing/invocation.sql", invocation)
                .fetch_optional(pool)
                .await?
                .ok_or(Error::Denied)?,
        )
    } else {
        None
    };
    for resource in &mut message.payload {
        for scope_spans in &mut resource.scope_spans {
            for span in &mut scope_spans.spans {
                if span.trace_id.len() != 16
                    || span.trace_id.iter().all(|b| *b == 0)
                    || span.span_id.len() != 8
                    || span.span_id.iter().all(|b| *b == 0)
                    || !matches!(span.parent_span_id.len(), 0 | 8)
                    || span.end_time_unix_nano < span.start_time_unix_nano
                    || span.end_time_unix_nano > i64::MAX as u64
                {
                    return Err(Error::Denied);
                }
                if let Some(scope) = &scope {
                    let parts: Vec<_> = scope.execution_traceparent.split('-').collect();
                    if parts.len() != 4
                        || parts[1] != hex::encode(&span.trace_id)
                        || parts[2] == hex::encode(&span.span_id)
                    {
                        return Err(Error::Denied);
                    }
                    span.attributes.retain(|a| !a.key.starts_with("tilde."));
                    for (key, value) in [
                        ("tilde.invocation.id", invocation.unwrap()),
                        ("tilde.agent.id", scope.agent_id),
                        ("tilde.run.id", scope.run_id),
                        ("tilde.thread.id", scope.thread_id),
                    ] {
                        span.attributes.push(KeyValue {
                            key: key.into(),
                            value: Some(AnyValue {
                                value: Some(Value::StringValue(value.to_string())),
                            }),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
    Ok(())
}
async fn persist(
    pool: &PgPool,
    batch: &[Message<ResourceSpans>],
    forward: bool,
) -> Result<(), Error> {
    let mut tx = pool.begin().await?;
    let mut rows = Rows::default();
    let mut buffer_bytes = 0;
    for message in batch {
        for resource in &message.payload {
            for scope in &resource.scope_spans {
                for span in &scope.spans {
                    let envelope = ResourceSpans {
                        resource: resource.resource.clone(),
                        schema_url: resource.schema_url.clone(),
                        scope_spans: vec![opentelemetry_proto::tonic::trace::v1::ScopeSpans {
                            scope: scope.scope.clone(),
                            schema_url: scope.schema_url.clone(),
                            spans: vec![span.clone()],
                        }],
                    };
                    let otlp = ExportTraceServiceRequest {
                        resource_spans: vec![envelope],
                    }
                    .encode_to_vec();
                    buffer_bytes += otlp.len();
                    rows.otlp.push(otlp);
                    rows.trace_ids.push(span.trace_id.clone());
                    rows.span_ids.push(span.span_id.clone());
                    rows.parents.push(span.parent_span_id.clone());
                    let invocation = span
                        .attributes
                        .iter()
                        .find(|a| a.key == "tilde.invocation.id")
                        .and_then(|a| a.value.as_ref())
                        .and_then(|v| match &v.value {
                            Some(Value::StringValue(v)) => v.parse::<Uuid>().ok(),
                            _ => None,
                        });
                    rows.invocations.push(invocation);
                    rows.starts.push(span.start_time_unix_nano as i64);
                    rows.ends.push(span.end_time_unix_nano as i64);
                    rows.deliveries.push(if forward {
                        "pending".to_owned()
                    } else {
                        "disabled".to_owned()
                    });
                    if buffer_bytes >= 4 * 1024 * 1024 {
                        rows.flush(&mut tx).await?;
                        buffer_bytes = 0;
                    }
                }
            }
        }
    }
    rows.flush(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Column buffers for one bounded bulk insert; a Rotel batch may need several.
#[derive(Default)]
struct Rows {
    trace_ids: Vec<Vec<u8>>,
    span_ids: Vec<Vec<u8>>,
    parents: Vec<Vec<u8>>,
    invocations: Vec<Option<Uuid>>,
    starts: Vec<i64>,
    ends: Vec<i64>,
    otlp: Vec<Vec<u8>>,
    deliveries: Vec<String>,
}
impl Rows {
    async fn flush(&mut self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), Error> {
        if self.trace_ids.is_empty() {
            return Ok(());
        }
        sqlx::query_file!(
            "../../queries/tracing/insert.sql",
            &self.trace_ids,
            &self.span_ids,
            &self.parents,
            &self.invocations as &[Option<Uuid>],
            &self.starts,
            &self.ends,
            &self.otlp,
            &self.deliveries
        )
        .execute(&mut **tx)
        .await?;
        *self = Self::default();
        Ok(())
    }
}
