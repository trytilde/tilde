//! Agent metrics: authenticated Rotel ingestion, stateless durable queues (`crate::telemetry::spool`),
//! ClickHouse history in Rotel's per-kind metric tables, and optional OTLP forwarding.
//! Metrics have no management viewer yet; they are stored for the same ClickHouse tooling
//! logs and spans use, and forwarded to whatever collector the operator names.
pub mod ingress;
use crate::telemetry::{clickhouse, spool, tracing::Destination};
use crate::{config::Config, error::Error};
use axum::{Router, extract::Request, response::IntoResponse};
use opentelemetry_proto::tonic::{
    collector::metrics::v1::ExportMetricsServiceRequest, metrics::v1::ResourceMetrics,
};
use prost::Message as _;
use rotel::{
    bounded_channel::{BoundedReceiver, BoundedSender, bounded},
    receivers::{otlp::otlp_http::build_service, otlp_output::OTLPOutput},
    topology::{
        batch::BatchConfig,
        fanout::Fanout,
        generic_pipeline::{Inspect, Pipeline},
        payload::{Ack, ExporterError, Message},
        processors::Processors,
    },
};
use std::{convert::Infallible, time::Duration};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

const TABLES: &[&str] = &[
    "otel_metrics_sum",
    "otel_metrics_gauge",
    "otel_metrics_histogram",
    "otel_metrics_exponential_histogram",
    "otel_metrics_summary",
];

#[derive(Clone)]
pub struct Delivery {
    local: spool::Spool,
    external: Option<spool::Spool>,
}
impl Delivery {
    pub async fn accept(
        &self,
        agent: String,
        request: &ExportMetricsServiceRequest,
    ) -> Result<(), Error> {
        let bytes = request.encode_to_vec();
        self.local
            .accept(agent.clone(), bytes.clone())
            .await
            .map_err(|_| Error::Invalid("Metric queue full or unavailable".into()))?;
        if let Some(external) = &self.external
            && external.accept(agent, bytes).await.is_err()
        {
            tracing::warn!("Dropped external metric copy: forwarding queue full or unavailable");
        }
        Ok(())
    }
}
pub struct Runtime {
    pub delivery: Delivery,
    sender: BoundedSender<Message<ResourceMetrics>>,
    pipeline_cancel: CancellationToken,
    cancel: CancellationToken,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    pipeline: tokio::task::JoinHandle<()>,
    collector: tokio::task::JoinHandle<()>,
}
impl Runtime {
    pub fn start(
        pool: crate::database::Pool,
        config: &Config,
        objects: crate::agent::avatar::ObjectStore,
    ) -> Result<Self, Error> {
        let store = clickhouse::Store::from_config(config)?;
        let external = Destination::new(
            config
                .metrics_otlp_endpoint
                .clone()
                .filter(|v| !v.trim().is_empty()),
            config.metrics_otlp_headers.clone(),
        )?;
        let queue =
            |name: &str, limit: u64| spool::Spool::open(pool.clone(), objects.clone(), name, limit);
        let local = queue("metrics/clickhouse", 1024 * 1024 * 1024);
        let remote = external
            .as_ref()
            .map(|_| queue("metrics/external", 256 * 1024 * 1024));
        let delivery = Delivery {
            local: local.clone(),
            external: remote.clone(),
        };
        let cancel = CancellationToken::new();
        let pipeline_cancel = CancellationToken::new();
        let (tx, rx) = bounded(2);
        let exporter = store
            .exporter_builder()?
            .build_metrics_exporter(rx, None)
            .map_err(|_| Error::Invalid("Invalid metrics exporter".into()))?;
        let export_cancel = cancel.clone();
        let mut tasks = vec![
            tokio::spawn(async move {
                let _ = exporter.start(export_cancel).await;
            }),
            tokio::spawn(store_worker(store, local, tx, cancel.clone())),
        ];
        if let (Some(destination), Some(queue)) = (external, remote) {
            tasks.push(tokio::spawn(external_worker(
                destination,
                queue,
                cancel.clone(),
            )));
        }
        let (tx, received, pipeline) = pipeline(pipeline_cancel.clone());
        let sink = delivery.clone();
        let collector = tokio::spawn(collect(received, sink));
        Ok(Self {
            delivery,
            sender: tx,
            pipeline_cancel,
            cancel,
            tasks,
            pipeline,
            collector,
        })
    }
    /// `/v1/metrics` on the agent runtime listener, under the same credentials as logs.
    pub fn router(&self, tracing: &crate::telemetry::tracing::Tracing) -> Router {
        tracing.authorize_logs_router(receiver(self.sender.clone()))
    }
    pub async fn shutdown(mut self) {
        self.pipeline_cancel.cancel();
        let drain = async {
            let _ = (&mut self.pipeline).await;
            let _ = (&mut self.collector).await;
        };
        if tokio::time::timeout(Duration::from_secs(10), drain)
            .await
            .is_err()
        {
            self.pipeline.abort();
            self.collector.abort();
        }
        self.cancel.cancel();
        for mut task in self.tasks {
            if tokio::time::timeout(Duration::from_secs(10), &mut task)
                .await
                .is_err()
            {
                task.abort();
            }
        }
    }
}
pub type MetricPipeline = (
    BoundedSender<Message<ResourceMetrics>>,
    BoundedReceiver<Vec<Message<ResourceMetrics>>>,
    tokio::task::JoinHandle<()>,
);
/// Rotel batching between the OTLP receiver and whoever delivers: the gateway queue or a
/// sidecar's relay.
pub fn pipeline(cancel: CancellationToken) -> MetricPipeline {
    let (tx, rx) = bounded(32);
    let (out, received) = bounded(8);
    let mut pipeline = Pipeline::new(
        "metrics",
        rx,
        Fanout::new("metrics", vec![("delivery", out)]),
        None,
        BatchConfig {
            max_size: 512,
            timeout: Duration::from_millis(200),
            disabled: false,
        },
        Processors::empty(),
        vec![],
    );
    let task = tokio::spawn(async move {
        let _ = pipeline.start(NoInspect, cancel).await;
    });
    (tx, received, task)
}
/// The OTLP/HTTP metrics receiver, expecting the ownership headers the authorizing layer sets.
pub fn receiver(sender: BoundedSender<Message<ResourceMetrics>>) -> Router {
    let output = OTLPOutput::new(sender)
        .with_validator(ingress::validate)
        .with_ack();
    let service = build_service(
        None,
        Some(output),
        None,
        "/v1/traces".into(),
        "/v1/metrics".into(),
        "/v1/logs".into(),
        true,
        vec![
            "x-tilde-trace-parent".into(),
            "x-tilde-trace-agent".into(),
            "x-tilde-trace-run".into(),
            "x-tilde-trace-thread".into(),
            "x-tilde-trace-invocation".into(),
            "x-tilde-deployment".into(),
            "x-tilde-log-scope".into(),
        ],
    );
    Router::new().route_service(
        "/v1/metrics",
        tower::service_fn(move |request: Request| {
            let service = service.clone();
            async move {
                Ok::<_, Infallible>(match service.oneshot(request).await {
                    Ok(r) => r.map(axum::body::Body::new).into_response(),
                    Err(_) => http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
                })
            }
        }),
    )
}
async fn collect(mut rx: BoundedReceiver<Vec<Message<ResourceMetrics>>>, sink: Delivery) {
    while let Some(batch) = rx.next().await {
        for mut message in batch {
            let result = match ingress::prepare(&mut message) {
                Ok(agent) => sink
                    .accept(
                        agent,
                        &ExportMetricsServiceRequest {
                            resource_metrics: message.payload,
                        },
                    )
                    .await
                    .map_err(|_| 503),
                Err(code) => Err(code),
            };
            if let Some(meta) = message.metadata {
                match result {
                    Ok(()) => {
                        let _ = meta.ack().await;
                    }
                    Err(400) => meta.reject(400).await,
                    Err(_) => {
                        let _ = meta
                            .nack(ExporterError::ExportFailed {
                                error_code: 503,
                                error_message: "Metric queue unavailable".into(),
                            })
                            .await;
                    }
                }
            }
        }
    }
}
struct NoInspect;
impl Inspect<ResourceMetrics> for NoInspect {
    fn inspect(&self, _: &[ResourceMetrics]) {}
    fn inspect_with_prefix(&self, _: Option<String>, _: &[ResourceMetrics]) {}
}
async fn store_worker(
    store: clickhouse::Store,
    queue: spool::Spool,
    sender: BoundedSender<Vec<Message<ResourceMetrics>>>,
    cancel: CancellationToken,
) {
    let schema = async || {
        store
            .execute_all(include_str!("../../../../../queries/_metrics/schema.sql"))
            .await?;
        store.apply_retention(TABLES).await?;
        store
            .query(
                include_str!("../../../../../queries/_metrics/ready.sql"),
                &[],
            )
            .await
    };
    while schema().await.is_err() && !cancel.is_cancelled() {
        tracing::warn!("ClickHouse metric schema unavailable; delivery will retry");
        spool::pause(&cancel, Duration::from_secs(5)).await;
    }
    let sender = &sender;
    queue
        .drain(&cancel, |key, bytes| async move {
            let Ok(request) = ExportMetricsServiceRequest::decode(bytes.as_slice()) else {
                tracing::error!("Discarded corrupt batch");
                return Ok(());
            };
            if clickhouse::export(sender, &key, request.resource_metrics).await {
                Ok(())
            } else {
                Err(None)
            }
        })
        .await;
}
async fn external_worker(destination: Destination, queue: spool::Spool, cancel: CancellationToken) {
    queue
        .drain(&cancel, |_, bytes| async {
            destination.send_metrics(bytes).await.map_err(Some)
        })
        .await;
}
