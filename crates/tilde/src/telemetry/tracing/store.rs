//! The ClickHouse span store: schema, and delivery through Rotel's ClickHouse exporter.
use crate::error::Error;
use crate::telemetry::clickhouse;
use opentelemetry_proto::tonic::trace::v1::ResourceSpans;
use rotel::{
    bounded_channel::{BoundedSender, bounded},
    topology::payload::Message,
};
use tokio_util::sync::CancellationToken;

/// Spans share the telemetry ClickHouse database with logs.
#[derive(Clone)]
pub struct Store(pub clickhouse::Store);
type ExporterTask = (
    BoundedSender<Vec<Message<ResourceSpans>>>,
    tokio::task::JoinHandle<()>,
);
impl Store {
    pub async fn query(
        &self,
        sql: &str,
        parameters: &[(String, String)],
    ) -> Result<serde_json::Value, Error> {
        self.0.query(sql, parameters).await
    }
    pub async fn ready(&self) -> Result<(), Error> {
        self.query(
            include_str!("../../../../../queries/_traces/ready.sql"),
            &[],
        )
        .await
        .map(|_| ())
    }
    pub async fn initialize(&self) -> Result<(), Error> {
        self.query(
            include_str!("../../../../../queries/_traces/schema.sql"),
            &[],
        )
        .await?;
        self.0.apply_retention(&["otel_traces"]).await?;
        self.ready().await
    }
    pub fn exporter(&self, cancel: CancellationToken) -> Result<ExporterTask, Error> {
        let (tx, rx) = bounded(2);
        let exporter = self
            .0
            .exporter_builder()?
            .build_traces_exporter(rx, None)
            .map_err(|_| Error::Invalid("Invalid traces exporter".into()))?;
        Ok((
            tx,
            tokio::spawn(async move {
                let _ = exporter.start(cancel).await;
            }),
        ))
    }
}
