//! Suspended runs: how a run outlives the invocation that started it, so that neither a host's
//! runtime limit (Lambda's fifteen minutes) nor a slow tool call is the agent's loop's concern.
//!
//! - A slow call answers "pending". A caller that can suspend its loop says so on the call
//!   (`InvokeToolRequest.suspendable`, set by the SDKs' framework adapters); a call of its still
//!   running after the threshold (`TILDE_TOOL_PENDING_AFTER_MS`, a minute), or at the
//!   invocation's deadline, answers `pending` and goes on in the background, and its record
//!   (`chat_tool_calls`) is its outcome.
//! - The deadline is Tilde's: a Lambda (or hosted) invocation is asked to suspend `MARGIN`
//!   before Lambda would end it (`chat_invocations.suspend_at`, delivered as the control
//!   stream's suspend command), and calls in flight then answer pending.
//! - A wait for a person is a call too: `thread.wait_for_reply` answers pending at once, and its
//!   output is the next message routed to the agent in the thread (`answer_reply_wait`).
//! - The adapter then saves its conversation in its model's message format with the pending
//!   call IDs (`suspend_run`) and ends the invocation; the run waits. Once none of those calls is
//!   running and the agent runs nothing else in the thread, `resume_suspended` starts the run's
//!   next invocation (`InvokeRequest.resumes`), which reads the state and the outcomes
//!   (`checkpoint`) and continues its loop. A message that arrived meanwhile started its own run,
//!   as any message does; the suspended run resumes after it.
//!
//! Gateway invocations only: a sidecar never answers pending and refuses `SuspendRun`.
use super::{Chat, ChatError, Result, Scope, activity};
use crate::proto::tilde::runtime::v1 as runtime_pb;
use crate::proto::tilde::types::v1 as types;
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

pub const REPLY_WAIT: &str = "thread.wait_for_reply";
/// What an invocation keeps of its host's limit to save its state and report.
const MARGIN: Duration = Duration::from_secs(120);
/// Lambda's longest invocation, which every Lambda and hosted deployment is held to. A Lambda
/// configured with a shorter timeout is ended by Lambda before the deadline, failing as before.
const LAMBDA_LIMIT: Duration = Duration::from_secs(900);
const MAX_STATE: usize = 32 * 1024 * 1024;
const MAX_PENDING: usize = 256;

/// How long a suspendable caller's call runs before it answers pending.
fn threshold() -> Duration {
    std::env::var("TILDE_TOOL_PENDING_AFTER_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(Duration::from_secs(60), Duration::from_millis)
}

/// When an invocation on a deployment of `target` is asked to suspend, from its wake.
pub(crate) fn deadline(target: Option<&str>) -> Option<Duration> {
    let limited = target == Some("lambda");
    limited.then_some(LAMBDA_LIMIT - MARGIN)
}

pub(crate) fn waits_for_reply(definition: &types::ToolDefinition) -> bool {
    definition.name == REPLY_WAIT && definition.provider_id == "tilde"
}

pub(crate) fn pending_output(call: Uuid) -> Value {
    json!({
        "status": "pending",
        "ticket": call,
        "message": "Still running. Your work is suspended and continues with this call's outcome once it finishes.",
    })
}

/// How long this call may run before it answers pending; `None` when it never does: the caller
/// cannot suspend, its input streams, it is a background tool (answered with a ticket at once)
/// or this is a sidecar.
pub(crate) async fn pending_after(
    chat: &Chat,
    scope: &Scope,
    first: &runtime_pb::InvokeToolRequest,
    definition: &types::ToolDefinition,
) -> Result<Option<Duration>> {
    if !first.suspendable || !first.finish || definition.detached || chat.local().is_some() {
        return Ok(None);
    }
    let client = chat.pg()?.get().await?;
    // From now on the invocation is asked to suspend at its deadline.
    crate::chat::db::invocation_suspendable_execute(&client, scope.id).await?;
    if waits_for_reply(definition) {
        return Ok(Some(Duration::ZERO));
    }
    let suspend_at = crate::chat::db::invocation_suspend_at_opt(&client, scope.id).await?;
    let until_deadline =
        suspend_at.map(|at| (at - chrono::Utc::now()).to_std().unwrap_or_default());
    Ok(Some(
        until_deadline.map_or(threshold(), |left| left.min(threshold())),
    ))
}

impl Chat {
    /// Save the invocation's state and the calls it waits for, and move its run to `suspending`:
    /// when the invocation ends the run waits for `resume_suspended`.
    pub async fn suspend_run(
        &self,
        scope: &Scope,
        state_json: &str,
        pending: &[Uuid],
    ) -> Result<()> {
        if self.local().is_some() {
            return Err(ChatError::Invalid(
                "Runs suspend on the gateway only".into(),
            ));
        }
        if state_json.len() > MAX_STATE || serde_json::from_str::<Value>(state_json).is_err() {
            return Err(ChatError::Invalid("Invalid suspension state".into()));
        }
        let mut pending = pending.to_vec();
        pending.sort();
        pending.dedup();
        if pending.len() > MAX_PENDING {
            return Err(ChatError::Invalid("Too many pending calls".into()));
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, scope.thread_id).await?;
        // Calls of this run only: another's outcome is not this agent's to read, and an unknown
        // ID would keep the run waiting forever.
        let calls = crate::chat::db::checkpoint_results_all(&tx, &pending, scope.run_id).await?;
        if calls.len() != pending.len() {
            return Err(ChatError::Invalid(
                "A pending call is not this run's".into(),
            ));
        }
        if crate::chat::db::controls_suspend_execute(&tx, scope.run_id, scope.id, "pending").await?
            == 0
        {
            return Err(ChatError::Conflict);
        }
        crate::chat::db::checkpoint_save_execute(&tx, scope.run_id, scope.id, state_json, &pending)
            .await?;
        crate::chat::db::invocation_checkpointed_execute(&tx, scope.id).await?;
        // A reply that arrived while the loop was still deciding to wait is queued on this
        // invocation, unseen: it answers the wait, so the run resumes now rather than at the
        // person's next message.
        if let Some(wait) = calls
            .iter()
            .find(|call| call.name == REPLY_WAIT && call.status == "running")
        {
            let replies = crate::chat::db::input_answers_all(&tx, scope.id).await?;
            if let Some(first) = replies.first() {
                let text = replies
                    .iter()
                    .map(|r| r.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let tool = types::ToolCall {
                    id: wait.id.to_string(),
                    name: wait.name.clone(),
                    provider_id: wait.provider_id.clone(),
                    status: "completed".into(),
                    output_json: json!({"message_id": first.id, "text": text}).to_string(),
                    ..Default::default()
                };
                self.write_tool_call(&tx, scope, tool).await?;
                for reply in &replies {
                    crate::chat::db::input_accept_execute(&tx, scope.id, reply.id).await?;
                }
            }
        }
        activity(
            self.pg()?,
            &tx,
            scope.thread_id,
            "invocation.suspended",
            scope.id,
            "",
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The state the run's previous invocation suspended with and its pending calls' records.
    pub async fn checkpoint(&self, scope: &Scope) -> Result<runtime_pb::GetCheckpointResponse> {
        if self.local().is_some() {
            return Err(ChatError::NotFound);
        }
        let client = self.pg()?.get().await?;
        let saved = crate::chat::db::checkpoint_get_opt(&client, scope.run_id)
            .await?
            .filter(|saved| saved.invocation_id != scope.id)
            .ok_or(ChatError::NotFound)?;
        let results =
            crate::chat::db::checkpoint_results_all(&client, &saved.pending_call_ids, scope.run_id)
                .await?;
        Ok(runtime_pb::GetCheckpointResponse {
            state_json: saved.state_json,
            results: results
                .into_iter()
                .map(|call| types::ToolCall {
                    id: call.id.to_string(),
                    name: call.name,
                    provider_id: call.provider_id,
                    status: call.status,
                    output_json: call.output_json,
                    error: call.error,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }

    /// Ask invocations at their deadline to suspend (one pass of the chat worker's).
    pub async fn suspend_due(&self) -> Result<()> {
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        for due in crate::chat::db::runs_suspend_due_all(&tx).await? {
            activity(
                self.pg()?,
                &tx,
                due.thread_id,
                "invocation.suspend_requested",
                due.id,
                "deadline",
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Start the next invocation of every suspended run whose pending calls have all finished
    /// (one pass of the chat worker's).
    pub async fn resume_suspended(&self) -> Result<()> {
        let runs = crate::chat::db::runs_resumable_all(&self.pg()?.get().await?).await?;
        for run in runs {
            // Another process may resume it first, or the agent start something meanwhile.
            if let Err(error) = self.resume_run(run).await
                && !matches!(error, ChatError::Conflict)
            {
                tracing::debug!(%run, %error, "Resuming a suspended run failed");
            }
        }
        Ok(())
    }

    /// The next message routed to the agent in a thread where it waits for a reply is that
    /// wait's output, written in the transaction that claims the message (which then starts no
    /// run of its own). False when the wait was answered meanwhile: the message routes as usual.
    pub(crate) async fn answer_reply_wait(
        &self,
        tx: &crate::database::Transaction<'_>,
        wait: crate::chat::db::ReplyWaitRow,
        thread: Uuid,
        message: Uuid,
        text: &str,
    ) -> Result<bool> {
        let scope = Scope {
            capabilities: Default::default(),
            id: wait.invocation_id,
            run_id: Uuid::nil(),
            thread_id: thread,
            agent_id: Uuid::nil(),
            participant_id: wait.participant_id,
            inference: vec![],
        };
        let tool = types::ToolCall {
            id: wait.id.to_string(),
            name: wait.name,
            provider_id: wait.provider_id,
            status: "completed".into(),
            output_json: json!({"message_id": message, "text": text}).to_string(),
            ..Default::default()
        };
        match self.write_tool_call(tx, &scope, tool).await {
            Ok(()) => Ok(true),
            Err(ChatError::Conflict) => Ok(false),
            Err(error) => Err(error),
        }
    }
}
