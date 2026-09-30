//! Runtime reads for agents and the read-only management history. Prompts are registered
//! with deployments (crate::deployment), never through these services.
use super::{Prompt, PromptVersion, Prompts, Usage};
use crate::error::Error;
use crate::proto::tilde::{
    management::v1 as management, runtime::v1 as runtime, types::v1 as types,
};
use crate::services::tilde::management::v1::PromptService as ManagementPromptService;
use crate::services::tilde::runtime::v1::PromptService as RuntimePromptService;
use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use std::sync::Arc;
use uuid::Uuid;

fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Invalid UUID".into()))
}
fn timestamp(date: chrono::DateTime<chrono::Utc>) -> buffa_types::google::protobuf::Timestamp {
    buffa_types::google::protobuf::Timestamp {
        seconds: date.timestamp(),
        nanos: date.timestamp_subsec_nanos() as i32,
        ..Default::default()
    }
}
pub(crate) fn version_wire(v: PromptVersion) -> types::PromptVersion {
    types::PromptVersion {
        id: v.id.to_string(),
        prompt_id: v.prompt_id.to_string(),
        number: v.number.max(0) as u32,
        hash: hex::encode(v.hash),
        template: v.template,
        sections: v
            .sections
            .into_iter()
            .map(|s| types::PromptSection {
                name: s.name,
                content: s.content,
                hash: hex::encode(s.hash),
                ..Default::default()
            })
            .collect(),
        config: v.config,
        variables: v.variables,
        created_at: timestamp(v.created_at).into(),
        deployment_id: v.deployment_id.map(|d| d.to_string()).unwrap_or_default(),
        commit_sha: v.commit_sha.unwrap_or_default(),
        format: v.format.wire().into(),
        origin: v.origin,
        ..Default::default()
    }
}
fn prompt_wire(p: Prompt) -> types::Prompt {
    types::Prompt {
        id: p.id.to_string(),
        agent_id: p.agent_id.to_string(),
        name: p.name,
        created_at: timestamp(p.created_at).into(),
        latest: p.latest.map(version_wire).into(),
        ..Default::default()
    }
}
fn usage_wire(u: Usage) -> types::PromptVersionUsage {
    types::PromptVersionUsage {
        version_id: u.version_id.to_string(),
        requests: u.requests,
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
        cost_micros: u.cost_micros,
        average_latency_ms: u.average_latency_ms,
        last_used_at: timestamp(u.last_used_at).into(),
        ..Default::default()
    }
}

struct Runtime {
    prompts: Prompts,
    chat: crate::chat::Chat,
}
/// Invocation-token RPCs; mounted behind the runtime guard like every other agent RPC.
pub fn runtime_router(prompts: Prompts, chat: crate::chat::Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Runtime { prompts, chat })),
        64 * 1024,
    )
}
impl RuntimePromptService for Runtime {
    /// Another agent's prompts need agents.read on it; its own need nothing.
    async fn list_prompts<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime::ListPromptsRequest>,
    ) -> ServiceResult<impl Encodable<runtime::ListPromptsResponse> + Send + use<'a>> {
        use crate::iam::capabilities::Capability;
        let scope = crate::chat::rpc::invocation_scope(&self.chat, &ctx).await?;
        let agent = if request.agent_id.is_empty() {
            scope.agent_id
        } else {
            let agent = id(request.agent_id)?;
            if agent != scope.agent_id {
                scope
                    .capabilities
                    .require(Capability::AgentsRead, request.agent_id)?;
            }
            agent
        };
        Response::ok(runtime::ListPromptsResponse {
            prompts: self
                .prompts
                .list(agent)
                .await?
                .into_iter()
                .map(prompt_wire)
                .collect(),
            ..Default::default()
        })
    }
}

struct Management(Prompts);
pub fn management_router(prompts: Prompts) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Management(prompts))),
        256 * 1024,
    )
}
impl ManagementPromptService for Management {
    async fn list_prompts<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListPromptsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListPromptsResponse> + Send + use<'a>> {
        let agent = id(request.agent_id)?;
        Response::ok(management::ListPromptsResponse {
            prompts: self
                .0
                .list(agent)
                .await?
                .into_iter()
                .map(prompt_wire)
                .collect(),
            ..Default::default()
        })
    }
    async fn get_prompt<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetPromptRequest>,
    ) -> ServiceResult<impl Encodable<management::GetPromptResponse> + Send + use<'a>> {
        let (prompt, versions, usage) = self.0.get(id(request.id)?).await?;
        Response::ok(management::GetPromptResponse {
            prompt: prompt_wire(prompt).into(),
            versions: versions.into_iter().map(version_wire).collect(),
            usage: usage.into_iter().map(usage_wire).collect(),
            ..Default::default()
        })
    }
}
