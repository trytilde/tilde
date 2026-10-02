use super::db;
use crate::{
    chat::{Chat, ChatError, Result, activity, id},
    deployment::tokens::IngressClaims,
    proto::tilde::{provider::tilde::v1 as wire, types::v1 as types},
};
use uuid::Uuid;
impl Chat {
    pub(crate) async fn tilde_sessions(
        &self,
        scope: &IngressClaims,
        query: &str,
        after: &str,
        limit: u32,
    ) -> Result<(Vec<wire::Session>, String)> {
        if query.chars().count() > 256 {
            return Err(ChatError::Invalid("Search is too long".into()));
        }
        let size = if limit == 0 { 30 } else { limit.min(100) } as usize;
        let after = if after.is_empty() {
            None
        } else {
            Some(id(after)?)
        };
        let mut rows = db::sessions(
            &self.pg()?.get().await?,
            scope.agent_id,
            scope.user_id,
            query,
            after,
            size as i64 + 1,
        )
        .await?;
        let more = rows.len() > size;
        rows.truncate(size);
        let next = if more {
            rows.last().map(|r| r.id.to_string()).unwrap_or_default()
        } else {
            String::new()
        };
        let mut sessions = vec![];
        for row in rows {
            sessions.push(wire::Session {
                thread: self.thread(row.id).await?.into(),
                unread: row.unread,
                preview: row.preview,
                ..Default::default()
            });
        }
        Ok((sessions, next))
    }
    pub(crate) async fn tilde_session(
        &self,
        scope: &IngressClaims,
        thread: Uuid,
    ) -> Result<wire::Session> {
        let roster = self.thread(thread).await?;
        let unread = if let Some(user) = scope.user_id {
            db::unread(&self.pg()?.get().await?, thread, user).await?
        } else {
            false
        };
        let run = match db::latest_run(&self.pg()?.get().await?, thread, scope.agent_id).await? {
            Some(row) => Some(self.run(row.id).await?),
            None => None,
        };
        let preview = self
            .messages(thread, 1)
            .await?
            .first()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        Ok(wire::Session {
            thread: roster.into(),
            unread,
            preview,
            run: run.into(),
            ..Default::default()
        })
    }
    pub(crate) async fn rename_session(&self, thread: Uuid, title: &str) -> Result<()> {
        if title.trim().is_empty() || title.chars().count() > 200 {
            return Err(ChatError::Invalid("Title must be 1–200 characters".into()));
        }
        if let Some(local) = self.local() {
            return local.rename_session(thread, title).await;
        }
        let mut client = self.pg()?.get().await?;
        let tx = client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, thread).await?;
        db::rename(&tx, thread, title).await?;
        activity(self.pg()?, &tx, thread, "thread.updated", thread, "").await?;
        tx.commit().await?;
        Ok(())
    }
    pub(crate) async fn set_read_state(
        &self,
        thread: Uuid,
        user: Uuid,
        sequence: i64,
        unread: bool,
    ) -> Result<()> {
        if sequence < 0 {
            return Err(ChatError::Invalid("Invalid activity sequence".into()));
        }
        let mut client = self.pg()?.get().await?;
        let tx = client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, thread).await?;
        // Never acknowledge unseen future events, even if the client supplies a larger cursor.
        let latest = db::sequence(&tx, thread).await?;
        db::read_state(&tx, thread, user, sequence.min(latest), unread).await?;
        tx.commit().await?;
        Ok(())
    }
    pub(crate) async fn queued_messages(
        &self,
        thread: Uuid,
        agent: Uuid,
    ) -> Result<Vec<types::QueuedInput>> {
        if let Some(local) = self.local() {
            return local.queued_messages(thread).await;
        }
        Ok(db::queue(&self.pg()?.get().await?, thread, agent)
            .await?
            .into_iter()
            .map(queued_input)
            .collect())
    }
    pub(crate) async fn change_queue(
        &self,
        thread: Uuid,
        agent: Uuid,
        item: Uuid,
        change: QueueChange,
    ) -> Result<()> {
        if let Some(local) = self.local() {
            return local.change_queue(thread, item, change).await;
        }
        let mut client = self.pg()?.get().await?;
        let tx = client.transaction().await?;
        // Match dispatch's lock order; the thread lock makes consume/remove/reorder atomic.
        if matches!(change, QueueChange::Steer) {
            crate::chat::db::agent_available_opt(&tx, agent)
                .await?
                .ok_or(ChatError::Conflict)?;
        }
        crate::chat::db::thread_lock_one(&tx, thread).await?;
        let mut inputs = db::queue(&tx, thread, agent).await?;
        let position = inputs
            .iter()
            .position(|i| i.id == item)
            .ok_or(ChatError::Conflict)?;
        let selected = inputs.remove(position);
        match change {
            QueueChange::Remove => {
                db::remove_input(&tx, selected.invocation_id, item).await?;
                db::cancel_message(&tx, thread, item).await?;
                activity(self.pg()?, &tx, thread, "message.aborted", item, "").await?;
            }
            QueueChange::Before(before) => {
                let index = match before {
                    Some(before) => inputs
                        .iter()
                        .position(|i| i.id == before)
                        .ok_or(ChatError::Conflict)?,
                    None => inputs.len(),
                };
                inputs.insert(index, selected);
                for (index, input) in inputs.iter().enumerate() {
                    db::order_input(&tx, input.invocation_id, input.id, index as i64).await?;
                }
            }
            QueueChange::Steer => {
                inputs.insert(0, selected.clone());
                for (index, input) in inputs.iter().enumerate() {
                    db::order_input(&tx, input.invocation_id, input.id, index as i64).await?;
                }
                let invocation = if let Some(active) =
                    crate::chat::db::invocation_active_opt(&tx, thread, agent).await?
                {
                    crate::chat::db::invocation_endpoint_one(&tx, active.id).await?
                } else {
                    crate::chat::db::invocation_endpoint_one(&tx, selected.invocation_id).await?
                };
                // Stop/restart can leave pending inputs on an older invocation. Bring
                // every pending item into the selected queue before dispatching it.
                let sources: std::collections::HashSet<_> =
                    inputs.iter().map(|input| input.invocation_id).collect();
                for source in sources {
                    if source != invocation.id {
                        crate::chat::db::input_move_execute(&tx, source, invocation.id).await?;
                    }
                }
                if matches!(invocation.status.as_str(), "pending" | "running") {
                    crate::chat::db::invocation_finish_one(&tx, invocation.id, "canceled").await?;
                    crate::chat::db::run_status_execute(&tx, invocation.run_id, "canceled").await?;
                    activity(
                        self.pg()?,
                        &tx,
                        thread,
                        "invocation.ended",
                        invocation.id,
                        "canceled",
                    )
                    .await?;
                }
                crate::chat::agent_lifecycle::requeue_inputs(
                    self.pg()?,
                    &tx,
                    invocation.id,
                    invocation.run_id,
                )
                .await?;
            }
        }
        activity(self.pg()?, &tx, thread, "queue.updated", item, "").await?;
        tx.commit().await?;
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub(crate) enum QueueChange {
    Remove,
    Before(Option<Uuid>),
    Steer,
}
/// A free function rather than `From`: both the query row and the contract type now live in
/// other crates (`tilde-queries` and `tilde-contracts`), so an impl here would be an orphan.
pub(crate) fn queued_input(row: db::Queue) -> types::QueuedInput {
    types::QueuedInput {
        id: row.id.to_string(),
        invocation_id: row.invocation_id.to_string(),
        text: row.text,
        history_through_message_id: row.history_through_message_id,
        ..Default::default()
    }
}
