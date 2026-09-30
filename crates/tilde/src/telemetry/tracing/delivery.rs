//! Durable trace delivery. Accepted batches are enriched once, written to the trace bucket and
//! delivered from there to the ClickHouse span store and, independently, to an external OTLP
//! collector. Media and oversized payloads go to the separately configured media bucket
//! before queueing. Nothing is held on the gateway; see `crate::telemetry::spool`.
use super::{forwarding::Destination, ingress, store::Store};
use crate::database::Pool;
use crate::error::Error;
use crate::telemetry::spool::{Spool, pause};
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest, trace::v1::ResourceSpans,
};
use prost::Message as _;
use rotel::{
    bounded_channel::{BoundedReceiver, BoundedSender},
    topology::payload::{Ack, ExporterError, Message},
};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Queue {
    pool: Pool,
    store: Spool,
    external: Option<Spool>,
    /// The media bucket; without it, media and oversized payloads stay inline.
    media: Option<crate::agent::avatar::ObjectStore>,
}
impl Queue {
    pub fn new(
        pool: Pool,
        store: Spool,
        external: Option<Spool>,
        media: Option<crate::agent::avatar::ObjectStore>,
    ) -> Self {
        Self {
            pool,
            store,
            external,
            media,
        }
    }
    /// Replication calls this only after authenticating the source and normalizing scope.
    pub async fn accept(&self, payload: &[u8]) -> Result<(), Error> {
        if payload.len() > 16 * 1024 * 1024 {
            return Err(Error::Invalid("Trace batch exceeds delivery limit".into()));
        }
        let mut request = ExportTraceServiceRequest::decode(payload)
            .map_err(|_| Error::Invalid("Invalid OTLP trace batch".into()))?;
        // Strip reserved session fields before hashing: a retried batch is the same batch
        // however it was enriched, and must not be queued twice while the first is pending.
        super::session_metadata::clear(&mut request.resource_spans);
        let id = hex::encode(Sha256::digest(request.encode_to_vec()));
        let client = self.pool.get().await?;
        super::session_metadata::capture(&client, &mut request.resource_spans).await?;
        crate::pricing::price_spans(&client, &mut request.resource_spans).await?;
        drop(client);
        if let Some(media) = &self.media {
            super::media::extract(media, &mut request.resource_spans).await?;
        }
        let agent = request
            .resource_spans
            .iter()
            .flat_map(|r| &r.scope_spans)
            .flat_map(|s| &s.spans)
            .find_map(agent_of)
            .unwrap_or_default();
        let bytes = request.encode_to_vec();
        self.store
            .accept_as(agent.clone(), &id, bytes.clone())
            .await
            .map_err(|_| Error::Invalid("Trace queue full or unavailable".into()))?;
        if let Some(external) = &self.external
            && external.accept_as(agent, &id, bytes).await.is_err()
        {
            ::tracing::warn!(
                "Dropped external trace copy: forwarding queue full or unavailable; local delivery retained"
            );
        }
        Ok(())
    }
    pub async fn collect(self, mut receiver: BoundedReceiver<Vec<Message<ResourceSpans>>>) {
        while let Some(batch) = receiver.next().await {
            // Keep receipts associated with their original request even when Rotel splits it.
            for mut message in batch {
                if ingress::prepare(&mut message).is_err() {
                    if let Some(meta) = &message.metadata {
                        meta.reject(400).await;
                    }
                    continue;
                }
                super::mapping::normalize(&mut message.payload);
                let request = ExportTraceServiceRequest {
                    resource_spans: std::mem::take(&mut message.payload),
                };
                let bytes = request.encode_to_vec();
                let mut backoff = 1;
                let mut released = false;
                loop {
                    let result =
                        tokio::time::timeout(Duration::from_secs(30), self.accept(&bytes)).await;
                    if matches!(result, Ok(Ok(()))) {
                        if !released && let Some(meta) = &message.metadata {
                            let _ = meta.ack().await;
                        }
                        break;
                    }
                    if !released {
                        if let Some(meta) = &message.metadata {
                            let _ = meta
                                .nack(ExporterError::ExportFailed {
                                    error_code: 503,
                                    error_message: "Trace queue unavailable".into(),
                                })
                                .await;
                        }
                        released = true;
                    }
                    ::tracing::warn!("Trace queue unavailable; applying backpressure");
                    tokio::time::sleep(Duration::from_secs(backoff)).await;
                    backoff = (backoff * 2).min(30);
                }
            }
        }
    }
    pub async fn wait_empty(&self) -> Result<(), Error> {
        for queue in std::iter::once(&self.store).chain(&self.external) {
            while queue
                .pending()
                .await
                .map_err(|_| Error::Invalid("Trace queue unavailable".into()))?
                > 0
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        Ok(())
    }
}
fn agent_of(span: &opentelemetry_proto::tonic::trace::v1::Span) -> Option<String> {
    use opentelemetry_proto::tonic::common::v1::any_value::Value;
    span.attributes
        .iter()
        .find(|attribute| attribute.key == "tilde.agent.id")
        .and_then(|attribute| attribute.value.as_ref())
        .and_then(|value| match &value.value {
            Some(Value::StringValue(id)) => Some(id.clone()),
            _ => None,
        })
}

/// Writes queued batches into the span table through Rotel's ClickHouse exporter.
pub async fn store_worker(
    store: Store,
    queue: Spool,
    sender: BoundedSender<Vec<Message<ResourceSpans>>>,
    cancel: CancellationToken,
) {
    while store.initialize().await.is_err() && !cancel.is_cancelled() {
        ::tracing::warn!("ClickHouse trace schema unavailable; delivery will retry");
        pause(&cancel, Duration::from_secs(5)).await;
    }
    let sender = &sender;
    queue
        .drain(&cancel, |key, bytes| async move {
            let Ok(request) = ExportTraceServiceRequest::decode(bytes.as_slice()) else {
                ::tracing::error!("Discarded corrupt batch");
                return Ok(());
            };
            if crate::telemetry::clickhouse::export(sender, &key, request.resource_spans).await {
                Ok(())
            } else {
                Err(None)
            }
        })
        .await;
}
/// Forwards queued batches to an external OTLP collector, independently of the span store.
pub async fn external_worker(destination: Destination, queue: Spool, cancel: CancellationToken) {
    let destination = &destination;
    queue
        .drain(&cancel, |_, bytes| async move {
            let Ok(request) = ExportTraceServiceRequest::decode(bytes.as_slice()) else {
                return Ok(());
            };
            match super::forwarding::send(destination, &request).await {
                super::forwarding::Outcome::Finished => Ok(()),
                super::forwarding::Outcome::Retry(after) => Err(after),
            }
        })
        .await;
}
