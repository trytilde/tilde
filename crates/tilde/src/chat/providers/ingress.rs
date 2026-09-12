//! Verified provider callbacks share durable, idempotent conversation ingestion.
//! Each adapter verifies its own signature and interprets only its provider's payload.
use super::*;
use crate::chat::{Chat, ChatError, activity};
use crate::proto::tilde::types::v1 as types;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::post,
};
use base64::Engine;
use hmac::{Hmac, Mac};
use http::{HeaderMap, StatusCode};
use sha2::{Digest, Sha256};

pub struct RemoteAttachment {
    pub id: String,
    pub filename: String,
    pub media_type: String,
    pub size: i64,
    pub url: Option<String>,
    pub provider_id: Option<String>,
}
#[derive(Clone, Copy)]
pub enum IncomingKind {
    Message,
    Updated,
    Deleted,
    Typing(bool),
    Membership(bool),
}
pub struct IncomingMessage {
    pub subject: Option<String>,
    pub kind: IncomingKind,
    pub event_id: String,
    pub message_id: String,
    pub thread_id: String,
    pub sender: crate::chat::access::identity::Identity,
    pub sender_name: String,
    pub attachments: Vec<RemoteAttachment>,
    pub text: String,
    pub format: &'static str,
    pub reply_to: Option<String>,
}
pub enum Webhook {
    Challenge(String),
    Messages(Vec<IncomingMessage>),
}
#[derive(Clone)]
struct Webhooks {
    chat: Chat,
    connections: Connections,
}
pub fn router(chat: Chat, connections: Connections) -> axum::Router {
    axum::Router::new()
        .route(
            "/connections/webhooks/{connection_id}",
            post(receive).get(challenge),
        )
        .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024))
        .with_state(Webhooks { chat, connections })
}
#[derive(serde::Deserialize)]
struct Challenge {
    #[serde(rename = "hub.mode")]
    mode: String,
    #[serde(rename = "hub.verify_token")]
    token: String,
    #[serde(rename = "hub.challenge")]
    challenge: String,
}
async fn challenge(
    State(s): State<Webhooks>,
    Path(id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<Challenge>,
) -> Response {
    use subtle::ConstantTimeEq;
    let result: ToolResult<String> = async {
        let row = s.connections.get(id).await?;
        if row.provider_id != "whatsapp" || q.mode != "subscribe" {
            return Err(ConnectError::permission_denied(
                "Invalid webhook verification",
            ));
        }
        let values = s.connections.resolve(id).await?;
        if value(&values, "verify_token")?
            .as_bytes()
            .ct_eq(q.token.as_bytes())
            .unwrap_u8()
            != 1
        {
            return Err(ConnectError::permission_denied(
                "Invalid webhook verification",
            ));
        }
        Ok(q.challenge)
    }
    .await;
    match result {
        Ok(value) => value.into_response(),
        Err(_) => (StatusCode::UNAUTHORIZED, "Invalid webhook verification").into_response(),
    }
}
async fn receive(
    State(s): State<Webhooks>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let result: ToolResult<Option<String>> = async {
        let row = s.connections.get(id).await?;
        let provider = adapter(&row.provider_id, &row.type_id)
            .ok_or_else(|| ConnectError::not_found("Unknown channel"))?;
        let access = Access {
            connection_id: id,
            values: s.connections.resolve(id).await?,
            http: s.connections.http.clone(),
            endpoints: s.connections.endpoints.clone(),
        };
        let result = provider.webhook(&access, &headers, &body).await?;
        match result {
            Webhook::Challenge(value) => Ok(Some(value)),
            Webhook::Messages(messages) => {
                let owner = sqlx::query_file!("../../queries/chat/channel_owner.sql", id)
                    .fetch_optional(&s.connections.pool)
                    .await
                    .map_err(ChatError::from)?;
                if let Some(owner) = owner
                    && owner.deployment_mode == "sidecar"
                {
                    // The replica owning the conversation ingests it; the gateway only relays.
                    let deployments = s.chat.deployments.as_ref().ok_or_else(|| {
                        ConnectError::failed_precondition("Sidecar routing is unavailable")
                    })?;
                    for message in messages {
                        deployments
                            .forward_provider_event(owner.agent_id, id, message.into())
                            .await?;
                    }
                    return Ok(None);
                }
                for message in messages {
                    s.chat.ingest(id, message).await?;
                }
                Ok(None)
            }
        }
    }
    .await;
    match result {
        Ok(Some(challenge)) => axum::Json(json!({"challenge":challenge})).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            tracing::warn!(code=?error.code,connection_id=%id,"Chat webhook rejected");
            (StatusCode::BAD_REQUEST, "Webhook could not be accepted").into_response()
        }
    }
}
/// Header lookup never returns authentication material in an error.
pub fn header<'a>(h: &'a HeaderMap, key: &str) -> ToolResult<&'a str> {
    h.get(key)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ConnectError::unauthenticated("Missing webhook signature"))
}
pub fn hmac_verify(key: &[u8], bytes: &[u8], signature: &[u8]) -> ToolResult<()> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .map_err(|_| ConnectError::unauthenticated("Invalid signature"))?;
    mac.update(bytes);
    mac.verify_slice(signature)
        .map_err(|_| ConnectError::unauthenticated("Invalid signature"))
}
pub fn fresh(timestamp: &str) -> ToolResult<()> {
    let at = timestamp
        .parse::<i64>()
        .map_err(|_| ConnectError::unauthenticated("Invalid timestamp"))?;
    if chrono::Utc::now().timestamp().abs_diff(at) > 300 {
        return Err(ConnectError::unauthenticated("Expired webhook"));
    }
    Ok(())
}
pub fn sha256_header(a: &Access, h: &HeaderMap, body: &[u8], key: &str) -> ToolResult<()> {
    let signature = header(h, "x-hub-signature-256")?
        .strip_prefix("sha256=")
        .ok_or_else(|| ConnectError::unauthenticated("Invalid signature"))?;
    let signature =
        hex::decode(signature).map_err(|_| ConnectError::unauthenticated("Invalid signature"))?;
    hmac_verify(a.secret(key)?.as_bytes(), body, &signature)
}
pub fn standard(a: &Access, h: &HeaderMap, body: &[u8], key: &str, prefix: &str) -> ToolResult<()> {
    let id = header(h, &format!("{prefix}-id"))?;
    let timestamp = header(h, &format!("{prefix}-timestamp"))?;
    fresh(timestamp)?;
    let key = a.secret(key)?;
    let key = zeroize::Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(key.strip_prefix("whsec_").unwrap_or(key))
            .map_err(|_| ConnectError::unauthenticated("Invalid webhook key"))?,
    );
    let mut signed = format!("{id}.{timestamp}.").into_bytes();
    signed.extend_from_slice(body);
    for signature in header(h, &format!("{prefix}-signature"))?
        .split_whitespace()
        .filter_map(|s| s.strip_prefix("v1,"))
    {
        if let Ok(signature) = base64::engine::general_purpose::STANDARD.decode(signature)
            && hmac_verify(&key, &signed, &signature).is_ok()
        {
            return Ok(());
        }
    }
    Err(ConnectError::unauthenticated("Invalid webhook signature"))
}
pub fn payload(body: &[u8]) -> ToolResult<Value> {
    serde_json::from_slice(body)
        .map_err(|_| ConnectError::invalid_argument("Invalid webhook payload"))
}
pub fn at<'a>(v: &'a Value, path: &str) -> Option<&'a str> {
    v.pointer(path).and_then(Value::as_str)
}
pub(crate) fn stable(connection: Uuid, kind: &str, key: &str) -> Uuid {
    let digest = Sha256::digest(format!("{connection}:{kind}:{key}"));
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(bytes)
}
impl Chat {
    /// Message, thread, participants and replay receipt commit together before the callback is acknowledged.
    pub async fn ingest(&self, connection: Uuid, m: IncomingMessage) -> crate::chat::Result<()> {
        if m.text.len() > 1024 * 1024
            || m.message_id.is_empty()
            || m.thread_id.is_empty()
            || m.sender.value.is_empty()
        {
            return Err(ChatError::Invalid("Invalid inbound message".into()));
        }
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            connection.to_string()
        )
        .execute(&mut *tx)
        .await?;
        let owner = sqlx::query_file!("../../queries/chat/channel_owner.sql", connection)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ChatError::NotFound)?;
        if sqlx::query_file!("../../queries/chat/receipt_get.sql", connection, m.event_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some()
        {
            return Ok(());
        }
        m.sender
            .validate()
            .map_err(|_| ChatError::Invalid("Invalid sender identity".into()))?;
        let kind = crate::chat::access::identity::kind_name(m.sender.identity_type);
        let value = &m.sender.value;
        let user = stable(connection, "user", &format!("{kind}:{value}"));
        sqlx::query_file!("../../queries/chat/channel_user.sql", user, value)
            .execute(&mut *tx)
            .await?;
        let identity = sqlx::query_file!(
            "../../queries/channel_access/identity_upsert.sql",
            user,
            connection,
            kind,
            value
        )
        .fetch_one(&mut *tx)
        .await?
        .id;
        let accepted = sqlx::query_file!(
            "../../queries/channel_access/allowed.sql",
            owner.agent_id,
            identity
        )
        .fetch_one(&mut *tx)
        .await?
        .allowed;
        sqlx::query_file!(
            "../../queries/channel_access/audit.sql",
            connection,
            m.event_id,
            owner.agent_id,
            identity,
            owner.access_mode,
            accepted
        )
        .execute(&mut *tx)
        .await?;
        if !accepted {
            sqlx::query_file!(
                "../../queries/chat/receipt_insert.sql",
                connection,
                m.event_id,
                None::<Uuid>
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(());
        }
        let thread = stable(
            connection,
            "thread",
            &format!("{}:{}", owner.agent_id, m.thread_id),
        );
        let participant = stable(connection, "participant", &format!("{thread}:{user}"));
        let agent_participant = stable(connection, "agent", &thread.to_string());
        let message = stable(connection, "message", &format!("{thread}:{}", m.message_id));
        let created = sqlx::query_file!(
            "../../queries/chat/channel_thread_create.sql",
            thread,
            m.sender_name,
            owner.agent_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", thread)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query_file!(
            "../../queries/chat/channel_thread_bind.sql",
            thread,
            connection,
            m.thread_id,
            owner.agent_id
        )
        .execute(&mut *tx)
        .await?;
        for (pid, uid, aid) in [
            (participant, Some(user), None),
            (agent_participant, None, Some(owner.agent_id)),
        ] {
            if sqlx::query_file!(
                "../../queries/chat/participant_join.sql",
                pid,
                thread,
                uid,
                aid
            )
            .execute(&mut *tx)
            .await?
            .rows_affected()
                > 0
            {
                activity(&mut tx, thread, "participant.joined", pid, "").await?;
            }
        }
        if created {
            activity(&mut tx, thread, "thread.created", thread, "").await?;
        }
        match m.kind {
            IncomingKind::Typing(typing) => {
                let expires_at = if typing {
                    Some(
                        sqlx::query_file!("../../queries/chat/typing_set.sql", thread, participant)
                            .fetch_one(&mut *tx)
                            .await?
                            .expires_at,
                    )
                } else {
                    sqlx::query_file!("../../queries/chat/typing_clear.sql", thread, participant)
                        .execute(&mut *tx)
                        .await?;
                    None
                };
                crate::chat::audit::append(
                    &mut tx,
                    thread,
                    types::Activity {
                        kind: "typing.changed".into(),
                        entity_id: participant.to_string(),
                        participant_id: participant.to_string(),
                        detail: Some(
                            types::Typing {
                                participant_id: participant.to_string(),
                                typing,
                                expires_at: expires_at.map(crate::chat::audit::timestamp).into(),
                                ..Default::default()
                            }
                            .into(),
                        ),
                        ..Default::default()
                    },
                )
                .await?;
            }
            IncomingKind::Membership(active) => {
                sqlx::query_file!(
                    "../../queries/chat/participant_active.sql",
                    thread,
                    participant,
                    active
                )
                .execute(&mut *tx)
                .await?;
                activity(
                    &mut tx,
                    thread,
                    if active {
                        "participant.joined"
                    } else {
                        "participant.left"
                    },
                    participant,
                    "",
                )
                .await?;
            }
            IncomingKind::Updated | IncomingKind::Deleted => {
                let existing = sqlx::query_file!(
                    "../../queries/chat/delivery_lookup.sql",
                    connection,
                    m.message_id,
                    thread
                )
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(ChatError::NotFound)?;
                let deleted = matches!(m.kind, IncomingKind::Deleted);
                sqlx::query_file!(
                    "../../queries/chat/message_edit.sql",
                    existing.message_id,
                    m.text,
                    if deleted { "deleted" } else { "complete" }
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/chat/cache_invalidate.sql",
                    existing.message_id
                )
                .execute(&mut *tx)
                .await?;
                activity(
                    &mut tx,
                    thread,
                    if deleted {
                        "message.deleted"
                    } else {
                        "message.updated"
                    },
                    existing.message_id,
                    "",
                )
                .await?;
            }
            IncomingKind::Message => {}
        }
        if !matches!(m.kind, IncomingKind::Message) {
            sqlx::query_file!(
                "../../queries/chat/receipt_insert.sql",
                connection,
                m.event_id,
                None::<Uuid>
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(());
        }
        let exists = sqlx::query_file!("../../queries/chat/cache_message_get.sql", message, thread)
            .fetch_optional(&mut *tx)
            .await?;
        if exists.is_none() {
            let reply = if let Some(reply) = m.reply_to {
                sqlx::query_file!(
                    "../../queries/chat/delivery_lookup.sql",
                    connection,
                    reply,
                    thread
                )
                .fetch_optional(&mut *tx)
                .await?
                .map(|r| r.message_id)
            } else {
                None
            };
            sqlx::query_file!(
                "../../queries/chat/message_create.sql",
                message,
                thread,
                participant,
                m.text,
                "complete",
                None::<Uuid>,
                reply,
                crate::telemetry::context::capture().0,
                crate::telemetry::context::capture().1
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query_file!(
                "../../queries/channel_access/message_source.sql",
                message,
                identity
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query_file!("../../queries/chat/message_format.sql", message, m.format)
                .execute(&mut *tx)
                .await?;
            sqlx::query_file!(
                "../../queries/chat/delivery_insert.sql",
                message,
                connection,
                m.thread_id,
                m.message_id
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query_file!("../../queries/chat/message_subject.sql", message, m.subject)
                .execute(&mut *tx)
                .await?;
            if m.attachments.len() > 20 {
                return Err(ChatError::Invalid("Too many attachments".into()));
            }
            for attachment in m.attachments {
                let aid = stable(
                    connection,
                    "attachment",
                    &format!("{}:{}", m.message_id, attachment.id),
                );
                let source = attachment
                    .url
                    .map(|url| {
                        self.encryption
                            .seal(
                                crate::encryption::SecretBinding {
                                    resource_kind: "chat_attachment",
                                    resource_id: aid,
                                    name: "source_url",
                                },
                                &secrecy::SecretString::from(url),
                            )
                            .map(|s| s.into_bytes())
                            .map_err(|_| ChatError::Transport)
                    })
                    .transpose()?;
                sqlx::query_file!(
                    "../../queries/chat/attachment_remote.sql",
                    aid,
                    thread,
                    attachment.filename,
                    attachment.media_type,
                    attachment.size,
                    connection,
                    attachment.provider_id,
                    source
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/chat/attachment_attach.sql",
                    thread,
                    message,
                    aid
                )
                .execute(&mut *tx)
                .await?;
            }
            activity(&mut tx, thread, "message.completed", message, "").await?;
        }
        sqlx::query_file!(
            "../../queries/chat/receipt_insert.sql",
            connection,
            m.event_id,
            message
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
