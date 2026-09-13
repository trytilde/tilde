//! Owners that stop heartbeating lose their threads. Postgres serializes each
//! reassignment; the new owner receives a directive to restart the run from its
//! objective, or the run fails under the stop policy.
use super::{Deployments, LIVENESS};
use crate::{error::Error, proto::tilde::agent_event_ingress::v1 as wire};
use sqlx::{Postgres, Transaction};
use std::time::Duration;
use uuid::Uuid;
/// One conversation moving away from an owner that is gone.
pub(crate) struct TakeOver {
    pub thread: Uuid,
    pub participant: Uuid,
    pub agent: Uuid,
    pub previous: Uuid,
    pub owner: Uuid,
    pub generation: i64,
    /// Stop policy for a dead owner with no replacement: the assignment stays put, marked stopped.
    pub mark_stopped: bool,
    /// Reassign policy: the new owner restarts the run; otherwise the run fails.
    pub restart_run: bool,
}
impl Deployments {
    /// Move a conversation to a new generation: queued work and the active run follow
    /// it, and the previous owner's invocations are over. Used by recovery and by
    /// claims that find the current owner dead, so both paths leave the same state.
    pub(crate) async fn take_over(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        t: TakeOver,
    ) -> Result<(), Error> {
        sqlx::query_file!(
            "../../queries/deployment/assign.sql",
            t.thread,
            t.participant,
            t.agent,
            t.owner,
            t.generation,
            t.mark_stopped
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query_file!(
            "../../queries/deployment/directives_reassign.sql",
            t.thread,
            t.previous,
            t.owner
        )
        .execute(&mut **tx)
        .await?;
        for failed in sqlx::query_file!(
            "../../queries/deployment/fail_old_invocations.sql",
            t.thread,
            t.agent
        )
        .fetch_all(&mut **tx)
        .await?
        {
            crate::chat::activity(tx, t.thread, "invocation.ended", failed.id, "").await?;
        }
        crate::chat::activity(tx, t.thread, "participant.assigned", t.participant, "").await?;
        let run = sqlx::query_file!(
            "../../queries/deployment/recovery_run.sql",
            t.thread,
            t.agent
        )
        .fetch_optional(&mut **tx)
        .await?;
        let Some(run) = run else {
            return Ok(());
        };
        if !t.restart_run {
            sqlx::query_file!("../../queries/deployment/stop_run.sql", run.run_id)
                .execute(&mut **tx)
                .await?;
            crate::chat::activity(tx, t.thread, "run.updated", run.run_id, "").await?;
            return Ok(());
        }
        let key = Uuid::new_v4();
        let directive = wire::Directive {
            id: key.to_string(),
            thread_id: t.thread.to_string(),
            generation: t.generation as u64,
            action: Some(
                wire::RecoverRun {
                    run_id: run.run_id.to_string(),
                    participant_id: t.participant.to_string(),
                    objective: run.objective,
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        };
        let payload = self.seal_record(key, "directive", &directive)?;
        sqlx::query_file!(
            "../../queries/deployment/directive_insert.sql",
            key,
            t.agent,
            t.owner,
            Some(t.thread),
            t.generation,
            payload
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
    pub async fn recover(&self) -> Result<usize, Error> {
        let rows = sqlx::query_file!("../../queries/deployment/dead_owners.sql")
            .fetch_all(&self.pool)
            .await?;
        let mut count = 0;
        for row in rows {
            let mut tx = self.pool.begin().await?;
            crate::chat::access::lock_thread_route(&mut tx, row.thread_id).await?;
            let Some(current) = sqlx::query_file!(
                "../../queries/deployment/assignment_lock.sql",
                row.thread_id,
                row.participant_id
            )
            .fetch_optional(&mut *tx)
            .await?
            else {
                continue;
            };
            if current.generation != row.generation || current.stopped {
                continue;
            }
            if sqlx::query_file!(
                "../../queries/deployment/instance_live.sql",
                row.agent_id,
                current.owner_instance_id
            )
            .fetch_one(&mut *tx)
            .await?
            .live
            {
                continue;
            }
            let stopped = row.failure_mode == "stop";
            let owner = if stopped {
                row.owner_instance_id
            } else {
                let Some(node) = sqlx::query_file!(
                    "../../queries/deployment/replacement.sql",
                    row.agent_id,
                    row.owner_instance_id
                )
                .fetch_optional(&mut *tx)
                .await?
                else {
                    continue;
                };
                node.instance_id
            };
            let generation = current.generation.checked_add(1).ok_or(Error::Conflict)?;
            self.take_over(
                &mut tx,
                TakeOver {
                    thread: row.thread_id,
                    participant: row.participant_id,
                    agent: row.agent_id,
                    previous: row.owner_instance_id,
                    owner,
                    generation,
                    mark_stopped: stopped,
                    restart_run: !stopped,
                },
            )
            .await?;
            tx.commit().await?;
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
        let mut cleanup = tokio::time::interval(Duration::from_secs(600));
        loop {
            tokio::select! {
                _=shutdown.changed()=>return,
                _=changes.changed()=>{ changes.borrow_and_update(); },
                _=tick.tick()=>{},
                _=cleanup.tick()=>{ let _ = sqlx::query_file!("../../queries/deployment/directive_cleanup.sql").execute(&self.pool).await; },
            }
            if self.recover().await.is_err() {
                tracing::warn!("Sidecar recovery will retry");
            }
        }
    }
}
