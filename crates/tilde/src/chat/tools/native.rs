//! Native-thread sendMessage owns plain-text arguments and textDelta chunks. Other providers
//! supply their own definitions and handlers; they need not expose a sending tool at all.
use super::{Context, InputStream, Provider, ToolResult};
use crate::chat::{Chat, ChatError, Scope, activity, id};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::{StreamExt, future::BoxFuture};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
pub struct Native {
    pub chat: Chat,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    text: String,
    #[serde(default)]
    addressed_participant_ids: Vec<String>,
    in_reply_to_message_id: Option<String>,
    #[serde(default)]
    attachment_ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Chunk {
    text_delta: String,
}
impl Provider for Native {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            if self.chat.has_channel(scope.thread_id).await? {
                return Ok(vec![]);
            }
            Ok(vec![definition()])
        })
    }
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        input: Value,
        chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(invoke(context, name, input, chunks))
    }
}

/// Native eligibility is read alongside connection tools by the gateway catalog.
pub(crate) fn definition() -> types::ToolDefinition {
    types::ToolDefinition {
            name:"sendMessage".into(),provider_id:"native".into(),
            description:"Send a visible plain-text message to this native conversation. Ordinary reasoning and tool results are never sent. For incremental input, start with empty text and stream textDelta chunks.".into(),
            input_schema_json:json!({"type":"object","properties":{"text":{"type":"string"},"addressedParticipantIds":{"type":"array","items":{"type":"string","format":"uuid"}},"inReplyToMessageId":{"type":"string","format":"uuid"},"attachmentIds":{"type":"array","maxItems":20,"items":{"type":"string","format":"uuid"}}},"required":["text"],"additionalProperties":false}).to_string(),
            chunk_schema_json:json!({"type":"object","properties":{"textDelta":{"type":"string"}},"required":["textDelta"],"additionalProperties":false}).to_string(),..Default::default()
        }
}

pub(crate) async fn invoke(
    context: Context,
    name: &str,
    input: Value,
    mut chunks: InputStream,
) -> ToolResult<Value> {
    if name != "sendMessage" {
        return Err(ConnectError::not_found("Unknown native tool"));
    }
    let input: Input = serde_json::from_value(input)
        .map_err(|_| ConnectError::invalid_argument("Invalid native message input"))?;
    let mut events = context
        .begin_message(
            context.call_id,
            &input.addressed_participant_ids,
            input.in_reply_to_message_id.as_deref(),
        )
        .await?;
    let result: ToolResult<()> = async {
        events.attach(&input.attachment_ids).await?;
        events.append(&input.text).await?;
        while let Some(chunk) = chunks.next().await {
            let chunk: Chunk = serde_json::from_value(chunk?)
                .map_err(|_| ConnectError::invalid_argument("Invalid native message chunk"))?;
            events.append(&chunk.text_delta).await?;
        }
        Ok(())
    }
    .await;
    if let Err(error) = result {
        events.abort().await;
        return Err(error);
    }
    let message = events.finish().await?;
    serde_json::to_value(&message).map_err(|_| ConnectError::internal("Unable to encode message"))
}
/// The dispatcher calls this only after validating the complete input envelope.
/// Successful calls publish the canonical message and tool result in one transaction.
/// Failed attempts roll back all message writes before using the normal failure recorder.
pub(crate) async fn invoke_complete(context: &Context, input: Value) -> ToolResult<Value> {
    let mut result = async {
        context.authorize().await?;
        let message_input: Input = serde_json::from_value(input.clone())
            .map_err(|_| ConnectError::invalid_argument("Invalid native message input"))?;
        if message_input.text.len() > 1024 * 1024 {
            return Err(ConnectError::resource_exhausted("Message is too large"));
        }
        complete_message(context, &message_input, &input.to_string())
            .await
            .map_err(Into::into)
    }
    .await;
    if result.is_err() {
        // The unique claim also protects this fallback against a concurrent successful
        // call or an uncertain COMMIT: it cannot overwrite an existing terminal result.
        let execution = super::audit::Execution::begin(context, &definition(), &input).await?;
        execution.finish(&mut result).await?;
    }
    result
}
async fn complete_message(
    context: &Context,
    input: &Input,
    input_json: &str,
) -> crate::chat::Result<Value> {
    use crate::chat::audit::{self};
    if input.text.is_empty() && input.attachment_ids.is_empty() {
        return Err(ChatError::Invalid("Message needs content".into()));
    }
    if input.addressed_participant_ids.len() > 100 {
        return Err(ChatError::Invalid("Too many recipients".into()));
    }
    if input.attachment_ids.len() > 20 {
        return Err(ChatError::Invalid("At most 20 attachments".into()));
    }
    let mut recipients = input
        .addressed_participant_ids
        .iter()
        .map(|v| id(v))
        .collect::<Result<Vec<_>, _>>()?;
    let attachments = input
        .attachment_ids
        .iter()
        .map(|v| id(v))
        .collect::<Result<Vec<_>, _>>()?;
    let reply = input
        .in_reply_to_message_id
        .as_deref()
        .map(id)
        .transpose()?;
    let scope = context.scope();
    let mut tx_client = context.chat.pg()?.get().await?;
    let tx = tx_client.transaction().await?;
    let prepared = crate::chat::db::native_message_prepare_opt(&tx, scope.thread_id, &attachments)
        .await?
        .ok_or(ChatError::NotFound)?;
    // Reject missing/scoped-out/duplicate IDs now: a concurrently uploaded attachment
    // must not become linkable in the next snapshot without its metadata in the result.
    if prepared.attachments.0.len() != attachments.len() {
        return Err(ChatError::Invalid("Invalid scoped attachments".into()));
    }
    let at = prepared.at;
    recipients.sort_unstable();
    let message = types::Message {
        id: context.call_id.to_string(),
        thread_id: scope.thread_id.to_string(),
        participant_id: scope.participant_id.to_string(),
        text: input.text.clone(),
        status: "complete".into(),
        format: "text".into(),
        in_reply_to_message_id: reply.map(|v| v.to_string()),
        addressed_participant_ids: recipients.iter().map(ToString::to_string).collect(),
        attachments: prepared.attachments.0.into_iter().map(Into::into).collect(),
        // NOW() is constant throughout this transaction, including the INSERT default.
        created_at: audit::timestamp(at).into(),
        ..Default::default()
    };
    let output = serde_json::to_value(&message).map_err(|_| ChatError::Transport)?;
    let output_json = output.to_string();
    let event_count =
        4_i64 + i64::from(!attachments.is_empty()) + i64::from(!input.text.is_empty());
    let (traceparent, tracestate) = crate::telemetry::tracing::context::capture();
    let row = crate::chat::db::native_message_complete_opt(
        &tx,
        context.call_id,
        scope.thread_id,
        scope.participant_id,
        &(input.text),
        scope.id,
        reply,
        &recipients,
        &attachments,
        &(traceparent),
        &(tracestate),
        event_count,
        input_json,
        &(output_json),
    )
    .await?
    .ok_or(ChatError::Conflict)?;
    if row.target_count != recipients.len() as i64 {
        return Err(ChatError::NotFound);
    }
    let event = |kind: &str| types::Activity {
        kind: kind.into(),
        entity_id: message.id.clone(),
        participant_id: scope.participant_id.to_string(),
        invocation_id: scope.id.to_string(),
        ..Default::default()
    };
    let mut started = message.clone();
    started.status = "streaming".into();
    started.text.clear();
    started.attachments.clear();
    let tool = types::ToolCall {
        id: context.call_id.to_string(),
        name: "sendMessage".into(),
        provider_id: "native".into(),
        status: "running".into(),
        input_json: input_json.into(),
        ..Default::default()
    };
    let mut events = vec![
        types::Activity {
            detail: Some(tool.clone().into()),
            ..event("tool.started")
        },
        types::Activity {
            detail: Some(started.clone().into()),
            ..event("message.started")
        },
    ];
    if !attachments.is_empty() {
        started.attachments = message.attachments.clone();
        events.push(types::Activity {
            detail: Some(started.into()),
            ..event("message.attachments")
        });
    }
    if !input.text.is_empty() {
        events.push(types::Activity {
            text_delta: input.text.clone(),
            detail: Some(
                types::MessageChunk {
                    message_id: message.id.clone(),
                    text_delta: input.text.clone(),
                    ..Default::default()
                }
                .into(),
            ),
            ..event("message.delta")
        });
    }
    events.push(types::Activity {
        detail: Some(message.clone().into()),
        ..event("message.completed")
    });
    events.push(types::Activity {
        detail: Some(
            types::ToolCall {
                status: "completed".into(),
                input_json: String::new(),
                output_json,
                ..tool
            }
            .into(),
        ),
        ..event("tool.completed")
    });
    for (offset, mut event) in events.into_iter().enumerate() {
        audit::prepare(
            scope.thread_id,
            row.audit_sequence + offset as i64,
            at,
            &mut event,
        );
        audit::enqueue(
            context.chat.pg()?,
            scope.thread_id,
            row.transaction_id,
            at,
            event,
        )?;
    }
    tx.commit().await?;
    drop(tx_client);
    context.chat.warm.forget_history(scope.thread_id);
    Ok(output)
}

/// Scoped canonical message event publication for provider handlers. These primitives do not
/// send to an external service; the provider owns that behavior and when to publish each event.
pub struct MessageEvents {
    chat: Chat,
    scope: Scope,
    capability: SecretString,
    id: Uuid,
    total: usize,
    attachments: usize,
    armed: bool,
}
impl Context {
    pub async fn begin_message(
        &self,
        message_id: Uuid,
        recipients: &[String],
        reply: Option<&str>,
    ) -> ToolResult<MessageEvents> {
        self.authorize().await?;
        if let Some(local) = self.chat.local() {
            local
                .begin_message(&self.scope, message_id, recipients, reply)
                .await?;
            return Ok(MessageEvents {
                chat: self.chat.clone(),
                scope: self.scope.clone(),
                capability: SecretString::from(self.capability.expose_secret()),
                id: message_id,
                total: 0,
                attachments: 0,
                armed: true,
            });
        }
        let mut tx_client = self.chat.pg()?.get().await.map_err(ChatError::from)?;
        let tx = tx_client.transaction().await.map_err(ChatError::from)?;
        crate::chat::db::thread_lock_one(&tx, self.scope.thread_id)
            .await
            .map_err(ChatError::from)?;
        crate::chat::db::message_create_execute(
            &tx,
            message_id,
            self.scope.thread_id,
            self.scope.participant_id,
            "",
            "streaming",
            Some(self.scope.id),
            reply.map(id).transpose()?,
            &(crate::telemetry::tracing::context::capture().0),
            &(crate::telemetry::tracing::context::capture().1),
        )
        .await
        .map_err(ChatError::from)?;
        self.chat
            .targets(&tx, self.scope.thread_id, message_id, recipients)
            .await?;
        activity(
            self.chat.pg()?,
            &tx,
            self.scope.thread_id,
            "message.started",
            message_id,
            "",
        )
        .await?;
        tx.commit().await.map_err(ChatError::from)?;
        drop(tx_client);
        self.chat.warm.forget_history(self.scope.thread_id);
        Ok(MessageEvents {
            chat: self.chat.clone(),
            scope: self.scope.clone(),
            capability: SecretString::from(self.capability.expose_secret()),
            id: message_id,
            total: 0,
            attachments: 0,
            armed: true,
        })
    }
}
impl MessageEvents {
    pub async fn attach(&mut self, ids: &[String]) -> ToolResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        if ids.len() > 20 {
            return Err(ConnectError::invalid_argument("At most 20 attachments"));
        }
        self.chat.scope(self.capability.expose_secret()).await?;
        if let Some(local) = self.chat.local() {
            local
                .attach_message(self.scope.thread_id, self.id, ids)
                .await?;
            self.attachments += ids.len();
            return Ok(());
        }
        let mut tx_client = self.chat.pg()?.get().await.map_err(ChatError::from)?;
        let tx = tx_client.transaction().await.map_err(ChatError::from)?;
        crate::chat::db::thread_lock_one(&tx, self.scope.thread_id)
            .await
            .map_err(ChatError::from)?;
        for attachment in ids {
            crate::chat::db::attachment_attach_execute(
                &tx,
                self.scope.thread_id,
                self.id,
                id(attachment)?,
            )
            .await
            .map_err(ChatError::from)?;
        }
        if !ids.is_empty() {
            activity(
                self.chat.pg()?,
                &tx,
                self.scope.thread_id,
                "message.attachments",
                self.id,
                "",
            )
            .await?;
        }
        tx.commit().await.map_err(ChatError::from)?;
        drop(tx_client);
        self.chat.warm.forget_history(self.scope.thread_id);
        self.attachments += ids.len();
        Ok(())
    }

    pub async fn append(&mut self, text: &str) -> ToolResult<()> {
        self.chat.scope(self.capability.expose_secret()).await?;
        self.total += text.len();
        if self.total > 1024 * 1024 {
            return Err(ConnectError::resource_exhausted("Message is too large"));
        }
        if let Some(local) = self.chat.local() {
            return Ok(local
                .append_message(self.scope.thread_id, self.id, text)
                .await?);
        }
        let mut tx_client = self.chat.pg()?.get().await.map_err(ChatError::from)?;
        let tx = tx_client.transaction().await.map_err(ChatError::from)?;
        crate::chat::db::thread_lock_one(&tx, self.scope.thread_id)
            .await
            .map_err(ChatError::from)?;
        if crate::chat::db::message_chunk_execute(&tx, self.id, text)
            .await
            .map_err(ChatError::from)?
            != 1
        {
            return Err(ConnectError::failed_precondition(
                "Message stream is no longer active",
            ));
        }
        if !text.is_empty() {
            activity(
                self.chat.pg()?,
                &tx,
                self.scope.thread_id,
                "message.delta",
                self.id,
                text,
            )
            .await?;
        }
        tx.commit().await.map_err(ChatError::from)?;
        drop(tx_client);
        self.chat.warm.forget_history(self.scope.thread_id);
        Ok(())
    }
    pub async fn finish(mut self) -> ToolResult<types::Message> {
        if self.total == 0 && self.attachments == 0 {
            return Err(ConnectError::invalid_argument("Message needs content"));
        }
        self.chat.scope(self.capability.expose_secret()).await?;
        if let Some(local) = self.chat.local() {
            let result = local
                .finish_message(self.scope.thread_id, self.id, "complete")
                .await?;
            self.armed = false;
            return Ok(result);
        }
        let mut tx_client = self.chat.pg()?.get().await.map_err(ChatError::from)?;
        let tx = tx_client.transaction().await.map_err(ChatError::from)?;
        crate::chat::db::thread_lock_one(&tx, self.scope.thread_id)
            .await
            .map_err(ChatError::from)?;
        if crate::chat::db::message_finish_execute(&tx, self.id, "complete")
            .await
            .map_err(ChatError::from)?
            != 1
        {
            return Err(ConnectError::failed_precondition(
                "Message stream is no longer active",
            ));
        }
        activity(
            self.chat.pg()?,
            &tx,
            self.scope.thread_id,
            "message.completed",
            self.id,
            "",
        )
        .await?;
        tx.commit().await.map_err(ChatError::from)?;
        drop(tx_client);
        self.chat.warm.forget_history(self.scope.thread_id);
        self.armed = false;
        Ok(self.chat.message(self.id).await?)
    }
    pub async fn abort(mut self) {
        let _ = self.chat.abort_message(self.id, self.scope.thread_id).await;
        self.armed = false;
    }
}
impl Drop for MessageEvents {
    fn drop(&mut self) {
        if self.armed {
            let chat = self.chat.clone();
            let id = self.id;
            let thread = self.scope.thread_id;
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = chat.abort_message(id, thread).await;
                });
            }
        }
    }
}
