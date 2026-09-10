//! Embedded Rotel receiver and batching pipeline. Postgres commits acknowledge
//! ingress; a separate durable export worker handles optional external delivery.
pub mod context;
mod forwarding;
mod storage;

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
}
pub struct Runtime {
    pub tracing: Tracing,
    pub provider: SdkTracerProvider,
    pipeline: tokio::task::JoinHandle<()>,
    storage: tokio::task::JoinHandle<()>,
    forwarder: tokio::task::JoinHandle<()>,
    cancel: CancellationToken,
}
impl Runtime {
    pub fn start(
        pool: PgPool,
        tokens: Tokens,
        destination: Option<forwarding::Destination>,
        retention_days: i32,
    ) -> Self {
        let (sender, receiver) = bounded(QUEUE_BATCHES);
        let (store_tx, store_rx) = bounded(8);
        let mut pipeline = Pipeline::new(
            "traces",
            receiver,
            Fanout::new("traces", vec![("postgres", store_tx)]),
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
        let pipeline_cancel = cancel.clone();
        let pipeline = tokio::spawn(async move {
            let _ = pipeline.start(NoInspect, pipeline_cancel).await;
        });
        let storage_pool = pool.clone();
        let forward = destination.is_some();
        let storage = tokio::spawn(async move {
            storage::run(storage_pool, store_rx, forward).await;
        });
        let forward_cancel = cancel.clone();
        let forward_pool = pool.clone();
        let forwarder = tokio::spawn(async move {
            forwarding::run(forward_pool, destination, retention_days, forward_cancel).await;
        });
        let processor = BatchSpanProcessor::builder(PlatformExporter {
            sender: sender.clone(),
            resource: Resource::builder().with_service_name("tilde").build(),
        })
        .with_batch_config(
            BatchConfigBuilder::default()
                .with_max_queue_size(4096)
                .with_max_export_batch_size(256)
                .with_scheduled_delay(Duration::from_millis(200))
                .build(),
        )
        .build();
        let provider = SdkTracerProvider::builder()
            .with_sampler(Sampler::AlwaysOn)
            .with_span_processor(processor)
            .with_resource(Resource::builder().with_service_name("tilde").build())
            .build();
        Self {
            tracing: Tracing {
                sender,
                tokens,
                pool,
                admission: Arc::new(tokio::sync::Semaphore::new(32)),
            },
            provider,
            pipeline,
            storage,
            forwarder,
            cancel,
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
        // Rotel cancellation drains its input and partial batch. Store drains the output.
        self.cancel.cancel();
        let drain = async {
            let _ = (&mut self.pipeline).await;
            let _ = (&mut self.storage).await;
            let _ = (&mut self.forwarder).await;
        };
        if tokio::time::timeout(Duration::from_secs(20), drain)
            .await
            .is_err()
        {
            self.pipeline.abort();
            self.storage.abort();
            self.forwarder.abort();
            ::tracing::warn!("Trace shutdown drain timed out; unacknowledged clients must retry");
        }
    }
}
impl Tracing {
    /// Only the runtime listener's composition exposes this OTLP/HTTP receiver.
    pub(crate) fn agent_ingestion_router(&self) -> Router {
        let service = build_service(
            Some(
                OTLPOutput::new(self.sender.clone())
                    .with_ack()
                    .with_validator(storage::validate_request),
            ),
            None,
            None,
            AGENT_TRACES_PATH.into(),
            "/v1/metrics".into(),
            "/v1/logs".into(),
            true,
            vec![INVOCATION_HEADER.into(), "x-tilde-trace-parent".into()],
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
    request.headers_mut().insert(
        INVOCATION_HEADER,
        claims
            .invocation_id
            .to_string()
            .parse()
            .expect("UUID header"),
    );
    // Bounds body upload, queue admission and persistence wait. A timed-out write
    // may still commit; span identity makes the client's retry idempotent.
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
