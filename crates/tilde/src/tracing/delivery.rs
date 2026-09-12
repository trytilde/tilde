//! Temporary, bounded OTLP delivery and replay receipts. No historical trace reads.
use super::{
    forwarding::{Destination, Outcome},
    ingress,
};
use crate::error::Error;
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest, trace::v1::ResourceSpans,
};
use prost::Message as _;
use rotel::{
    bounded_channel::BoundedReceiver,
    topology::payload::{Ack, ExporterError, Message},
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const MAX_PENDING_BYTES: i64 = 256 * 1024 * 1024;
#[derive(Clone)]
pub struct Queue {
    pool: PgPool,
    enabled: bool,
}
impl Queue {
    pub fn new(pool: PgPool, enabled: bool) -> Self {
        Self { pool, enabled }
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    /// Replication calls this only after authenticating the source and normalizing scope.
    pub async fn accept(&self, payload: &[u8]) -> Result<(), Error> {
        if !self.enabled {
            return Ok(());
        }
        if payload.len() > 16 * 1024 * 1024 {
            return Err(Error::Invalid("Trace batch exceeds delivery limit".into()));
        }
        let id = Sha256::digest(payload).to_vec();
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!("../../queries/telemetry/lock.sql")
            .execute(&mut *tx)
            .await?;
        if !sqlx::query_file!("../../queries/telemetry/exists.sql", &id)
            .fetch_one(&mut *tx)
            .await?
            .exists
        {
            let bytes = sqlx::query_file!("../../queries/telemetry/capacity.sql")
                .fetch_one(&mut *tx)
                .await?
                .bytes;
            if bytes + payload.len() as i64 > MAX_PENDING_BYTES {
                return Err(Error::Invalid("Trace delivery queue is full".into()));
            }
            sqlx::query_file!("../../queries/telemetry/enqueue.sql", &id, payload)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
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
                        tokio::time::timeout(Duration::from_secs(10), self.accept(&bytes)).await;
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
        if !self.enabled {
            return Ok(());
        }
        while sqlx::query_file!("../../queries/telemetry/capacity.sql")
            .fetch_one(&self.pool)
            .await?
            .bytes
            > 0
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(())
    }
    pub async fn export(self, destination: Option<Destination>, cancel: CancellationToken) {
        let Some(destination) = destination else {
            if sqlx::query_file!("../../queries/telemetry/discard.sql")
                .execute(&self.pool)
                .await
                .is_err()
            {
                ::tracing::warn!("Unable to discard disabled telemetry backlog");
            }
            return;
        };
        let notifications = crate::database::notifications::Notifications::default();
        let mut changed = loop {
            match notifications.subscribe(&self.pool, "tilde_telemetry").await {
                Ok(value) => break value,
                Err(_) => {
                    ::tracing::warn!("Trace delivery notifications unavailable");
                }
            }
            tokio::select! {_=cancel.cancelled()=>return,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        };
        changed.mark_changed();
        let mut cleanup = tokio::time::interval(Duration::from_secs(3600));
        let mut retry = None;
        loop {
            tokio::select! {
                _=cancel.cancelled()=>return,
                _=cleanup.tick()=>{match sqlx::query_file!("../../queries/telemetry/cleanup.sql").execute(&self.pool).await { Ok(result) if result.rows_affected() > 0 => ::tracing::info!(expired = result.rows_affected(), "Expired telemetry batches and delivery receipts after seven days"), Ok(_) => {}, Err(_) => ::tracing::warn!("Trace delivery expiration failed") }continue;},
                _=changed.changed()=>{},
                _=async{match retry{Some(at)=>tokio::time::sleep_until(at).await,None=>std::future::pending().await}}=>{},
            }
            changed.borrow_and_update();
            if self.send_pending(&destination).await.is_err() {
                ::tracing::warn!("Langfuse delivery will retry");
            }
            retry = match sqlx::query_file!("../../queries/telemetry/deadline.sql")
                .fetch_one(&self.pool)
                .await
            {
                Ok(row) => row.retry_at.map(|at| {
                    tokio::time::Instant::now()
                        + (at - chrono::Utc::now())
                            .to_std()
                            .unwrap_or_default()
                            .max(Duration::from_millis(250))
                }),
                Err(_) => Some(tokio::time::Instant::now() + Duration::from_secs(1)),
            };
        }
    }
    async fn send_pending(&self, destination: &Destination) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let Some(row) = sqlx::query_file!("../../queries/telemetry/pending.sql")
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(());
        };
        let request = ExportTraceServiceRequest::decode(row.payload.as_slice())
            .map_err(|_| Error::Invalid("Invalid queued OTLP".into()))?;
        match super::forwarding::send(destination, &request).await {
            Outcome::Finished => {
                sqlx::query_file!("../../queries/telemetry/complete.sql", &row.id)
                    .execute(&mut *tx)
                    .await?;
            }
            Outcome::Retry(after) => {
                let delay = after
                    .unwrap_or(Duration::from_secs(
                        2u64.pow((row.attempts + 1).min(8) as u32),
                    ))
                    .as_secs()
                    .clamp(1, 86400) as i32;
                sqlx::query_file!("../../queries/telemetry/retry.sql", &row.id, delay)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
