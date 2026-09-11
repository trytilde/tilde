use super::{id, project, wire};
use crate::agent::{Agents, CreateAgent, UpdateAgent};
use crate::iam::capabilities::Capability;
use crate::proto::tilde::runtime::v1 as runtime_pb;
use crate::services::tilde::runtime::v1::AgentService;
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::SecretString;
use std::sync::Arc;
use uuid::Uuid;
struct Rpc {
    agents: Agents,
    chat: crate::chat::Chat,
}
pub fn router(agents: Agents, chat: crate::chat::Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc { agents, chat })),
        256 * 1024,
    )
}
impl Rpc {
    async fn authority(
        &self,
        ctx: &RequestContext,
    ) -> Result<crate::chat::Scope, connectrpc::ConnectError> {
        crate::chat::rpc::invocation_scope(&self.chat, ctx).await
    }
}
impl AgentService for Rpc {
    async fn create_agent<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::CreateAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::CreateAgentResponse> + Send + use<'a>>
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
        {
            let scope = self.authority(&ctx).await?;
            scope.capabilities.require(Capability::AgentsCreate, "")?;
            if !caps.is_empty() {
                authorize_grant(&scope.capabilities, target, &caps)?;
            }
        }
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
        Response::ok(runtime_pb::CreateAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn get_agent<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::GetAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::GetAgentResponse> + Send + use<'a>>
    {
        {
            let scope = self.authority(&ctx).await?;
            scope
                .capabilities
                .require(Capability::AgentsRead, request.id)?;
        }
        let agent = self.agents.get(id(request.id)?).await?;
        Response::ok(runtime_pb::GetAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn list_agents<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::ListAgentsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::ListAgentsResponse> + Send + use<'a>>
    {
        let authority = self.authority(&ctx).await?;
        if !authority.capabilities.grants(Capability::AgentsRead) {
            return Err(connectrpc::ConnectError::permission_denied(
                "Agent read capability required",
            ));
        }
        let page = self
            .agents
            .list(request.page_size, request.page_token)
            .await?;
        let mut page = page;
        {
            let scope = authority;
            page.agents.retain(|agent| {
                scope
                    .capabilities
                    .permits(Capability::AgentsRead, &agent.id.to_string())
            });
        }
        let ids = page.agents.iter().map(|agent| agent.id).collect::<Vec<_>>();
        let mut metrics = self.agents.metrics(&ids).await?;
        Response::ok(runtime_pb::ListAgentsResponse {
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
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::UpdateAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::UpdateAgentResponse> + Send + use<'a>>
    {
        let body = request.to_owned_message();
        let caps = body
            .capabilities
            .into_option()
            .map(crate::iam::capabilities::Capabilities::from_wire)
            .transpose()?;
        let authority = self.authority(&ctx).await?;
        let agent = self
            .agents
            .update_as(
                UpdateAgent {
                    capabilities: caps,
                    id: id(&body.id)?,
                    name: body.name,
                    endpoint_url: body.endpoint_url,
                },
                Some(&authority.capabilities),
            )
            .await?;
        Response::ok(runtime_pb::UpdateAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn delete_agent<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::DeleteAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::DeleteAgentResponse> + Send + use<'a>>
    {
        {
            let scope = self.authority(&ctx).await?;
            scope
                .capabilities
                .require(Capability::AgentsDelete, request.id)?;
        }
        self.agents.delete(id(request.id)?).await?;
        Response::ok(runtime_pb::DeleteAgentResponse::default())
    }
}
fn authorize_grant(
    caller: &crate::iam::capabilities::Capabilities,
    target: Uuid,
    grants: &crate::iam::capabilities::Capabilities,
) -> Result<(), connectrpc::ConnectError> {
    caller.require(Capability::AgentsGrant, &target.to_string())?;
    if !grants.subset_of(caller) {
        return Err(connectrpc::ConnectError::permission_denied(
            "Cannot grant capabilities beyond invocation authority",
        ));
    }
    Ok(())
}
