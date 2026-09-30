//! Built-in tools every gateway invocation may receive. Each appears only when the invocation's
//! capabilities and state make it usable, and `tools_invoke` filters them like any other name.
use super::{Tools, db};
use crate::chat::{
    Chat, Scope,
    tools::{Context, InputStream, Provider, ToolResult},
};
use crate::error::Error;
use crate::iam::capabilities::Capability;
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::{StreamExt, future::BoxFuture};
use serde_json::{Value, json};
use uuid::Uuid;

pub struct Builtin(pub Tools);

pub(super) fn definition(
    name: &str,
    summary: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
) -> types::ToolDefinition {
    types::ToolDefinition {
        name: name.into(),
        provider_id: "tilde".into(),
        description: description.into(),
        summary: summary.into(),
        input_schema_json: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}).to_string(),
        annotations: types::ToolAnnotations { read_only, idempotent: read_only, ..Default::default() }.into(),
        ..Default::default()
    }
}
impl Builtin {
    async fn result(&self, scope: &Scope, input: &Value) -> ToolResult<Value> {
        let ticket = input["ticket"]
            .as_str()
            .and_then(|t| Uuid::parse_str(t).ok())
            .ok_or_else(|| ConnectError::invalid_argument("Invalid ticket"))?;
        let row = db::tool_result_opt(
            &self.0.connections.pool.get().await.map_err(Error::from)?,
            ticket,
            scope.thread_id,
            scope.agent_id,
        )
        .await
        .map_err(Error::from)?
        .ok_or_else(|| {
            ConnectError::not_found("No background call with this ticket in this conversation")
        })?;
        Ok(match row.status.as_str() {
            "running" => json!({"tool":row.name,"status":"processing"}),
            "completed" => {
                json!({"tool":row.name,"status":"completed","output":serde_json::from_str::<Value>(&row.output_json).unwrap_or(Value::Null)})
            }
            status => json!({"tool":row.name,"status":status,"error":row.error}),
        })
    }
}
impl Builtin {
    async fn agents(&self, scope: &Scope) -> ToolResult<Value> {
        let rows =
            db::agents_visible_all(&self.0.connections.pool.get().await.map_err(Error::from)?)
                .await
                .map_err(Error::from)?;
        Ok(json!({"agents": rows.into_iter()
            .filter(|a| a.id != scope.agent_id && scope.capabilities.permits(Capability::AgentsRead, &a.id.to_string()))
            .map(|a| json!({"agent_id": a.id, "name": a.name}))
            .collect::<Vec<_>>()}))
    }
    async fn message(&self, context: &Context, input: &Value) -> ToolResult<Value> {
        let scope = context.scope();
        let agent = input["agent_id"].as_str().unwrap_or_default();
        scope
            .capabilities
            .require(Capability::AgentsInvoke, agent)?;
        if crate::chat::id(agent)? == scope.agent_id {
            return Err(ConnectError::invalid_argument(
                "An agent cannot message itself",
            ));
        }
        let thread = context.chat.thread(scope.thread_id).await?;
        if !thread
            .participants
            .iter()
            .any(|p| p.active && p.agent_id.as_deref() == Some(agent))
        {
            context
                .chat
                .add_participant(crate::chat::AddParticipant {
                    thread_id: scope.thread_id.to_string(),
                    participant: Some(types::ParticipantRef {
                        agent_id: Some(agent.into()),
                        ..Default::default()
                    }),
                })
                .await?;
        }
        // The call ID keys the run, so a retried call starts nothing twice.
        let run = context
            .chat
            .start_run(crate::chat::StartRun {
                thread_id: scope.thread_id.to_string(),
                agent_id: agent.into(),
                objective: input["message"].as_str().unwrap_or_default().into(),
                goal_id: None,
                idempotency_key: context.call_id.to_string(),
            })
            .await?;
        Ok(json!({"run_id": run.id, "status": run.status}))
    }
    async fn wait(&self, context: &Context, input: &Value) -> ToolResult<Value> {
        let scope = context.scope();
        let run = crate::chat::id(input["run_id"].as_str().unwrap_or_default())?;
        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_secs(input["timeout_seconds"].as_u64().unwrap_or(60));
        loop {
            let state = context.chat.run(run).await?;
            if state.thread_id != scope.thread_id.to_string() {
                return Err(ConnectError::not_found("No such run in this conversation"));
            }
            scope
                .capabilities
                .require(Capability::AgentsInvoke, state.agent_id.as_str())?;
            let finished = state.status != "active" && state.status != "suspending";
            if finished || tokio::time::Instant::now() >= deadline {
                let messages = db::run_messages_all(
                    &self.0.connections.pool.get().await.map_err(Error::from)?,
                    run,
                    scope.thread_id,
                )
                .await
                .map_err(Error::from)?;
                return Ok(json!({"status": state.status, "finished": finished,
                    "messages": messages.into_iter().map(|m| json!({"message_id": m.id, "text": m.text})).collect::<Vec<_>>()}));
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            context.authorize().await?;
        }
    }
    async fn search(&self, context: &Context, input: &Value) -> ToolResult<Value> {
        let scope = context.scope();
        scope.capabilities.require(Capability::ThreadRead, "")?;
        let ids = crate::chat::providers::tilde::db::search_messages(
            &self.0.connections.pool.get().await.map_err(Error::from)?,
            scope.thread_id,
            input["query"].as_str().unwrap_or_default(),
            None,
            input["limit"].as_i64().unwrap_or(10),
        )
        .await
        .map_err(Error::from)?;
        let mut messages = Vec::new();
        for id in ids {
            let m = context.chat.message(id).await?;
            messages.push(
                json!({"message_id": m.id, "participant_id": m.participant_id, "text": m.text}),
            );
        }
        Ok(json!({"messages": messages}))
    }
}
impl Provider for Builtin {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            let mut out = Vec::new();
            // Only useful to an agent that can start background calls.
            if self
                .0
                .catalog(scope)
                .await?
                .iter()
                .any(|(d, _, _)| d.detached)
            {
                out.push(definition(
                    "tools.result",
                    "Checked a background task",
                    "Read the status and outcome of a background tool call by its ticket.",
                    json!({"ticket":{"type":"string","format":"uuid"}}),
                    &["ticket"],
                    true,
                ));
            }
            // Agent and conversation tools follow the capabilities the agent already holds.
            let can = |capability| scope.capabilities.grants(capability);
            if can(Capability::AgentsRead) {
                out.push(definition(
                    "agents.list",
                    "Looked up agents",
                    "List the agents you may see, with the IDs used to message them.",
                    json!({}),
                    &[],
                    true,
                ));
            }
            if can(Capability::AgentsInvoke) {
                out.push(definition(
                    "agents.message",
                    "Messaged an agent",
                    "Ask another agent to do something in this conversation. It joins if needed and works in the background; use agents.wait for its reply.",
                    json!({"agent_id":{"type":"string","format":"uuid"},"message":{"type":"string","minLength":1,"maxLength":16000}}),
                    &["agent_id", "message"],
                    false,
                ));
                out.push(definition(
                    "agents.wait",
                    "Waited for an agent",
                    "Wait for a run started with agents.message to stop, and read what that agent said.",
                    json!({"run_id":{"type":"string","format":"uuid"},"timeout_seconds":{"type":"integer","minimum":1,"maximum":120}}),
                    &["run_id"],
                    true,
                ));
            }
            if can(Capability::ThreadRead) {
                out.push(definition(
                    "thread.participants",
                    "Checked who is here",
                    "List the people and agents in this conversation.",
                    json!({}),
                    &[],
                    true,
                ));
                out.push(definition(
                    "thread.search",
                    "Searched the conversation",
                    "Find earlier messages in this conversation by text.",
                    json!({"query":{"type":"string","minLength":1,"maxLength":200},"limit":{"type":"integer","minimum":1,"maximum":20}}),
                    &["query"],
                    true,
                ));
            }
            Ok(out)
        })
    }
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        input: Value,
        mut chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            while let Some(chunk) = chunks.next().await {
                chunk?;
            }
            context.authorize().await?;
            // Listing and calling share one gate: a tool absent from the list cannot be called.
            let definition = self
                .tools(context.scope())
                .await?
                .into_iter()
                .find(|d| d.name == name)
                .ok_or_else(|| ConnectError::not_found("Tool is not available to this agent"))?;
            let schema: Value = serde_json::from_str(&definition.input_schema_json)
                .map_err(|_| ConnectError::internal("Invalid built-in schema"))?;
            if !jsonschema::validator_for(&schema)
                .map_err(|_| ConnectError::internal("Invalid built-in schema"))?
                .is_valid(&input)
            {
                return Err(ConnectError::invalid_argument(
                    "Input does not match this tool's schema",
                ));
            }
            match name {
                "tools.result" => self.result(context.scope(), &input).await,
                "agents.list" => self.agents(context.scope()).await,
                "agents.message" => self.message(&context, &input).await,
                "agents.wait" => self.wait(&context, &input).await,
                "thread.participants" => {
                    context
                        .scope()
                        .capabilities
                        .require(Capability::ThreadRead, "")?;
                    let thread = context.chat.thread(context.scope().thread_id).await?;
                    Ok(
                        json!({"participants": thread.participants.iter().filter(|p| p.active).map(|p| json!({
                        "participant_id": p.id, "name": p.name, "agent_id": p.agent_id, "user_id": p.user_id,
                    })).collect::<Vec<_>>()}),
                    )
                }
                "thread.search" => self.search(&context, &input).await,
                _ => Err(ConnectError::not_found("Unknown built-in tool")),
            }
        })
    }
}

impl Chat {
    /// Tell the agent a background call finished. The result is queued on the agent's live
    /// invocation in the thread, never steered into it: a steer applies the agent's concurrency
    /// policy, and `interrupt` would cancel the work that started the call. A queued input is
    /// read by that invocation or starts a fresh one when it ends, so it is never lost. With no
    /// live invocation it starts a run. The call ID keys both, so repeated delivery changes nothing.
    pub(crate) async fn deliver_detached_result(
        &self,
        scope: &Scope,
        ticket: Uuid,
        name: &str,
        result: &ToolResult<Value>,
    ) -> crate::chat::Result<()> {
        let mut text = match result {
            Ok(output) => {
                format!("Background tool call {name} (ticket {ticket}) completed. Output: {output}")
            }
            Err(error) => format!(
                "Background tool call {name} (ticket {ticket}) failed: {}",
                error.message.as_deref().unwrap_or("unknown error")
            ),
        };
        // Inputs are model context; tools.result serves the full output.
        if text.len() > 16 * 1024 {
            let mut end = 16 * 1024;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            text.push_str("… (truncated; read the full output with tools.result)");
        }
        // An invocation may end, or a run start, between the check and the delivery: try again.
        for _ in 0..3 {
            let active = crate::chat::db::invocation_active_opt(
                &self.pg()?.get().await?,
                scope.thread_id,
                scope.agent_id,
            )
            .await?;
            let delivered = match active {
                Some(active) => self.queue_input(active.id, ticket, &text).await,
                None => self
                    .start_run(crate::chat::StartRun {
                        thread_id: scope.thread_id.to_string(),
                        agent_id: scope.agent_id.to_string(),
                        objective: text.clone(),
                        goal_id: None,
                        idempotency_key: ticket.to_string(),
                    })
                    .await
                    .map(drop),
            };
            match delivered {
                Err(crate::chat::ChatError::Conflict) => continue,
                other => return other,
            }
        }
        Err(crate::chat::ChatError::Conflict)
    }
}
