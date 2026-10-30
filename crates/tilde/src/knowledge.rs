//! The memory and wiki tools of an agent beside a sidecar, served only behind the registry
//! relay (`tilde.runtime.v1.KnowledgeService`). Memory banks and wikis live in the gateway's
//! Postgres, so a sidecar relays its agent's listing and calls of these tools here; gateway-hosted
//! agents reach the same tools directly through the tool registry.
use crate::chat::tools::ToolResult;
use crate::memory::tools::MemoryTools;
use crate::proto::tilde::runtime::v1 as runtime;
use crate::services::tilde::runtime::v1::KnowledgeService;
use crate::wikis::tools::WikiTools;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use std::sync::Arc;
use uuid::Uuid;

pub fn runtime_router(tools: &crate::tools::Tools, chat: crate::chat::Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Runtime {
            memory: MemoryTools(tools.memory.clone()),
            wikis: WikiTools(tools.wikis.clone()),
            chat,
        })),
        1024 * 1024,
    )
}
struct Runtime {
    memory: MemoryTools,
    wikis: WikiTools,
    chat: crate::chat::Chat,
}
impl Runtime {
    async fn call(
        &self,
        scope: &crate::chat::Scope,
        call_id: Uuid,
        name: &str,
        input: serde_json::Value,
    ) -> ToolResult<serde_json::Value> {
        if name.starts_with("wiki.") {
            self.wikis.call(scope, call_id, name, input).await
        } else {
            self.memory.call(scope, call_id, name, input).await
        }
    }
}
impl KnowledgeService for Runtime {
    async fn list_knowledge_tools<'a>(
        &'a self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, runtime::ListKnowledgeToolsRequest>,
    ) -> ServiceResult<impl Encodable<runtime::ListKnowledgeToolsResponse> + Send + use<'a>> {
        let scope = crate::chat::rpc::invocation_scope(&self.chat, &ctx).await?;
        let mut tools = self.memory.list(scope.agent_id).await?;
        tools.extend(self.wikis.list(scope.agent_id).await?);
        Response::ok(runtime::ListKnowledgeToolsResponse {
            tools,
            ..Default::default()
        })
    }
    async fn invoke_knowledge_tool<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime::InvokeKnowledgeToolRequest>,
    ) -> ServiceResult<impl Encodable<runtime::InvokeKnowledgeToolResponse> + Send + use<'a>> {
        let scope = crate::chat::rpc::invocation_scope(&self.chat, &ctx).await?;
        let r = request.to_owned_message();
        // The sidecar's registry filtered the catalog by tools_invoke; check it again here.
        if !scope
            .capabilities
            .permits(crate::iam::capabilities::Capability::ToolsInvoke, &r.name)
        {
            return Err(ConnectError::permission_denied(
                "Tool is not available to this agent",
            ));
        }
        let input = serde_json::from_str(&r.input_json)
            .map_err(|_| ConnectError::invalid_argument("Invalid tool input"))?;
        let call_id = Uuid::parse_str(&r.call_id)
            .map_err(|_| ConnectError::invalid_argument("Invalid call id"))?;
        let output = self.call(&scope, call_id, &r.name, input).await?;
        Response::ok(runtime::InvokeKnowledgeToolResponse {
            output_json: output.to_string(),
            ..Default::default()
        })
    }
}
