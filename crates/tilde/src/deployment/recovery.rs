//! Leases held by replicas that stopped heartbeating. The holder's interrupted
//! invocations end; under the reassign policy the lease moves to a live replica,
//! which restarts the run from its objective when it hydrates the thread; under
//! the stop policy the run fails and the lease is dropped.
use super::{Deployments, LIVENESS};
use crate::error::Error;
use sqlx::{Postgres, Transaction};
use std::time::Duration;
use uuid::Uuid;
impl Deployments {
    /// End a dead holder's work on one thread. `next` receives the lease and the
    /// holder's queued directives; `stop` fails the active run instead of leaving
    /// it for the new holder to restart.
    pub(crate) async fn take_over(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        agent: Uuid,
        thread: Uuid,
        previous: Uuid,
        next: Option<Uuid>,
        stop: bool,
    ) -> Result<(), Error> {
        for failed in sqlx::query_file!(
            "../../queries/deployment/fail_old_invocations.sql",
            thread,
            agent
        )
        .fetch_all(&mut **tx)
        .await?
        {
            crate::chat::activity(tx, thread, "invocation.ended", failed.id, "").await?;
        }
        if stop {
            for run in sqlx::query_file!("../../queries/deployment/stop_runs.sql", thread, agent)
                .fetch_all(&mut **tx)
                .await?
            {
                crate::chat::activity(tx, thread, "run.updated", run.id, "").await?;
            }
        }
        match next {
            Some(next) => {
                sqlx::query_file!(
                    "../../queries/deployment/lease_move.sql",
                    thread,
                    agent,
                    next
                )
                .execute(&mut **tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/deployment/directives_move.sql",
                    thread,
                    previous,
                    next
                )
                .execute(&mut **tx)
                .await?;
            }
            None => {
                sqlx::query_file!("../../queries/deployment/lease_delete.sql", thread, agent)
                    .execute(&mut **tx)
                    .await?;
            }
        }
        Ok(())
    }
    pub async fn recover(&self) -> Result<usize, Error> {
        let rows = sqlx::query_file!("../../queries/deployment/dead_leases.sql")
            .fetch_all(&self.pool)
            .await?;
        let mut count = 0;
        for row in rows {
            let mut tx = self.pool.begin().await?;
            crate::chat::access::lock_thread_route(&mut tx, row.thread_id).await?;
            let Some(current) = sqlx::query_file!(
                "../../queries/deployment/lease_lock.sql",
                row.thread_id,
                row.agent_id
            )
            .fetch_optional(&mut *tx)
            .await?
            else {
                continue;
            };
            if current.instance_id != row.instance_id
                || sqlx::query_file!(
                    "../../queries/deployment/instance_live.sql",
                    row.agent_id,
                    row.instance_id
                )
                .fetch_one(&mut *tx)
                .await?
                .live
            {
                continue;
            }
            if !row.active {
                // Nothing was running: the next replica to touch the thread takes it.
                sqlx::query_file!(
                    "../../queries/deployment/lease_delete.sql",
                    row.thread_id,
                    row.agent_id
                )
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                continue;
            }
            let stop = row.failure_mode == "stop";
            let next = if stop {
                None
            } else {
                // Failover stays within the dead instance's deployment when one is live.
                let Some(node) = sqlx::query_file!(
                    "../../queries/deployment/live_node.sql",
                    row.agent_id,
                    Some(row.instance_id),
                    row.deployment_id
                )
                .fetch_optional(&mut *tx)
                .await?
                else {
                    // No replacement yet; the run waits for one.
                    continue;
                };
                Some(node.instance_id)
            };
            self.take_over(
                &mut tx,
                row.agent_id,
                row.thread_id,
                row.instance_id,
                next,
                stop,
            )
            .await?;
            tx.commit().await?;
            tracing::info!(agent_id=%row.agent_id, thread_id=%row.thread_id, previous=%row.instance_id, next=?next, "Recovered a thread from a dead replica");
            count += 1;
        }
        Ok(count)
    }
    pub async fn recovery_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut changes = match self
            .channels
            .recovery
            .subscribe(&self.pool, "tilde_sidecar_recovery")
            .await
        {
            Ok(v) => v,
            Err(_) => return,
        };
        let mut tick = tokio::time::interval(LIVENESS / 3);
        let mut prune = tokio::time::interval(Duration::from_secs(600));
        loop {
            tokio::select! {
                _=shutdown.changed()=>return,
                _=changes.changed()=>{ changes.borrow_and_update(); },
                _=tick.tick()=>{},
                _=prune.tick()=>{ let _ = sqlx::query_file!("../../queries/deployment/prune.sql").fetch_one(&self.pool).await; },
            }
            if self.recover().await.is_err() {
                tracing::warn!("Sidecar recovery will retry");
            }
        }
    }
}
