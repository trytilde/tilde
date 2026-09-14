//! The run protocol as served by a sidecar to the agent process beside it. The agent
//! dials in with the node's deployment token; wakes are the invocations this replica
//! decided to run, and reports are verified with the invocation capability the
//! runtime itself issued. The gateway never sees this instance.
use super::sidecar::Node;
use crate::chat::ChatError;
use crate::proto::tilde::run::v1 as run;
use crate::services::tilde::run::v1::RunService;
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use sha2::Digest;
use std::{sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use uuid::Uuid;

struct Rpc(Node);
pub fn router(node: Node) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(node))),
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
    fn deployment_token(&self, ctx: &RequestContext) -> Result<(), connectrpc::ConnectError> {
        let token = bearer(ctx).ok_or_else(|| {
            connectrpc::ConnectError::unauthenticated("Deployment token required")
        })?;
        let hash: [u8; 32] = sha2::Sha256::digest(token.as_bytes()).into();
        if !bool::from(hash.ct_eq(self.0.token_hash())) {
            return Err(connectrpc::ConnectError::unauthenticated(
                "Invalid deployment token",
            ));
        }
        Ok(())
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
        self.deployment_token(&ctx)?;
        let instance = crate::chat::id(r.instance_id)?;
        let runtime = self.0.runtime.clone();
        let deployment = self.0.deployment_id.clone();
        let mut wakes = runtime.attach_local(instance);
        Response::stream_ok(async_stream::stream! {
            yield Ok(run::WatchResponse { frame: Some(run::RunRegistered {
                agent_id: runtime.agent_id.to_string(),
                deployment_id: deployment,
                instance_id: instance.to_string(),
                ..Default::default()
            }.into()), ..Default::default() });
            loop {
                tokio::select! {
                    wake = wakes.recv() => match wake {
                        Some(wake) => yield Ok(run::WatchResponse { frame: Some(run::watch_response::Frame::Wake(Box::new(wake))), ..Default::default() }),
                        None => break,
                    },
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {
                        yield Ok(run::WatchResponse { frame: Some(run::RunPing::default().into()), ..Default::default() });
                    }
                }
            }
            runtime.detach_local(instance);
        })
    }
    async fn heartbeat<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, run::HeartbeatRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<run::HeartbeatResponse> + Send + use<'a>> {
        self.deployment_token(&ctx)?;
        let instance = crate::chat::id(r.instance_id)?;
        if !self.0.runtime.local_heartbeat(instance, r.ready) {
            return Err(connectrpc::ConnectError::failed_precondition(
                "Open Watch before heartbeating",
            ));
        }
        Response::ok(run::HeartbeatResponse::default())
    }
    async fn report<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, run::ReportRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<run::ReportResponse> + Send + use<'a>> {
        let token = crate::chat::rpc::capability(&ctx)?;
        let claims = self.0.runtime.verified_claims(token, true).await?;
        let request = r.to_owned_message();
        let invocation: Uuid = crate::chat::id(&request.invocation_id)?;
        if invocation != claims.invocation_id {
            return Err(connectrpc::ConnectError::permission_denied(
                "Report is scoped to the capability's invocation",
            ));
        }
        let event = request.event.ok_or_else(|| {
            connectrpc::ConnectError::invalid_argument("Report requires an event")
        })?;
        match self.0.runtime.report_local(invocation, event).await {
            Ok(()) => Response::ok(run::ReportResponse::default()),
            // A report for an execution this replica is no longer driving is not an error
            // for the agent; the lease moved and the new holder's state is canonical.
            Err(ChatError::NotFound) => Response::ok(run::ReportResponse::default()),
            Err(error) => Err(error.into()),
        }
    }
}
