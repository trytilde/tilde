use super::{Deployments, id};
use crate::error::Error;
use crate::proto::tilde::{agent_event_ingress::v1 as ingress, management::v1 as management};
use crate::services::tilde::{
    agent_event_ingress::v1::SidecarService, management::v1::DeploymentService,
};
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::ExposeSecret;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
struct Rpc(Deployments);
/// Sized for RegisterDeployment's declarations: skill text travels inline and larger or
/// binary files come through UploadDeploymentFile (10 MiB each) first.
pub fn management_router(service: Deployments) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(service))),
        32 * 1024 * 1024,
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
    async fn missing_deployment_files<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::MissingDeploymentFilesRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::MissingDeploymentFilesResponse> + Send + use<'a>,
    > {
        let r = r.to_owned_message();
        Response::ok(management::MissingDeploymentFilesResponse {
            sha256: self.0.skills().missing_files(&r.sha256).await?,
            ..Default::default()
        })
    }
    async fn upload_deployment_file<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::UploadDeploymentFileRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::UploadDeploymentFileResponse> + Send + use<'a>,
    > {
        let data = r.data.to_vec();
        Response::ok(management::UploadDeploymentFileResponse {
            sha256: self.0.skills().upload_file(data).await?,
            ..Default::default()
        })
    }
    async fn get_deployment_contents<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::GetDeploymentContentsRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::GetDeploymentContentsResponse> + Send + use<'a>,
    > {
        let agent = id(r.agent_id)?;
        Response::ok(self.0.contents(agent, id(r.deployment_id)?).await?)
    }
    async fn get_deployment<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::GetDeploymentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::GetDeploymentResponse> + Send + use<'a>>
    {
        let agent = id(r.agent_id)?;
        Response::ok(management::GetDeploymentResponse {
            deployment: self.0.get(agent).await?.into(),
            instances: self.0.instances(agent).await?,
            deployments: self.0.deployments(agent).await?,
            ..Default::default()
        })
    }
    async fn set_deployment_weights<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::SetDeploymentWeightsRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::SetDeploymentWeightsResponse> + Send + use<'a>,
    > {
        let r = r.to_owned_message();
        let agent = id(&r.agent_id)?;
        let weights = r
            .weights
            .iter()
            .map(|w| Ok((id(&w.deployment_id)?, w.weight)))
            .collect::<Result<Vec<_>, Error>>()?;
        self.0.set_weights(agent, weights).await?;
        Response::ok(management::SetDeploymentWeightsResponse {
            deployment: self.0.get(agent).await?.into(),
            deployments: self.0.deployments(agent).await?,
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
                    r.failure_mode.as_known().ok_or_else(|| {
                        connectrpc::ConnectError::invalid_argument("Invalid failure policy")
                    })?,
                    r.routing.as_known().unwrap_or_default(),
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
        let token = self
            .0
            .issue_token(id(r.agent_id)?, id(r.deployment_id)?)
            .await?;
        Response::ok(management::IssueDeploymentTokenResponse {
            token: token.expose_secret().into(),
            ..Default::default()
        })
    }
    async fn register_deployment<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::RegisterDeploymentRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::RegisterDeploymentResponse> + Send + use<'a>,
    > {
        let agent = id(r.agent_id)?;
        let request = r.to_owned_message();
        let (deployment, token, created) = self
            .0
            .register_deployment(
                agent,
                super::RegisterDeployment {
                    source: request.source.as_known().unwrap_or_default(),
                    target: request.target.as_known().unwrap_or_default(),
                    target_reference: request.target_reference,
                    repository: request.repository,
                    commit_sha: request.commit_sha,
                    external_id: request.external_id,
                    label: request.label,
                    commit_message: request.commit_message,
                    branch: request.branch,
                    commit_author: request.commit_author,
                    declarations: request.declarations.into_option().unwrap_or_default(),
                },
            )
            .await?;
        Response::ok(management::RegisterDeploymentResponse {
            deployment: deployment.into(),
            token: token
                .map(|t| t.expose_secret().to_owned())
                .unwrap_or_default(),
            created,
            ..Default::default()
        })
    }
    async fn promote_deployment<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::PromoteDeploymentRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::PromoteDeploymentResponse> + Send + use<'a>,
    > {
        Response::ok(management::PromoteDeploymentResponse {
            deployment: self
                .0
                .promote(id(r.agent_id)?, id(r.deployment_id)?)
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn retire_deployment<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::RetireDeploymentRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::RetireDeploymentResponse> + Send + use<'a>,
    > {
        Response::ok(management::RetireDeploymentResponse {
            deployment: self
                .0
                .retire(id(r.agent_id)?, id(r.deployment_id)?)
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn issue_ingress_token<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::IssueIngressTokenRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::IssueIngressTokenResponse> + Send + use<'a>,
    > {
        let token = self
            .0
            .issue_ingress_token(id(r.agent_id)?, r.thread_id.map(str::to_owned))
            .await?;
        Response::ok(management::IssueIngressTokenResponse {
            token: token.expose_secret().into(),
            expires_in: 900,
            ..Default::default()
        })
    }
}
struct Control(Deployments);
fn bearer(ctx: &RequestContext) -> Option<&str> {
    ctx.headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}
impl Control {
    /// The sidecar protocol accepts sidecar deployment tokens only.
    async fn authenticated(
        &self,
        ctx: &RequestContext,
    ) -> Result<super::Authenticated, connectrpc::ConnectError> {
        let token = bearer(ctx).ok_or_else(|| {
            connectrpc::ConnectError::unauthenticated("Deployment token required")
        })?;
        let auth =
            self.0.authenticate(token).await.map_err(|_| {
                connectrpc::ConnectError::unauthenticated("Invalid deployment token")
            })?;
        if auth.target != crate::proto::tilde::types::v1::DeploymentTarget::Sidecar {
            return Err(connectrpc::ConnectError::permission_denied(
                "This deployment token is not for a sidecar",
            ));
        }
        Ok(auth)
    }
    async fn agent(&self, ctx: &RequestContext) -> Result<Uuid, connectrpc::ConnectError> {
        Ok(self.authenticated(ctx).await?.agent)
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
        let auth = self.authenticated(&ctx).await?;
        let agent = auth.agent;
        let token = bearer(&ctx).unwrap_or_default().to_owned();
        let request = r.to_owned_message();
        let instance = self.0.register(agent, auth.deployment, &request).await?;
        let connection = self.0.open_watch(agent, instance).await?;
        let service = self.0.clone();
        let mut configuration = service
            .channels
            .configuration
            .subscribe(&service.pool, "tilde_sidecar_configuration")
            .await
            .map_err(Error::from)?;
        let mut leases = service
            .channels
            .leases
            .subscribe(&service.pool, "tilde_sidecar_leases")
            .await
            .map_err(Error::from)?;
        // One channel for every agent: each wake re-reads this instance's pending
        // directives (one indexed query). Scope the channel by agent if hosts grow into
        // the hundreds.
        let mut directives = service
            .channels
            .directives
            .subscribe(&service.pool, "tilde_sidecar_directives")
            .await
            .map_err(Error::from)?;
        configuration.borrow_and_update();
        leases.borrow_and_update();
        directives.borrow_and_update();
        Response::stream_ok(async_stream::try_stream! {
            let _connection = connection;
            let mut since = chrono::Utc::now();
            let snapshot = service.snapshot(agent, auth.deployment, instance).await?;
            yield ingress::WatchResponse { frame: Some(snapshot.into()), ..Default::default() };
            let mut sent = std::collections::BTreeSet::new();
            for directive in service.pending_directives(agent, instance).await? {
                sent.insert(directive.id.clone());
                yield ingress::WatchResponse { frame: Some(directive.into()), ..Default::default() };
            }
            enum Wake { Configuration, Leases, Directives, Ping, Closed }
            loop {
                let wake = tokio::select! {
                    changed = configuration.changed() => if changed.is_ok() { Wake::Configuration } else { Wake::Closed },
                    changed = leases.changed() => if changed.is_ok() { Wake::Leases } else { Wake::Closed },
                    changed = directives.changed() => if changed.is_ok() { Wake::Directives } else { Wake::Closed },
                    _ = tokio::time::sleep(Duration::from_secs(10)) => Wake::Ping,
                };
                match wake {
                    Wake::Closed => break,
                    Wake::Ping => {
                        // A rotated or revoked token ends the stream instead of living on until TCP notices.
                        if service.authenticate(&token).await.ok().map(|a| a.deployment) != Some(auth.deployment) { break; }
                        yield ingress::WatchResponse { frame: Some(ingress::Ping::default().into()), ..Default::default() }
                    }
                    Wake::Configuration => {
                        configuration.borrow_and_update();
                        yield ingress::WatchResponse { frame: Some(service.configuration(agent).await?.into()), ..Default::default() };
                    }
                    Wake::Leases => {
                        leases.borrow_and_update();
                        // Look back past any transaction that started before the last send but committed after it; replicas ignore repeats.
                        for (lease, updated_at) in service.leases_since(agent, since - chrono::Duration::seconds(30)).await? {
                            since = since.max(updated_at);
                            yield ingress::WatchResponse { frame: Some(lease.into()), ..Default::default() };
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
        let auth = self.authenticated(&ctx).await?;
        let request = r.to_owned_message();
        let instance = id(&request.instance_id)?;
        Response::ok(
            self.0
                .publish(auth.agent, auth.deployment, instance, request)
                .await?,
        )
    }
    async fn hydrate<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::HydrateRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::HydrateResponse> + Send + use<'a>> {
        let auth = self.authenticated(&ctx).await?;
        let request = r.to_owned_message();
        let instance = if request.instance_id.is_empty() {
            None
        } else {
            Some(id(&request.instance_id)?)
        };
        Response::ok(
            self.0
                .hydrate(auth.agent, auth.deployment, instance, request)
                .await?,
        )
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
        if let Some(directive) = request.provider_event.into_option() {
            let event = directive.event.into_option().ok_or_else(|| {
                connectrpc::ConnectError::invalid_argument("Forward requires a provider event")
            })?;
            self.0
                .forward_provider_event(agent, id(&directive.connection_id)?, event)
                .await?;
            return Response::ok(ingress::ForwardResponse {
                result: ingress::CallResult {
                    status: 204,
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            });
        }
        let call = request
            .call
            .into_option()
            .ok_or_else(|| connectrpc::ConnectError::invalid_argument("Forward requires a call"))?;
        let result = self.0.forward(agent, thread, call).await?;
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
