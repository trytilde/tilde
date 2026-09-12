//! Tool execution belongs in the durable audit trail, including agent-local tool calls.
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
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", scope.thread_id)
            .fetch_one(&mut *tx)
            .await?;
        let old = sqlx::query_file!("../../queries/chat/tool_get.sql", call)
            .fetch_optional(&mut *tx)
            .await?;
        let kind = match old {
            None => {
                if tool.status != "running"
                    || tool.input_json.is_empty()
                    || !tool.output_json.is_empty()
                    || !tool.input_delta.is_empty()
                {
                    return Err(ChatError::Invalid("Tool call must begin with input".into()));
                }
                sqlx::query_file!(
                    "../../queries/chat/tool_insert.sql",
                    call,
                    scope.thread_id,
                    scope.id,
                    scope.participant_id,
                    tool.name,
                    tool.provider_id,
                    tool.input_json
                )
                .execute(&mut *tx)
                .await?;
                "tool.started"
            }
            Some(old) => {
                if old.invocation_id != scope.id
                    || old.thread_id != scope.thread_id
                    || old.participant_id != scope.participant_id
                    || old.name != tool.name
                    || old.provider_id != tool.provider_id
                    || (!tool.input_json.is_empty() && old.input_json != tool.input_json)
                {
                    return Err(ChatError::Conflict);
                }
                if old.status != "running" {
                    if old.status == tool.status
                        && old.output_json == tool.output_json
                        && old.error == tool.error
                    {
                        return Ok(());
                    }
                    return Err(ChatError::Conflict);
                }
                if tool.status == "running" {
                    if tool.input_delta.is_empty() {
                        return Err(ChatError::Conflict);
                    }
                    "tool.input.delta"
                } else {
                    if !["completed", "failed", "aborted"].contains(&tool.status.as_str()) {
                        return Err(ChatError::Invalid("Invalid tool status".into()));
                    }
                    sqlx::query_file!(
                        "../../queries/chat/tool_finish.sql",
                        call,
                        tool.status,
                        tool.output_json,
                        tool.error
                    )
                    .execute(&mut *tx)
                    .await?;
                    match tool.status.as_str() {
                        "completed" => "tool.completed",
                        "failed" => "tool.failed",
                        _ => "tool.aborted",
                    }
                }
            }
        };
        crate::chat::audit::append(
            &mut tx,
            scope.thread_id,
            types::Activity {
                kind: kind.into(),
                entity_id: call.to_string(),
                participant_id: scope.participant_id.to_string(),
                invocation_id: scope.id.to_string(),
                detail: Some(tool.into()),
                ..Default::default()
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub(crate) async fn expire_tool_calls(&self) -> Result<()> {
        for row in sqlx::query_file!("../../queries/chat/tool_interrupted.sql")
            .fetch_all(self.pg()?)
            .await?
        {
            self.report_tool_call(
                &Scope {
                    capabilities: Default::default(),
                    id: row.invocation_id,
                    run_id: Uuid::nil(),
                    thread_id: row.thread_id,
                    agent_id: Uuid::nil(),
                    participant_id: row.participant_id,
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
