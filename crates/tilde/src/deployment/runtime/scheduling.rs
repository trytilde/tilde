//! Input scheduling under the thread lease. The existing command list retains queued
//! inputs; hosts receive fresh invocation snapshots and outbound stop controls.
use super::*;
use types::agent_command::Action;

impl Runtime {
    fn concurrency_policy(&self) -> Result<crate::agent::ConcurrencyPolicy> {
        crate::agent::ConcurrencyPolicy::from_wire(
            self.configuration()?.agent.concurrency_policy.to_i32(),
        )
        .map_err(|_| ChatError::Invalid("Invalid concurrency policy".into()))
    }

    /// Apply policy after inserting an input. Caller holds the thread lock.
    pub(crate) fn schedule_input(
        &self,
        t: &mut ThreadState,
        invocation: &types::InvocationState,
    ) -> Result<()> {
        use crate::agent::ConcurrencyPolicy as Policy;
        match self.concurrency_policy()? {
            Policy::Interrupt => {
                let mut ended = invocation.clone();
                ended.status = "canceled".into();
                ended.ended_at = now();
                let run_id = id(&ended.run_id)?;
                let mut run = t.runs.get(&run_id).cloned().ok_or(ChatError::NotFound)?;
                run.status = "canceled".into();
                run.invocation_status = "canceled".into();
                t.invocations.insert(id(&ended.id)?, ended.clone());
                t.runs.insert(run_id, run.clone());
                self.emit(t, "invocation.ended", ended.clone().into(), None)?;
                self.emit(t, "run.updated", run.into(), None)?;
                self.dispatch_queued(t, &ended)
            }
            Policy::QueueAndBatch if invocation.status == "pending" => {
                let inputs = self.queued(t, &invocation.id);
                if inputs.is_empty() {
                    return Ok(());
                }
                let mut current = invocation.clone();
                let run_id = id(&current.run_id)?;
                let mut run = t.runs.get(&run_id).cloned().ok_or(ChatError::NotFound)?;
                for (_, input) in &inputs {
                    run.objective.push('\n');
                    run.objective.push_str(&input.text);
                    current.history_through_message_id = input.history_through_message_id.clone();
                }
                current.objective = run.objective.clone();
                for command in &mut t.commands {
                    if let Some(Action::Invoke(wake)) = &mut command.command.action
                        && wake.invocation_id == current.id
                    {
                        wake.objective = run.objective.clone();
                    }
                }
                for (index, _) in inputs {
                    t.commands[index].finished_at = Some(now());
                }
                t.invocations.insert(id(&current.id)?, current.clone());
                t.runs.insert(run_id, run.clone());
                self.emit(t, "run.updated", run.into(), None)?;
                self.emit(t, "invocation.pending", current.into(), None)?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn queued(&self, t: &ThreadState, invocation: &str) -> Vec<(usize, types::SteerCommand)> {
        t.commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| {
                if command.finished_at.is_some() {
                    return None;
                }
                match &command.command.action {
                    Some(Action::Steer(input)) if input.invocation_id == invocation => {
                        Some((index, (**input).clone()))
                    }
                    _ => None,
                }
            })
            .collect()
    }

    /// Start one event or one batch, and transfer remaining inputs to the fresh invocation.
    pub(crate) fn dispatch_queued(
        &self,
        t: &mut ThreadState,
        previous: &types::InvocationState,
    ) -> Result<()> {
        let inputs = self.queued(t, &previous.id);
        if inputs.is_empty() {
            return Ok(());
        }
        let count = if self.concurrency_policy()? == crate::agent::ConcurrencyPolicy::Queue {
            1
        } else {
            inputs.len()
        };
        let selected = &inputs[..count];
        let last = &selected.last().expect("nonempty batch").1;
        let next_id = Uuid::new_v5(&id(&previous.id)?, selected[0].1.input_id.as_bytes());
        let mut next = types::Run {
            id: next_id.to_string(),
            thread_id: previous.thread_id.clone(),
            agent_id: previous.agent_id.clone(),
            objective: selected
                .iter()
                .map(|(_, input)| input.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            status: "active".into(),
            ..Default::default()
        };
        let mut origin = t
            .run_meta
            .get(&id(&previous.run_id)?)
            .cloned()
            .unwrap_or_default();
        if last.message.is_set() {
            let message = &last.message;
            origin.channel_origin = message.delivery.is_set();
            origin.source_identity_id = t
                .thread
                .participants
                .iter()
                .find(|p| p.id == message.participant_id)
                .and_then(|p| p.user_id.clone())
                .unwrap_or_default();
        }
        t.run_meta.insert(next_id, origin);
        self.begin_invocation(
            t,
            &mut next,
            id(&previous.participant_id)?,
            Some(last.history_through_message_id.clone()),
        )?;
        for (index, _) in selected {
            t.commands[*index].finished_at = Some(now());
        }
        for (index, _) in &inputs[count..] {
            if let Some(Action::Steer(input)) = &mut t.commands[*index].command.action {
                input.invocation_id = next.invocation_id.clone();
            }
        }
        Ok(())
    }
}
