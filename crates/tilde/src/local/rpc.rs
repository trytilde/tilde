use super::Caller;
use crate::proto::tilde::local::v1 as wire;
use crate::services::tilde::local::v1::LocalAgentService;
use crate::{chat::Chat, deployment::Deployments, skills::FileBody};
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use std::sync::Arc;

struct Rpc {
    deployments: Deployments,
    chat: Chat,
}
pub(super) fn router(deployments: Deployments, chat: Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc { deployments, chat })),
        16 * 1024 * 1024,
    )
}
fn caller(ctx: &RequestContext) -> Result<Caller, ConnectError> {
    ctx.extensions()
        .get::<Caller>()
        .cloned()
        .ok_or_else(|| ConnectError::unauthenticated("A local agent's key is required"))
}
fn unsupported(what: &str) -> ConnectError {
    ConnectError::failed_precondition(format!("This agent's harness does not support {what}"))
}

impl LocalAgentService for Rpc {
    async fn ingest<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::IngestRequest>,
    ) -> ServiceResult<impl Encodable<wire::IngestResponse> + Send + use<'a>> {
        let caller = caller(&ctx)?;
        if !caller.harness.features().sessions {
            return Err(unsupported("sessions"));
        }
        super::ingest::ingest(
            &self.chat,
            self.deployments.telemetry.as_ref(),
            &caller,
            request.to_owned_message(),
        )
        .await?;
        Response::ok(wire::IngestResponse::default())
    }
    async fn list_skills<'a>(
        &'a self,
        ctx: RequestContext,
        _: ServiceRequest<'_, wire::ListSkillsRequest>,
    ) -> ServiceResult<impl Encodable<wire::ListSkillsResponse> + Send + use<'a>> {
        let caller = caller(&ctx)?;
        if !caller.harness.features().skills {
            return Err(unsupported("skills"));
        }
        Response::ok(wire::ListSkillsResponse {
            skills: self
                .chat
                .skills()?
                .for_agent(caller.agent, None)
                .await?
                .into_iter()
                .map(crate::skills::rpc::summary_wire)
                .collect(),
            ..Default::default()
        })
    }
    async fn read_skill_file<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::ReadSkillFileRequest>,
    ) -> ServiceResult<impl Encodable<wire::ReadSkillFileResponse> + Send + use<'a>> {
        let caller = caller(&ctx)?;
        if !caller.harness.features().skills {
            return Err(unsupported("skills"));
        }
        let (content, download_url) = match self
            .chat
            .skills()?
            .read(caller.agent, None, request.name, request.path)
            .await?
        {
            FileBody::Text(text, _) => (text, String::new()),
            FileBody::Url(url, _) => (String::new(), url),
        };
        Response::ok(wire::ReadSkillFileResponse {
            content,
            download_url,
            ..Default::default()
        })
    }
}
