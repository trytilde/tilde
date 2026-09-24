//! Leases held by replicas that stopped heartbeating. The holder's interrupted
//! invocations end; under the reassign policy the lease moves to a live replica,
//! which restarts the run from its objective when it hydrates the thread; under
//! the stop policy the run fails and the lease is dropped.
use super::{Deployments, LIVENESS, liveness_secs};
use crate::database::Transaction;
use crate::error::Error;
use std::time::Duration;
use uuid::Uuid;
impl Deployments {
    /// End a dead holder's work on one thread. `next` receives the lease and the
    /// holder's queued directives; `stop` fails the active run instead of leaving
    /// it for the new holder to restart.
    pub(crate) async fn take_over(
        &self,
        tx: &Transaction<'_>,
        agent: Uuid,
        thread: Uuid,
        previous: Uuid,
        next: Option<Uuid>,
        stop: bool,
    ) -> Result<(), Error> {
        for failed in crate::deployment::db::fail_old_invocations_all(tx, thread, agent).await? {
            crate::chat::activity(&self.pool, tx, thread, "invocation.ended", failed.id, "")
                .await?;
        }
        if stop {
            for run in crate::deployment::db::stop_runs_all(tx, thread, agent).await? {
                crate::chat::activity(&self.pool, tx, thread, "run.updated", run.id, "").await?;
            }
        }
        match next {
            Some(next) => {
                crate::deployment::db::lease_move_execute(tx, thread, agent, next).await?;
                crate::deployment::db::directives_move_execute(tx, thread, previous, next).await?;
            }
            None => {
                crate::deployment::db::lease_delete_execute(tx, thread, agent).await?;
            }
        }
        Ok(())
    }
    pub async fn recover(&self) -> Result<usize, Error> {
        let rows = crate::deployment::db::dead_leases_all(&self.pool.get().await?, liveness_secs())
            .await?;
        let mut count = 0;
        for row in rows {
            let mut tx_client = self.pool.get().await?;
            let tx = tx_client.transaction().await?;
            crate::chat::access::lock_thread_route(&tx, row.thread_id).await?;
            let Some(current) =
                crate::deployment::db::lease_lock_opt(&tx, row.thread_id, row.agent_id).await?
            else {
                continue;
            };
            if current.instance_id != row.instance_id
                || crate::deployment::db::instance_live_one(
                    &tx,
                    row.agent_id,
                    row.instance_id,
                    liveness_secs(),
                )
                .await?
                .live
            {
                continue;
            }
            if !row.active {
                // Nothing was running: the next replica to touch the thread takes it.
                crate::deployment::db::lease_delete_execute(&tx, row.thread_id, row.agent_id)
                    .await?;
                tx.commit().await?;
                drop(tx_client);
                continue;
            }
            let stop = row.failure_mode == "stop";
            let next = if stop {
                None
            } else {
                // Failover stays within the dead instance's deployment when one is live.
                let Some(node) = crate::deployment::db::live_node_opt(
                    &tx,
                    row.agent_id,
                    Some(row.instance_id),
                    row.deployment_id,
                    liveness_secs(),
                )
                .await?
                else {
                    // No replacement yet; the run waits for one.
                    continue;
                };
                Some(node.instance_id)
            };
            self.take_over(
                &tx,
                row.agent_id,
                row.thread_id,
                row.instance_id,
                next,
                stop,
            )
            .await?;
            tx.commit().await?;
            drop(tx_client);
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
                _=prune.tick()=>{ let _ = async { crate::deployment::db::prune_one(&self.pool.get().await?).await }.await; },
            }
            if self.recover().await.is_err() {
                tracing::warn!("Sidecar recovery will retry");
            }
        }
    }
}
