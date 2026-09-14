use super::{AgentAccess, VerificationRequest};
use crate::proto::tilde::{management::v1 as management, setup::v1 as setup, types::v1 as types};
use crate::services::tilde::{
    management::v1::AgentAccessService, setup::v1::IdentityVerificationService,
};
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use std::sync::Arc;
use uuid::Uuid;
struct Rpc {
    service: AgentAccess,
}
pub fn management_router(service: AgentAccess) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Management(Rpc { service }))),
        32 * 1024,
    )
}
pub fn public_router(service: AgentAccess) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Public(Rpc { service }))),
        32 * 1024,
    )
}
struct Management(Rpc);
struct Public(Rpc);
fn id(value: &str) -> Result<Uuid, ConnectError> {
    Uuid::parse_str(value).map_err(|_| ConnectError::invalid_argument("Invalid UUID"))
}
fn cursor(value: &str) -> Result<Option<Uuid>, ConnectError> {
    if value.is_empty() {
        Ok(None)
    } else {
        id(value).map(Some)
    }
}
fn size(value: u32) -> u32 {
    if value == 0 { 50 } else { value.min(100) }
}
impl AgentAccessService for Management {
    async fn list_channel_access<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ListChannelAccessRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::ListChannelAccessResponse> + Send + use<'a>,
    > {
        let (routes, next_page_token) = self
            .0
            .service
            .routes(
                id(request.agent_id)?,
                cursor(request.page_token)?,
                size(request.page_size),
            )
            .await?;
        Response::ok(management::ListChannelAccessResponse {
            routes,
            next_page_token,
            ..Default::default()
        })
    }
    async fn set_channel_access<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::SetChannelAccessRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::SetChannelAccessResponse> + Send + use<'a>,
    > {
        let mode = match request.mode.to_i32() {
            1 => types::ChannelAccessMode::Private,
            2 => types::ChannelAccessMode::Public,
            3 => types::ChannelAccessMode::Disabled,
            _ => return Err(ConnectError::invalid_argument("Invalid access mode")),
        };
        self.0
            .service
            .set_mode(id(request.connection_id)?, id(request.agent_id)?, mode)
            .await?;
        Response::ok(management::SetChannelAccessResponse::default())
    }
    async fn list_channel_identities<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ListChannelIdentitiesRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::ListChannelIdentitiesResponse> + Send + use<'a>,
    > {
        let body = request.to_owned_message();
        let (identities, next_page_token) = self
            .0
            .service
            .identities(
                id(&body.agent_id)?,
                body.connection_id.as_deref().map(id).transpose()?,
                cursor(&body.page_token)?,
                size(body.page_size),
            )
            .await?;
        Response::ok(management::ListChannelIdentitiesResponse {
            identities,
            next_page_token,
            ..Default::default()
        })
    }
    async fn set_identity_access<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::SetIdentityAccessRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::SetIdentityAccessResponse> + Send + use<'a>,
    > {
        self.0
            .service
            .set_allowed(
                id(request.connection_id)?,
                id(request.agent_id)?,
                id(request.identity_id)?,
                request.allowed,
            )
            .await?;
        Response::ok(management::SetIdentityAccessResponse::default())
    }
    async fn request_identity_verification<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::RequestIdentityVerificationRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::RequestIdentityVerificationResponse> + Send + use<'a>,
    > {
        let body = request.to_owned_message();
        let (verification_id, status) = self
            .0
            .service
            .request_verification(VerificationRequest {
                id: id(&body.id)?,
                connection: id(&body.connection_id)?,
                agent: id(&body.agent_id)?,
                identity_type: match body.identity_type.to_i32() {
                    1 => types::IdentityType::Email,
                    2 => types::IdentityType::PhoneNumber,
                    3 => types::IdentityType::Username,
                    _ => return Err(ConnectError::invalid_argument("Select an identity type")),
                },
                value: body.value,
                template_name: body.template_name,
                template_language: body.template_language,
            })
            .await?;
        Response::ok(management::RequestIdentityVerificationResponse {
            verification_id: verification_id.to_string(),
            status,
            ..Default::default()
        })
    }
}
impl IdentityVerificationService for Public {
    async fn get_identity_verification<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, setup::GetIdentityVerificationRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<setup::GetIdentityVerificationResponse> + Send + use<'a>,
    > {
        let verification = self
            .0
            .service
            .verification(id(request.id)?, request.identity_verification_token)
            .await?;
        Response::ok(setup::GetIdentityVerificationResponse {
            verification: verification.into(),
            ..Default::default()
        })
    }
    async fn approve_identity_verification<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, setup::ApproveIdentityVerificationRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<setup::ApproveIdentityVerificationResponse> + Send + use<'a>,
    > {
        let verification = self
            .0
            .service
            .approve(id(request.id)?, request.identity_verification_token)
            .await?;
        Response::ok(setup::ApproveIdentityVerificationResponse {
            verification: verification.into(),
            ..Default::default()
        })
    }
}
