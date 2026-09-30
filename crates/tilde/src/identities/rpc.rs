use super::Identities;
use crate::{iam::parse_id, proto::tilde::management::v1 as wire};
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use std::sync::Arc;
pub fn router(service: Identities) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(service)),
        64 * 1024,
    )
}
fn cursor(value: &str) -> Result<Option<uuid::Uuid>, ConnectError> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse_id(value).map(Some)
    }
}
fn size(value: u32) -> usize {
    if value == 0 { 50 } else { value as usize }
}
impl crate::services::tilde::management::v1::IdentitiesService for Identities {
    async fn create_identity<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::CreateIdentityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::CreateIdentityResponse> + Send + use<'a>>
    {
        let identity = self.create(r.to_owned_message()).await?;
        Response::ok(wire::CreateIdentityResponse {
            identity: identity.into(),
            ..Default::default()
        })
    }
    async fn get_identity<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::GetIdentityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetIdentityResponse> + Send + use<'a>> {
        let identity = self.get(parse_id(r.id)?).await?;
        Response::ok(wire::GetIdentityResponse {
            identity: identity.into(),
            ..Default::default()
        })
    }
    async fn list_identities<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::ListIdentitiesRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListIdentitiesResponse> + Send + use<'a>>
    {
        let r = r.to_owned_message();
        // Unfiltered lists span providers.
        let (identities, next_page_token) = self
            .list(
                cursor(&r.page_token)?,
                size(r.page_size),
                r.root_identity_id.as_deref().map(parse_id).transpose()?,
                r.connection_id.as_deref().map(parse_id).transpose()?,
            )
            .await?;
        Response::ok(wire::ListIdentitiesResponse {
            identities,
            next_page_token,
            ..Default::default()
        })
    }
    async fn create_root_identity<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::CreateRootIdentityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::CreateRootIdentityResponse> + Send + use<'a>>
    {
        let r = r.to_owned_message();
        let root_identity = self
            .create_root(
                parse_id(&r.id)?,
                r.identity_ids
                    .iter()
                    .map(|id| parse_id(id))
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .await?;
        Response::ok(wire::CreateRootIdentityResponse {
            root_identity: root_identity.into(),
            ..Default::default()
        })
    }
    async fn get_root_identity<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::GetRootIdentityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetRootIdentityResponse> + Send + use<'a>>
    {
        let root_identity = self.get_root(parse_id(r.id)?).await?;
        Response::ok(wire::GetRootIdentityResponse {
            root_identity: root_identity.into(),
            ..Default::default()
        })
    }
    async fn list_root_identities<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::ListRootIdentitiesRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListRootIdentitiesResponse> + Send + use<'a>>
    {
        let (root_identities, next_page_token) = self
            .list_roots(cursor(r.page_token)?, size(r.page_size))
            .await?;
        Response::ok(wire::ListRootIdentitiesResponse {
            root_identities,
            next_page_token,
            ..Default::default()
        })
    }
    async fn link_identity<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::LinkIdentityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::LinkIdentityResponse> + Send + use<'a>>
    {
        // Roots group identities across providers.
        let identity = self
            .link(
                parse_id(r.identity_id)?,
                parse_id(r.root_identity_id)?,
                false,
            )
            .await?;
        Response::ok(wire::LinkIdentityResponse {
            identity: identity.into(),
            ..Default::default()
        })
    }
    async fn unlink_identity<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::UnlinkIdentityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::UnlinkIdentityResponse> + Send + use<'a>>
    {
        // Roots group identities across providers.
        let identity = self
            .link(
                parse_id(r.identity_id)?,
                parse_id(r.root_identity_id)?,
                true,
            )
            .await?;
        Response::ok(wire::UnlinkIdentityResponse {
            identity: identity.into(),
            ..Default::default()
        })
    }
}
