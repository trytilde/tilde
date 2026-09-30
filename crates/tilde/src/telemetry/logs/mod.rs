//! Agent logs: authenticated Rotel ingestion, independent durable queues, ClickHouse history.
//! Queues live in the log bucket and Postgres pointers (`crate::telemetry::spool`), never on the gateway.
use crate::telemetry::clickhouse;
pub mod ingress;
pub mod viewer;
use crate::telemetry::{spool, tracing::Destination};
use crate::{config::Config, error::Error};
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
use std::{convert::Infallible, time::Duration};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

#[derive(Clone)]
pub struct Delivery {
    local: spool::Spool,
    external: Option<spool::Spool>,
}
impl Delivery {
    /// Trusted gateway projection supplies verified ownership, never expired agent credentials.
    pub async fn accept(
        &self,
        agent: String,
        request: &ExportLogsServiceRequest,
    ) -> Result<(), Error> {
        let bytes = request.encode_to_vec();
        self.local
            .accept(agent.clone(), bytes.clone())
            .await
            .map_err(|_| Error::Invalid("Local log queue full or unavailable".into()))?;
        if let Some(external) = &self.external
            && external.accept(agent, bytes).await.is_err()
        {
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
}
impl Runtime {
    /// `objects` is the log bucket that queues batches for ClickHouse and any collector.
    pub fn start(
        pool: crate::database::Pool,
        config: &Config,
        objects: crate::agent::avatar::ObjectStore,
    ) -> Result<Self, Error> {
        let store = clickhouse::Store::from_config(config)?;
        let external = Destination::new(
            config
                .logs_otlp_endpoint
                .clone()
                .filter(|v| !v.trim().is_empty()),
            config.logs_otlp_headers.clone(),
        )?;
        let queue =
            |name: &str, limit: u64| spool::Spool::open(pool.clone(), objects.clone(), name, limit);
        let local = queue("logs/clickhouse", 1024 * 1024 * 1024);
        let remote = external
            .as_ref()
            .map(|_| queue("logs/external", 256 * 1024 * 1024));
        let delivery = Delivery {
            local: local.clone(),
            external: remote.clone(),
        };
        let reader = viewer::Reader::new(pool, store.clone());
        let cancel = CancellationToken::new();
        let pipeline_cancel = CancellationToken::new();
        let (tx, task) = store.exporter(cancel.clone())?;
        let mut tasks = vec![
            task,
            tokio::spawn(store_worker(store, local, tx, cancel.clone())),
        ];
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
        })
    }
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
pub fn receiver(sender: BoundedSender<Message<ResourceLogs>>) -> Router {
    let output = OTLPOutput::new(sender)
        .with_validator(ingress::validate)
        .with_ack();
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
            "x-tilde-deployment".into(),
            "x-tilde-log-scope".into(),
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

async fn store_worker(
    store: clickhouse::Store,
    queue: spool::Spool,
    sender: BoundedSender<Vec<Message<ResourceLogs>>>,
    cancel: CancellationToken,
) {
    while store.initialize().await.is_err() && !cancel.is_cancelled() {
        tracing::warn!("ClickHouse log schema unavailable; delivery will retry");
        spool::pause(&cancel, Duration::from_secs(5)).await;
    }
    let sender = &sender;
    queue
        .drain(&cancel, |key, bytes| async move {
            let Ok(request) = ExportLogsServiceRequest::decode(bytes.as_slice()) else {
                tracing::error!("Discarded corrupt batch");
                return Ok(());
            };
            if clickhouse::export(sender, &key, request.resource_logs).await {
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
            destination.send_logs(bytes).await.map_err(Some)
        })
        .await;
}
