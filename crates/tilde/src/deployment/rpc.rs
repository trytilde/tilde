use super::{Deployments, id};
use crate::proto::tilde::{agent_event_ingress::v1 as ingress, management::v1 as management};
use crate::services::tilde::{
    agent_event_ingress::v1::SidecarService, management::v1::DeploymentService,
};
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use uuid::Uuid;
struct Rpc(Deployments);
pub fn management_router(service: Deployments) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(service))),
        1024 * 1024,
    )
}
pub fn agent_event_ingress_router(service: Deployments) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Control(service.clone()))),
        192 * 1024 * 1024,
    )
    .merge(super::attachments::router(service.clone()))
    .merge(
        axum::Router::new()
            .route(
                "/agents/{agent}/registry/{target}",
                axum::routing::get(agent_reference),
            )
            .with_state(service),
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
                    r.retention_days,
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
        .map_err(crate::error::Error::from)?
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
    async fn locate_conversation<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::LocateConversationRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<ingress::LocateConversationResponse> + Send + use<'a>,
    > {
        Response::ok(ingress::LocateConversationResponse {
            storage: self
                .0
                .locate(self.agent(&ctx).await?, id(r.thread_id)?)
                .await?,
            ..Default::default()
        })
    }
    async fn ingest_provider_event<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::IngestProviderEventRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<ingress::IngestProviderEventResponse> + Send + use<'a>,
    > {
        let body = r.to_owned_message();
        self.0
            .ingest_archived(
                self.agent(&ctx).await?,
                id(&body.connection_id)?,
                body.event.into_option().ok_or_else(|| {
                    connectrpc::ConnectError::invalid_argument("Provider event required")
                })?,
            )
            .await?;
        Response::ok(ingress::IngestProviderEventResponse::default())
    }

    async fn watch_commands(
        &self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::WatchCommandsRequest>,
    ) -> ServiceResult<
        connectrpc::ServiceStream<
            impl connectrpc::Encodable<ingress::WatchCommandsResponse> + Send + use<>,
        >,
    > {
        let agent = self.agent(&ctx).await?;
        let instance = id(r.instance_id)?;
        let service = self.0.clone();
        let notifications = crate::database::notifications::Notifications::default();
        let mut changes = notifications
            .subscribe(&service.pool, "tilde_sidecar_commands")
            .await
            .map_err(crate::error::Error::from)?;
        Response::stream_ok(async_stream::try_stream! {
            let _notifications=notifications;let mut delivered=std::collections::BTreeSet::new();
            loop {
                changes.borrow_and_update();
                let pending = service.pending_commands(agent,instance).await?;
                let keys = pending.iter().map(|c| (c.id.clone(),c.generation)).collect::<std::collections::BTreeSet<_>>();
                delivered.retain(|key| keys.contains(key));
                for command in pending {
                    if delivered.insert((command.id.clone(),command.generation)) { yield ingress::WatchCommandsResponse{command:command.into(),..Default::default()}; }
                }
                if changes.changed().await.is_err(){break;}
            }
        })
    }
    async fn get_invocation<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::GetInvocationRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::GetInvocationResponse> + Send + use<'a>>
    {
        Response::ok(ingress::GetInvocationResponse {
            invocation: self
                .0
                .invocation_request(
                    self.agent(&ctx).await?,
                    id(r.instance_id)?,
                    id(r.command_id)?,
                    r.generation as i64,
                )
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn acknowledge_command<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::AcknowledgeCommandRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<ingress::AcknowledgeCommandResponse> + Send + use<'a>,
    > {
        self.0
            .acknowledge(
                self.agent(&ctx).await?,
                id(r.instance_id)?,
                id(r.command_id)?,
                r.generation as i64,
            )
            .await?;
        Response::ok(ingress::AcknowledgeCommandResponse::default())
    }
    async fn complete_command<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::CompleteCommandRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::CompleteCommandResponse> + Send + use<'a>>
    {
        let body = r.to_owned_message();
        self.0
            .complete(
                self.agent(&ctx).await?,
                id(&body.instance_id)?,
                id(&body.command_id)?,
                body.generation as i64,
                &body.status,
                &body.pending_input_ids,
            )
            .await?;
        Response::ok(ingress::CompleteCommandResponse::default())
    }
    async fn report_activity<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::ReportActivityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::ReportActivityResponse> + Send + use<'a>>
    {
        self.0
            .report_reasoning(
                self.agent(&ctx).await?,
                id(r.instance_id)?,
                id(r.command_id)?,
                r.generation as i64,
                r.reasoning_delta,
            )
            .await?;
        Response::ok(ingress::ReportActivityResponse::default())
    }

    async fn register_sidecar<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::RegisterSidecarRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::RegisterSidecarResponse> + Send + use<'a>>
    {
        Response::ok(
            self.0
                .register(self.agent(&ctx).await?, r.to_owned_message())
                .await?,
        )
    }
    async fn heartbeat<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, ingress::HeartbeatRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::HeartbeatResponse> + Send + use<'a>>
    {
        Response::ok(
            self.0
                .heartbeat(self.agent(&ctx).await?, r.to_owned_message())
                .await?,
        )
    }
    async fn get_configuration<'a>(
        &'a self,
        ctx: RequestContext,
        _: ServiceRequest<'_, ingress::GetConfigurationRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<ingress::GetConfigurationResponse> + Send + use<'a>>
    {
        Response::ok(self.0.configuration(self.agent(&ctx).await?).await?)
    }
}

async fn agent_reference(
    axum::extract::State(service): axum::extract::State<Deployments>,
    axum::extract::Path((agent, target)): axum::extract::Path<(Uuid, Uuid)>,
    headers: http::HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let result = async {
        let token = headers
            .get(http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or(crate::error::Error::Denied)?;
        if service.authenticate(token).await? != agent {
            return Err(crate::error::Error::Denied);
        }
        let target = service.agents.get(target).await?;
        Ok::<_, crate::error::Error>(super::gateway::AgentReference {
            id: target.id,
            name: target.name,
        })
    }
    .await;
    match result {
        Ok(reference) => axum::Json(reference).into_response(),
        Err(error) => connectrpc::ConnectError::from(error).into_response(),
    }
}
