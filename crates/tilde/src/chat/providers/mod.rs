//! Connection-backed tool catalogs. Each provider owns its schemas, formats and upstream calls.
//! The shared adapter only selects ready assignments, decrypts credentials and audits execution.
use crate::proto::tilde::types::v1 as types;
pub mod agentmail;
mod files;
pub mod github;
pub mod ingress;
pub mod linq;
pub mod slack;
pub mod whatsapp;
use super::{
    Scope,
    tools::{Context, InputStream, Provider, ToolResult},
};
use crate::connections::{
    model::{Values, value},
    service::Connections,
};
use connectrpc::ConnectError;
use futures::{StreamExt, future::BoxFuture};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct Channels {
    connections: Connections,
}
pub struct Access {
    pub connection_id: Uuid,
    pub values: Values,
    http: crate::connections::oauth::Http,
    endpoints: crate::connections::catalog::Endpoints,
}
impl Access {
    pub fn secret(&self, key: &str) -> ToolResult<&str> {
        value(&self.values, key).map_err(Into::into)
    }
    pub fn url(&self, key: &str, base: &str, segments: &[&str]) -> ToolResult<url::Url> {
        let mut url = url::Url::parse(
            self.endpoints
                .0
                .get(key)
                .map(String::as_str)
                .unwrap_or(base),
        )
        .map_err(|_| ConnectError::internal("Invalid provider endpoint"))?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| ConnectError::internal("Invalid provider path"))?;
            path.pop_if_empty();
            for s in segments {
                path.push(s);
            }
        }
        Ok(url)
    }
    pub fn post(&self, url: url::Url, key: &str) -> ToolResult<reqwest::RequestBuilder> {
        Ok(self.http.client.post(url).bearer_auth(self.secret(key)?))
    }
    pub async fn json(&self, request: reqwest::RequestBuilder) -> ToolResult<Value> {
        let response = self.http.json(request).await?;
        Ok(response.0.clone())
    }
}
/// Two operations; no universal message body or mandatory send method.
pub trait Adapter: Send + Sync {
    fn identity_types(&self) -> &'static [types::IdentityType];
    fn verification_recipient(
        &self,
        identity_type: types::IdentityType,
        value: &str,
    ) -> ToolResult<crate::chat::access::identity::Identity>;
    fn verification_supported(&self) -> bool {
        true
    }
    fn supports_verification_template(&self) -> bool {
        false
    }
    fn send_verification<'a>(
        &'a self,
        access: &'a Access,
        message: crate::chat::access::identity::VerificationMessage<'a>,
    ) -> BoxFuture<'a, ToolResult<()>>;

    fn tools(&self) -> Vec<types::ToolDefinition>;
    fn webhook<'a>(
        &'a self,
        access: &'a Access,
        headers: &'a http::HeaderMap,
        body: &'a [u8],
    ) -> BoxFuture<'a, ToolResult<ingress::Webhook>>;
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        context: &'a Context,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>>;
}
pub(crate) fn adapter(provider: &str, typ: &str) -> Option<&'static dyn Adapter> {
    match (provider, typ) {
        ("slack", "slack_app") => Some(&slack::Slack),
        ("github", "github_app") => Some(&github::Github),
        ("agentmail", "inbox") => Some(&agentmail::Agentmail),
        ("linq", "account") => Some(&linq::Linq),
        ("whatsapp", "meta") => Some(&whatsapp::Whatsapp { telnyx: false }),
        ("telnyx", "whatsapp") => Some(&whatsapp::Whatsapp { telnyx: true }),
        _ => None,
    }
}
impl Channels {
    pub(crate) async fn access(&self, connection: Uuid) -> ToolResult<Access> {
        Ok(Access {
            connection_id: connection,
            values: self.connections.resolve(connection).await?,
            http: self.connections.http.clone(),
            endpoints: self.connections.endpoints.clone(),
        })
    }
    pub fn new(connections: Connections) -> Self {
        Self { connections }
    }
    pub fn provider(self) -> Arc<dyn Provider> {
        Arc::new(self)
    }
}
impl Provider for Channels {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            let rows = sqlx::query_file!("../../queries/chat/agent_channels.sql", scope.agent_id)
                .fetch_all(&self.connections.pool)
                .await
                .map_err(super::ChatError::from)?;
            let binding =
                sqlx::query_file!("../../queries/chat/channel_binding.sql", scope.thread_id)
                    .fetch_optional(&self.connections.pool)
                    .await
                    .map_err(super::ChatError::from)?;
            let mut out = Vec::new();
            for c in rows {
                if let Some(provider) = adapter(&c.provider_id, &c.type_id) {
                    for mut tool in provider.tools() {
                        tool.name = format!("channel_{}.{}", c.id.simple(), tool.name);
                        tool.description = format!(
                            "{} [{}]: {}",
                            c.name,
                            c.account_label.as_deref().unwrap_or_default(),
                            tool.description
                        );
                        if let Some(b) = binding.as_ref().filter(|b| b.connection_id == c.id) {
                            tool.description.push_str(&format!(
                                " Current conversation reference: {}.",
                                b.external_id
                            ));
                        }
                        tool.provider_id = c.provider_id.clone();
                        out.push(tool);
                    }
                }
            }
            Ok(out)
        })
    }
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        mut input: Value,
        mut chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let (connection, name) = name
                .strip_prefix("channel_")
                .and_then(|n| n.split_once('.'))
                .ok_or_else(|| ConnectError::not_found("Unknown channel tool"))?;
            let connection = Uuid::parse_str(connection)
                .map_err(|_| ConnectError::not_found("Unknown channel"))?;
            // Stream text only where this provider advertises textDelta; other fields remain typed input.
            while let Some(chunk) = chunks.next().await {
                let chunk = chunk?;
                if chunk.as_object().is_none_or(|v| v.len() != 1) {
                    return Err(ConnectError::invalid_argument("Invalid message chunk"));
                }
                let delta = chunk
                    .get("textDelta")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ConnectError::invalid_argument("Expected textDelta"))?;
                let text = input
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| ConnectError::invalid_argument("Expected text input"))?;
                input["text"] = Value::String(text + delta);
            }
            context.authorize().await?;
            let row = self.connections.get(connection).await?;
            if row.status != "ready"
                || !row
                    .associated_agents
                    .iter()
                    .any(|a| a.id == context.scope().agent_id)
            {
                return Err(ConnectError::permission_denied(
                    "Connection is not ready and assigned to this agent",
                ));
            }
            let provider = adapter(&row.provider_id, &row.type_id)
                .ok_or_else(|| ConnectError::not_found("No installed chat adapter"))?;
            let definition = provider
                .tools()
                .into_iter()
                .find(|d| d.name == name)
                .ok_or_else(|| ConnectError::not_found("Unsupported provider tool"))?;
            let schema: Value = serde_json::from_str(&definition.input_schema_json)
                .map_err(|_| ConnectError::internal("Invalid provider schema"))?;
            let validator = jsonschema::validator_for(&schema)
                .map_err(|_| ConnectError::internal("Invalid provider schema"))?;
            if !validator.is_valid(&input) {
                return Err(ConnectError::invalid_argument(
                    "Input does not match this provider tool schema",
                ));
            }
            let access = Access {
                connection_id: connection,
                values: self.connections.resolve(connection).await?,
                http: self.connections.http.clone(),
                endpoints: self.connections.endpoints.clone(),
            };
            provider.invoke(&access, &context, name, input).await
        })
    }
}
/// Shared schema envelope; each provider declares its own arguments and supported actions.
pub fn tool(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    streaming: bool,
) -> types::ToolDefinition {
    types::ToolDefinition{name:name.into(),provider_id:"channel".into(),description:description.into(),input_schema_json:json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}).to_string(),chunk_schema_json:if streaming{json!({"type":"object","properties":{"textDelta":{"type":"string"}},"required":["textDelta"],"additionalProperties":false}).to_string()}else{String::new()},..Default::default()}
}
pub fn string() -> Value {
    json!({"type":"string","minLength":1,"maxLength":2000})
}
pub fn text() -> Value {
    json!({"type":"string","maxLength":65536})
}
pub fn required<'a>(input: &'a Value, key: &str) -> ToolResult<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ConnectError::invalid_argument("Required provider input missing"))
}
pub fn optional<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}
/// Publish canonical visible content only after provider acceptance; the returned result is not a message.
pub async fn sent(
    context: &Context,
    access: &Access,
    text: &str,
    format: &str,
    destination: &str,
    external: &str,
    subject: Option<&str>,
) -> ToolResult<Value> {
    // The upstream effect already happened. Record its acceptance even if the invocation was
    // canceled while the request was in flight; authorization was checked before dispatch.
    let scope = context.scope();
    let thread = ingress::stable(
        access.connection_id,
        "thread",
        &format!("{}:{}", scope.agent_id, destination),
    );
    let participant = ingress::stable(access.connection_id, "agent", &thread.to_string());
    let message = context.call_id;
    let mut tx = context
        .chat
        .pool
        .begin()
        .await
        .map_err(super::ChatError::from)?;
    sqlx::query_file!(
        "../../queries/connections/connection_lock.sql",
        access.connection_id.to_string()
    )
    .execute(&mut *tx)
    .await
    .map_err(super::ChatError::from)?;
    let created = sqlx::query_file!(
        "../../queries/chat/channel_thread_create.sql",
        thread,
        destination,
        scope.agent_id
    )
    .execute(&mut *tx)
    .await
    .map_err(super::ChatError::from)?
    .rows_affected()
        > 0;
    sqlx::query_file!("../../queries/chat/thread_lock.sql", thread)
        .fetch_one(&mut *tx)
        .await
        .map_err(super::ChatError::from)?;
    sqlx::query_file!(
        "../../queries/chat/channel_thread_bind.sql",
        thread,
        access.connection_id,
        destination,
        scope.agent_id
    )
    .execute(&mut *tx)
    .await
    .map_err(super::ChatError::from)?;
    if sqlx::query_file!(
        "../../queries/chat/participant_join.sql",
        participant,
        thread,
        None::<Uuid>,
        Some(scope.agent_id)
    )
    .execute(&mut *tx)
    .await
    .map_err(super::ChatError::from)?
    .rows_affected()
        > 0
    {
        super::activity(&mut tx, thread, "participant.joined", participant, "").await?;
    }
    if created {
        super::activity(&mut tx, thread, "thread.created", thread, "").await?;
    }
    sqlx::query_file!(
        "../../queries/chat/message_create.sql",
        message,
        thread,
        participant,
        text,
        "complete",
        Some(scope.id),
        None::<Uuid>,
        crate::telemetry::context::capture().0,
        crate::telemetry::context::capture().1
    )
    .execute(&mut *tx)
    .await
    .map_err(super::ChatError::from)?;
    sqlx::query_file!("../../queries/chat/message_format.sql", message, format)
        .execute(&mut *tx)
        .await
        .map_err(super::ChatError::from)?;
    sqlx::query_file!(
        "../../queries/chat/delivery_insert.sql",
        message,
        access.connection_id,
        destination,
        external
    )
    .execute(&mut *tx)
    .await
    .map_err(super::ChatError::from)?;
    sqlx::query_file!("../../queries/chat/message_subject.sql", message, subject)
        .execute(&mut *tx)
        .await
        .map_err(super::ChatError::from)?;
    super::activity(&mut tx, thread, "message.completed", message, "").await?;
    tx.commit().await.map_err(super::ChatError::from)?;
    Ok(json!({"messageId":message,"threadId":thread,"externalMessageId":external,"accepted":true}))
}
