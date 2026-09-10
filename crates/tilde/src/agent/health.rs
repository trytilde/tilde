//! Persisted runtime readiness, 12 hourly history buckets, and registry statistics.
//! One bounded Tokio task polls every 30s. A transaction advisory lock serializes active
//! sweeps across replicas; cancellation drops its JoinSet and releases the lock.
use crate::proto::tilde::agent_host::v1 as host;
use crate::proto::tilde::types::v1 as types;
use crate::{
    encryption::{Encryption, SealedSecret, SecretBinding},
    error::Error,
};
use chrono::{DateTime, Utc};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc, time::Duration};

const INTERVAL: Duration = Duration::from_secs(30);
const TIMEOUT: Duration = Duration::from_secs(5);
const CONCURRENCY: usize = 32;

#[derive(Clone)]
pub struct AgentHealth {
    pool: PgPool,
    encryption: Arc<Encryption>,
}
impl AgentHealth {
    /// Bind health sampling to the existing database and encryption service.
    pub fn new(pool: PgPool, encryption: Arc<Encryption>) -> Self {
        Self { pool, encryption }
    }

    /// Remove expired observations on startup and hourly thereafter.
    pub async fn cleanup(&self) -> Result<u64, Error> {
        Ok(
            sqlx::query_file!("../../queries/agent_health/cleanup.sql", Utc::now())
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    /// Run one sweep. Missing endpoints have no observations, rather than invented failures.
    pub async fn poll_once(&self) -> Result<(), Error> {
        let mut lease = self.pool.begin().await?;
        if !sqlx::query_file!("../../queries/agent_health/lock.sql")
            .fetch_one(&mut *lease)
            .await?
            .acquired
        {
            return Ok(());
        }
        let targets = sqlx::query_file!("../../queries/agent_health/targets.sql")
            .fetch_all(&self.pool)
            .await?;
        let mut checks = tokio::task::JoinSet::new();
        for target in targets {
            if checks.len() >= CONCURRENCY {
                observe(checks.join_next().await);
            }
            let service = self.clone();
            checks.spawn(async move {
                let started = std::time::Instant::now();
                let checked_at = Utc::now();
                let outcome = async {
                    let key = service
                        .encryption
                        .open(
                            SecretBinding {
                                resource_kind: "agent",
                                resource_id: target.id,
                                name: "webhook_signing_key",
                            },
                            SealedSecret::from_bytes(&target.webhook_signing_key)
                                .map_err(|_| "credentials")?,
                        )
                        .map_err(|_| "credentials")?;
                    let request = host::HealthzRequest::default();
                    let client = crate::chat::runtime::client(
                        &target.endpoint_url,
                        key.expose_secret(),
                        "Healthz",
                        &request,
                    )
                    .map_err(|_| "transport")?;
                    drop(key);
                    let response = client.healthz(request).await.map_err(|_| "transport")?;
                    if response.view().ready {
                        Ok(())
                    } else {
                        Err("not_ready")
                    }
                };
                let result = tokio::time::timeout(TIMEOUT, outcome)
                    .await
                    .unwrap_or(Err("timeout"));
                let error_code = result.err();
                let latency_ms = started.elapsed().as_millis().min(i32::MAX as u128) as i32;
                sqlx::query_file!(
                    "../../queries/agent_health/insert.sql",
                    target.id,
                    target.endpoint_url,
                    checked_at,
                    error_code.is_none(),
                    latency_ms,
                    error_code
                )
                .execute(&service.pool)
                .await?;
                Ok::<_, Error>(())
            });
        }
        while let Some(result) = checks.join_next().await {
            observe(Some(result));
        }
        lease.rollback().await?;
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
fn observe(result: Option<Result<Result<(), Error>, tokio::task::JoinError>>) {
    match result {
        Some(Ok(Err(error))) => tracing::warn!(error=%error,"Unable to persist agent health check"),
        Some(Err(error)) => tracing::error!(error=%error,"Agent health check task failed"),
        _ => {}
    }
}

/// Batched metrics for the current registry page; no per-agent queries or guessed totals.
pub async fn metrics(
    pool: &PgPool,
    ids: &[uuid::Uuid],
    now: DateTime<Utc>,
) -> Result<HashMap<uuid::Uuid, types::AgentMetrics>, Error> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_file!("../../queries/agent_health/metrics.sql", ids, now)
        .fetch_all(pool)
        .await?;
    let mut result = HashMap::with_capacity(rows.len());
    for row in rows {
        let status = match (row.healthy, row.checked_at) {
            (Some(healthy), Some(at)) if now - at <= chrono::Duration::seconds(90) => {
                if healthy {
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
    for row in sqlx::query_file!("../../queries/agent_health/history.sql", ids, now)
        .fetch_all(pool)
        .await?
    {
        if let Some(metrics) = result.get_mut(&row.id) {
            metrics.health_history.push(types::AgentHealthHour {
                hour_start: timestamp(row.hour_start).into(),
                total_checks: row.total_checks as u32,
                failed_checks: row.failed_checks as u32,
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
