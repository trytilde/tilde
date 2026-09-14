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
        8 * 1024 * 1024,
    )
}
impl AgentService for Rpc {
    async fn upload_agent_avatar<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::UploadAgentAvatarRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::UploadAgentAvatarResponse> + Send + use<'a>,
    > {
        let body = request.to_owned_message();
        let agent = self
            .agents
            .upload_avatar(id(&body.id)?, &body.media_type, body.content.to_vec())
            .await?;
        Response::ok(management::UploadAgentAvatarResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }

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
            .list_filtered(request.page_size, request.page_token, request.search)
            .await?;

        let ids = page.agents.iter().map(|agent| agent.id).collect::<Vec<_>>();
        let mut metrics = self.agents.metrics(&ids).await?;
        Response::ok(management::ListAgentsResponse {
            agents: futures::future::try_join_all(page.agents.into_iter().map(|agent| {
                let detail = metrics.remove(&agent.id);
                async move {
                    let avatar_url = self.agents.avatar_url(&agent).await?;
                    Ok::<_, crate::error::Error>(wire(agent, detail, avatar_url))
                }
            }))
            .await?,
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
    async fn pause_agent<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::PauseAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::PauseAgentResponse> + Send + use<'a>>
    {
        let agent = self.agents.pause(id(request.id)?).await?;
        Response::ok(management::PauseAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn resume_agent<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ResumeAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::ResumeAgentResponse> + Send + use<'a>>
    {
        let agent = self.agents.resume(id(request.id)?).await?;
        Response::ok(management::ResumeAgentResponse {
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
