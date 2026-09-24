//! Canonical tool-call records remain durable; typed history is flushed asynchronously.
//! A claimed call ID is never executed twice. Interrupted external effects are marked aborted,
//! not silently retried: upstream APIs differ in their idempotency guarantees.
use super::{Context, ToolResult};
use crate::chat::{Chat, ChatError, Result, Scope, id};
use crate::proto::tilde::types::v1 as types;
use uuid::Uuid;

impl Chat {
    pub async fn report_tool_call(&self, scope: &Scope, tool: types::ToolCall) -> Result<()> {
        if let Some(local) = self.local() {
            return local.report_tool_call(scope, tool).await;
        }
        if tool.name.is_empty()
            || tool.name.len() > 128
            || tool.input_json.len() > 1024 * 1024
            || tool.output_json.len() > 1024 * 1024
            || tool.input_delta.len() > 64 * 1024
            || tool.error.len() > 2048
        {
            return Err(ChatError::Invalid("Invalid tool audit record".into()));
        }
        for json in [&tool.input_json, &tool.output_json] {
            if !json.is_empty() && serde_json::from_str::<serde_json::Value>(json).is_err() {
                return Err(ChatError::Invalid("Invalid tool JSON".into()));
            }
        }
        let call = id(&tool.id)?;
        let kind = match tool.status.as_str() {
            "running" if tool.input_delta.is_empty() => "tool.started",
            "running" => "tool.input.delta",
            "completed" => "tool.completed",
            "failed" => "tool.failed",
            "aborted" => "tool.aborted",
            _ => return Err(ChatError::Invalid("Invalid tool status".into())),
        };
        let valid_start =
            kind == "tool.started" && !tool.input_json.is_empty() && tool.output_json.is_empty();
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        let (sequence, at) = crate::chat::audit::lock(&tx, scope.thread_id).await?;
        let mut event = types::Activity {
            kind: kind.into(),
            entity_id: call.to_string(),
            participant_id: scope.participant_id.to_string(),
            invocation_id: scope.id.to_string(),
            detail: Some(tool.clone().into()),
            ..Default::default()
        };
        crate::chat::audit::prepare(scope.thread_id, sequence, at, &mut event);
        let result = crate::chat::db::tool_write_one(
            &tx,
            call,
            scope.thread_id,
            scope.id,
            scope.participant_id,
            &(tool.name),
            &(tool.provider_id),
            &(tool.status),
            &(tool.input_json),
            &(tool.output_json),
            &(tool.error),
            kind,
            sequence,
        )
        .await?;
        if !result.applied && !result.replayed {
            if !result.existed && !valid_start {
                return Err(ChatError::Invalid("Tool call must begin with input".into()));
            }
            return Err(ChatError::Conflict);
        }
        if result.applied {
            crate::chat::audit::enqueue(
                self.pg()?,
                scope.thread_id,
                result.transaction_id,
                at,
                event,
            )?;
        }
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    pub(crate) async fn expire_tool_calls(&self) -> Result<()> {
        for row in crate::chat::db::tool_interrupted_all(&self.pg()?.get().await?).await? {
            self.report_tool_call(
                &Scope {
                    capabilities: Default::default(),
                    id: row.invocation_id,
                    run_id: Uuid::nil(),
                    thread_id: row.thread_id,
                    agent_id: Uuid::nil(),
                    participant_id: row.participant_id,
                    inference: vec![],
                },
                types::ToolCall {
                    id: row.id.to_string(),
                    name: row.name,
                    provider_id: row.provider_id,
                    status: "aborted".into(),
                    error: "Invocation ended before tool completion".into(),
                    ..Default::default()
                },
            )
            .await?;
        }
        Ok(())
    }
}
/// Drop is best-effort; the durable invocation expiry worker also closes abandoned calls.
pub struct Execution {
    chat: Chat,
    scope: Scope,
    tool: types::ToolCall,
    armed: bool,
}
impl Execution {
    pub async fn begin(
        context: &Context,
        name: &str,
        provider: &str,
        input: &serde_json::Value,
    ) -> ToolResult<Self> {
        let tool = types::ToolCall {
            id: context.call_id.to_string(),
            name: name.into(),
            provider_id: provider.into(),
            status: "running".into(),
            input_json: input.to_string(),
            ..Default::default()
        };
        context
            .chat
            .report_tool_call(context.scope(), tool.clone())
            .await?;
        Ok(Self {
            chat: context.chat.clone(),
            scope: context.scope().clone(),
            tool,
            armed: true,
        })
    }
    pub async fn finish(mut self, result: &ToolResult<serde_json::Value>) -> ToolResult<()> {
        self.tool.input_json.clear();
        match result {
            Ok(output) => {
                self.tool.status = "completed".into();
                self.tool.output_json = output.to_string();
            }
            Err(error) => {
                self.tool.status = "failed".into();
                self.tool.error = format!("Tool failed: {:?}", error.code);
            }
        }
        self.chat
            .report_tool_call(&self.scope, self.tool.clone())
            .await?;
        self.armed = false;
        Ok(())
    }
}
impl Drop for Execution {
    fn drop(&mut self) {
        if self.armed {
            let chat = self.chat.clone();
            let scope = self.scope.clone();
            let mut tool = self.tool.clone();
            tool.input_json.clear();
            tool.status = "aborted".into();
            tool.error = "Tool execution interrupted".into();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = chat.report_tool_call(&scope, tool).await;
                });
            }
        }
    }
}
