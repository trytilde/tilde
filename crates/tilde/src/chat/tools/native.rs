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
            Ok(vec![types::ToolDefinition {
            name:"sendMessage".into(),provider_id:"native".into(),
            description:"Send a visible plain-text message to this native conversation. Ordinary reasoning and tool results are never sent. For incremental input, start with empty text and stream textDelta chunks.".into(),
            input_schema_json:json!({"type":"object","properties":{"text":{"type":"string"},"addressedParticipantIds":{"type":"array","items":{"type":"string","format":"uuid"}},"inReplyToMessageId":{"type":"string","format":"uuid"},"attachmentIds":{"type":"array","maxItems":20,"items":{"type":"string","format":"uuid"}}},"required":["text"],"additionalProperties":false}).to_string(),
            chunk_schema_json:json!({"type":"object","properties":{"textDelta":{"type":"string"}},"required":["textDelta"],"additionalProperties":false}).to_string(),..Default::default()
        }])
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
                    let chunk: Chunk = serde_json::from_value(chunk?).map_err(|_| {
                        ConnectError::invalid_argument("Invalid native message chunk")
                    })?;
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
            serde_json::to_value(&message)
                .map_err(|_| ConnectError::internal("Unable to encode message"))
        })
    }
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
        let mut tx = self.chat.pg()?.begin().await.map_err(ChatError::from)?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", self.scope.thread_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(ChatError::from)?;
        sqlx::query_file!(
            "../../queries/chat/message_create.sql",
            message_id,
            self.scope.thread_id,
            self.scope.participant_id,
            "",
            "streaming",
            Some(self.scope.id),
            reply.map(id).transpose()?,
            crate::telemetry::context::capture().0,
            crate::telemetry::context::capture().1
        )
        .execute(&mut *tx)
        .await
        .map_err(ChatError::from)?;
        self.chat
            .targets(&mut tx, self.scope.thread_id, message_id, recipients)
            .await?;
        activity(
            &mut tx,
            self.scope.thread_id,
            "message.started",
            message_id,
            "",
        )
        .await?;
        tx.commit().await.map_err(ChatError::from)?;
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
        let mut tx = self.chat.pg()?.begin().await.map_err(ChatError::from)?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", self.scope.thread_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(ChatError::from)?;
        for attachment in ids {
            sqlx::query_file!(
                "../../queries/chat/attachment_attach.sql",
                self.scope.thread_id,
                self.id,
                id(attachment)?
            )
            .execute(&mut *tx)
            .await
            .map_err(ChatError::from)?;
        }
        if !ids.is_empty() {
            activity(
                &mut tx,
                self.scope.thread_id,
                "message.attachments",
                self.id,
                "",
            )
            .await?;
        }
        tx.commit().await.map_err(ChatError::from)?;
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
        let mut tx = self.chat.pg()?.begin().await.map_err(ChatError::from)?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", self.scope.thread_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(ChatError::from)?;
        if sqlx::query_file!("../../queries/chat/message_chunk.sql", self.id, text)
            .execute(&mut *tx)
            .await
            .map_err(ChatError::from)?
            .rows_affected()
            != 1
        {
            return Err(ConnectError::failed_precondition(
                "Message stream is no longer active",
            ));
        }
        if !text.is_empty() {
            activity(
                &mut tx,
                self.scope.thread_id,
                "message.delta",
                self.id,
                text,
            )
            .await?;
        }
        tx.commit().await.map_err(ChatError::from)?;
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
        let mut tx = self.chat.pg()?.begin().await.map_err(ChatError::from)?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", self.scope.thread_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(ChatError::from)?;
        if sqlx::query_file!("../../queries/chat/message_finish.sql", self.id, "complete")
            .execute(&mut *tx)
            .await
            .map_err(ChatError::from)?
            .rows_affected()
            != 1
        {
            return Err(ConnectError::failed_precondition(
                "Message stream is no longer active",
            ));
        }
        activity(
            &mut tx,
            self.scope.thread_id,
            "message.completed",
            self.id,
            "",
        )
        .await?;
        tx.commit().await.map_err(ChatError::from)?;
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
