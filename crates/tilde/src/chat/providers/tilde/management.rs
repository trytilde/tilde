//! Credentials only; access policy and identities use the generic AgentAccessService.
use crate::{
    chat::{Chat, id},
    proto::tilde::management::v1 as wire,
    services::tilde::management::v1::TildeChatProviderService,
};
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::ExposeSecret;
use std::sync::Arc;
struct Rpc(Chat);
pub(crate) fn router(chat: Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(chat))),
        1024 * 1024,
    )
}
impl TildeChatProviderService for Rpc {
    async fn get_credentials<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::GetCredentialsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetCredentialsResponse> + Send + use<'a>>
    {
        let agent = id(r.agent_id)?;
        let key = self.0.tilde_chat_key(agent, false).await?;
        Ok(Response::new(wire::GetCredentialsResponse {
            api_key: key.expose_secret().into(),
            ..Default::default()
        })
        .with_header("cache-control", "no-store"))
    }
    async fn rotate_credentials<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::RotateCredentialsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::RotateCredentialsResponse> + Send + use<'a>>
    {
        let agent = id(r.agent_id)?;
        let key = self.0.tilde_chat_key(agent, true).await?;
        Ok(Response::new(wire::RotateCredentialsResponse {
            api_key: key.expose_secret().into(),
            ..Default::default()
        })
        .with_header("cache-control", "no-store"))
    }
}
