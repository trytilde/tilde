//! Owners that stop heartbeating lose their threads. Postgres serializes each
//! reassignment; the new owner receives a directive to restart the run from its
//! objective, or the run fails under the stop policy.
use super::{Deployments, LIVENESS};
use crate::{error::Error, proto::tilde::agent_event_ingress::v1 as wire};
use std::time::Duration;
impl Deployments {
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
            sqlx::query_file!(
                "../../queries/deployment/assign.sql",
                row.thread_id,
                row.participant_id,
                row.agent_id,
                owner,
                generation,
                stopped
            )
            .execute(&mut *tx)
            .await?;
            for failed in sqlx::query_file!(
                "../../queries/deployment/fail_old_invocations.sql",
                row.thread_id,
                row.agent_id
            )
            .fetch_all(&mut *tx)
            .await?
            {
                crate::chat::activity(&mut tx, row.thread_id, "invocation.ended", failed.id, "")
                    .await?;
            }
            crate::chat::activity(
                &mut tx,
                row.thread_id,
                "participant.assigned",
                row.participant_id,
                "",
            )
            .await?;
            let run = sqlx::query_file!(
                "../../queries/deployment/recovery_run.sql",
                row.thread_id,
                row.agent_id
            )
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(run) = run {
                if stopped {
                    sqlx::query_file!("../../queries/deployment/stop_run.sql", run.run_id)
                        .execute(&mut *tx)
                        .await?;
                    crate::chat::activity(&mut tx, row.thread_id, "run.updated", run.run_id, "")
                        .await?;
                } else {
                    let key = uuid::Uuid::new_v4();
                    let directive = wire::Directive {
                        id: key.to_string(),
                        thread_id: row.thread_id.to_string(),
                        generation: generation as u64,
                        action: Some(
                            wire::RecoverRun {
                                run_id: run.run_id.to_string(),
                                participant_id: row.participant_id.to_string(),
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
                        row.agent_id,
                        owner,
                        Some(row.thread_id),
                        generation,
                        payload
                    )
                    .execute(&mut *tx)
                    .await?;
                }
            }
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
