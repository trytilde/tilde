use super::{id, project, wire};
use crate::agent::{Agents, CreateAgent, UpdateAgent};
use crate::iam::authz::{self, Action, Kind, Resource};
use crate::iam::capabilities::{Capabilities, Capability, Reach};
use crate::proto::tilde::management::v1 as management;
use crate::services::tilde::management::v1::AgentService;
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use std::sync::Arc;
use uuid::Uuid;
struct Rpc {
    agents: Agents,
}
/// An agent's capabilities let it act on other agents, so an owner may only hand out reach
/// they hold themselves: view to read or invoke, edit to change, and nothing
/// installation-wide unless they hold it installation-wide. Reaches left as they are stay valid.
async fn authorize_capabilities(
    ctx: &RequestContext,
    requested: &Capabilities,
    current: Option<&Capabilities>,
) -> Result<(), connectrpc::ConnectError> {
    let access = authz::caller(ctx)?;
    if access.admin {
        return Ok(());
    }
    let denied =
        || connectrpc::ConnectError::permission_denied("Capabilities exceed your own access");
    for (action, reach) in &requested.0 {
        if current.is_some_and(|current| current.0.get(action) == Some(reach)) {
            continue;
        }
        let needed = match action {
            Capability::AgentsRead | Capability::AgentsInvoke => Action::View,
            Capability::AgentsUpdate | Capability::AgentsDelete | Capability::AgentsGrant => {
                Action::Edit
            }
            _ => continue,
        };
        match reach {
            Reach::All => authz::authz(ctx)?
                .check(access, Resource::all(Kind::Agent), needed)
                .await
                .map_err(|_| denied())?,
            Reach::Selected { ids } => {
                let ids = ids
                    .iter()
                    .map(|id| Uuid::parse_str(id).map_err(|_| denied()))
                    .collect::<Result<Vec<_>, _>>()?;
                let held = authz::authz(ctx)?
                    .held_many(access, Kind::Agent, needed, &ids)
                    .await?;
                if ids.iter().any(|id| {
                    authz::decide(held.get(id).copied().unwrap_or_default())
                        != authz::Decision::Allow
                }) {
                    return Err(denied());
                }
            }
            _ => {}
        }
    }
    Ok(())
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
        ctx: RequestContext,
        request: ServiceRequest<'_, management::UploadAgentAvatarRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::UploadAgentAvatarResponse> + Send + use<'a>,
    > {
        let body = request.to_owned_message();
        authz::require(&ctx, Resource::agent(id(&body.id)?), Action::Edit).await?;
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
        ctx: RequestContext,
        request: ServiceRequest<'_, management::CreateAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::CreateAgentResponse> + Send + use<'a>>
    {
        let access = authz::require_create(&ctx, Kind::Agent).await?;
        let body = request.to_owned_message();
        let target = if body.id.is_empty() {
            Uuid::new_v4()
        } else {
            id(&body.id)?
        };
        let caps = crate::iam::capabilities::Capabilities::from_wire(
            body.capabilities.into_option().unwrap_or_default(),
        )?;
        authorize_capabilities(&ctx, &caps, None).await?;
        // Creation is retry-safe by id; a retry must not adopt somebody else's agent.
        if let Ok(existing) = self.agents.get(target).await {
            authz::require(&ctx, Resource::agent(existing.id), Action::Edit).await?;
        }

        let agent = self
            .agents
            .create_as(
                CreateAgent {
                    concurrency_policy: crate::agent::ConcurrencyPolicy::from_wire(
                        body.concurrency_policy.to_i32(),
                    )?,
                    id: target,
                    capabilities: caps,
                    name: body.name,
                },
                Some(access),
            )
            .await?;
        Response::ok(management::CreateAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn get_agent<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, management::GetAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::GetAgentResponse> + Send + use<'a>>
    {
        authz::require(&ctx, Resource::agent(id(request.id)?), Action::View).await?;
        let agent = self.agents.get(id(request.id)?).await?;
        Response::ok(management::GetAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn list_agents<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, management::ListAgentsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::ListAgentsResponse> + Send + use<'a>>
    {
        let page = self
            .agents
            .list_filtered(
                request.page_size,
                request.page_token,
                request.search,
                authz::caller(&ctx)?,
            )
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
        ctx: RequestContext,
        request: ServiceRequest<'_, management::UpdateAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::UpdateAgentResponse> + Send + use<'a>>
    {
        let body = request.to_owned_message();
        let caps = body
            .capabilities
            .into_option()
            .map(crate::iam::capabilities::Capabilities::from_wire)
            .transpose()?;
        authz::require(&ctx, Resource::agent(id(&body.id)?), Action::Edit).await?;
        if let Some(caps) = &caps {
            let current = self.agents.get(id(&body.id)?).await?;
            authorize_capabilities(&ctx, caps, Some(&current.capabilities.0)).await?;
        }

        let agent = self
            .agents
            .update_as(
                UpdateAgent {
                    concurrency_policy: body
                        .concurrency_policy
                        .map(|value| crate::agent::ConcurrencyPolicy::from_wire(value.to_i32()))
                        .transpose()?,
                    capabilities: caps,
                    id: id(&body.id)?,
                    name: body.name,
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
        ctx: RequestContext,
        request: ServiceRequest<'_, management::PauseAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::PauseAgentResponse> + Send + use<'a>>
    {
        authz::require(&ctx, Resource::agent(id(request.id)?), Action::Edit).await?;
        let agent = self.agents.pause(id(request.id)?).await?;
        Response::ok(management::PauseAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn resume_agent<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, management::ResumeAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::ResumeAgentResponse> + Send + use<'a>>
    {
        authz::require(&ctx, Resource::agent(id(request.id)?), Action::Edit).await?;
        let agent = self.agents.resume(id(request.id)?).await?;
        Response::ok(management::ResumeAgentResponse {
            agent: project(&self.agents, agent).await?.into(),
            ..Default::default()
        })
    }
    async fn delete_agent<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteAgentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::DeleteAgentResponse> + Send + use<'a>>
    {
        authz::require(&ctx, Resource::agent(id(request.id)?), Action::Edit).await?;
        self.agents.delete(id(request.id)?).await?;
        Response::ok(management::DeleteAgentResponse::default())
    }
}
