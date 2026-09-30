use super::{
    credentials::{member, scope},
    db,
};
use crate::chat::{Chat, id};
use crate::proto::tilde::provider::tilde::v1 as wire;
use crate::proto::tilde::types::v1 as types;
use connectrpc::{
    ConnectError, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use std::sync::Arc;
struct Rpc(Chat);
pub(crate) fn router(chat: Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(chat.clone()))),
        192 * 1024 * 1024,
    )
    .layer(axum::middleware::from_fn_with_state(
        chat.clone(),
        identity_guard,
    ))
}
use crate::services::tilde::provider::tilde::v1::ChatService;

/// An activity as end users see it: a hidden tool call not at all, a summary-only one without
/// its input, output or error. Traces and management views keep the full call.
fn for_end_user(mut activity: types::Activity) -> Option<types::Activity> {
    if let Some(types::activity::Detail::ToolCall(tool)) = &mut activity.detail {
        match tool.display.as_known() {
            Some(types::ToolDisplay::TOOL_DISPLAY_HIDDEN) => return None,
            Some(types::ToolDisplay::TOOL_DISPLAY_SUMMARY) => {
                tool.input_json.clear();
                tool.output_json.clear();
                tool.input_delta.clear();
                tool.error.clear();
            }
            _ => {}
        }
    }
    Some(activity)
}

impl ChatService for Rpc {
    async fn search_messages<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, wire::SearchMessagesRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::SearchMessagesResponse> + Send + use<'a>>
    {
        if r.query.is_empty() || r.query.chars().count() > 256 {
            return Err(ConnectError::invalid_argument(
                "Search must be 1–256 characters",
            ));
        }
        let size = if r.page_size == 0 {
            30
        } else {
            r.page_size.min(100)
        } as usize;
        let after = if r.page_token.is_empty() {
            None
        } else {
            Some(id(r.page_token)?)
        };
        let mut ids = db::search_messages(
            &self.0.pg()?.get().await?,
            id(r.thread_id)?,
            r.query,
            after,
            size as i64 + 1,
        )
        .await
        .map_err(crate::chat::ChatError::from)?;
        let more = ids.len() > size;
        ids.truncate(size);
        let next_page_token = if more {
            ids.last().map(ToString::to_string).unwrap_or_default()
        } else {
            String::new()
        };
        let mut messages = vec![];
        for id in ids {
            messages.push(self.0.message(id).await?);
        }
        Response::ok(wire::SearchMessagesResponse {
            messages,
            next_page_token,
            ..Default::default()
        })
    }

    async fn get_identity<'a>(
        &'a self,
        ctx: RequestContext,
        _r: ServiceRequest<'_, wire::GetIdentityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetIdentityResponse> + Send + use<'a>> {
        let claims = scope(&ctx)?;
        let user = claims
            .user_id
            .ok_or_else(|| ConnectError::permission_denied("Identity required"))?;
        let row = crate::chat::db::user_get_opt(&self.0.pg()?.get().await?, user)
            .await
            .map_err(crate::chat::ChatError::from)?
            .ok_or(crate::chat::ChatError::NotFound)?;
        Response::ok(wire::GetIdentityResponse {
            user: crate::proto::tilde::types::v1::User {
                id: row.id.to_string(),
                name: row.name,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
    }

    async fn list_agents<'a>(
        &'a self,
        ctx: RequestContext,
        _r: ServiceRequest<'_, wire::ListAgentsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListAgentsResponse> + Send + use<'a>> {
        let claims = scope(&ctx)?;
        let agent = db::agent(&self.0.pg()?.get().await?, claims.agent_id)
            .await
            .map_err(crate::chat::ChatError::from)?
            .ok_or(crate::chat::ChatError::NotFound)?;
        Response::ok(wire::ListAgentsResponse {
            agents: vec![wire::ChatAgent {
                id: agent.id.to_string(),
                name: agent.name,
                ..Default::default()
            }],
            ..Default::default()
        })
    }

    async fn list_sessions<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, wire::ListSessionsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListSessionsResponse> + Send + use<'a>>
    {
        let (sessions, next_page_token) = self
            .0
            .tilde_sessions(&scope(&ctx)?, r.query, r.page_token, r.page_size)
            .await?;
        Response::ok(wire::ListSessionsResponse {
            sessions,
            next_page_token,
            ..Default::default()
        })
    }

    async fn get_session<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, wire::GetSessionRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetSessionResponse> + Send + use<'a>> {
        let thread = id(r.thread_id)?;
        let session = self.0.tilde_session(&scope(&ctx)?, thread).await?;
        let (cursor, sequence, messages, next_message_token) = if r.include_history {
            let (cursor, sequence, page) = self.0.native_history_snapshot(thread).await?;
            (cursor, sequence, page.messages, page.next_page_token)
        } else {
            let (cursor, sequence) = self.0.current_ingress_cursor(thread).await?;
            (cursor, sequence, vec![], String::new())
        };
        let recent_activity = if r.include_history {
            self.0
                .activity_page(thread, sequence.saturating_sub(100).max(0), 100)
                .await?
                .events
                .into_iter()
                .filter(|event| event.sequence <= sequence)
                .filter_map(for_end_user)
                .collect()
        } else {
            vec![]
        };
        Response::ok(wire::GetSessionResponse {
            session: session.into(),
            cursor,
            sequence,
            messages,
            next_message_token,
            recent_activity,
            ..Default::default()
        })
    }

    async fn rename_session<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, wire::RenameSessionRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::RenameSessionResponse> + Send + use<'a>>
    {
        self.0.rename_session(id(r.thread_id)?, r.title).await?;
        Response::ok(wire::RenameSessionResponse::default())
    }

    async fn set_read_state<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, wire::SetReadStateRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::SetReadStateResponse> + Send + use<'a>>
    {
        let user = scope(&ctx)?
            .user_id
            .ok_or_else(|| ConnectError::permission_denied("Identity required"))?;
        self.0
            .set_read_state(id(r.thread_id)?, user, r.through_sequence, r.unread)
            .await?;
        Response::ok(wire::SetReadStateResponse::default())
    }

    async fn list_queued_messages<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, wire::ListQueuedMessagesRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListQueuedMessagesResponse> + Send + use<'a>>
    {
        Response::ok(wire::ListQueuedMessagesResponse {
            messages: self
                .0
                .queued_messages(id(r.thread_id)?, scope(&ctx)?.agent_id)
                .await?,
            ..Default::default()
        })
    }

    async fn remove_queued_message<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, wire::RemoveQueuedMessageRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::RemoveQueuedMessageResponse> + Send + use<'a>>
    {
        self.0
            .change_queue(
                id(r.thread_id)?,
                scope(&ctx)?.agent_id,
                id(r.id)?,
                super::sessions::QueueChange::Remove,
            )
            .await?;
        Response::ok(wire::RemoveQueuedMessageResponse::default())
    }

    async fn reorder_queued_message<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, wire::ReorderQueuedMessageRequest>,
    ) -> ServiceResult<
        impl connectrpc::Encodable<wire::ReorderQueuedMessageResponse> + Send + use<'a>,
    > {
        if r.id != r.before_id {
            self.0
                .change_queue(
                    id(r.thread_id)?,
                    scope(&ctx)?.agent_id,
                    id(r.id)?,
                    super::sessions::QueueChange::Before(if r.before_id.is_empty() {
                        None
                    } else {
                        Some(id(r.before_id)?)
                    }),
                )
                .await?;
        }
        Response::ok(wire::ReorderQueuedMessageResponse::default())
    }

    async fn steer_queued_message<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, wire::SteerQueuedMessageRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::SteerQueuedMessageResponse> + Send + use<'a>>
    {
        self.0
            .change_queue(
                id(r.thread_id)?,
                scope(&ctx)?.agent_id,
                id(r.id)?,
                super::sessions::QueueChange::Steer,
            )
            .await?;
        Response::ok(wire::SteerQueuedMessageResponse::default())
    }

    async fn list_threads<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, wire::ListThreadsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListThreadsResponse> + Send + use<'a>> {
        let claims = scope(&ctx)?;
        let (sessions, next_page_token) = self
            .0
            .tilde_sessions(&claims, "", r.page_token, r.page_size)
            .await?;
        let threads = sessions
            .into_iter()
            .filter_map(|s| s.thread.into_option())
            .collect();
        Response::ok(wire::ListThreadsResponse {
            threads,
            next_page_token,
            ..Default::default()
        })
    }
    async fn add_participant<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::AddParticipantRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::AddParticipantResponse> + Send + use<'a>>
    {
        Response::ok(wire::AddParticipantResponse {
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
        r: ServiceRequest<'_, wire::RemoveParticipantRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::RemoveParticipantResponse> + Send + use<'a>>
    {
        Response::ok(wire::RemoveParticipantResponse {
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
        r: ServiceRequest<'_, wire::SetTypingRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::SetTypingResponse> + Send + use<'a>> {
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
        Response::ok(wire::SetTypingResponse::default())
    }
    async fn list_activity<'a>(
        &'a self,
        _: RequestContext,
        r: ServiceRequest<'_, wire::ListActivityRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListActivityResponse> + Send + use<'a>>
    {
        let (events, next_cursor, has_more) = self
            .0
            .ingress_activity(id(r.thread_id)?, r.after_cursor, r.limit)
            .await?;
        Response::ok(wire::ListActivityResponse {
            events: events.into_iter().filter_map(for_end_user).collect(),
            next_cursor,
            has_more,
            ..Default::default()
        })
    }
    async fn upload_attachment<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, wire::UploadAttachmentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::UploadAttachmentResponse> + Send + use<'a>>
    {
        Response::ok(wire::UploadAttachmentResponse {
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
        r: ServiceRequest<'_, wire::DownloadAttachmentRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::DownloadAttachmentResponse> + Send + use<'a>>
    {
        let file = self
            .0
            .download_attachment(id(r.thread_id)?, id(r.attachment_id)?)
            .await?;
        Response::ok(wire::DownloadAttachmentResponse {
            attachment: file.attachment.into(),
            content: file.content,
            ..Default::default()
        })
    }
    async fn create_user<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::CreateUserRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::CreateUserResponse> + Send + use<'a>> {
        let _ = _ctx;
        Response::ok(wire::CreateUserResponse {
            user: if let Some(user) = scope(&_ctx)?.user_id {
                let row = crate::chat::db::user_get_opt(&self.0.pg()?.get().await?, user)
                    .await
                    .map_err(crate::chat::ChatError::from)?
                    .ok_or(crate::chat::ChatError::NotFound)?;
                crate::proto::tilde::types::v1::User {
                    id: row.id.to_string(),
                    name: row.name,
                    ..Default::default()
                }
                .into()
            } else {
                self.0.create_user(request.name).await?.into()
            },
            ..Default::default()
        })
    }
    async fn create_thread<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::CreateThreadRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::CreateThreadResponse> + Send + use<'a>>
    {
        let claims = scope(&_ctx)?;
        let mut input = request.to_owned_message();
        if let Some(user) = claims.user_id {
            input.primary_agent_id = claims.agent_id.to_string();
            input.participants = vec![
                crate::proto::tilde::types::v1::ParticipantRef {
                    user_id: Some(user.to_string()),
                    ..Default::default()
                },
                crate::proto::tilde::types::v1::ParticipantRef {
                    agent_id: Some(claims.agent_id.to_string()),
                    ..Default::default()
                },
            ];
        }
        Response::ok(wire::CreateThreadResponse {
            thread: self.0.create_thread(input.into()).await?.into(),
            ..Default::default()
        })
    }
    async fn get_thread<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::GetThreadRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetThreadResponse> + Send + use<'a>> {
        Response::ok(wire::GetThreadResponse {
            thread: self.0.thread(id(request.id)?).await?.into(),
            ..Default::default()
        })
    }
    async fn post_message<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::PostMessageRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::PostMessageResponse> + Send + use<'a>> {
        let claims = scope(&_ctx)?;
        let mut input = request.to_owned_message();
        if claims.user_id.is_some() && input.addressed_participant_ids.is_empty() {
            let roster = self.0.thread(id(&input.thread_id)?).await?;
            let target = roster
                .participants
                .iter()
                .find(|p| p.active && p.agent_id.as_deref() == Some(&claims.agent_id.to_string()))
                .ok_or_else(|| ConnectError::permission_denied("Agent is not a participant"))?;
            input.addressed_participant_ids = vec![target.id.clone()];
        }
        Response::ok(wire::PostMessageResponse {
            message: self.0.post(input.into()).await?.into(),
            ..Default::default()
        })
    }
    async fn list_messages<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::ListMessagesRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListMessagesResponse> + Send + use<'a>>
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

        Response::ok(wire::ListMessagesResponse {
            messages: page.messages,
            next_page_token: page.next_page_token,
            ..Default::default()
        })
    }
    async fn start_run<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::StartRunRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::StartRunResponse> + Send + use<'a>> {
        Response::ok(wire::StartRunResponse {
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
        request: ServiceRequest<'_, wire::ResumeRunRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ResumeRunResponse> + Send + use<'a>> {
        Response::ok(wire::ResumeRunResponse {
            run: self.0.resume_run(id(request.id)?).await?.into(),
            ..Default::default()
        })
    }
    async fn get_run<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::GetRunRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetRunResponse> + Send + use<'a>> {
        let _ = _ctx;
        Response::ok(wire::GetRunResponse {
            run: self.0.run(id(request.id)?).await?.into(),
            ..Default::default()
        })
    }
    async fn suspend_invocation<'a>(
        &'a self,
        _ctx: RequestContext,
        r: ServiceRequest<'_, wire::SuspendInvocationRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::SuspendInvocationResponse> + Send + use<'a>>
    {
        self.0.suspend_invocation(id(r.invocation_id)?).await?;
        Response::ok(wire::SuspendInvocationResponse::default())
    }
    async fn cancel_invocation<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::CancelInvocationRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::CancelInvocationResponse> + Send + use<'a>>
    {
        let _ = _ctx;
        self.0.cancel_invocation(id(request.invocation_id)?).await?;
        Response::ok(wire::CancelInvocationResponse::default())
    }
    async fn steer_invocation<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::SteerInvocationRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::SteerInvocationResponse> + Send + use<'a>>
    {
        let _ = _ctx;
        self.0
            .steer_invocation(request.to_owned_message().into())
            .await?;
        Response::ok(wire::SteerInvocationResponse::default())
    }
    async fn watch_thread(
        &self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, wire::WatchThreadRequest>,
    ) -> ServiceResult<
        ServiceStream<impl connectrpc::Encodable<wire::WatchThreadResponse> + Send + use<>>,
    > {
        let claims = scope(&_ctx)?;
        let chat = self.0.clone();
        let thread = id(request.thread_id)?;
        let mut stream = self.0.watch_ingress(thread, request.after_cursor).await?;
        Response::stream_ok(async_stream::try_stream! {
            use futures::StreamExt;
            while let Some(value)=stream.next().await {
                if chrono::Utc::now().timestamp()>=claims.exp {Err(ConnectError::unauthenticated("Provider credential expired"))?;}
                member(&chat,&claims,thread).await?;
                let (activity,cursor)=value?;
                let Some(activity)=for_end_user(activity) else {continue};
                yield wire::WatchThreadResponse{activity:activity.into(),cursor,..Default::default()};
            }
        })
    }
}

impl From<wire::CreateThreadRequest> for crate::chat::CreateThread {
    fn from(r: wire::CreateThreadRequest) -> Self {
        Self {
            title: r.title,
            participants: r.participants,
            primary_agent_id: r.primary_agent_id,
        }
    }
}

impl From<wire::PostMessageRequest> for crate::chat::PostMessage {
    fn from(r: wire::PostMessageRequest) -> Self {
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

impl From<wire::StartRunRequest> for crate::chat::StartRun {
    fn from(r: wire::StartRunRequest) -> Self {
        Self {
            thread_id: r.thread_id,
            agent_id: r.agent_id,
            objective: r.objective,
            goal_id: r.goal_id,
            idempotency_key: r.idempotency_key,
        }
    }
}

impl From<wire::SteerInvocationRequest> for crate::chat::SteerInvocation {
    fn from(r: wire::SteerInvocationRequest) -> Self {
        Self {
            invocation_id: r.invocation_id,
            input_id: r.input_id,
            text: r.text,
        }
    }
}

impl From<wire::AddParticipantRequest> for crate::chat::AddParticipant {
    fn from(r: wire::AddParticipantRequest) -> Self {
        Self {
            thread_id: r.thread_id,
            participant: r.participant.into_option(),
        }
    }
}

impl From<wire::UploadAttachmentRequest> for crate::chat::UploadAttachment {
    fn from(r: wire::UploadAttachmentRequest) -> Self {
        Self {
            id: r.id,
            thread_id: r.thread_id,
            filename: r.filename,
            media_type: r.media_type,
            content: r.content,
        }
    }
}

async fn identity_guard(
    axum::extract::State(chat): axum::extract::State<Chat>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let (mut parts, body) = request.into_parts();
    let Some(claims) = parts
        .extensions
        .get::<crate::deployment::tokens::IngressClaims>()
        .cloned()
    else {
        return ConnectError::unauthenticated("Provider scope required").into_response();
    };
    let bytes = match axum::body::to_bytes(body, crate::deployment::routing::MAX_BODY).await {
        Ok(b) => b,
        Err(_) => return ConnectError::resource_exhausted("Request too large").into_response(),
    };
    let bytes = match crate::deployment::routing::plain_body(&mut parts.headers, bytes) {
        Ok(b) => b,
        Err(e) => return ConnectError::from(e).into_response(),
    };
    let result: Result<(), crate::chat::ChatError> = async {
        let Some(user) = claims.user_id else {
            return Ok(());
        };
        let method = parts.uri.path().rsplit('/').next().unwrap_or("");
        let content_type = parts
            .headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        use crate::chat::ChatError;
        use crate::deployment::routing::decode;
        let target = match method {
            "StartRun" => Some(id(
                &decode::<wire::StartRunRequest>(&bytes, content_type)?.agent_id
            )?),
            "AddParticipant" => decode::<wire::AddParticipantRequest>(&bytes, content_type)?
                .participant
                .agent_id
                .as_deref()
                .map(id)
                .transpose()?,
            "GetRun" | "ResumeRun" => {
                let key = if method == "GetRun" {
                    decode::<wire::GetRunRequest>(&bytes, content_type)?.id
                } else {
                    decode::<wire::ResumeRunRequest>(&bytes, content_type)?.id
                };
                Some(id(&chat.run(id(&key)?).await?.agent_id)?)
            }
            "SteerInvocation" | "CancelInvocation" | "SuspendInvocation" => {
                let key = match method {
                    "SteerInvocation" => {
                        decode::<wire::SteerInvocationRequest>(&bytes, content_type)?.invocation_id
                    }
                    "CancelInvocation" => {
                        decode::<wire::CancelInvocationRequest>(&bytes, content_type)?.invocation_id
                    }
                    _ => {
                        decode::<wire::SuspendInvocationRequest>(&bytes, content_type)?
                            .invocation_id
                    }
                };
                Some(if let Some(local) = chat.local() {
                    id(&local.invocation(id(&key)?).await?.agent_id)?
                } else {
                    crate::chat::db::invocation_endpoint_one(&chat.pg()?.get().await?, id(&key)?)
                        .await?
                        .agent_id
                })
            }
            _ => None,
        };
        if target.is_some_and(|target| target != claims.agent_id) {
            return Err(ChatError::Forbidden);
        }
        let thread =
            crate::deployment::routing::thread(&chat, method, &bytes, content_type).await?;
        if let Some(thread) = thread {
            member(&chat, &claims, thread).await?;
            if method == "PostMessage" {
                let input = decode::<wire::PostMessageRequest>(&bytes, content_type)?;
                let roster = chat.thread(thread).await?;
                if input.addressed_participant_ids.iter().any(|id| {
                    roster.participants.iter().any(|p| {
                        &p.id == id
                            && p.agent_id
                                .as_deref()
                                .is_some_and(|a| a != claims.agent_id.to_string())
                    })
                }) {
                    return Err(ChatError::Forbidden);
                }
            }

            let participant = match method {
                "PostMessage" => Some(
                    crate::deployment::routing::decode::<wire::PostMessageRequest>(
                        &bytes,
                        content_type,
                    )?
                    .participant_id,
                ),
                "SetTyping" => Some(
                    crate::deployment::routing::decode::<wire::SetTypingRequest>(
                        &bytes,
                        content_type,
                    )?
                    .participant_id,
                ),
                _ => None,
            };
            if let Some(participant) = participant
                && !chat.thread(thread).await?.participants.iter().any(|p| {
                    p.active
                        && p.id == participant
                        && p.user_id.as_deref() == Some(&user.to_string())
                })
            {
                return Err(crate::chat::ChatError::Forbidden);
            }
        }
        Ok(())
    }
    .await;
    if let Err(error) = result {
        return ConnectError::from(error).into_response();
    }
    next.run(http::Request::from_parts(
        parts,
        axum::body::Body::from(bytes),
    ))
    .await
}
