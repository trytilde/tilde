//! Agent OTLP arrives at the replica, is stamped with its verified invocation
//! scope, and travels to the gateway as Telemetry frames. Langfuse credentials
//! and external export exist only at the gateway.
use super::{Deployments, runtime::Runtime, sidecar::Node};
use crate::{
    chat::{ChatError, Result},
    error::Error,
    iam::tokens::Claims,
    proto::tilde::agent_event_ingress::v1 as wire,
};
use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use opentelemetry_proto::tonic::{
    collector::{logs::v1::ExportLogsServiceRequest, trace::v1::ExportTraceServiceRequest},
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::ResourceSpans,
};
use prost::Message as _;
use rotel::{
    bounded_channel::bounded,
    receivers::{otlp::otlp_http::build_service, otlp_output::OTLPOutput},
    topology::{
        batch::BatchConfig,
        fanout::Fanout,
        generic_pipeline::{Inspect, Pipeline},
        payload::{Ack, ExporterError, Message, RequestContext},
        processors::Processors,
    },
};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use tower::ServiceExt;
use uuid::Uuid;
pub(crate) fn normalize(
    body: &[u8],
    json: bool,
    claims: &Claims,
    parent: Option<&str>,
) -> Result<ExportTraceServiceRequest> {
    let mut request = if json {
        serde_json::from_slice::<ExportTraceServiceRequest>(body)
            .map_err(|_| ChatError::Invalid("Invalid OTLP JSON".into()))?
    } else {
        ExportTraceServiceRequest::decode(body)
            .map_err(|_| ChatError::Invalid("Invalid OTLP Protobuf".into()))?
    };
    let mut count = 0;
    for resource in &mut request.resource_spans {
        for scope in &mut resource.scope_spans {
            for span in &mut scope.spans {
                count += 1;
                if count > 4096
                    || span.trace_id.len() != 16
                    || span.trace_id.iter().all(|b| *b == 0)
                    || span.span_id.len() != 8
                    || span.span_id.iter().all(|b| *b == 0)
                    || !matches!(span.parent_span_id.len(), 0 | 8)
                    || span.end_time_unix_nano < span.start_time_unix_nano
                {
                    return Err(ChatError::Invalid("Invalid OTLP span".into()));
                }
                if let Some(parent) = parent {
                    let fields = parent.split('-').collect::<Vec<_>>();
                    if fields.len() != 4
                        || fields[1] != hex::encode(&span.trace_id)
                        || fields[2] == hex::encode(&span.span_id)
                    {
                        return Err(ChatError::Denied);
                    }
                }
                span.attributes.retain(|a| !a.key.starts_with("tilde."));
                for (key, id) in [
                    ("tilde.agent.id", claims.sub),
                    ("tilde.thread.id", claims.thread_id),
                    ("tilde.run.id", claims.run_id),
                    ("tilde.invocation.id", claims.invocation_id),
                ] {
                    span.attributes.push(KeyValue {
                        key: key.into(),
                        value: Some(AnyValue {
                            value: Some(Value::StringValue(id.to_string())),
                        }),
                        ..Default::default()
                    });
                }
            }
        }
    }
    Ok(request)
}
/// Queue a verified trace batch for the gateway when tracing is enabled.
pub fn relay_traces(runtime: &Runtime, request: &ExportTraceServiceRequest) -> Result<()> {
    if !runtime.configuration()?.tracing_enabled {
        return Ok(());
    }
    let payload = request.encode_to_vec();
    if payload.len() > 8 * 1024 * 1024 {
        return Err(ChatError::Invalid("Trace batch is too large".into()));
    }
    runtime.ensure_capacity()?;
    runtime.push(
        wire::Telemetry {
            kind: wire::TelemetryKind::Traces.into(),
            payload,
            ..Default::default()
        }
        .into(),
    );
    Ok(())
}
fn message_request(
    message: &Message<ResourceSpans>,
) -> std::result::Result<ExportTraceServiceRequest, u16> {
    let Some(RequestContext::Http(h)) = &message.request_context else {
        return Err(400);
    };
    let id = |key: &str| {
        h.get(key)
            .ok_or(400u16)?
            .parse::<Uuid>()
            .map_err(|_| 400u16)
    };
    let claims = Claims {
        iss: String::new(),
        aud: String::new(),
        sub: id("x-tilde-trace-agent")?,
        thread_id: id("x-tilde-trace-thread")?,
        invocation_id: id("x-tilde-trace-invocation")?,
        run_id: id("x-tilde-trace-run")?,
        capabilities: Default::default(),
        iat: 0,
        exp: 0,
    };
    let parent = h
        .get("x-tilde-trace-parent")
        .filter(|s| !s.is_empty())
        .map(String::as_str);
    normalize(
        &ExportTraceServiceRequest {
            resource_spans: message.payload.clone(),
        }
        .encode_to_vec(),
        false,
        &claims,
        parent,
    )
    .map_err(|_| 400)
}
struct Quiet;
impl Inspect<ResourceSpans> for Quiet {
    fn inspect(&self, _: &[ResourceSpans]) {}
    fn inspect_with_prefix(&self, _: Option<String>, _: &[ResourceSpans]) {}
}
pub fn router(node: Node) -> Router {
    let (tx, rx) = bounded(32);
    let (out_tx, mut out_rx) = bounded::<Vec<Message<ResourceSpans>>>(8);
    let mut pipeline = Pipeline::new(
        "traces",
        rx,
        Fanout::new("traces", vec![("sidecar", out_tx)]),
        None,
        BatchConfig {
            max_size: 512,
            timeout: Duration::from_millis(200),
            disabled: false,
        },
        Processors::empty(),
        vec![],
    );
    tokio::spawn(async move {
        let _ = pipeline
            .start(Quiet, tokio_util::sync::CancellationToken::new())
            .await;
    });
    let runtime = node.runtime.clone();
    tokio::spawn(async move {
        while let Some(batch) = out_rx.next().await {
            for message in batch {
                let request = match message_request(&message) {
                    Ok(value) => value,
                    Err(code) => {
                        if let Some(meta) = &message.metadata {
                            meta.reject(code).await;
                        }
                        continue;
                    }
                };
                match relay_traces(&runtime, &request) {
                    Ok(()) => {
                        if let Some(meta) = &message.metadata {
                            let _ = meta.ack().await;
                        }
                    }
                    Err(_) => {
                        if let Some(meta) = &message.metadata {
                            let _ = meta
                                .nack(ExporterError::ExportFailed {
                                    error_code: 503,
                                    error_message: "Trace relay unavailable".into(),
                                })
                                .await;
                        }
                    }
                }
            }
        }
    });
    let output = OTLPOutput::new(tx)
        .with_ack()
        .with_validator(|m| message_request(m).map(|_| ()));
    let service = build_service(
        Some(output),
        None,
        None,
        "/v1/traces".into(),
        "/v1/metrics".into(),
        "/v1/logs".into(),
        true,
        vec![
            "x-tilde-trace-agent".into(),
            "x-tilde-trace-thread".into(),
            "x-tilde-trace-invocation".into(),
            "x-tilde-trace-run".into(),
            "x-tilde-trace-parent".into(),
        ],
    );
    let service = tower::service_fn(move |request: Request| {
        let service = service.clone();
        async move {
            Ok::<_, std::convert::Infallible>(match service.oneshot(request).await {
                Ok(response) => response.map(axum::body::Body::new).into_response(),
                Err(_) => http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
            })
        }
    });
    Router::new()
        .route_service("/v1/traces", service)
        .merge(super::logs::router(node.clone()))
        .layer(middleware::from_fn_with_state(node, authorize))
}
async fn authorize(State(node): State<Node>, mut request: Request, next: Next) -> Response {
    let token = request
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    let Some(token) = token else {
        return http::StatusCode::UNAUTHORIZED.into_response();
    };
    let claims = match node.runtime.verified_claims(token, true).await {
        Ok(claims) => claims,
        Err(_) => return http::StatusCode::UNAUTHORIZED.into_response(),
    };
    let parent = node
        .runtime
        .invocation(claims.invocation_id)
        .await
        .map(|v| v.traceparent)
        .unwrap_or_default();
    for (key, value) in [
        ("x-tilde-trace-agent", claims.sub.to_string()),
        ("x-tilde-trace-thread", claims.thread_id.to_string()),
        ("x-tilde-trace-invocation", claims.invocation_id.to_string()),
        ("x-tilde-trace-run", claims.run_id.to_string()),
        ("x-tilde-trace-parent", parent),
    ] {
        let Ok(value) = value.parse() else {
            return http::StatusCode::BAD_REQUEST.into_response();
        };
        request.headers_mut().insert(key, value);
    }
    request.headers_mut().remove(http::header::AUTHORIZATION);
    match tokio::time::timeout(Duration::from_secs(15), next.run(request)).await {
        Ok(response) => response,
        Err(_) => http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
impl Deployments {
    /// The authenticated deployment, not payload attributes, owns the agent scope.
    pub(crate) async fn accept_telemetry(
        &self,
        agent: Uuid,
        telemetry: wire::Telemetry,
    ) -> std::result::Result<(), Error> {
        match telemetry.kind.as_known() {
            Some(wire::TelemetryKind::Traces) => {
                let Some(queue) = &self.telemetry else {
                    return Ok(());
                };
                let mut request =
                    ExportTraceServiceRequest::decode(telemetry.payload.as_slice())
                        .map_err(|_| Error::Invalid("Invalid relayed OTLP traces".into()))?;
                for resource in &mut request.resource_spans {
                    for scope in &mut resource.scope_spans {
                        for span in &mut scope.spans {
                            span.attributes.retain(|a| a.key != "tilde.agent.id");
                            span.attributes.push(KeyValue {
                                key: "tilde.agent.id".into(),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue(agent.to_string())),
                                }),
                                ..Default::default()
                            });
                        }
                    }
                }
                crate::telemetry::mapping::normalize(&mut request.resource_spans);
                queue.accept(&request.encode_to_vec()).await
            }
            Some(wire::TelemetryKind::Logs) => {
                let Some(delivery) = &self.logs else {
                    return Ok(());
                };
                if !delivery.enabled() {
                    return Ok(());
                }
                let request = ExportLogsServiceRequest::decode(telemetry.payload.as_slice())
                    .map_err(|_| Error::Invalid("Invalid relayed OTLP logs".into()))?;
                super::logs::accept(delivery, agent, request).await
            }
            _ => Ok(()),
        }
    }
}
pub type PlatformRoutes = Arc<RwLock<BTreeMap<Uuid, Arc<Runtime>>>>;
/// One process-wide provider routes internal spans by their server-assigned agent ID.
pub fn platform(
    routes: PlatformRoutes,
) -> (
    opentelemetry_sdk::trace::SdkTracerProvider,
    tokio::task::JoinHandle<()>,
) {
    let (tx, mut rx) = bounded::<Message<ResourceSpans>>(32);
    let provider = crate::telemetry::platform_provider(tx, true, "tilde-sidecar");
    let worker = tokio::spawn(async move {
        while let Some(message) = rx.next().await {
            for resource in message.payload {
                for scope in &resource.scope_spans {
                    for span in &scope.spans {
                        let agent = span
                            .attributes
                            .iter()
                            .find(|a| a.key == "tilde.agent.id")
                            .and_then(|a| a.value.as_ref())
                            .and_then(|a| match &a.value {
                                Some(Value::StringValue(v)) => v.parse::<Uuid>().ok(),
                                _ => None,
                            });
                        let runtime = agent.and_then(|id| routes.read().ok()?.get(&id).cloned());
                        if let Some(runtime) = runtime {
                            let resource = ResourceSpans {
                                resource: resource.resource.clone(),
                                schema_url: resource.schema_url.clone(),
                                scope_spans: vec![
                                    opentelemetry_proto::tonic::trace::v1::ScopeSpans {
                                        scope: scope.scope.clone(),
                                        schema_url: scope.schema_url.clone(),
                                        spans: vec![span.clone()],
                                    },
                                ],
                            };
                            if relay_traces(
                                &runtime,
                                &ExportTraceServiceRequest {
                                    resource_spans: vec![resource],
                                },
                            )
                            .is_err()
                            {
                                tracing::warn!(agent_id=%runtime.agent_id,"Sidecar platform telemetry could not be relayed");
                            }
                        }
                    }
                }
            }
        }
    });
    (provider, worker)
}
/// Node routing already determines the agent; never infer it from incoming attributes.
pub async fn capture(State(node): State<Node>, mut request: Request, next: Next) -> Response {
    request
        .extensions_mut()
        .insert(crate::telemetry::context::AgentOwner(node.runtime.agent_id));
    crate::telemetry::context::request(request, next).await
}
