use super::{id, project, wire};
use crate::agent::{Agents, CreateAgent, UpdateAgent};
use crate::proto::tilde::management::v1 as management;
use crate::services::tilde::management::v1::AgentService;
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::SecretString;
use std::sync::Arc;
use uuid::Uuid;
struct Rpc {
    agents: Agents,
}
pub fn router(agents: Agents) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc { agents })),
        256 * 1024,
    )
}
impl AgentService for Rpc {
    async fn create_agent<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::CreateAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::CreateAgentResponse> + Send + use<'a>>
    {
        let body = request.to_owned_message();
        let target = if body.id.is_empty() {
            Uuid::new_v4()
        } else {
            id(&body.id)?
        };
        let caps = crate::iam::capabilities::Capabilities::from_wire(
            body.capabilities.into_option().unwrap_or_default(),
        )?;

        let agent = self
            .agents
            .create(CreateAgent {
                id: target,
                capabilities: caps,
                name: body.name,
                webhook_signing_key: SecretString::from(body.webhook_signing_key),
                endpoint_url: body.endpoint_url,
            })
            .await?;
        Response::ok(management::CreateAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn get_agent<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::GetAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::GetAgentResponse> + Send + use<'a>>
    {
        let agent = self.agents.get(id(request.id)?).await?;
        Response::ok(management::GetAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn list_agents<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ListAgentsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::ListAgentsResponse> + Send + use<'a>>
    {
        let page = self
            .agents
            .list(request.page_size, request.page_token)
            .await?;

        let ids = page.agents.iter().map(|agent| agent.id).collect::<Vec<_>>();
        let mut metrics = self.agents.metrics(&ids).await?;
        Response::ok(management::ListAgentsResponse {
            agents: page
                .agents
                .into_iter()
                .map(|agent| {
                    let detail = metrics.remove(&agent.id);
                    wire(agent, detail)
                })
                .collect(),
            next_page_token: page.next_page_token,
            ..Default::default()
        })
    }
    async fn update_agent<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::UpdateAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::UpdateAgentResponse> + Send + use<'a>>
    {
        let body = request.to_owned_message();
        let caps = body
            .capabilities
            .into_option()
            .map(crate::iam::capabilities::Capabilities::from_wire)
            .transpose()?;

        let agent = self
            .agents
            .update_as(
                UpdateAgent {
                    capabilities: caps,
                    id: id(&body.id)?,
                    name: body.name,
                    endpoint_url: body.endpoint_url,
                },
                None,
            )
            .await?;
        Response::ok(management::UpdateAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn delete_agent<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::DeleteAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::DeleteAgentResponse> + Send + use<'a>>
    {
        self.agents.delete(id(request.id)?).await?;
        Response::ok(management::DeleteAgentResponse::default())
    }
}
