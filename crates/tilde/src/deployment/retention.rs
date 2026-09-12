//! Whole-conversation retirement. Peers fence local mutations, acknowledge their
//! complete event sets, and the gateway verifies those sets are archived before
//! deleting any replica. Archived conversations are never rehydrated.
use super::{Deployments, corrosion::client::statement, runtime::Runtime};
use crate::{
    chat::{ChatError, Result},
    error::Error,
};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;
#[derive(Deserialize)]
struct Retirement {
    thread_id: String,
    epoch: String,
    phase: String,
}
#[derive(Deserialize)]
struct Ack {
    instance_id: String,
    ready: i64,
    event_count: i64,
}
#[derive(Deserialize)]
struct Activity {
    last_activity_at: i64,
}
#[derive(Deserialize)]
struct Count {
    count: i64,
    max_sequence: i64,
}
impl Runtime {
    pub async fn retiring(&self, thread: Uuid) -> Result<bool> {
        Ok(!self
            .client
            .query::<Retirement>(
                "SELECT * FROM retirements WHERE thread_id=?",
                vec![json!(thread)],
            )
            .await?
            .is_empty())
    }
    pub async fn retirement_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        loop {
            if let Ok(mut changes) = self
                .client
                .subscribe::<Retirement>("SELECT * FROM retirements", vec![])
                .await
            {
                loop {
                    tokio::select! {_=shutdown.changed()=>return,value=changes.next()=>match value{
                        Some(Ok(change)) if !change.deleted=>{if self.ack_retirement(change.value).await.is_err(){break;}},Some(Ok(_))=>{},_=>break,
                    }}
                }
            }
            tokio::select! {_=shutdown.changed()=>return,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        }
    }
    async fn ack_retirement(&self, r: Retirement) -> Result<()> {
        if r.phase != "prepare" {
            return Ok(());
        }
        let _guard = self.mutation.lock().await;
        let active=self.client.query::<Count>("SELECT (SELECT COUNT(*) FROM invocations WHERE thread_id=? AND status IN ('pending','running'))+(SELECT COUNT(*) FROM commands WHERE thread_id=? AND finished_at IS NULL)+(SELECT COUNT(*) FROM tool_calls WHERE thread_id=? AND status='running') AS count,0 AS max_sequence",vec![json!(r.thread_id),json!(r.thread_id),json!(r.thread_id)]).await?.pop().ok_or(ChatError::Transport)?.count;
        let events=self.client.query::<Count>("SELECT COUNT(*) AS count,COALESCE(MAX(origin_sequence),0) AS max_sequence FROM events WHERE thread_id=? AND origin_instance_id=? AND (origin_agent_id=? OR origin_agent_id='')",vec![json!(r.thread_id),json!(self.instance_id),json!(self.agent_id)]).await?.pop().ok_or(ChatError::Transport)?;
        self.client.transaction(vec![statement("INSERT INTO retirement_acks(id,thread_id,epoch,instance_id,ready,event_count,max_sequence) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET ready=excluded.ready,event_count=excluded.event_count,max_sequence=excluded.max_sequence",vec![json!(format!("{}:{}:{}",r.thread_id,r.epoch,self.instance_id)),json!(r.thread_id),json!(r.epoch),json!(self.instance_id),json!(i32::from(active==0)),json!(events.count),json!(events.max_sequence)])]).await?;
        Ok(())
    }
}
impl Deployments {
    pub async fn retire_conversations(&self) -> std::result::Result<usize, Error> {
        let rows = sqlx::query_file!("../../queries/deployment/retention/candidates.sql")
            .fetch_all(&self.pool)
            .await?;
        let mut retired = 0;
        for row in rows {
            let Some(peer) = self.peer(row.agent_id).await? else {
                continue;
            };
            let Some(activity) = peer
                .client
                .query::<Activity>(
                    "SELECT last_activity_at FROM threads WHERE id=?",
                    vec![json!(row.thread_id)],
                )
                .await?
                .pop()
            else {
                let committed = peer
                    .client
                    .query::<Retirement>(
                        "SELECT * FROM retirements WHERE thread_id=? AND phase='committed'",
                        vec![json!(row.thread_id)],
                    )
                    .await?
                    .into_iter()
                    .any(|r| {
                        row.retirement_epoch
                            .is_some_and(|epoch| r.epoch == epoch.to_string())
                    });
                if row.storage == "retiring" && committed {
                    sqlx::query_file!(
                        "../../queries/deployment/retention/commit.sql",
                        row.thread_id,
                        row.agent_id
                    )
                    .execute(&self.pool)
                    .await?;
                }
                continue;
            };
            let age = chrono::Utc::now()
                .timestamp_millis()
                .saturating_sub(activity.last_activity_at);
            if !row.paused && age < row.retention_days.saturating_mul(86_400_000) {
                peer.client
                    .transaction(vec![
                        statement(
                            "DELETE FROM retirements WHERE thread_id=?",
                            vec![json!(row.thread_id)],
                        ),
                        statement(
                            "DELETE FROM retirement_acks WHERE thread_id=?",
                            vec![json!(row.thread_id)],
                        ),
                    ])
                    .await?;
                sqlx::query_file!(
                    "../../queries/deployment/retention/cancel.sql",
                    row.thread_id,
                    row.agent_id
                )
                .execute(&self.pool)
                .await?;
                continue;
            }
            let current = peer
                .client
                .query::<Retirement>(
                    "SELECT * FROM retirements WHERE thread_id=?",
                    vec![json!(row.thread_id)],
                )
                .await?
                .pop();
            let retirement = if let Some(current) = current {
                current
            } else {
                let proposed = Uuid::new_v4();
                let Some(record) = sqlx::query_file!(
                    "../../queries/deployment/retention/prepare.sql",
                    row.thread_id,
                    row.agent_id,
                    proposed
                )
                .fetch_optional(&self.pool)
                .await?
                else {
                    continue;
                };
                let epoch = record.retirement_epoch.ok_or(Error::Conflict)?.to_string();
                peer.client.transaction(vec![statement("INSERT INTO retirements(thread_id,epoch,phase) VALUES(?,?,'prepare') ON CONFLICT(epoch) DO NOTHING",vec![json!(row.thread_id),json!(epoch)])]).await?;
                continue;
            };
            let acks = peer
                .client
                .query::<Ack>(
                    "SELECT * FROM retirement_acks WHERE thread_id=? AND epoch=?",
                    vec![json!(row.thread_id), json!(retirement.epoch)],
                )
                .await?;
            if acks.iter().any(|a| a.ready == 0) {
                peer.client
                    .transaction(vec![
                        statement(
                            "DELETE FROM retirements WHERE thread_id=?",
                            vec![json!(row.thread_id)],
                        ),
                        statement(
                            "DELETE FROM retirement_acks WHERE thread_id=?",
                            vec![json!(row.thread_id)],
                        ),
                    ])
                    .await?;
                sqlx::query_file!(
                    "../../queries/deployment/retention/cancel.sql",
                    row.thread_id,
                    row.agent_id
                )
                .execute(&self.pool)
                .await?;
                continue;
            }
            let nodes = sqlx::query_file!(
                "../../queries/deployment/retention/participants.sql",
                row.agent_id
            )
            .fetch_all(&self.pool)
            .await?;
            let mut archived = true;
            for node in nodes {
                let Some(ack) = acks
                    .iter()
                    .find(|a| a.instance_id == node.instance_id.to_string())
                else {
                    archived = false;
                    break;
                };
                // Incarnations can restart with their predecessor's database.
                // Verify each fenced peer's actual immutable IDs, including
                // events written by older incarnations, before deleting it.
                let client = peer.client.at(&format!(
                    "{}/corrosion",
                    node.agent_ingress_url.trim_end_matches('/')
                ))?;
                let records = match client.query::<super::runtime::IdRow>("SELECT id FROM events WHERE thread_id=? AND (origin_agent_id=? OR origin_agent_id='') AND origin_instance_id<>?", vec![json!(row.thread_id),json!(row.agent_id),json!(Uuid::nil())]).await {
                    Ok(records) => records, Err(_) => { archived=false; break; }
                };
                let ids = records
                    .iter()
                    .map(|r| super::id(&r.id))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if ack.event_count > ids.len() as i64 {
                    archived = false;
                    break;
                }
                for batch in ids.chunks(1000) {
                    let stored = sqlx::query_file!(
                        "../../queries/deployment/retention/archived_ids.sql",
                        row.agent_id,
                        batch
                    )
                    .fetch_one(&self.pool)
                    .await?;
                    if stored.count != batch.len() as i64 {
                        archived = false;
                        break;
                    }
                }
                if !archived {
                    break;
                }
            }
            if !archived {
                continue;
            }
            let pending=peer.client.query::<Count>("SELECT COUNT(*) AS count,0 AS max_sequence FROM attachments WHERE thread_id=? AND uploaded=0",vec![json!(row.thread_id)]).await?.pop().ok_or(Error::NotFound)?;
            if pending.count > 0 {
                continue;
            }
            let mut tx = self.pool.begin().await?;
            let current = sqlx::query_file!(
                "../../queries/deployment/retention/last_activity.sql",
                row.thread_id,
                row.agent_id
            )
            .fetch_one(&mut *tx)
            .await?;
            if current.storage != "retiring" {
                continue;
            }
            let mut deletes = vec![];
            for sql in [
                "DELETE FROM converted_messages WHERE thread_id=?",
                "DELETE FROM tool_calls WHERE thread_id=?",
                "DELETE FROM typing WHERE thread_id=?",
                "DELETE FROM channel_receipts WHERE thread_id=?",
                "DELETE FROM channel_threads WHERE thread_id=?",
                "DELETE FROM attachments WHERE thread_id=?",
                "DELETE FROM tasks WHERE thread_id=?",
                "DELETE FROM goals WHERE thread_id=?",
                "DELETE FROM commands WHERE thread_id=?",
                "DELETE FROM control_receipts WHERE thread_id=?",
                "DELETE FROM message_dispatch WHERE thread_id=?",
                "DELETE FROM invocations WHERE thread_id=?",
                "DELETE FROM runs WHERE thread_id=?",
                "DELETE FROM messages WHERE thread_id=?",
                "DELETE FROM bridge_message_versions WHERE thread_id=?",
                "DELETE FROM participants WHERE thread_id=?",
                "DELETE FROM assignments WHERE thread_id=?",
                "DELETE FROM events WHERE thread_id=?",
                "DELETE FROM retirement_acks WHERE thread_id=?",
                "DELETE FROM threads WHERE id=?",
            ] {
                deletes.push(statement(sql, vec![json!(row.thread_id)]));
            }
            deletes.push(statement(
                "UPDATE retirements SET phase='committed' WHERE epoch=?",
                vec![json!(retirement.epoch)],
            ));
            // Retain proof of the completed deletion until Postgres commits its
            // storage decision. An arbitrary missing replica is never proof.
            peer.client.transaction(deletes).await?;
            sqlx::query_file!(
                "../../queries/deployment/retention/commit.sql",
                row.thread_id,
                row.agent_id
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            retired += 1;
        }
        Ok(retired)
    }
}

impl Deployments {
    pub async fn retention_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {_=shutdown.changed()=>return,_=tick.tick()=>{
                tokio::select!{_=shutdown.changed()=>return,result=async { self.retire_conversations().await?; self.reap_telemetry().await }=>{if result.is_err(){tracing::warn!("Conversation retirement will retry");}}}
            }}
        }
    }
}
