//! Resolve conversation scope before selecting a persistence location. Input is
//! decoded using the actual ingress contract, never an arbitrary metadata bag.
use crate::{
    chat::{Chat, ChatError, Result, id},
    proto::tilde::ingress::v1 as wire,
};
use buffa::Message;
use serde::de::DeserializeOwned;
use uuid::Uuid;
pub fn decode<M: Message + DeserializeOwned>(body: &[u8], content_type: &str) -> Result<M> {
    let body = if content_type.contains("connect+") {
        if body.len() < 5 || body[0] != 0 {
            return Err(ChatError::Invalid("Invalid Connect request frame".into()));
        }
        let length =
            u32::from_be_bytes(body[1..5].try_into().map_err(|_| ChatError::Transport)?) as usize;
        body.get(5..5 + length)
            .ok_or_else(|| ChatError::Invalid("Incomplete Connect request".into()))?
    } else {
        body
    };
    if content_type.contains("json") {
        serde_json::from_slice(body).map_err(|_| ChatError::Invalid("Invalid request JSON".into()))
    } else {
        M::decode_from_slice(body)
            .map_err(|_| ChatError::Invalid("Invalid request Protobuf".into()))
    }
}
pub async fn thread(
    chat: &Chat,
    method: &str,
    body: &[u8],
    content_type: &str,
) -> Result<Option<Uuid>> {
    macro_rules! request {
        ($name:ident) => {
            decode::<wire::$name>(body, content_type)?
        };
    }
    let thread = match method {
        "GetThread" => id(&request!(GetThreadRequest).id)?,
        "PostMessage" => id(&request!(PostMessageRequest).thread_id)?,
        "ListMessages" => id(&request!(ListMessagesRequest).thread_id)?,
        "StartRun" => id(&request!(StartRunRequest).thread_id)?,
        "AddParticipant" => id(&request!(AddParticipantRequest).thread_id)?,
        "RemoveParticipant" => id(&request!(RemoveParticipantRequest).thread_id)?,
        "SetTyping" => id(&request!(SetTypingRequest).thread_id)?,
        "ListActivity" => id(&request!(ListActivityRequest).thread_id)?,
        "UploadAttachment" => id(&request!(UploadAttachmentRequest).thread_id)?,
        "DownloadAttachment" => id(&request!(DownloadAttachmentRequest).thread_id)?,
        "WatchThread" => id(&request!(WatchThreadRequest).thread_id)?,
        "GetRun" => id(&chat.run(id(&request!(GetRunRequest).id)?).await?.thread_id)?,
        "ResumeRun" => id(&chat
            .run(id(&request!(ResumeRunRequest).id)?)
            .await?
            .thread_id)?,
        "SteerInvocation" | "CancelInvocation" | "SuspendInvocation" => {
            let invocation = if method == "SteerInvocation" {
                id(&request!(SteerInvocationRequest).invocation_id)?
            } else if method == "SuspendInvocation" {
                id(&request!(SuspendInvocationRequest).invocation_id)?
            } else {
                id(&request!(CancelInvocationRequest).invocation_id)?
            };
            if let Some(local) = chat.local() {
                id(&local.invocation(invocation).await?.thread_id)?
            } else {
                sqlx::query_file!("../../queries/deployment/invocation_thread.sql", invocation)
                    .fetch_optional(chat.pg()?)
                    .await?
                    .ok_or(ChatError::NotFound)?
                    .thread_id
            }
        }
        "CreateThread" | "CreateUser" | "ListThreads" => return Ok(None),
        _ => return Err(ChatError::Invalid("Unknown ingress method".into())),
    };
    Ok(Some(thread))
}
pub fn check_creation_scope(
    agent: Uuid,
    method: &str,
    body: &[u8],
    content_type: &str,
) -> Result<()> {
    if method == "CreateThread"
        && id(&decode::<wire::CreateThreadRequest>(body, content_type)?.primary_agent_id)? != agent
    {
        return Err(ChatError::Denied);
    }
    if method == "ListThreads"
        && decode::<wire::ListThreadsRequest>(body, content_type)?
            .agent_id
            .as_deref()
            .map(id)
            .transpose()?
            .is_some_and(|a| a != agent)
    {
        return Err(ChatError::Denied);
    }
    Ok(())
}
