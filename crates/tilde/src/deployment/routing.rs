//! Resolve conversation scope before selecting a persistence location. Input is
//! decoded using the actual ingress contract, never an arbitrary metadata bag.
use crate::{
    chat::{Chat, ChatError, Result, id},
    proto::tilde::provider::tilde::v1 as wire,
};
use buffa::Message;
use serde::de::DeserializeOwned;
use uuid::Uuid;
/// Largest request body the chat ingress paths buffer, compressed or not.
pub const MAX_BODY: usize = 192 * 1024 * 1024;

/// These ingress paths read the body to authorize and route it before the Connect handler
/// runs, and may forward it to another replica with only its content type. Undo gzip here,
/// once, and drop the encoding headers so everything downstream sees plain bytes.
pub fn plain_body(
    headers: &mut http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<axum::body::Bytes> {
    let streaming = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("connect+"));
    let name = if streaming {
        http::HeaderName::from_static("connect-content-encoding")
    } else {
        http::header::CONTENT_ENCODING
    };
    let encoding = headers
        .get(&name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("identity")
        .trim()
        .to_ascii_lowercase();
    if encoding.is_empty() || encoding == "identity" {
        return Ok(body);
    }
    if encoding != "gzip" {
        return Err(ChatError::Invalid(format!(
            "Unsupported request compression: {encoding}"
        )));
    }
    let plain = if streaming {
        let mut plain = Vec::with_capacity(body.len());
        let mut rest = &body[..];
        while !rest.is_empty() {
            let frame = rest
                .get(..5)
                .ok_or_else(|| ChatError::Invalid("Invalid Connect request frame".into()))?;
            let length = u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize;
            let message = rest
                .get(5..5 + length)
                .ok_or_else(|| ChatError::Invalid("Incomplete Connect request".into()))?;
            let message = if frame[0] & 1 == 1 {
                gunzip(message, MAX_BODY.saturating_sub(plain.len()))?
            } else {
                message.to_vec()
            };
            plain.push(frame[0] & !1);
            plain.extend_from_slice(&(message.len() as u32).to_be_bytes());
            plain.extend_from_slice(&message);
            rest = &rest[5 + length..];
        }
        plain
    } else {
        gunzip(&body, MAX_BODY)?
    };
    headers.remove(&name);
    headers.remove(http::header::CONTENT_LENGTH);
    Ok(plain.into())
}

fn gunzip(data: &[u8], limit: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut plain = Vec::new();
    flate2::read::GzDecoder::new(data)
        .take(limit as u64 + 1)
        .read_to_end(&mut plain)
        .map_err(|_| ChatError::Invalid("Invalid gzip request body".into()))?;
    if plain.len() > limit {
        return Err(ChatError::Invalid("Request exceeds limit".into()));
    }
    Ok(plain)
}
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
        "SearchMessages" => id(&request!(SearchMessagesRequest).thread_id)?,
        "GetSession" => id(&request!(GetSessionRequest).thread_id)?,
        "RenameSession" => id(&request!(RenameSessionRequest).thread_id)?,
        "SetReadState" => id(&request!(SetReadStateRequest).thread_id)?,
        "ListQueuedMessages" => id(&request!(ListQueuedMessagesRequest).thread_id)?,
        "RemoveQueuedMessage" => id(&request!(RemoveQueuedMessageRequest).thread_id)?,
        "ReorderQueuedMessage" => id(&request!(ReorderQueuedMessageRequest).thread_id)?,
        "SteerQueuedMessage" => id(&request!(SteerQueuedMessageRequest).thread_id)?,
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
                crate::chat::db::invocation_endpoint_opt(&chat.pg()?.get().await?, invocation)
                    .await?
                    .ok_or(ChatError::NotFound)?
                    .thread_id
            }
        }
        "CreateThread" | "CreateUser" | "ListThreads" | "GetIdentity" | "ListAgents"
        | "ListSessions" => return Ok(None),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }
    fn headers(content_type: &str, name: &str, encoding: &str) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.insert(http::header::CONTENT_TYPE, content_type.parse().unwrap());
        headers.insert(
            http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            encoding.parse().unwrap(),
        );
        headers
    }

    #[test]
    fn gzip_bodies_become_plain_for_routing_and_forwarding() {
        // Unary: the whole body is compressed and named by Content-Encoding.
        let request = wire::ListMessagesRequest {
            thread_id: "0b6f3c52-3f1d-4b9a-8c47-5e2d9a7c4b03".into(),
            ..Default::default()
        };
        let plain = request.encode_to_vec();
        let mut unary = headers("application/proto", "content-encoding", "gzip");
        let body = plain_body(&mut unary, gzip(&plain).into()).unwrap();
        assert_eq!(&body[..], &plain[..]);
        assert!(unary.get(http::header::CONTENT_ENCODING).is_none());
        let decoded: wire::ListMessagesRequest = decode(&body, "application/proto").unwrap();
        assert_eq!(decoded.thread_id, request.thread_id);

        // Streaming: each envelope carries its own compressed flag; mixed frames are legal.
        let mut framed = Vec::new();
        for (flag, message) in [(1u8, gzip(&plain)), (0u8, plain.clone())] {
            framed.push(flag);
            framed.extend_from_slice(&(message.len() as u32).to_be_bytes());
            framed.extend_from_slice(&message);
        }
        let mut streaming = headers(
            "application/connect+proto",
            "connect-content-encoding",
            "gzip",
        );
        let body = plain_body(&mut streaming, framed.into()).unwrap();
        let mut expected = Vec::new();
        for _ in 0..2 {
            expected.push(0u8);
            expected.extend_from_slice(&(plain.len() as u32).to_be_bytes());
            expected.extend_from_slice(&plain);
        }
        assert_eq!(&body[..], &expected[..]);
        assert!(streaming.get("connect-content-encoding").is_none());
        let decoded: wire::ListMessagesRequest =
            decode(&body, "application/connect+proto").unwrap();
        assert_eq!(decoded.thread_id, request.thread_id);

        // Identity passes through untouched; corrupt or unknown encodings are refused.
        let mut identity = headers("application/proto", "content-encoding", "identity");
        assert_eq!(
            &plain_body(&mut identity, plain.clone().into()).unwrap()[..],
            &plain[..]
        );
        let mut corrupt = headers("application/proto", "content-encoding", "gzip");
        assert!(plain_body(&mut corrupt, b"not gzip".to_vec().into()).is_err());
        let mut unknown = headers("application/proto", "content-encoding", "br");
        assert!(plain_body(&mut unknown, plain.into()).is_err());
    }
}
