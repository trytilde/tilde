use super::*;
use crate::connections::{model as m, service::Connections};
use crate::proto::tilde::management::v1 as management;
use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::SecretString;
use std::sync::Arc;
use uuid::Uuid;
struct Rpc(Connections);
use crate::services::tilde::management::v1::ConnectionsService;
pub fn router(connections: Connections) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(connections))),
        2 * 1024 * 1024,
    )
}
use base64::Engine;
impl ConnectionsService for Rpc {
    async fn assign_capability<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::AssignCapabilityRequest>,
    ) -> ServiceResult<impl Encodable<management::AssignCapabilityResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let assignment = assignment_model(
            request
                .assignment
                .into_option()
                .ok_or_else(|| m::invalid("Assignment required"))?,
        )?;
        Response::ok(management::AssignCapabilityResponse {
            connection: connection_wire(
                self.0
                    .assign(id(&request.connection_id)?, &assignment)
                    .await?,
                &self.0.event_ingress_url,
            )
            .into(),
            ..Default::default()
        })
    }
    async fn unassign_capability<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::UnassignCapabilityRequest>,
    ) -> ServiceResult<impl Encodable<management::UnassignCapabilityResponse> + Send + use<'a>>
    {
        let request = request.to_owned_message();
        let assignment = assignment_model(
            request
                .assignment
                .into_option()
                .ok_or_else(|| m::invalid("Assignment required"))?,
        )?;
        Response::ok(management::UnassignCapabilityResponse {
            connection: connection_wire(
                self.0
                    .unassign(id(&request.connection_id)?, &assignment)
                    .await?,
                &self.0.event_ingress_url,
            )
            .into(),
            ..Default::default()
        })
    }
    async fn list_providers<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ListProvidersRequest>,
    ) -> ServiceResult<impl Encodable<management::ListProvidersResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let (providers, next) = self
            .0
            .providers(
                &request.page_token,
                request.search.as_deref(),
                request.page_size,
            )
            .await?;
        Response::ok(management::ListProvidersResponse {
            providers: providers.into_iter().map(provider_wire).collect(),
            next_page_token: next,
            ..Default::default()
        })
    }
    async fn get_provider<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::GetProviderRequest>,
    ) -> ServiceResult<impl Encodable<management::GetProviderResponse> + Send + use<'a>> {
        Response::ok(management::GetProviderResponse {
            provider: provider_wire(self.0.provider(request.id).await?).into(),
            ..Default::default()
        })
    }
    async fn register_provider<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::RegisterProviderRequest>,
    ) -> ServiceResult<impl Encodable<management::RegisterProviderResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let provider = provider_model(
            request
                .provider
                .into_option()
                .ok_or_else(|| m::invalid("Provider definition required"))?,
        )?;
        Response::ok(management::RegisterProviderResponse {
            provider: provider_wire(
                self.0
                    .register_provider(provider, request.backend_token.map(SecretString::from))
                    .await?,
            )
            .into(),
            ..Default::default()
        })
    }
    async fn start_connection<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::StartConnectionRequest>,
    ) -> ServiceResult<impl Encodable<management::StartConnectionResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let started = self
            .0
            .start(
                if request.id.is_empty() {
                    Uuid::new_v4()
                } else {
                    id(&request.id)?
                },
                &request.name,
                &request.provider_id,
                &request.type_id,
                &request
                    .assignments
                    .into_iter()
                    .map(assignment_model)
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .await?;
        Response::ok(management::StartConnectionResponse {
            connection: connection_wire(started.connection, &self.0.event_ingress_url).into(),
            brokering_url: started.brokering_url,
            ..Default::default()
        })
    }
    async fn get_connection<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::GetConnectionRequest>,
    ) -> ServiceResult<impl Encodable<management::GetConnectionResponse> + Send + use<'a>> {
        Response::ok(management::GetConnectionResponse {
            connection: connection_wire(
                self.0.get(id(request.id)?).await?,
                &self.0.event_ingress_url,
            )
            .into(),
            ..Default::default()
        })
    }
    async fn list_connections<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ListConnectionsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListConnectionsResponse> + Send + use<'a>> {
        let after = if request.page_token.is_empty() {
            None
        } else {
            if request.page_token.len() > 512 {
                return Err(m::invalid("Invalid page token").into());
            }
            let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(request.page_token)
                .map_err(|_| m::invalid("Invalid page token"))?;
            Some(
                serde_json::from_slice::<(chrono::DateTime<chrono::Utc>, Uuid)>(&decoded)
                    .map_err(|_| m::invalid("Invalid page token"))?,
            )
        };
        let (connections, next) = self
            .0
            .list(
                after,
                request.page_size,
                request.agent_id.map(id).transpose()?,
                request.unassigned_channel_only,
            )
            .await?;
        let next = next
            .map(|value| {
                serde_json::to_vec(&value)
                    .map(|value| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value))
            })
            .transpose()
            .map_err(|_| m::invalid("Unable to encode page token"))?
            .unwrap_or_default();
        Response::ok(management::ListConnectionsResponse {
            connections: connections
                .into_iter()
                .map(|c| connection_wire(c, &self.0.event_ingress_url))
                .collect(),
            next_page_token: next,
            ..Default::default()
        })
    }
    async fn reconnect<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ReconnectRequest>,
    ) -> ServiceResult<impl Encodable<management::ReconnectResponse> + Send + use<'a>> {
        let started = self.0.reconnect(id(request.id)?).await?;
        Response::ok(management::ReconnectResponse {
            connection: connection_wire(started.connection, &self.0.event_ingress_url).into(),
            brokering_url: started.brokering_url,
            ..Default::default()
        })
    }
    async fn disconnect<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::DisconnectRequest>,
    ) -> ServiceResult<impl Encodable<management::DisconnectResponse> + Send + use<'a>> {
        self.0.disconnect(id(request.id)?).await?;
        Response::ok(management::DisconnectResponse::default())
    }
}
