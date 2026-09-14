//! The run protocol for hosts that dial in, served on the agent runtime listener.
//! `Watch` and `Heartbeat` authenticate a direct deployment token and make the process
//! an instance of that deployment; wakes for it arrive as frames. `Report` carries an
//! invocation capability and is how every host, woken by stream, HTTP or a cloud invoke
//! API, tells the gateway about acceptance, reasoning and the end of an invocation.
use super::{Deployments, id};
use crate::chat::Chat;
use crate::proto::tilde::{agent_event_ingress::v1 as wire, run::v1 as run, types::v1 as types};
use crate::services::tilde::run::v1::RunService;
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

struct Rpc {
    deployments: Deployments,
    chat: Chat,
}
pub fn router(deployments: Deployments, chat: Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc { deployments, chat })),
        4 * 1024 * 1024,
    )
}
fn bearer(ctx: &RequestContext) -> Option<&str> {
    ctx.headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}
impl Rpc {
    async fn direct(
        &self,
        ctx: &RequestContext,
    ) -> Result<super::Authenticated, connectrpc::ConnectError> {
        let token = bearer(ctx).ok_or_else(|| {
            connectrpc::ConnectError::unauthenticated("Deployment token required")
        })?;
        let auth =
            self.deployments.authenticate(token).await.map_err(|_| {
                connectrpc::ConnectError::unauthenticated("Invalid deployment token")
            })?;
        if auth.target != types::DeploymentTarget::Direct {
            return Err(connectrpc::ConnectError::permission_denied(
                "Only direct deployments connect through the run protocol",
            ));
        }
        Ok(auth)
    }
}
impl RunService for Rpc {
    async fn watch(
        &self,
        ctx: RequestContext,
        r: ServiceRequest<'_, run::WatchRequest>,
    ) -> ServiceResult<
        connectrpc::ServiceStream<impl connectrpc::Encodable<run::WatchResponse> + Send + use<>>,
    > {
        let auth = self.direct(&ctx).await?;
        let token = bearer(&ctx).unwrap_or_default().to_owned();
        let request = r.to_owned_message();
        let instance = id(&request.instance_id)?;
        let public_url = if request.public_url.is_empty() {
            String::new()
        } else {
            crate::agent::validate_endpoint(request.public_url.clone())?
        };
        self.deployments
            .register(
                auth.agent,
                auth.deployment,
                &wire::WatchRequest {
                    instance_id: request.instance_id.clone(),
                    public_url: public_url.clone(),
                    runtime_url: public_url,
                    ..Default::default()
                },
            )
            .await?;
        let service = self.deployments.clone();
        let mut directives = service
            .channels
            .directives
            .subscribe(&service.pool, "tilde_sidecar_directives")
            .await
            .map_err(crate::error::Error::from)?;
        directives.borrow_and_update();
        Response::stream_ok(async_stream::try_stream! {
            yield run::WatchResponse { frame: Some(run::RunRegistered {
                agent_id: auth.agent.to_string(),
                deployment_id: auth.deployment.to_string(),
                instance_id: instance.to_string(),
                ..Default::default()
            }.into()), ..Default::default() };
            let mut sent = std::collections::BTreeSet::new();
            loop {
                let pending = service.pending_directives(auth.agent, instance).await?;
                let ids: std::collections::BTreeSet<String> = pending.iter().map(|d| d.id.clone()).collect();
                sent.retain(|id| ids.contains(id));
                for directive in pending {
                    if let Some(wire::directive::Action::Wake(wake)) = directive.action
                        && sent.insert(directive.id.clone())
                    {
                        let mut wake = *wake;
                        wake.command_id = directive.id.clone();
                        yield run::WatchResponse { frame: Some(run::watch_response::Frame::Wake(Box::new(wake))), ..Default::default() };
                    }
                }
                tokio::select! {
                    changed = directives.changed() => { if changed.is_err() { break; } directives.borrow_and_update(); }
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {
                        // A rotated or retired token ends the stream instead of living on until TCP notices.
                        if service.authenticate(&token).await.ok().map(|a| a.deployment) != Some(auth.deployment) { break; }
                        yield run::WatchResponse { frame: Some(run::RunPing::default().into()), ..Default::default() };
                    }
                }
            }
        })
    }
    async fn heartbeat<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, run::HeartbeatRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<run::HeartbeatResponse> + Send + use<'a>> {
        let auth = self.direct(&ctx).await?;
        self.deployments
            .heartbeat(
                auth.agent,
                id(r.instance_id)?,
                &wire::Heartbeat {
                    ready: r.ready,
                    agent_ready: r.ready,
                    ..Default::default()
                },
            )
            .await?;
        Response::ok(run::HeartbeatResponse::default())
    }
    async fn report<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, run::ReportRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<run::ReportResponse> + Send + use<'a>> {
        let token = crate::chat::rpc::capability(&ctx)?;
        let claims = self.chat.tokens.verify_trace(token).await?;
        let request = r.to_owned_message();
        let invocation = id(&request.invocation_id)?;
        if invocation != claims.invocation_id {
            return Err(connectrpc::ConnectError::permission_denied(
                "Report is scoped to the capability's invocation",
            ));
        }
        self.chat
            .report(claims.sub, claims.thread_id, invocation, request.event)
            .await?;
        Response::ok(run::ReportResponse::default())
    }
}
impl Deployments {
    /// A live instance of `deployment` that dialed in through the run protocol.
    pub(crate) async fn connected_instance(
        &self,
        agent: Uuid,
        deployment: Uuid,
    ) -> Result<Option<Uuid>, crate::error::Error> {
        Ok(sqlx::query_file!(
            "../../queries/deployment/connected_instance.sql",
            agent,
            deployment
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|r| r.instance_id))
    }
    /// Acknowledge a wake directive from whichever instance accepted it.
    pub(crate) async fn acknowledge_wake(
        &self,
        agent: Uuid,
        key: Uuid,
    ) -> Result<bool, crate::error::Error> {
        Ok(
            sqlx::query_file!("../../queries/deployment/directive_ack.sql", key, agent)
                .fetch_optional(&self.pool)
                .await?
                .is_some(),
        )
    }
}
