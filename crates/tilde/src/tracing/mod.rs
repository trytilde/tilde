//! Langfuse ingestion, durable transient delivery, and management-only trace queries.
pub mod context;
pub mod delivery;
mod forwarding;
pub(crate) mod ingress;
pub mod mapping;
pub mod viewer;

use crate::iam::tokens::Tokens;
use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use opentelemetry_proto::{
    tonic::trace::v1::ResourceSpans,
    transform::{
        common::tonic::ResourceAttributesWithSchema,
        trace::tonic::group_spans_by_resource_and_scope,
    },
};
use opentelemetry_sdk::{
    Resource,
    error::{OTelSdkError, OTelSdkResult},
    trace::{
        BatchConfigBuilder, BatchSpanProcessor, Sampler, SdkTracerProvider, SpanData, SpanExporter,
    },
};
use rotel::{
    bounded_channel::{BoundedSender, bounded},
    receivers::{otlp::otlp_http::build_service, otlp_output::OTLPOutput},
    topology::{
        batch::BatchConfig,
        fanout::Fanout,
        generic_pipeline::{Inspect, Pipeline},
        payload::Message,
        processors::Processors,
    },
};
use sqlx::PgPool;
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

pub const INVOCATION_HEADER: &str = "x-tilde-trace-invocation";
pub const AGENT_TRACES_PATH: &str = "/v1/traces";
const QUEUE_BATCHES: usize = 32;

#[derive(Clone)]
pub struct Tracing {
    sender: BoundedSender<Message<ResourceSpans>>,
    tokens: Tokens,
    pool: PgPool,
    admission: Arc<tokio::sync::Semaphore>,
    enabled: bool,
}
pub struct Runtime {
    pub tracing: Tracing,
    pub provider: SdkTracerProvider,
    pipeline: tokio::task::JoinHandle<()>,
    forwarder: tokio::task::JoinHandle<()>,
    collector: tokio::task::JoinHandle<()>,
    pub queue: delivery::Queue,
    cancel: CancellationToken,
    pipeline_cancel: CancellationToken,
}
impl Runtime {
    pub fn start(
        pool: PgPool,
        tokens: Tokens,
        destination: Option<forwarding::Destination>,
    ) -> Self {
        let enabled = destination.is_some();
        let (sender, receiver) = bounded(QUEUE_BATCHES);
        let (export_tx, export_rx) = bounded(8);
        let mut pipeline = Pipeline::new(
            "traces",
            receiver,
            Fanout::new("traces", vec![("otlp", export_tx)]),
            None,
            BatchConfig {
                max_size: 512,
                timeout: Duration::from_millis(200),
                disabled: false,
            },
            Processors::empty(),
            vec![],
        );
        let cancel = CancellationToken::new();
        let pipeline_cancel = CancellationToken::new();
        let pipeline_stop = pipeline_cancel.clone();
        let pipeline = tokio::spawn(async move {
            let _ = pipeline.start(NoInspect, pipeline_stop).await;
        });
        let queue = delivery::Queue::new(pool.clone(), enabled);
        let collect_queue = queue.clone();
        let collector = tokio::spawn(async move {
            collect_queue.collect(export_rx).await;
        });
        let export_queue = queue.clone();
        let export_cancel = cancel.clone();
        let forwarder = tokio::spawn(async move {
            export_queue.export(destination, export_cancel).await;
        });
        let provider = platform_provider(sender.clone(), enabled, "tilde");
        Self {
            tracing: Tracing {
                sender,
                tokens,
                pool,
                admission: Arc::new(tokio::sync::Semaphore::new(32)),
                enabled,
            },
            provider,
            pipeline,
            forwarder,
            collector,
            queue,
            cancel,
            pipeline_cancel,
        }
    }
    pub async fn shutdown(mut self) {
        let provider = self.provider.clone();
        let _ = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::task::spawn_blocking(move || provider.shutdown()),
        )
        .await;
        drop(self.tracing);
        // Drain ingress into the durable queue before stopping the external worker.
        self.pipeline_cancel.cancel();
        let drain = async {
            let _ = (&mut self.pipeline).await;
            let _ = (&mut self.collector).await;
            let _ = tokio::time::timeout(Duration::from_secs(5), self.queue.wait_empty()).await;
        };
        if tokio::time::timeout(Duration::from_secs(20), drain)
            .await
            .is_err()
        {
            self.pipeline.abort();
            self.collector.abort();
            ::tracing::warn!("Trace ingestion drain timed out; unacknowledged clients must retry");
        }
        self.cancel.cancel();
        if tokio::time::timeout(Duration::from_secs(10), &mut self.forwarder)
            .await
            .is_err()
        {
            self.forwarder.abort();
        }
    }
}
impl Tracing {
    /// Logs share connect-token scope and the five-minute terminal upload window.
    pub fn authorize_router(&self, router: Router) -> Router {
        router.layer(middleware::from_fn_with_state(self.clone(), authorize))
    }

    /// Only the runtime listener's composition exposes this OTLP/HTTP receiver.
    pub(crate) fn agent_ingestion_router(&self) -> Router {
        let output = OTLPOutput::new(self.sender.clone()).with_validator(ingress::validate_request);
        let output = if self.enabled {
            output.with_ack()
        } else {
            output.with_discard(true)
        };
        let service = build_service(
            Some(output),
            None,
            None,
            AGENT_TRACES_PATH.into(),
            "/v1/metrics".into(),
            "/v1/logs".into(),
            true,
            vec![
                INVOCATION_HEADER.into(),
                "x-tilde-trace-parent".into(),
                "x-tilde-trace-agent".into(),
                "x-tilde-trace-run".into(),
                "x-tilde-trace-thread".into(),
            ],
        );
        let service = tower::service_fn(move |request: Request| {
            let service = service.clone();
            async move {
                Ok::<_, Infallible>(match service.oneshot(request).await {
                    Ok(response) => response.map(axum::body::Body::new).into_response(),
                    Err(_) => http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
                })
            }
        });
        Router::new()
            .route_service(AGENT_TRACES_PATH, service)
            .layer(middleware::from_fn_with_state(self.clone(), authorize))
    }
}
async fn authorize(State(state): State<Tracing>, mut request: Request, next: Next) -> Response {
    let Ok(_permit) = state.admission.clone().try_acquire_owned() else {
        return busy();
    };
    let token = request
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    let Some(token) = token else {
        return http::StatusCode::UNAUTHORIZED.into_response();
    };
    let claims = match state.tokens.verify_trace(token).await {
        Ok(claims) => claims,
        Err(_) => return http::StatusCode::UNAUTHORIZED.into_response(),
    };
    let scope =
        match sqlx::query_file!("../../queries/tracing/invocation.sql", claims.invocation_id)
            .fetch_optional(&state.pool)
            .await
        {
            Ok(Some(scope)) => scope,
            _ => return busy(),
        };
    let Ok(parent) = scope.execution_traceparent.parse() else {
        return http::StatusCode::FORBIDDEN.into_response();
    };
    request.headers_mut().insert("x-tilde-trace-parent", parent);
    request.headers_mut().remove(http::header::AUTHORIZATION);
    for (key, id) in [
        ("x-tilde-trace-agent", claims.sub),
        ("x-tilde-trace-run", claims.run_id),
        ("x-tilde-trace-thread", claims.thread_id),
    ] {
        request
            .headers_mut()
            .insert(key, id.to_string().parse().expect("UUID header"));
    }

    request.headers_mut().insert(
        INVOCATION_HEADER,
        claims
            .invocation_id
            .to_string()
            .parse()
            .expect("UUID header"),
    );
    // Enabled requests wait for durable queue acceptance; disabled requests are
    // decoded and validated by Rotel before successful discard.
    match tokio::time::timeout(Duration::from_secs(15), next.run(request)).await {
        Ok(response) => response,
        Err(_) => busy(),
    }
}
fn busy() -> Response {
    (
        http::StatusCode::SERVICE_UNAVAILABLE,
        [(http::header::RETRY_AFTER, "1")],
    )
        .into_response()
}
struct NoInspect;
impl Inspect<ResourceSpans> for NoInspect {
    fn inspect(&self, _: &[ResourceSpans]) {}
    fn inspect_with_prefix(&self, _: Option<String>, _: &[ResourceSpans]) {}
}
#[derive(Clone)]
struct PlatformExporter {
    sender: BoundedSender<Message<ResourceSpans>>,
    resource: Resource,
}
impl std::fmt::Debug for PlatformExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PlatformExporter")
    }
}
impl SpanExporter for PlatformExporter {
    async fn export(&self, spans: Vec<SpanData>) -> OTelSdkResult {
        let payload = group_spans_by_resource_and_scope(
            spans,
            &ResourceAttributesWithSchema::from(&self.resource),
        );
        self.sender
            .send(Message::new(None, payload, None))
            .await
            .map_err(|_| {
                ::tracing::warn!("Platform trace pipeline closed");
                OTelSdkError::InternalFailure("Trace pipeline closed".into())
            })
    }
    fn set_resource(&mut self, resource: &Resource) {
        self.resource = resource.clone();
    }
}
pub use forwarding::Destination;

/// Shared platform provider; sidecars route its bounded output through their local relay.
pub fn platform_provider(
    sender: BoundedSender<Message<ResourceSpans>>,
    enabled: bool,
    service: &'static str,
) -> SdkTracerProvider {
    let processor = BatchSpanProcessor::builder(PlatformExporter {
        sender,
        resource: Resource::builder().with_service_name(service).build(),
    })
    .with_batch_config(
        BatchConfigBuilder::default()
            .with_max_queue_size(4096)
            .with_max_export_batch_size(256)
            .with_scheduled_delay(Duration::from_millis(200))
            .build(),
    )
    .build();
    SdkTracerProvider::builder()
        .with_sampler(if enabled {
            Sampler::AlwaysOn
        } else {
            Sampler::AlwaysOff
        })
        .with_span_processor(processor)
        .with_resource(Resource::builder().with_service_name(service).build())
        .build()
}
