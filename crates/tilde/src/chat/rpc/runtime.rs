use crate::chat::{Chat, Scope, id};
use crate::iam::capabilities::Capability;
use crate::proto::tilde::runtime::v1 as runtime_pb;

use super::{capability, invocation_scope};
use crate::services::tilde::runtime::v1::ChatService;
use connectrpc::{
    ConnectError, InboundStream, RequestContext, Response, ServiceRequest, ServiceResult,
};
use secrecy::SecretString;
use std::sync::Arc;
struct Rpc(Chat, Arc<crate::chat::tools::Registry>);
impl Rpc {
    async fn scope(&self, ctx: &RequestContext) -> Result<Scope, ConnectError> {
        invocation_scope(&self.0, ctx).await
    }
}
pub fn router(chat: Chat) -> axum::Router {
    let tools = crate::chat::tools::Registry::for_chat(&chat);
    router_with_tools(chat, tools)
}
pub fn router_with_tools(chat: Chat, tools: crate::chat::tools::Registry) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(chat, Arc::new(tools)))),
        192 * 1024 * 1024,
    )
}
impl ChatService for Rpc {
    async fn set_typing<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, runtime_pb::SetTypingRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::SetTypingResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope
            .capabilities
            .require(Capability::ToolsInvoke, "sendMessage")?;
        self.0
            .typing(scope.thread_id, scope.participant_id, r.typing)
            .await?;
        Response::ok(runtime_pb::SetTypingResponse::default())
    }
    async fn cache_converted_messages<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, runtime_pb::CacheConvertedMessagesRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<runtime_pb::CacheConvertedMessagesResponse> + Send + use<'a>,
    > {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::ThreadRead, "")?;
        self.0
            .cache_converted_messages(
                &scope,
                r.to_owned_message()
                    .messages
                    .into_iter()
                    .map(|m| crate::chat::ConvertedMessage {
                        message_id: m.message_id,
                        message_json: m.message_json,
                    })
                    .collect(),
            )
            .await?;
        Response::ok(runtime_pb::CacheConvertedMessagesResponse {
            success: true,
            ..Default::default()
        })
    }
    async fn hydrate_converted_messages<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, runtime_pb::HydrateConvertedMessagesRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<runtime_pb::HydrateConvertedMessagesResponse> + Send + use<'a>,
    > {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::ThreadRead, "")?;
        Response::ok(runtime_pb::HydrateConvertedMessagesResponse {
            messages: self
                .0
                .hydrate_converted_messages(
                    scope.agent_id,
                    scope.thread_id,
                    &r.to_owned_message().message_ids,
                )
                .await?
                .into_iter()
                .map(cache_wire)
                .collect(),
            ..Default::default()
        })
    }
    async fn report_tool_call<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, runtime_pb::ReportToolCallRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<runtime_pb::ReportToolCallResponse> + Send + use<'a>,
    > {
        let scope = self.scope(&ctx).await?;
        let tool = r
            .to_owned_message()
            .tool_call
            .into_option()
            .ok_or_else(|| ConnectError::invalid_argument("Tool call required"))?;
        scope
            .capabilities
            .require(Capability::ToolsInvoke, &tool.name)?;
        if !tool.provider_id.is_empty() {
            return Err(ConnectError::invalid_argument(
                "Agent-local calls cannot claim a provider",
            ));
        }
        self.0.report_tool_call(&scope, tool).await?;
        Response::ok(runtime_pb::ReportToolCallResponse::default())
    }
    async fn renew_connect_token<'a>(
        &'a self,
        ctx: RequestContext,
        _: ServiceRequest<'_, runtime_pb::RenewConnectTokenRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<runtime_pb::RenewConnectTokenResponse> + Send + use<'a>,
    > {
        use secrecy::ExposeSecret;
        let token = self.0.tokens.renew(capability(&ctx)?).await?;
        Response::ok(runtime_pb::RenewConnectTokenResponse {
            token: token.expose_secret().to_owned(),
            expires_in: crate::iam::tokens::TOKEN_TTL_SECONDS as u32,
            ..Default::default()
        })
    }
    async fn list_goals<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::ListGoalsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::ListGoalsResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::WorkRead, "")?;
        let _ = request;
        Response::ok(runtime_pb::ListGoalsResponse {
            goals: self.0.goals(&scope).await?,
            ..Default::default()
        })
    }
    async fn create_goal<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::CreateGoalRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::CreateGoalResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::WorkWrite, "")?;
        Response::ok(runtime_pb::CreateGoalResponse {
            goal: self
                .0
                .create_goal(&scope, request.to_owned_message().into())
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn update_goal<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::UpdateGoalRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::UpdateGoalResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::WorkWrite, "")?;
        Response::ok(runtime_pb::UpdateGoalResponse {
            goal: self
                .0
                .update_goal(&scope, request.to_owned_message().into())
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn list_tasks<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::ListTasksRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::ListTasksResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::WorkRead, "")?;
        let _ = request;
        Response::ok(runtime_pb::ListTasksResponse {
            tasks: self.0.tasks(&scope).await?,
            ..Default::default()
        })
    }
    async fn create_task<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::CreateTaskRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::CreateTaskResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::WorkWrite, "")?;
        Response::ok(runtime_pb::CreateTaskResponse {
            task: self
                .0
                .create_task(&scope, request.to_owned_message().into())
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn update_task<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::UpdateTaskRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::UpdateTaskResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::WorkWrite, "")?;
        Response::ok(runtime_pb::UpdateTaskResponse {
            task: self
                .0
                .update_task(&scope, request.to_owned_message().into())
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn set_run_status<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime_pb::SetRunStatusRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::SetRunStatusResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::RunUpdate, "")?;
        self.0.set_run_status(&scope, request.status).await?;
        Response::ok(runtime_pb::SetRunStatusResponse::default())
    }
    async fn list_tools<'a>(
        &'a self,
        ctx: RequestContext,
        _: ServiceRequest<'_, runtime_pb::ListToolsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::ListToolsResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        Response::ok(runtime_pb::ListToolsResponse {
            tools: self.1.list(&scope).await?,
            ..Default::default()
        })
    }
    async fn invoke_tool<'a>(
        &'a self,
        ctx: RequestContext,
        requests: InboundStream<runtime_pb::InvokeToolRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::InvokeToolResponse> + Send + use<'a>>
    {
        let capability = SecretString::from(capability(&ctx)?.to_owned());
        let output = self.1.invoke(self.0.clone(), capability, requests).await?;
        let output_json = serde_json::to_string(&output)
            .map_err(|_| ConnectError::internal("Invalid tool result"))?;
        if output_json.len() > 1024 * 1024 {
            return Err(ConnectError::resource_exhausted("Tool result is too large"));
        }
        Response::ok(runtime_pb::InvokeToolResponse {
            output_json,
            ..Default::default()
        })
    }
    async fn get_thread<'a>(
        &'a self,
        ctx: RequestContext,
        _: ServiceRequest<'_, runtime_pb::GetThreadRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::GetThreadResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::ThreadRead, "")?;
        Response::ok(runtime_pb::GetThreadResponse {
            thread: self.0.thread(scope.thread_id).await?.into(),
            ..Default::default()
        })
    }
    async fn list_messages<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, runtime_pb::ListMessagesRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::ListMessagesResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::ThreadRead, "")?;
        let before = if r.before_message_id.is_empty() {
            None
        } else {
            Some(id(r.before_message_id)?)
        };
        let page = self
            .0
            .invocation_message_page(scope.thread_id, before, r.limit, Some(scope.id))
            .await?;
        let cached = self
            .0
            .hydrate_converted_messages(
                scope.agent_id,
                scope.thread_id,
                &page
                    .messages
                    .iter()
                    .map(|m| m.id.clone())
                    .collect::<Vec<_>>(),
            )
            .await?;
        Response::ok(runtime_pb::ListMessagesResponse {
            messages: page.messages,
            next_page_token: page.next_page_token,
            cached_messages: cached.into_iter().map(cache_wire).collect(),
            ..Default::default()
        })
    }
    async fn start_run<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, runtime_pb::StartRunRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<runtime_pb::StartRunResponse> + Send + use<'a>>
    {
        let scope = self.scope(&ctx).await?;
        scope
            .capabilities
            .require(Capability::AgentsInvoke, r.agent_id)?;
        let r = r.to_owned_message();
        Response::ok(runtime_pb::StartRunResponse {
            run: self
                .0
                .start_run(crate::chat::StartRun {
                    thread_id: scope.thread_id.to_string(),
                    agent_id: r.agent_id,
                    objective: r.objective,
                    goal_id: None,
                    idempotency_key: r.idempotency_key,
                })
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn upload_attachment<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, runtime_pb::UploadAttachmentRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<runtime_pb::UploadAttachmentResponse> + Send + use<'a>,
    > {
        let scope = self.scope(&ctx).await?;
        scope
            .capabilities
            .require(Capability::ToolsInvoke, "sendMessage")?;
        let r = r.to_owned_message();
        Response::ok(runtime_pb::UploadAttachmentResponse {
            attachment: self
                .0
                .upload_attachment(crate::chat::UploadAttachment {
                    id: r.id,
                    thread_id: scope.thread_id.to_string(),
                    filename: r.filename,
                    media_type: r.media_type,
                    content: r.content,
                })
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn download_attachment<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, runtime_pb::DownloadAttachmentRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<runtime_pb::DownloadAttachmentResponse> + Send + use<'a>,
    > {
        let scope = self.scope(&ctx).await?;
        scope.capabilities.require(Capability::ThreadRead, "")?;
        let file = self
            .0
            .download_attachment(scope.thread_id, id(r.attachment_id)?)
            .await?;
        Response::ok(runtime_pb::DownloadAttachmentResponse {
            attachment: file.attachment.into(),
            content: file.content,
            ..Default::default()
        })
    }
}

impl From<runtime_pb::CreateGoalRequest> for crate::chat::CreateGoal {
    fn from(r: runtime_pb::CreateGoalRequest) -> Self {
        Self {
            id: r.id,
            objective: r.objective,
        }
    }
}

impl From<runtime_pb::UpdateGoalRequest> for crate::chat::UpdateGoal {
    fn from(r: runtime_pb::UpdateGoalRequest) -> Self {
        Self {
            id: r.id,
            status: r.status,
        }
    }
}

impl From<runtime_pb::CreateTaskRequest> for crate::chat::CreateTask {
    fn from(r: runtime_pb::CreateTaskRequest) -> Self {
        Self {
            id: r.id,
            title: r.title,
            goal_id: r.goal_id,
            dependency_ids: r.dependency_ids,
        }
    }
}

impl From<runtime_pb::UpdateTaskRequest> for crate::chat::UpdateTask {
    fn from(r: runtime_pb::UpdateTaskRequest) -> Self {
        Self {
            id: r.id,
            status: r.status,
            blocked_reason: r.blocked_reason,
        }
    }
}

fn cache_wire(c: crate::chat::ConvertedMessage) -> runtime_pb::CachedAgentRepresentation {
    runtime_pb::CachedAgentRepresentation {
        message_id: c.message_id,
        message_json: c.message_json,
        ..Default::default()
    }
}
