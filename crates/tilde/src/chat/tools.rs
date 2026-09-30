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
pub mod dynamic;
pub mod native;
use super::{Chat, Scope};

use connectrpc::{ConnectError, InboundStream};
use futures::{Stream, StreamExt, future::BoxFuture};
use opentelemetry::{
    KeyValue,
    trace::{FutureExt, SpanKind, TraceContextExt},
};
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
    /// Tools kept out of the listed catalog: found with `tools.search`, called through
    /// `tools.execute`, and otherwise subject to every rule a listed tool is.
    fn deferred<'a>(
        &'a self,
        _scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async { Ok(vec![]) })
    }
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
        // The gateway reads native eligibility and connection tools in one catalog query.
        if let Some(channels) = &chat.channels {
            let mut providers = vec![channels.clone().provider()];
            if let Some(tools) = &chat.tools {
                providers.push(Arc::new(tools.clone()));
                providers.push(Arc::new(crate::tools::builtin::Builtin(tools.clone())));
                providers.push(Arc::new(crate::tools::personal::Personal(tools.clone())));
            }
            return Self::new(providers);
        }
        let mut providers: Vec<Arc<dyn Provider>> =
            vec![Arc::new(native::Native { chat: chat.clone() })];
        if let Some(local) = chat.local() {
            providers.push(Arc::new(
                crate::deployment::runtime::providers::LocalChannels(local.clone()),
            ));
        }
        Self::new(providers)
    }
    async fn entries(
        &self,
        scope: &Scope,
    ) -> ToolResult<Vec<(types::ToolDefinition, Arc<dyn Provider>, bool)>> {
        let mut names = BTreeSet::new();
        let mut result = Vec::new();
        let mut listed = 0;
        for provider in &self.providers {
            let direct = provider.tools(scope).await?.into_iter().map(|d| (d, false));
            let deferred = provider
                .deferred(scope)
                .await?
                .into_iter()
                .map(|d| (d, true));
            for (definition, deferred) in direct.chain(deferred).collect::<Vec<_>>() {
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
                        dynamic::SEARCH,
                        dynamic::SCHEMAS,
                        dynamic::EXECUTE,
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
                if definition.detached && !definition.chunk_schema_json.is_empty() {
                    return Err(ConnectError::internal(
                        "Detached tools cannot take streamed input",
                    ));
                }
                if definition.input_schema_json.is_empty() {
                    return Err(ConnectError::internal("Tool input schema is required"));
                }
                if scope.capabilities.permits(
                    crate::iam::capabilities::Capability::ToolsInvoke,
                    &definition.name,
                ) {
                    listed += usize::from(!deferred);
                    result.push((definition, provider.clone(), deferred));
                }
                // Listed tools cost the model context on every turn; deferred ones do not.
                if listed > 256 || result.len() > 4096 {
                    return Err(ConnectError::resource_exhausted(
                        "Invocation tool catalog is too large",
                    ));
                }
            }
        }
        Ok(result)
    }
    pub async fn list(&self, scope: &Scope) -> ToolResult<Vec<types::ToolDefinition>> {
        let entries = self.entries(scope).await?;
        let dynamic = entries.iter().any(|(_, _, deferred)| *deferred);
        let mut listed: Vec<_> = entries
            .into_iter()
            .filter(|(_, _, deferred)| !deferred)
            .map(|(d, _, _)| d)
            .collect();
        if dynamic {
            listed.extend(dynamic::definitions().into_iter().filter(|d| {
                scope
                    .capabilities
                    .permits(crate::iam::capabilities::Capability::ToolsInvoke, &d.name)
            }));
        }
        Ok(listed)
    }
    /// Envelope validation is independent of the provider's content formats.
    pub async fn invoke(
        &self,
        chat: Chat,
        scope: Scope,
        capability: SecretString,
        mut requests: InboundStream<runtime_pb::InvokeToolRequest>,
    ) -> ToolResult<Value> {
        // The RPC entry point supplies its validated scope. Later effects and streamed
        // input still check token expiry after waiting on the caller, without database reads.
        let mut first = next(&mut requests)
            .await?
            .ok_or_else(|| ConnectError::invalid_argument("Empty tool input stream"))?;
        if first.sequence != 0 || !first.chunk_json.is_empty() || first.input_json.len() > 64 * 1024
        {
            return Err(ConnectError::invalid_argument("Invalid first tool frame"));
        }
        let call_id = super::id(&first.call_id)?;
        let entries = self.entries(&scope).await?;
        if ![dynamic::SEARCH, dynamic::SCHEMAS, dynamic::EXECUTE].contains(&first.name.as_str()) {
            return self
                .call(chat, scope, capability, requests, first, entries, call_id)
                .await;
        }
        if !entries.iter().any(|(_, _, deferred)| *deferred)
            || !scope.capabilities.permits(
                crate::iam::capabilities::Capability::ToolsInvoke,
                &first.name,
            )
            || !first.finish
        {
            return Err(ConnectError::not_found(
                "Tool is not available in this invocation",
            ));
        }
        // The dynamic built-in is a tool call of its own in the trace; `tools.execute`'s inner call
        // nests under it with the name, schema, summary, audit record and async behavior of a
        // listed tool.
        let cx = span(
            &first.name,
            &first.call_id,
            &scope,
            "tools",
            &first.input_json,
            false,
        );
        let _end = crate::telemetry::tracing::context::EndOnDrop(cx.clone());
        let result = async {
            let input: Value = serde_json::from_str(&first.input_json)
                .map_err(|_| ConnectError::invalid_argument("Invalid tool input JSON"))?;
            if first.name != dynamic::EXECUTE {
                return dynamic::answer(
                    &chat,
                    &scope,
                    call_id,
                    capability,
                    &first.name,
                    &input,
                    entries,
                )
                .await;
            }
            let inner = input["name"]
                .as_str()
                .ok_or_else(|| ConnectError::invalid_argument("A tool name is required"))?;
            if dynamic::bundled(&chat, &scope)
                .await?
                .iter()
                .any(|t| t.name == inner)
            {
                return Err(ConnectError::failed_precondition(
                    "This tool is bundled with the agent and runs in its process; call it there instead of through tools.execute",
                ));
            }
            first.name = inner.to_owned();
            first.input_json = input
                .get("input")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}))
                .to_string();
            self.call(chat, scope, capability, requests, first, entries, call_id)
                .await
        }
        .with_context(cx.clone())
        .await;
        finish(&cx, &result);
        result
    }
    /// One tool call in its own `execute_tool` span, child of the current context.
    #[allow(clippy::too_many_arguments)]
    async fn call(
        &self,
        chat: Chat,
        scope: Scope,
        capability: SecretString,
        requests: InboundStream<runtime_pb::InvokeToolRequest>,
        first: runtime_pb::InvokeToolRequest,
        entries: Vec<(types::ToolDefinition, Arc<dyn Provider>, bool)>,
        call_id: Uuid,
    ) -> ToolResult<Value> {
        let (definition, provider, _) = entries
            .into_iter()
            .find(|(d, _, _)| d.name == first.name)
            .ok_or_else(|| ConnectError::not_found("Tool is not available in this invocation"))?;
        if !first.finish && definition.chunk_schema_json.is_empty() {
            return Err(ConnectError::invalid_argument(
                "This tool does not support streamed input",
            ));
        }
        let input = serde_json::from_str(&first.input_json)
            .map_err(|_| ConnectError::invalid_argument("Invalid tool input JSON"))?;
        // Server-executed tools appear in the agent's trace beside its model calls. The span
        // follows the GenAI conventions, which the trace store's mapping classifies as a tool.
        let cx = span(
            &first.name,
            &first.call_id,
            &scope,
            &definition.provider_id,
            &first.input_json,
            definition.detached,
        );
        let context = Context {
            chat: chat.clone(),
            scope,
            call_id,
            capability: SecretString::from(capability.expose_secret()),
        };
        let _end = crate::telemetry::tracing::context::EndOnDrop(cx.clone());
        let result = self
            .execute(
                chat, capability, requests, first, definition, provider, context, input,
            )
            .with_context(cx.clone())
            .await;
        finish(&cx, &result);
        result
    }
    #[allow(clippy::too_many_arguments)]
    async fn execute(
        &self,
        chat: Chat,
        capability: SecretString,
        mut requests: InboundStream<runtime_pb::InvokeToolRequest>,
        first: runtime_pb::InvokeToolRequest,
        definition: types::ToolDefinition,
        provider: Arc<dyn Provider>,
        context: Context,
        input: Value,
    ) -> ToolResult<Value> {
        let consumed = Arc::new(AtomicBool::new(false));
        // Native complete input has only database effects: its call claim, message and
        // terminal result can commit together. External effects and streams retain the
        // durable running claim before invoking their provider.
        if first.finish
            && first.name == "sendMessage"
            && definition.provider_id == "native"
            && chat.local().is_none()
        {
            if next(&mut requests).await?.is_some() {
                let _execution = audit::Execution::begin(&context, &definition, &input).await?;
                return Err(ConnectError::invalid_argument(
                    "Unexpected input after finish",
                ));
            }
            return native::invoke_complete(&context, input).await;
        }
        let execution = audit::Execution::begin(&context, &definition, &input).await?;
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
        if definition.detached {
            // The durable call stays running while the provider works; the agent gets a ticket
            // now and is woken with the outcome. Detached tools take complete input only.
            let ticket = context.call_id;
            let scope = context.scope.clone();
            let name = first.name.clone();
            let chat = context.chat.clone();
            // The call's span ends with the ticket; the work gets a child span that ends when the
            // provider does, so the trace shows how long the background call really took.
            let background = crate::telemetry::tracing::context::start(
                format!("background {name}"),
                SpanKind::Internal,
                &opentelemetry::Context::current(),
                vec![
                    KeyValue::new("gen_ai.operation.name", "execute_tool"),
                    KeyValue::new("gen_ai.tool.name", name.clone()),
                    KeyValue::new("gen_ai.tool.call.id", ticket.to_string()),
                    KeyValue::new("tilde.tool.background", true),
                ],
            );
            tokio::spawn(async move {
                let _end = crate::telemetry::tracing::context::EndOnDrop(background.clone());
                let mut result = provider
                    .invoke(context, &name, input, stream)
                    .with_context(background.clone())
                    .await;
                let recorded = execution.finish(&mut result).await;
                finish(&background, &result);
                if recorded.is_ok()
                    && let Err(error) = chat
                        .deliver_detached_result(&scope, ticket, &name, &result)
                        .await
                {
                    // The result stays readable by ticket through tools.result.
                    tracing::warn!(%ticket, %error, "Background tool result was not delivered");
                }
            });
            return Ok(serde_json::json!({
                "ticket": ticket,
                "status": "processing",
                "message": "Running in the background. You will be told when it finishes; read the outcome with tools.result.",
            }));
        }
        let mut result = provider.invoke(context, &first.name, input, stream).await;
        if result.is_ok() && !consumed.load(Ordering::Acquire) {
            result = Err(ConnectError::failed_precondition(
                "Provider did not consume the complete tool input",
            ));
        }
        execution.finish(&mut result).await?;
        result
    }
}
/// Trace payloads are a view of the call; the durable record keeps the full input and output.
fn bounded(json: &str) -> String {
    let mut end = json.len().min(64 * 1024);
    while !json.is_char_boundary(end) {
        end -= 1;
    }
    json[..end].to_owned()
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

/// An `execute_tool` span for one call, child of the current context. `background` marks a call
/// that answers with a ticket and finishes later.
fn span(
    name: &str,
    call_id: &str,
    scope: &Scope,
    provider_id: &str,
    input_json: &str,
    background: bool,
) -> opentelemetry::Context {
    crate::telemetry::tracing::context::start(
        format!("execute_tool {name}"),
        SpanKind::Internal,
        &opentelemetry::Context::current(),
        vec![
            KeyValue::new("gen_ai.operation.name", "execute_tool"),
            KeyValue::new("gen_ai.tool.name", name.to_owned()),
            KeyValue::new("gen_ai.tool.call.id", call_id.to_owned()),
            KeyValue::new("tilde.tool.provider_id", provider_id.to_owned()),
            KeyValue::new("tilde.tool.background", background),
            KeyValue::new("tilde.invocation.id", scope.id.to_string()),
            KeyValue::new("tilde.agent.id", scope.agent_id.to_string()),
            KeyValue::new("tilde.run.id", scope.run_id.to_string()),
            KeyValue::new("tilde.thread.id", scope.thread_id.to_string()),
            KeyValue::new("tilde.observation.input", bounded(input_json)),
        ],
    )
}
/// Record a call's output, or its error as the span's status.
fn finish(cx: &opentelemetry::Context, result: &ToolResult<Value>) {
    match result {
        Ok(output) => cx.span().set_attribute(KeyValue::new(
            "tilde.observation.output",
            bounded(&output.to_string()),
        )),
        Err(error) => cx
            .span()
            .set_status(opentelemetry::trace::Status::error(format!(
                "{:?}",
                error.code
            ))),
    }
}
