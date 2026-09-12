//! Agent logs: authenticated Rotel ingestion, independent durable queues, ClickHouse history.
pub mod clickhouse;
pub mod ingress;
pub mod spool;
pub mod viewer;
use crate::{config::Config, error::Error, telemetry::Destination};
use axum::{Router, extract::Request, response::IntoResponse};
use opentelemetry_proto::tonic::{
    collector::logs::v1::ExportLogsServiceRequest, logs::v1::ResourceLogs,
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
use std::{convert::Infallible, path::Path, time::Duration};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

#[derive(Clone)]
pub struct Delivery {
    local: Option<spool::Spool>,
    external: Option<spool::Spool>,
}
impl Delivery {
    pub fn enabled(&self) -> bool {
        self.local.is_some() || self.external.is_some()
    }
    /// Trusted gateway projection supplies verified ownership, never expired agent credentials.
    pub async fn accept(
        &self,
        agent: String,
        request: &ExportLogsServiceRequest,
    ) -> Result<(), Error> {
        let bytes = request.encode_to_vec();
        if let Some(local) = &self.local {
            local
                .accept(agent.clone(), bytes.clone())
                .await
                .map_err(|_| Error::Invalid("Local log queue full or unavailable".into()))?;
        }
        if let Some(external) = &self.external
            && external.accept(agent, bytes).await.is_err()
        {
            if self.local.is_none() {
                return Err(Error::Invalid(
                    "External log queue full or unavailable".into(),
                ));
            }
            tracing::warn!(
                "Dropped external log copy: forwarding queue full or unavailable; local delivery retained"
            );
        }
        Ok(())
    }
}
pub struct Runtime {
    pub delivery: Delivery,
    pub reader: viewer::Reader,
    sender: BoundedSender<Message<ResourceLogs>>,
    pipeline_cancel: CancellationToken,
    cancel: CancellationToken,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    pipeline: tokio::task::JoinHandle<()>,
    collector: tokio::task::JoinHandle<()>,
    _lock: std::fs::File,
}
impl Runtime {
    pub fn start(pool: sqlx::PgPool, config: &Config) -> Result<Self, Error> {
        let store = clickhouse::Store::from_config(config)?;
        let external = Destination::new(
            config
                .logs_otlp_endpoint
                .clone()
                .filter(|v| !v.trim().is_empty()),
            config.logs_otlp_headers.clone(),
        )?;
        let root = Path::new(&config.logs_queue_dir);
        let lock = spool::lock(root).map_err(|_| {
            Error::Invalid(
                "Unable to lock LOGS_QUEUE_DIR; use a separate persistent directory per gateway"
                    .into(),
            )
        })?;
        let local = store
            .as_ref()
            .map(|s| spool::Spool::open(root, "clickhouse", &s.identity(), 256 * 1024 * 1024))
            .transpose()
            .map_err(|_| Error::Invalid("Unable to open local log queue".into()))?;
        let remote = external
            .as_ref()
            .map(|s| spool::Spool::open(root, "external", &s.queue_identity(), 64 * 1024 * 1024))
            .transpose()
            .map_err(|_| Error::Invalid("Unable to open external log queue".into()))?;
        if local.is_none() {
            spool::Spool::discard_other(root, "clickhouse", None)
                .map_err(|_| Error::Invalid("Unable to discard disabled log queue".into()))?;
        }
        if remote.is_none() {
            spool::Spool::discard_other(root, "external", None)
                .map_err(|_| Error::Invalid("Unable to discard disabled external queue".into()))?;
        }
        let delivery = Delivery {
            local: local.clone(),
            external: remote.clone(),
        };
        let reader = viewer::Reader::new(pool, store.clone(), external.is_some());
        let cancel = CancellationToken::new();
        let pipeline_cancel = CancellationToken::new();
        let mut tasks = Vec::new();
        if let (Some(store), Some(queue)) = (store, local) {
            let (tx, task) = store.exporter(cancel.clone())?;
            tasks.push(task);
            tasks.push(tokio::spawn(store_worker(store, queue, tx, cancel.clone())));
        }
        if let (Some(destination), Some(queue)) = (external, remote) {
            tasks.push(tokio::spawn(external_worker(
                destination,
                queue,
                cancel.clone(),
            )));
        }
        let (sender, rx, pipeline) = pipeline(pipeline_cancel.clone());
        let sink = delivery.clone();
        let collector = tokio::spawn(async move {
            collect(rx, sink).await;
        });
        Ok(Self {
            delivery,
            reader,
            sender,
            pipeline_cancel,
            cancel,
            tasks,
            pipeline,
            collector,
            _lock: lock,
        })
    }
    pub fn router(&self, tracing: &crate::telemetry::Tracing) -> Router {
        tracing.authorize_router(receiver(self.sender.clone(), self.delivery.enabled()))
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
pub type LogPipeline = (
    BoundedSender<Message<ResourceLogs>>,
    BoundedReceiver<Vec<Message<ResourceLogs>>>,
    tokio::task::JoinHandle<()>,
);
pub fn pipeline(cancel: CancellationToken) -> LogPipeline {
    let (tx, rx) = bounded(32);
    let (out, received) = bounded(8);
    let mut pipeline = Pipeline::new(
        "logs",
        rx,
        Fanout::new("logs", vec![("delivery", out)]),
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
pub fn receiver(sender: BoundedSender<Message<ResourceLogs>>, enabled: bool) -> Router {
    let output = OTLPOutput::new(sender).with_validator(ingress::validate);
    let output = if enabled {
        output.with_ack()
    } else {
        output.with_discard(true)
    };
    let service = build_service(
        None,
        None,
        Some(output),
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
        ],
    );
    Router::new().route_service(
        "/v1/logs",
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
async fn collect(mut rx: BoundedReceiver<Vec<Message<ResourceLogs>>>, sink: Delivery) {
    while let Some(batch) = rx.next().await {
        for mut message in batch {
            let result = match ingress::prepare(&mut message) {
                Ok(agent) => sink
                    .accept(
                        agent,
                        &ExportLogsServiceRequest {
                            resource_logs: message.payload,
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
                        tracing::warn!(
                            "Log admission rejected; queue capacity or storage unavailable"
                        );
                        let _ = meta
                            .nack(ExporterError::ExportFailed {
                                error_code: 503,
                                error_message: "Log queue unavailable".into(),
                            })
                            .await;
                    }
                }
            }
        }
    }
}
struct NoInspect;
impl Inspect<ResourceLogs> for NoInspect {
    fn inspect(&self, _: &[ResourceLogs]) {}
    fn inspect_with_prefix(&self, _: Option<String>, _: &[ResourceLogs]) {}
}

async fn pause(cancel: &CancellationToken, duration: Duration) {
    tokio::select! { _ = cancel.cancelled() => {}, _ = tokio::time::sleep(duration) => {} }
}
async fn idle(cancel: &CancellationToken, queue: &spool::Spool) {
    tokio::select! { _ = cancel.cancelled() => {}, _ = queue.changed.notified() => {}, _ = tokio::time::sleep(Duration::from_secs(60)) => {} }
}
async fn store_worker(
    store: clickhouse::Store,
    queue: spool::Spool,
    sender: BoundedSender<Vec<Message<ResourceLogs>>>,
    cancel: CancellationToken,
) {
    let mut initialized = false;
    while !cancel.is_cancelled() {
        if !initialized {
            initialized = store.initialize().await.is_ok();
            if !initialized {
                tracing::warn!("ClickHouse log schema unavailable; delivery will retry");
                pause(&cancel, Duration::from_secs(5)).await;
                continue;
            }
        }
        match queue.next().await {
            Ok(Some((key, bytes))) => {
                let success = match ExportLogsServiceRequest::decode(bytes.as_slice()) {
                    Ok(request) => clickhouse::export(&sender, &key, request.resource_logs).await,
                    Err(_) => {
                        tracing::error!("Discarded corrupt local log batch");
                        true
                    }
                };
                if success {
                    if queue.complete(key).await.is_err() {
                        tracing::warn!("Unable to complete local log delivery");
                        pause(&cancel, Duration::from_secs(5)).await;
                    }
                } else {
                    tracing::warn!("ClickHouse log delivery will retry");
                    pause(&cancel, Duration::from_secs(5)).await;
                }
            }
            Ok(None) => idle(&cancel, &queue).await,
            Err(_) => {
                tracing::warn!("Local log queue read failed");
                pause(&cancel, Duration::from_secs(5)).await;
            }
        }
    }
}
async fn external_worker(destination: Destination, queue: spool::Spool, cancel: CancellationToken) {
    let mut failures = 0;
    while !cancel.is_cancelled() {
        match queue.next().await {
            Ok(Some((key, bytes))) => match destination.send_logs(bytes).await {
                Ok(()) => {
                    failures = 0;
                    if queue.complete(key).await.is_err() {
                        tracing::warn!("Unable to complete external log delivery");
                        pause(&cancel, Duration::from_secs(5)).await;
                    }
                }
                Err(delay) => {
                    failures = (failures + 1).min(8);
                    tracing::warn!("External log collector unavailable; delivery will retry");
                    pause(&cancel, delay.max(Duration::from_secs(1 << failures))).await;
                }
            },
            Ok(None) => idle(&cancel, &queue).await,
            Err(_) => {
                tracing::warn!("External log queue read failed");
                pause(&cancel, Duration::from_secs(5)).await;
            }
        }
    }
}
