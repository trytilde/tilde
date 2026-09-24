//! Native UI mutations execute under the same lease and lock as input dispatch.
use super::*;
use crate::chat::providers::tilde::QueueChange;
use types::agent_command::Action;
impl Runtime {
    pub(crate) async fn rename_session(&self, thread: Uuid, title: &str) -> Result<()> {
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        t.thread.title = title.into();
        let roster = t.thread.clone();
        self.emit(&mut t, "thread.updated", roster.into(), None)?;
        drop(t);
        self.changed(thread);
        Ok(())
    }
    fn queue_snapshot(&self, t: &ThreadState) -> Vec<types::QueuedInput> {
        t.commands
            .iter()
            .filter_map(|command| {
                if command.finished_at.is_some() {
                    return None;
                }
                match &command.command.action {
                    Some(Action::Steer(input)) => Some(types::QueuedInput {
                        id: input.input_id.clone(),
                        invocation_id: input.invocation_id.clone(),
                        text: input.text.clone(),
                        history_through_message_id: input.history_through_message_id.clone(),
                        ..Default::default()
                    }),
                    _ => None,
                }
            })
            .collect()
    }
    pub(crate) fn emit_queue(&self, t: &mut ThreadState) -> Result<()> {
        let state = types::QueueState {
            agent_id: self.agent_id.to_string(),
            items: self.queue_snapshot(t),
            ..Default::default()
        };
        self.emit(t, "queue.updated", state.into(), None)?;
        Ok(())
    }
    pub(crate) async fn queued_messages(&self, thread: Uuid) -> Result<Vec<types::QueuedInput>> {
        let shared = self.load(thread).await?;
        let t = shared.lock().await;
        self.require_owner(&t)?;
        Ok(self.queue_snapshot(&t))
    }
    pub(crate) async fn change_queue(
        &self,
        thread: Uuid,
        item: Uuid,
        change: QueueChange,
    ) -> Result<()> {
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        if matches!(change, QueueChange::Steer) && self.paused() {
            return Err(ChatError::Conflict);
        }
        let pending = |c: &store::Command| {
            c.finished_at.is_none() && matches!(&c.command.action, Some(Action::Steer(_)))
        };
        let index=t.commands.iter().position(|c|pending(c)&&matches!(&c.command.action,Some(Action::Steer(input)) if input.input_id==item.to_string())).ok_or(ChatError::Conflict)?;
        match change {
            QueueChange::Remove => {
                t.commands[index].finished_at = Some(now());
                if let Some(mut message) = t.message(item).cloned() {
                    message.status = "aborted".into();
                    t.upsert_message(message.clone());
                    self.emit(&mut t, "message.aborted", message.into(), None)?;
                }
            }
            QueueChange::Before(before) => {
                let target=match before {Some(before)=>t.commands.iter().position(|c|pending(c)&&matches!(&c.command.action,Some(Action::Steer(input)) if input.input_id==before.to_string())).ok_or(ChatError::Conflict)?,None=>t.commands.len()};
                let command = t.commands.remove(index);
                t.commands
                    .insert(if target > index { target - 1 } else { target }, command);
            }
            QueueChange::Steer => {
                let command = t.commands.remove(index);
                let Some(Action::Steer(input)) = &command.command.action else {
                    return Err(ChatError::Conflict);
                };
                let invocation = id(&input.invocation_id)?;
                t.commands.insert(0, command);
                let mut ended = if let Some(current) = t
                    .active_invocation(self.agent_id)
                    .cloned()
                    .or_else(|| t.invocations.get(&invocation).cloned())
                {
                    current
                } else {
                    let run = t
                        .runs
                        .values()
                        .find(|run| run.invocation_id == invocation.to_string())
                        .ok_or(ChatError::Conflict)?;
                    types::InvocationState {
                        id: invocation.to_string(),
                        run_id: run.id.clone(),
                        thread_id: thread.to_string(),
                        agent_id: self.agent_id.to_string(),
                        participant_id: self.agent_participant(&t)?.to_string(),
                        status: run.invocation_status.clone(),
                        ..Default::default()
                    }
                };
                let invocation = id(&ended.id)?;
                for command in t.commands.iter_mut().filter(|c| c.finished_at.is_none()) {
                    if let Some(Action::Steer(input)) = &mut command.command.action {
                        input.invocation_id = ended.id.clone();
                    }
                }
                ended.status = "canceled".into();
                ended.ended_at = now();
                let mut run = t
                    .runs
                    .get(&id(&ended.run_id)?)
                    .cloned()
                    .ok_or(ChatError::NotFound)?;
                run.status = "canceled".into();
                run.invocation_status = "canceled".into();
                t.invocations.insert(invocation, ended.clone());
                t.runs.insert(id(&run.id)?, run.clone());
                self.emit(&mut t, "invocation.ended", ended.clone().into(), None)?;
                self.emit(&mut t, "run.updated", run.into(), None)?;
                self.dispatch_queued(&mut t, &ended)?;
            }
        }
        self.emit_queue(&mut t)?;
        drop(t);
        self.changed(thread);
        Ok(())
    }
}
