//! Stateless durable delivery queues for telemetry. An accepted batch is written to object
//! storage before it is acknowledged, and a small pointer row in Postgres makes it visible to
//! every gateway replica. A replica claims a pointer under a short lease, delivers the object
//! and removes the pointer; if it dies or the destination is down, the lease runs out and the
//! batch becomes due again. Delivery is therefore at least once, and a gateway holds no
//! telemetry state of its own: no local disk, no process lock, no replica affinity.
//!
//! Each queue is one signal and destination, such as `traces/clickhouse` or `logs/external`.
//! Limits bound what is pending, never what the destination stores. Every enqueue writes its
//! own object (`<queue>/<agent>/<batch>/<enqueue>.pb`), never overwritten; the batch id
//! makes a retried upload one pending pointer.
//!
//! The engine never deletes an object. The pointer alone decides whether a batch is owed, and
//! the bucket's lifecycle rule, at least as long as the pointer retention, expires objects.
pub mod db;
use crate::agent::avatar::ObjectStore;
use crate::database::Pool;
use sha2::{Digest, Sha256};
use std::{io, sync::Arc, time::Duration};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

const MAX_OBJECTS: i64 = 65536;
const MAX_AGENT_BYTES: i64 = 256 * 1024 * 1024;
/// How long a pointer may wait for delivery. Bucket expiry must be at least this long.
const RETENTION_SECONDS: i32 = 86400;
/// Longer than any single delivery attempt, short enough that a crashed replica's work resumes soon.
const LEASE_SECONDS: i32 = 45;
const SWEEP_INTERVAL: Duration = Duration::from_secs(600);

#[derive(Clone)]
pub struct Spool {
    pool: Pool,
    store: ObjectStore,
    queue: String,
    limit: i64,
    /// Wakes this replica's worker at once; other replicas find new work by polling.
    pub changed: Arc<Notify>,
}
fn other(message: &'static str) -> io::Error {
    io::Error::other(message)
}
fn unavailable<T>(_: T) -> io::Error {
    other("queue unavailable")
}
enum Admission {
    Queued,
    /// The same batch is already pending; the caller's copy is redundant.
    Duplicate,
    Full,
}

impl Spool {
    pub fn open(pool: Pool, store: ObjectStore, queue: &str, limit: u64) -> Self {
        Self {
            pool,
            store,
            queue: queue.into(),
            limit: limit as i64,
            changed: Arc::new(Notify::new()),
        }
    }
    /// Durable once this returns: the object is stored and its pointer committed.
    pub async fn accept(&self, agent: String, bytes: Vec<u8>) -> io::Result<()> {
        let id = hex::encode(Sha256::digest(&bytes));
        self.accept_as(agent, &id, bytes).await
    }
    /// `id` names the batch independently of its bytes, so a retried upload that was enriched
    /// differently is still the same batch while the first copy is pending.
    pub async fn accept_as(&self, agent: String, id: &str, bytes: Vec<u8>) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let size = bytes.len() as i64;
        let key = format!(
            "{}/{}/{id}/{}.pb",
            self.queue,
            if agent.is_empty() { "platform" } else { &agent },
            uuid::Uuid::new_v4().simple()
        );
        // A full queue refuses without the lock or an upload; admission re-checks under it.
        {
            let client = self.pool.get().await.map_err(unavailable)?;
            let held = db::capacity(&client, &self.queue, &agent)
                .await
                .map_err(unavailable)?;
            if self.over(&held, size) {
                return Err(other("queue full"));
            }
        }
        // The object goes first so the pointer, once visible, always has something to read.
        self.store
            .write(reqwest::Method::PUT, &key, "application/x-protobuf", bytes)
            .await
            .map_err(|_| other("object storage unavailable"))?;
        // A refused or unacknowledged copy is left for the lifecycle rule.
        match self.admit(&key, &agent, id, size).await? {
            Admission::Queued => {
                self.changed.notify_waiters();
                Ok(())
            }
            Admission::Duplicate => Ok(()),
            Admission::Full => Err(other("queue full")),
        }
    }
    /// Capacity check and pointer insert under one per-queue lock, so concurrent accepts
    /// cannot each see room for themselves.
    async fn admit(&self, key: &str, agent: &str, id: &str, size: i64) -> io::Result<Admission> {
        let mut client = self.pool.get().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        db::lock(&tx, &self.queue).await.map_err(unavailable)?;
        let held = db::capacity(&tx, &self.queue, agent)
            .await
            .map_err(unavailable)?;
        if self.over(&held, size) {
            return Ok(Admission::Full);
        }
        let inserted = db::insert(&tx, key, &self.queue, agent, id, size)
            .await
            .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(if inserted == 1 {
            Admission::Queued
        } else {
            Admission::Duplicate
        })
    }
    fn over(&self, held: &db::Capacity, size: i64) -> bool {
        held.bytes + size > self.limit
            || held.agent_bytes + size > MAX_AGENT_BYTES
            || held.objects >= MAX_OBJECTS
    }
    /// The next due batch, leased to this caller.
    pub async fn next(&self) -> io::Result<Option<(String, Vec<u8>)>> {
        let client = self.pool.get().await.map_err(unavailable)?;
        let Some(key) = db::claim(&client, &self.queue, LEASE_SECONDS)
            .await
            .map_err(unavailable)?
        else {
            return Ok(None);
        };
        match self.store.read(&key).await {
            Ok(bytes) => Ok(Some((key, bytes))),
            Err(_) => Err(other("object storage unavailable")),
        }
    }
    /// Delivered: the pointer goes; the object waits for the bucket lifecycle rule.
    pub async fn complete(&self, key: String) -> io::Result<()> {
        let client = self.pool.get().await.map_err(unavailable)?;
        db::complete(&client, &key).await.map_err(unavailable)?;
        Ok(())
    }
    /// Delivery failed: make the batch due again shortly, for this or any other replica.
    async fn release(&self, key: &str) {
        if let Ok(client) = self.pool.get().await {
            let _ = db::release(&client, key, 2).await;
        }
    }
    pub async fn pending(&self) -> io::Result<i64> {
        let client = self.pool.get().await.map_err(unavailable)?;
        db::pending(&client, &self.queue).await.map_err(unavailable)
    }
    /// Delivers due batches until `cancel` fires. `deliver` returns `Err` to retry the batch,
    /// with the destination's requested delay if it gave one; failures back off exponentially.
    pub async fn drain<F>(
        &self,
        cancel: &CancellationToken,
        mut deliver: impl FnMut(String, Vec<u8>) -> F,
    ) where
        F: Future<Output = Result<(), Option<Duration>>>,
    {
        let mut failures = 0;
        while !cancel.is_cancelled() {
            let (key, bytes) = match self.next().await {
                Ok(Some(batch)) => batch,
                Ok(None) => {
                    // Woken by a local accept; otherwise polls for other replicas' work.
                    tokio::select! {
                        _ = cancel.cancelled() => {}
                        _ = self.changed.notified() => {}
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                    }
                    continue;
                }
                Err(error) => {
                    tracing::warn!(queue = %self.queue, %error, "Telemetry queue read failed");
                    pause(cancel, Duration::from_secs(5)).await;
                    continue;
                }
            };
            match deliver(key.clone(), bytes).await {
                Ok(()) => {
                    failures = 0;
                    if self.complete(key).await.is_err() {
                        pause(cancel, Duration::from_secs(5)).await;
                    }
                }
                Err(delay) => {
                    failures = (failures + 1).min(8);
                    self.release(&key).await;
                    tracing::warn!(queue = %self.queue, "Telemetry delivery failed; will retry");
                    let backoff = Duration::from_secs(1 << failures);
                    pause(cancel, delay.unwrap_or_default().max(backoff)).await;
                }
            }
        }
    }
}

/// Waits `duration` or until `cancel` fires, whichever comes first.
pub async fn pause(cancel: &CancellationToken, duration: Duration) {
    tokio::select! {
        _ = cancel.cancelled() => {}
        _ = tokio::time::sleep(duration) => {}
    }
}

/// Drops pointers nobody delivered within the retention period, in every queue, whether or
/// not this replica delivers to it. Runs at start and every ten minutes.
pub async fn sweep_worker(pool: Pool, cancel: CancellationToken) {
    while !cancel.is_cancelled() {
        if let Ok(client) = pool.get().await {
            match db::expire(&client, RETENTION_SECONDS).await {
                Ok(0) => {}
                Ok(dropped) => tracing::warn!(dropped, "Dropped expired telemetry batches"),
                Err(_) => tracing::warn!("Telemetry queue sweep failed"),
            }
        }
        tokio::select! {
            _ = cancel.cancelled() => {}
            _ = tokio::time::sleep(SWEEP_INTERVAL) => {}
        }
    }
}
