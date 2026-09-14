use crate::chat::{Chat, id};
use crate::proto::tilde::ingress::v1 as management;
use connectrpc::{
    ConnectError, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use std::sync::Arc;
struct Rpc(Chat);
pub fn router(chat: Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(chat))),
        192 * 1024 * 1024,
    )
}
use crate::services::tilde::ingress::v1::ChatService;

impl ChatService for Rpc {
    async fn list_threads<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, management::ListThreadsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::ListThreadsResponse> + Send + use<'a>>
    {
        let agent = ctx
            .extensions()
            .get::<crate::deployment::tokens::IngressClaims>()
            .map(|scope| scope.agent_id)
            .ok_or_else(|| ConnectError::unauthenticated("Ingress scope required"))?;
        let (threads, next_page_token) = self
            .0
            .list_ingress_threads(Some(agent), r.page_token, r.page_size)
            .await?;
        Response::ok(management::ListThreadsResponse {
            threads,
            next_page_token,
            ..Default::default()
        })
    }
    async fn add_participant<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::AddParticipantRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::AddParticipantResponse> + Send + use<'a>,
    > {
        Response::ok(management::AddParticipantResponse {
            thread: self
                .0
                .add_participant(r.to_owned_message().into())
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn remove_participant<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::RemoveParticipantRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::RemoveParticipantResponse> + Send + use<'a>,
    > {
        Response::ok(management::RemoveParticipantResponse {
            thread: self
                .0
                .remove_participant(id(r.thread_id)?, id(r.participant_id)?)
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn set_typing<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::SetTypingRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::SetTypingResponse> + Send + use<'a>>
    {
        let roster = self.0.thread(id(r.thread_id)?).await?;
        if !roster
            .participants
            .iter()
            .any(|p| p.id == r.participant_id && p.user_id.is_some())
        {
            return Err(ConnectError::permission_denied(
                "Agent typing requires its invocation",
            ));
        }
        self.0
            .typing(id(r.thread_id)?, id(r.participant_id)?, r.typing)
            .await?;
        Response::ok(management::SetTypingResponse::default())
    }
    async fn list_activity<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, management::ListActivityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::ListActivityResponse> + Send + use<'a>>
    {
        let (events, next_cursor, has_more) = self
            .0
            .ingress_activity(id(r.thread_id)?, r.after_cursor, r.limit)
            .await?;
        Response::ok(management::ListActivityResponse {
            events,
            next_cursor,
            has_more,
            ..Default::default()
        })
    }
    async fn upload_attachment<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, management::UploadAttachmentRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::UploadAttachmentResponse> + Send + use<'a>,
    > {
        Response::ok(management::UploadAttachmentResponse {
            attachment: self
                .0
                .upload_attachment(r.to_owned_message().into())
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn download_attachment<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, management::DownloadAttachmentRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::DownloadAttachmentResponse> + Send + use<'a>,
    > {
        let file = self
            .0
            .download_attachment(id(r.thread_id)?, id(r.attachment_id)?)
            .await?;
        Response::ok(management::DownloadAttachmentResponse {
            attachment: file.attachment.into(),
            content: file.content,
            ..Default::default()
        })
    }
    async fn create_user<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::CreateUserRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::CreateUserResponse> + Send + use<'a>>
    {
        let _ = _ctx;
        Response::ok(management::CreateUserResponse {
            user: self.0.create_user(request.name).await?.into(),
            ..Default::default()
        })
    }
    async fn create_thread<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::CreateThreadRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::CreateThreadResponse> + Send + use<'a>>
    {
        let _ = _ctx;
        Response::ok(management::CreateThreadResponse {
            thread: self
                .0
                .create_thread(request.to_owned_message().into())
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn get_thread<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetThreadRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::GetThreadResponse> + Send + use<'a>>
    {
        Response::ok(management::GetThreadResponse {
            thread: self.0.thread(id(request.id)?).await?.into(),
            ..Default::default()
        })
    }
    async fn post_message<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::PostMessageRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::PostMessageResponse> + Send + use<'a>>
    {
        let _ = _ctx;
        Response::ok(management::PostMessageResponse {
            message: self.0.post(request.to_owned_message().into()).await?.into(),
            ..Default::default()
        })
    }
    async fn list_messages<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListMessagesRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::ListMessagesResponse> + Send + use<'a>>
    {
        let before = if request.before_message_id.is_empty() {
            None
        } else {
            Some(id(request.before_message_id)?)
        };
        let page = self
            .0
            .message_page(id(request.thread_id)?, before, request.limit)
            .await?;

        Response::ok(management::ListMessagesResponse {
            messages: page.messages,
            next_page_token: page.next_page_token,
            ..Default::default()
        })
    }
    async fn start_run<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::StartRunRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::StartRunResponse> + Send + use<'a>>
    {
        Response::ok(management::StartRunResponse {
            run: self
                .0
                .start_run(request.to_owned_message().into())
                .await?
                .into(),
            ..Default::default()
        })
    }
    async fn resume_run<'a>(
        &'a self,
        __ctx: RequestContext,
        request: ServiceRequest<'_, management::ResumeRunRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::ResumeRunResponse> + Send + use<'a>>
    {
        Response::ok(management::ResumeRunResponse {
            run: self.0.resume_run(id(request.id)?).await?.into(),
            ..Default::default()
        })
    }
    async fn get_run<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetRunRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<management::GetRunResponse> + Send + use<'a>>
    {
        let _ = _ctx;
        Response::ok(management::GetRunResponse {
            run: self.0.run(id(request.id)?).await?.into(),
            ..Default::default()
        })
    }
    async fn suspend_invocation<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, management::SuspendInvocationRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::SuspendInvocationResponse> + Send + use<'a>,
    > {
        self.0.suspend_invocation(id(r.invocation_id)?).await?;
        Response::ok(management::SuspendInvocationResponse::default())
    }
    async fn cancel_invocation<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::CancelInvocationRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::CancelInvocationResponse> + Send + use<'a>,
    > {
        let _ = _ctx;
        self.0.cancel_invocation(id(request.invocation_id)?).await?;
        Response::ok(management::CancelInvocationResponse::default())
    }
    async fn steer_invocation<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SteerInvocationRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<management::SteerInvocationResponse> + Send + use<'a>,
    > {
        let _ = _ctx;
        self.0
            .steer_invocation(request.to_owned_message().into())
            .await?;
        Response::ok(management::SteerInvocationResponse::default())
    }
    async fn watch_thread(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::WatchThreadRequest>,
    ) -> ServiceResult<
        ServiceStream<impl connectrpc::Encodable<management::WatchThreadResponse> + Send + use<>>,
    > {
        let thread = id(request.thread_id)?;
        let mut stream = self.0.watch_ingress(thread, request.after_cursor).await?;
        Response::stream_ok(async_stream::try_stream! {
            use futures::StreamExt;
            while let Some(value)=stream.next().await {
                let (activity,cursor)=value?;
                yield management::WatchThreadResponse{activity:activity.into(),cursor,..Default::default()};
            }
        })
    }
}

impl From<management::CreateThreadRequest> for crate::chat::CreateThread {
    fn from(r: management::CreateThreadRequest) -> Self {
        Self {
            title: r.title,
            participants: r.participants,
            primary_agent_id: r.primary_agent_id,
        }
    }
}

impl From<management::PostMessageRequest> for crate::chat::PostMessage {
    fn from(r: management::PostMessageRequest) -> Self {
        Self {
            id: r.id,
            thread_id: r.thread_id,
            participant_id: r.participant_id,
            text: r.text,
            addressed_participant_ids: r.addressed_participant_ids,
            in_reply_to_message_id: r.in_reply_to_message_id,
            attachment_ids: r.attachment_ids,
        }
    }
}

impl From<management::StartRunRequest> for crate::chat::StartRun {
    fn from(r: management::StartRunRequest) -> Self {
        Self {
            thread_id: r.thread_id,
            agent_id: r.agent_id,
            objective: r.objective,
            goal_id: r.goal_id,
            idempotency_key: r.idempotency_key,
        }
    }
}

impl From<management::SteerInvocationRequest> for crate::chat::SteerInvocation {
    fn from(r: management::SteerInvocationRequest) -> Self {
        Self {
            invocation_id: r.invocation_id,
            input_id: r.input_id,
            text: r.text,
        }
    }
}

impl From<management::AddParticipantRequest> for crate::chat::AddParticipant {
    fn from(r: management::AddParticipantRequest) -> Self {
        Self {
            thread_id: r.thread_id,
            participant: r.participant.into_option(),
        }
    }
}

impl From<management::UploadAttachmentRequest> for crate::chat::UploadAttachment {
    fn from(r: management::UploadAttachmentRequest) -> Self {
        Self {
            id: r.id,
            thread_id: r.thread_id,
            filename: r.filename,
            media_type: r.media_type,
            content: r.content,
        }
    }
}
