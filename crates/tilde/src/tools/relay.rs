//! The tools of a sidecar-hosted agent that only the gateway can run (`Callee::on_sidecar`),
//! served only behind the registry relay (`tilde.runtime.v1.GatewayToolService`). The sidecar
//! relays its agent's listing and calls of these here; gateway-hosted agents reach the same tools
//! directly through the tool registry.
use crate::proto::tilde::runtime::v1 as runtime;
use crate::services::tilde::runtime::v1::GatewayToolService;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use std::sync::Arc;
use uuid::Uuid;

pub fn runtime_router(tools: &super::Tools, chat: crate::chat::Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Runtime {
            tools: tools.clone(),
            chat,
        })),
        1024 * 1024,
    )
}
struct Runtime {
    tools: super::Tools,
    chat: crate::chat::Chat,
}
impl GatewayToolService for Runtime {
    async fn list_gateway_tools<'a>(
        &'a self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, runtime::ListGatewayToolsRequest>,
    ) -> ServiceResult<impl Encodable<runtime::ListGatewayToolsResponse> + Send + use<'a>> {
        let scope = crate::chat::rpc::invocation_scope(&self.chat, &ctx).await?;
        let (tools, deferred) = self.tools.gateway_tools(&scope).await?;
        Response::ok(runtime::ListGatewayToolsResponse {
            tools,
            deferred,
            ..Default::default()
        })
    }
    async fn invoke_gateway_tool<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime::InvokeGatewayToolRequest>,
    ) -> ServiceResult<impl Encodable<runtime::InvokeGatewayToolResponse> + Send + use<'a>> {
        let scope = crate::chat::rpc::invocation_scope(&self.chat, &ctx).await?;
        let r = request.to_owned_message();
        let input = serde_json::from_str(&r.input_json)
            .map_err(|_| ConnectError::invalid_argument("Invalid tool input"))?;
        let call_id = Uuid::parse_str(&r.call_id)
            .map_err(|_| ConnectError::invalid_argument("Invalid call id"))?;
        let output = self
            .tools
            .call_gateway_tool(&scope, call_id, &r.name, input)
            .await?;
        Response::ok(runtime::InvokeGatewayToolResponse {
            output_json: output.to_string(),
            ..Default::default()
        })
    }
}
