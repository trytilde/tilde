use super::{Deployments, id};
use crate::error::Error;
use crate::proto::tilde::{agent_event_ingress::v1 as ingress, management::v1 as management};
use crate::services::tilde::{
    agent_event_ingress::v1::SidecarService, management::v1::DeploymentService,
};
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::{ExposeSecret, SecretString};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
struct Rpc(Deployments);
pub fn management_router(service: Deployments) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(service))),
        1024 * 1024,
    )
}
/// The complete sidecar-facing surface; every call authenticates a deployment token.
pub fn sidecar_router(service: Deployments) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Control(service))),
        192 * 1024 * 1024,
    )
}
impl DeploymentService for Rpc {
    async fn get_deployment<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::GetDeploymentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::GetDeploymentResponse> + Send + use<'a>>
    {
        let agent = id(r.agent_id)?;
        Response::ok(management::GetDeploymentResponse {
            deployment: self.0.get(agent).await?.into(),
            nodes: self.0.nodes(agent).await?,
            ..Default::default()
        })
    }
    async fn set_deployment<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::SetDeploymentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::SetDeploymentResponse> + Send + use<'a>>
    {
        Response::ok(management::SetDeploymentResponse {
            deployment: self
                .0
                .set(
                    id(r.agent_id)?,
                    r.mode.as_known().ok_or_else(|| {
                        connectrpc::ConnectError::invalid_argument("Invalid deployment mode")
                    })?,
                    r.endpoint_url.map(str::to_owned),
                    r.failure_mode.as_known().ok_or_else(|| {
                        connectrpc::ConnectError::invalid_argument("Invalid failure policy")
                    })?,
                )
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn issue_deployment_token<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::IssueDeploymentTokenRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::IssueDeploymentTokenResponse> + Send + use<'a>,
    > {
        let token = self.0.issue_token(id(r.agent_id)?).await?;
        Response::ok(management::IssueDeploymentTokenResponse {
            token: token.expose_secret().into(),
            ..Default::default()
        })
    }
    async fn issue_ingress_token<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, management::IssueIngressTokenRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::IssueIngressTokenResponse> + Send + use<'a>,
    > {
        let principal = match ctx.extensions().get::<crate::iam::Principal>() {
            Some(crate::iam::Principal::User { id }) => *id,
            _ => {
                return Err(connectrpc::ConnectError::unauthenticated(
                    "Management identity required",
                ));
            }
        };
        let token = self
            .0
            .issue_ingress_token(id(r.agent_id)?, r.thread_id.map(str::to_owned), principal)
            .await?;
        Response::ok(management::IssueIngressTokenResponse {
            token: token.expose_secret().into(),
            expires_in: 900,
            ..Default::default()
        })
    }
    async fn create_sidecar_agent<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::CreateSidecarAgentRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::CreateSidecarAgentResponse> + Send + use<'a>,
    > {
        let agent = if r.id.is_empty() {
            Uuid::new_v4()
        } else {
            id(r.id)?
        };
        let name = crate::agent::validate_name(r.name)?;
        crate::agent::validate_webhook_signing_key(r.webhook_signing_key)?;
        let caps = crate::iam::capabilities::Capabilities::from_wire(
            r.to_owned_message()
                .capabilities
                .into_option()
                .unwrap_or_default(),
        )?;
        let secret = SecretString::from(r.webhook_signing_key);
        let sealed = self
            .0
            .encryption
            .seal(
                crate::encryption::SecretBinding {
                    resource_kind: "agent",
                    resource_id: agent,
                    name: "webhook_signing_key",
                },
                &secret,
            )?
            .into_bytes();
        drop(secret);
        let caps = serde_json::to_value(caps)
            .map_err(|_| connectrpc::ConnectError::invalid_argument("Invalid capabilities"))?;
        let rows = sqlx::query_file!(
            "../../queries/deployment/create_agent.sql",
            agent,
            name,
            sealed,
            caps
        )
        .execute(&self.0.pool)
        .await
        .map_err(Error::from)?
        .rows_affected();
        if rows == 0 {
            return Err(connectrpc::ConnectError::already_exists(
                "Agent ID already exists",
            ));
        }
        Response::ok(management::CreateSidecarAgentResponse {
            agent: crate::agent::rpc::project(&self.0.agents, self.0.agents.get(agent).await?)
                .await?
                .into(),
            ..Default::default()
        })
    }
}
struct Control(Deployments);
impl Control {
    async fn agent(&self, ctx: &RequestContext) -> Result<Uuid, connectrpc::ConnectError> {
        let token = ctx
            .headers()
            .get(http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or_else(|| {
                connectrpc::ConnectError::unauthenticated("Deployment token required")
            })?;
        self.0
            .authenticate(token)
            .await
            .map_err(|_| connectrpc::ConnectError::unauthenticated("Invalid deployment token"))
    }
}
impl SidecarService for Control {
    async fn watch(
        &self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::WatchRequest>,
    ) -> ServiceResult<
        connectrpc::ServiceStream<
            impl connectrpc::Encodable<ingress::WatchResponse> + Send + use<>,
        >,
    > {
        let agent = self.agent(&ctx).await?;
        let request = r.to_owned_message();
        let instance = self.0.register(agent, &request).await?;
        let service = self.0.clone();
        let mut configuration = service
            .channels
            .configuration
            .subscribe(&service.pool, "tilde_sidecar_configuration")
            .await
            .map_err(Error::from)?;
        let mut assignments = service
            .channels
            .assignments
            .subscribe(&service.pool, "tilde_sidecar_assignments")
            .await
            .map_err(Error::from)?;
        let mut directives = service
            .channels
            .directives
            .subscribe(&service.pool, "tilde_sidecar_directives")
            .await
            .map_err(Error::from)?;
        configuration.borrow_and_update();
        assignments.borrow_and_update();
        directives.borrow_and_update();
        Response::stream_ok(async_stream::try_stream! {
            let mut since = chrono::Utc::now();
            let snapshot = service.snapshot(agent).await?;
            yield ingress::WatchResponse { frame: Some(snapshot.into()), ..Default::default() };
            let mut sent = std::collections::BTreeSet::new();
            for directive in service.pending_directives(agent, instance).await? {
                sent.insert(directive.id.clone());
                yield ingress::WatchResponse { frame: Some(directive.into()), ..Default::default() };
            }
            enum Wake { Configuration, Assignments, Directives, Ping, Closed }
            loop {
                let wake = tokio::select! {
                    changed = configuration.changed() => if changed.is_ok() { Wake::Configuration } else { Wake::Closed },
                    changed = assignments.changed() => if changed.is_ok() { Wake::Assignments } else { Wake::Closed },
                    changed = directives.changed() => if changed.is_ok() { Wake::Directives } else { Wake::Closed },
                    _ = tokio::time::sleep(Duration::from_secs(10)) => Wake::Ping,
                };
                match wake {
                    Wake::Closed => break,
                    Wake::Ping => yield ingress::WatchResponse { frame: Some(ingress::Ping::default().into()), ..Default::default() },
                    Wake::Configuration => {
                        configuration.borrow_and_update();
                        yield ingress::WatchResponse { frame: Some(service.configuration(agent).await?.into()), ..Default::default() };
                    }
                    Wake::Assignments => {
                        assignments.borrow_and_update();
                        for (assignment, updated_at) in service.assignments_since(agent, since).await? {
                            since = since.max(updated_at);
                            yield ingress::WatchResponse { frame: Some(assignment.into()), ..Default::default() };
                        }
                    }
                    Wake::Directives => {
                        directives.borrow_and_update();
                        let pending = service.pending_directives(agent, instance).await?;
                        let ids: std::collections::BTreeSet<String> = pending.iter().map(|d| d.id.clone()).collect();
                        sent.retain(|id| ids.contains(id));
                        for directive in pending {
                            if sent.insert(directive.id.clone()) {
                                yield ingress::WatchResponse { frame: Some(directive.into()), ..Default::default() };
                            }
                        }
                    }
                }
            }
        })
    }
    async fn publish<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::PublishRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::PublishResponse> + Send + use<'a>> {
        let agent = self.agent(&ctx).await?;
        let request = r.to_owned_message();
        let instance = id(&request.instance_id)?;
        Response::ok(self.0.publish(agent, instance, request).await?)
    }
    async fn hydrate<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::HydrateRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::HydrateResponse> + Send + use<'a>> {
        let agent = self.agent(&ctx).await?;
        let request = r.to_owned_message();
        let instance = if request.instance_id.is_empty() {
            None
        } else {
            Some(id(&request.instance_id)?)
        };
        Response::ok(self.0.hydrate(agent, instance, request).await?)
    }
    async fn forward<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::ForwardRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::ForwardResponse> + Send + use<'a>> {
        let agent = self.agent(&ctx).await?;
        let request = r.to_owned_message();
        let thread = if request.thread_id.is_empty() {
            None
        } else {
            Some(id(&request.thread_id)?)
        };
        let result = match request.work {
            Some(ingress::forward_request::Work::Call(call)) => {
                self.0.forward(agent, thread, *call).await?
            }
            Some(ingress::forward_request::Work::ProviderEvent(event)) => {
                let event = *event;
                self.0
                    .forward_provider_event(
                        agent,
                        id(&event.connection_id)?,
                        event.event.into_option().ok_or_else(|| {
                            connectrpc::ConnectError::invalid_argument("Provider event required")
                        })?,
                    )
                    .await?;
                ingress::CallResult {
                    status: 204,
                    ..Default::default()
                }
            }
            None => {
                return Err(connectrpc::ConnectError::invalid_argument(
                    "Forward requires work",
                ));
            }
        };
        Response::ok(ingress::ForwardResponse {
            result: result.into(),
            ..Default::default()
        })
    }
    async fn upload_attachment<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::UploadAttachmentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::UploadAttachmentResponse> + Send + use<'a>>
    {
        let agent = self.agent(&ctx).await?;
        let request = r.to_owned_message();
        let attachment = request.attachment.into_option().ok_or_else(|| {
            connectrpc::ConnectError::invalid_argument("Attachment metadata required")
        })?;
        Response::ok(ingress::UploadAttachmentResponse {
            attachment: self
                .0
                .store_attachment(agent, attachment, request.content)
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn download_attachment<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::DownloadAttachmentRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<ingress::DownloadAttachmentResponse> + Send + use<'a>,
    > {
        let agent = self.agent(&ctx).await?;
        Response::ok(ingress::DownloadAttachmentResponse {
            content: self
                .0
                .fetch_attachment(agent, id(r.thread_id)?, id(r.attachment_id)?)
                .await?,
            ..Default::default()
        })
    }
    async fn relay<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::RelayRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::RelayResponse> + Send + use<'a>> {
        let agent = self.agent(&ctx).await?;
        Response::ok(ingress::RelayResponse {
            result: self.0.relay(agent, r.to_owned_message()).await?.into(),
            ..Default::default()
        })
    }
    async fn resolve_participant<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::ResolveParticipantRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<ingress::ResolveParticipantResponse> + Send + use<'a>,
    > {
        self.agent(&ctx).await?;
        Response::ok(self.0.resolve_participant(r.to_owned_message()).await?)
    }
}
