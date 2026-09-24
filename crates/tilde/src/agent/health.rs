//! Persisted runtime readiness, 12 hourly history buckets, and registry statistics.
//! One bounded Tokio task polls every 30s. A transaction advisory lock serializes active
//! sweeps across replicas; cancellation rolls back the sampling transaction.
use crate::database::Pool;
use crate::error::Error;
use crate::proto::tilde::types::v1 as types;
use chrono::{DateTime, Utc};
use std::{collections::HashMap, time::Duration};

const INTERVAL: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct AgentHealth {
    pool: Pool,
}
impl AgentHealth {
    /// Sample readiness across the deployments selected by the agent routing policy.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Remove expired observations on startup and hourly thereafter.
    pub async fn cleanup(&self) -> Result<u64, Error> {
        Ok(crate::agent::db::health_cleanup_execute(&self.pool.get().await?, Utc::now()).await?)
    }

    /// Record a deployment-health sample even when the agent has no live deployment.
    pub async fn poll_once(&self) -> Result<(), Error> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        if !crate::agent::db::health_lock_one(&tx).await?.acquired {
            return Ok(());
        }
        crate::agent::db::health_sample_execute(&tx, Utc::now()).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Immediate first sweep, then best-effort 30s cadence; no overlapping sweeps or detached tasks.
    pub async fn run(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut poll = tokio::time::interval(INTERVAL);
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let hour = Duration::from_secs(3600);
        let mut retention = tokio::time::interval_at(tokio::time::Instant::now() + hour, hour);
        retention.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            if *shutdown.borrow() {
                break;
            }
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                _ = retention.tick() => {
                    tokio::select! {
                        biased;
                        _ = shutdown.changed() => break,
                        result = self.cleanup() => if let Err(error) = result { tracing::warn!(error=%error,"Agent health retention failed"); }
                    }
                }
                _ = poll.tick() => {
                    tokio::select! {
                        biased;
                        _ = shutdown.changed() => break,
                        result = self.poll_once() => if let Err(error) = result { tracing::warn!(error=%error,"Agent health sweep failed"); }
                    }
                }
            }
        }
    }
}
/// Batched metrics for the current registry page; no per-agent queries or guessed totals.
pub async fn metrics(
    pool: &Pool,
    ids: &[uuid::Uuid],
    now: DateTime<Utc>,
) -> Result<HashMap<uuid::Uuid, types::AgentMetrics>, Error> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = crate::agent::db::health_metrics_all(&pool.get().await?, ids, now).await?;
    let mut result = HashMap::with_capacity(rows.len());
    for row in rows {
        let status = match (row.healthy, row.checked_at) {
            (Some(healthy), Some(at)) if now - at <= chrono::Duration::seconds(90) => {
                if row.degraded == Some(true) {
                    types::AgentHealthStatus::Degraded
                } else if healthy {
                    types::AgentHealthStatus::Healthy
                } else {
                    types::AgentHealthStatus::Unhealthy
                }
            }
            _ => types::AgentHealthStatus::Unknown,
        };
        let mut metrics = types::AgentMetrics {
            health: status.into(),
            thread_count: row.thread_count as u64,
            average_turns_per_thread: row.average_turns_per_thread,
            average_response_ms: row.average_response_ms,
            ..Default::default()
        };
        if let Some(at) = row.checked_at {
            metrics.last_health_check_at = timestamp(at).into();
        }
        result.insert(row.id, metrics);
    }
    for row in crate::agent::db::health_history_all(&pool.get().await?, ids, now).await? {
        if let Some(metrics) = result.get_mut(&row.id) {
            metrics.health_history.push(types::AgentHealthHour {
                hour_start: timestamp(row.hour_start).into(),
                total_checks: row.total_checks as u32,
                failed_checks: row.failed_checks as u32,
                degraded_checks: row.degraded_checks as u32,
                ..Default::default()
            });
        }
    }
    Ok(result)
}
fn timestamp(at: DateTime<Utc>) -> buffa_types::google::protobuf::Timestamp {
    buffa_types::google::protobuf::Timestamp {
        seconds: at.timestamp(),
        nanos: at.timestamp_subsec_nanos() as i32,
        ..Default::default()
    }
}
