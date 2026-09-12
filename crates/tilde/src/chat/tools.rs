//! Provider-defined tools over invocation-scoped ConnectRPC. Sending is an ordinary tool,
//! not a mandatory provider trait method or a universal message format. Providers own their
//! descriptions, argument/chunk schemas, validation, upstream calls, and explicit event publication.
//! Tool results and reasoning are never implicitly converted to conversation messages.
//!
//! `sendMessage` is a convention: conversational providers should expose it, while providers
//! without sending capabilities need no dummy handler. Each provider defines its supported
//! formats, addressing, replies and constraints in its catalog. Preserve provider formatting
//! instead of coercing every payload into native plain text.
//! All calls use the streaming transport, even when input arrives in one frame. Frameworks
//! must explicitly feed incremental chunks to stream model output. Providers may buffer
//! before external delivery; completed input does not imply successful delivery. Interrupted
//! input must not publish a completed message, and delivery failures must not report success.
use crate::proto::tilde::runtime::v1 as runtime_pb;
use crate::proto::tilde::types::v1 as types;
pub mod audit;
pub mod native;
use super::{Chat, Scope};

use connectrpc::{ConnectError, InboundStream};
use futures::{Stream, StreamExt, future::BoxFuture};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;
pub type ToolResult<T> = Result<T, ConnectError>;
pub type InputStream = Pin<Box<dyn Stream<Item = ToolResult<Value>> + Send>>;

/// Constructed from the invocation capability, not model-supplied thread/agent identifiers.
pub struct Context {
    pub chat: Chat,
    scope: Scope,
    pub call_id: Uuid,
    capability: SecretString,
}
impl Context {
    /// Read-only invocation scope for provider-specific authorization and rendering.
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub async fn authorize(&self) -> ToolResult<()> {
        self.chat.scope(self.capability.expose_secret()).await?;
        Ok(())
    }
}
/// Each provider exposes only tools valid in this invocation. Names must not overlap.
/// Handlers validate their own schemas and consume streamed input before reporting success.
/// Providers should use call_id for upstream idempotency; a retry must not duplicate effects.
pub trait Provider: Send + Sync {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>>;
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        input: Value,
        chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>>;
}
pub struct Registry {
    providers: Vec<Arc<dyn Provider>>,
}
impl Registry {
    pub fn new(providers: Vec<Arc<dyn Provider>>) -> Self {
        Self { providers }
    }
    pub fn for_chat(chat: &Chat) -> Self {
        let mut providers: Vec<Arc<dyn Provider>> =
            vec![Arc::new(native::Native { chat: chat.clone() })];
        if let Some(local) = chat.local() {
            providers.push(Arc::new(
                crate::deployment::runtime::providers::LocalChannels(local.clone()),
            ));
        }
        if let Some(channels) = &chat.channels {
            providers.push(channels.clone().provider());
        }
        Self::new(providers)
    }
    async fn entries(
        &self,
        scope: &Scope,
    ) -> ToolResult<Vec<(types::ToolDefinition, Arc<dyn Provider>)>> {
        let mut names = BTreeSet::new();
        let mut result = Vec::new();
        for provider in &self.providers {
            for definition in provider.tools(scope).await? {
                if definition.name.is_empty()
                    || definition.name.len() > 128
                    || !definition
                        .name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    || ["__proto__", "prototype", "constructor"].contains(&definition.name.as_str())
                    || definition.provider_id.is_empty()
                    || definition.description.is_empty()
                    || !names.insert(definition.name.clone())
                    || [
                        "stop",
                        "goals.list",
                        "goals.create",
                        "goals.update",
                        "tasks.list",
                        "tasks.create",
                        "tasks.update",
                    ]
                    .contains(&definition.name.as_str())
                {
                    return Err(ConnectError::internal(
                        "Invalid or conflicting provider tool catalog",
                    ));
                }
                for schema in [&definition.input_schema_json, &definition.chunk_schema_json] {
                    if schema.len() > 64 * 1024
                        || (!schema.is_empty()
                            && !serde_json::from_str::<Value>(schema).is_ok_and(|v| v.is_object()))
                    {
                        return Err(ConnectError::internal("Invalid provider JSON Schema"));
                    }
                }
                if definition.input_schema_json.is_empty() {
                    return Err(ConnectError::internal("Tool input schema is required"));
                }
                if scope.capabilities.permits(
                    crate::iam::capabilities::Capability::ToolsInvoke,
                    &definition.name,
                ) {
                    result.push((definition, provider.clone()));
                }
                if result.len() > 256 {
                    return Err(ConnectError::resource_exhausted(
                        "Invocation tool catalog is too large",
                    ));
                }
            }
        }
        Ok(result)
    }
    pub async fn list(&self, scope: &Scope) -> ToolResult<Vec<types::ToolDefinition>> {
        Ok(self
            .entries(scope)
            .await?
            .into_iter()
            .map(|(d, _)| d)
            .collect())
    }
    /// Envelope validation is independent of the provider's content formats.
    pub async fn invoke(
        &self,
        chat: Chat,
        capability: SecretString,
        mut requests: InboundStream<runtime_pb::InvokeToolRequest>,
    ) -> ToolResult<Value> {
        let scope = chat.scope(capability.expose_secret()).await?;
        let first = next(&mut requests)
            .await?
            .ok_or_else(|| ConnectError::invalid_argument("Empty tool input stream"))?;
        if first.sequence != 0 || !first.chunk_json.is_empty() || first.input_json.len() > 64 * 1024
        {
            return Err(ConnectError::invalid_argument("Invalid first tool frame"));
        }
        let call_id = super::id(&first.call_id)?;
        let (definition, provider) = self
            .entries(&scope)
            .await?
            .into_iter()
            .find(|(d, _)| d.name == first.name)
            .ok_or_else(|| ConnectError::not_found("Tool is not available in this invocation"))?;
        if !first.finish && definition.chunk_schema_json.is_empty() {
            return Err(ConnectError::invalid_argument(
                "This tool does not support streamed input",
            ));
        }
        let input = serde_json::from_str(&first.input_json)
            .map_err(|_| ConnectError::invalid_argument("Invalid tool input JSON"))?;
        let consumed = Arc::new(AtomicBool::new(false));
        let context = Context {
            chat: chat.clone(),
            scope,
            call_id,
            capability: SecretString::from(capability.expose_secret()),
        };
        let execution =
            audit::Execution::begin(&context, &first.name, &definition.provider_id, &input).await?;
        let audit_chat = chat.clone();
        let audit_scope = context.scope().clone();
        let audit_provider = definition.provider_id.clone();
        let stream: InputStream = if first.finish {
            if next(&mut requests).await?.is_some() {
                return Err(ConnectError::invalid_argument(
                    "Unexpected input after finish",
                ));
            }
            consumed.store(true, Ordering::Release);
            Box::pin(futures::stream::empty())
        } else {
            let done = consumed.clone();
            let name = first.name.clone();
            let id = first.call_id.clone();
            Box::pin(async_stream::try_stream! {
                let mut sequence=1;let mut finished=false;let mut total=first.input_json.len();
                while let Some(frame)=next(&mut requests).await? {
                    chat.scope(capability.expose_secret()).await?;
                    if finished || frame.name!=name || frame.call_id!=id || frame.sequence!=sequence || !frame.input_json.is_empty() {Err(ConnectError::invalid_argument("Invalid or out-of-order tool stream frame"))?;}
                    total+=frame.chunk_json.len();
                    if total>1024*1024 || sequence>16384 {Err(ConnectError::resource_exhausted("Tool input stream is too large"))?;}
                    finished=frame.finish;sequence+=1;
                    if !frame.chunk_json.is_empty(){
                        audit_chat.report_tool_call(&audit_scope,types::ToolCall{id:id.clone(),name:name.clone(),provider_id:audit_provider.clone(),status:"running".into(),input_delta:frame.chunk_json.clone(),..Default::default()}).await?;
                        yield serde_json::from_str(&frame.chunk_json).map_err(|_|ConnectError::invalid_argument("Invalid tool chunk JSON"))?;}
                    else if !finished {Err(ConnectError::invalid_argument("Empty tool chunk"))?;}
                }
                if !finished {Err(ConnectError::invalid_argument("Tool input requires an explicit finish frame"))?;}
                done.store(true,Ordering::Release);
            })
        };
        context.authorize().await?;
        let mut result = provider.invoke(context, &first.name, input, stream).await;
        if result.is_ok() && !consumed.load(Ordering::Acquire) {
            result = Err(ConnectError::failed_precondition(
                "Provider did not consume the complete tool input",
            ));
        }
        execution.finish(&result).await?;
        result
    }
}
async fn next(
    requests: &mut InboundStream<runtime_pb::InvokeToolRequest>,
) -> ToolResult<Option<runtime_pb::InvokeToolRequest>> {
    tokio::time::timeout(std::time::Duration::from_secs(30), requests.next())
        .await
        .map_err(|_| ConnectError::deadline_exceeded("Tool input stream idle timeout"))?
        .transpose()
        .map(|frame| frame.map(|f| f.to_owned_message()))
}
