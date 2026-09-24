//! Immutable conversation audit snapshots, membership, attachments and agent conversion cache.
//! Protobuf snapshots are read models only: lifecycle/relationships live in typed SQL columns.
//! The conversion cache is explicitly agent-authored opaque JSON, never canonical conversation state.
use super::{Chat, ChatError, Result, Scope, activity, id};
use crate::chat as application;
use crate::database::Transaction;
use crate::encryption::{SealedSecret, SecretBinding};
use crate::proto::tilde::types::v1 as types;
use base64::Engine;

/// Covers large provider documents without requiring an object-store deployment.
pub const MAX_ATTACHMENT_BYTES: usize = 128 * 1024 * 1024;
use buffa::Message as _;
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub type AttachmentRecords = tokio_postgres::types::Json<Vec<AttachmentRecord>>;
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AttachmentRecord {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub filename: String,
    pub media_type: String,
    pub size_bytes: i64,
    pub sha256: String,
}
impl From<AttachmentRecord> for types::Attachment {
    fn from(a: AttachmentRecord) -> Self {
        Self {
            id: a.id.to_string(),
            thread_id: a.thread_id.to_string(),
            filename: a.filename,
            media_type: a.media_type,
            size_bytes: a.size_bytes,
            sha256: a.sha256,
            ..Default::default()
        }
    }
}
pub fn delivery(
    connection: Option<Uuid>,
    external: Option<String>,
    destination: Option<String>,
    status: Option<String>,
) -> Option<types::MessageDelivery> {
    connection.map(|connection| types::MessageDelivery {
        connection_id: connection.to_string(),
        external_message_id: external.unwrap_or_default(),
        destination: destination.unwrap_or_default(),
        status: status.unwrap_or_default(),
        ..Default::default()
    })
}
pub fn timestamp(at: DateTime<Utc>) -> buffa_types::google::protobuf::Timestamp {
    buffa_types::google::protobuf::Timestamp {
        seconds: at.timestamp(),
        nanos: at.timestamp_subsec_nanos() as i32,
        ..Default::default()
    }
}
/// Lock before reading mutable inputs; keep sequence order aligned with commit order.
pub(crate) async fn lock(tx: &Transaction<'_>, thread: Uuid) -> Result<(i64, DateTime<Utc>)> {
    let row = crate::chat::db::activity_lock_opt(tx, thread)
        .await?
        .ok_or(ChatError::NotFound)?;
    Ok((row.sequence, row.at))
}

pub(crate) fn prepare(thread: Uuid, sequence: i64, at: DateTime<Utc>, event: &mut types::Activity) {
    event.sequence = sequence;
    if event.event_id.is_empty() {
        event.event_id = Uuid::new_v5(&thread, &sequence.to_be_bytes()).to_string();
    }
    event.created_at = timestamp(at).into();
}

mod queue;
pub(crate) use queue::enqueue;
pub use queue::flush;

/// Append an explicit provider/tool snapshot using the same commit-ordered thread cursor.
pub async fn append(
    pool: &crate::database::Pool,
    tx: &Transaction<'_>,
    thread: Uuid,
    mut event: types::Activity,
) -> Result<()> {
    let row = crate::chat::db::activity_sequence_one(tx, thread).await?;
    let at = Utc::now();
    prepare(thread, row.activity_sequence, at, &mut event);
    enqueue(pool, thread, row.transaction_id, at, event)
}
/// Snapshot message and participant records inside the transaction that changed them.
pub async fn snapshot(
    pool: &crate::database::Pool,
    tx: &Transaction<'_>,
    thread: Uuid,
    kind: &str,
    entity: Uuid,
    delta: &str,
) -> Result<()> {
    let mut event = types::Activity {
        kind: kind.into(),
        entity_id: entity.to_string(),
        text_delta: delta.into(),
        ..Default::default()
    };
    if kind == "message.delta" {
        let actor = crate::chat::db::message_actor_one(tx, entity, thread).await?;
        event.participant_id = actor.participant_id.to_string();
        event.invocation_id = actor
            .invocation_id
            .map(|id| id.to_string())
            .unwrap_or_default();
        event.detail = Some(
            types::MessageChunk {
                message_id: entity.to_string(),
                text_delta: delta.into(),
                ..Default::default()
            }
            .into(),
        );
        return append(pool, tx, thread, event).await;
    }
    if kind.starts_with("message.") {
        let m = crate::chat::db::message_get_one(tx, entity).await?;
        event.participant_id = m.participant_id.to_string();
        event.invocation_id = m.invocation_id.map(|v| v.to_string()).unwrap_or_default();
        event.detail = Some(
            types::Message {
                delivery: delivery(
                    m.connection_id,
                    m.external_message_id,
                    m.destination,
                    m.delivery_status,
                )
                .into(),
                id: m.id.to_string(),
                thread_id: m.thread_id.to_string(),
                participant_id: m.participant_id.to_string(),
                text: m.text,
                status: m.status,
                in_reply_to_message_id: m.in_reply_to_message_id.map(|v| v.to_string()),
                addressed_participant_ids: m.targets.iter().map(ToString::to_string).collect(),
                attachments: m.attachments.0.into_iter().map(Into::into).collect(),
                created_at: timestamp(m.created_at).into(),
                format: m.format,
                subject: m.subject,
                ..Default::default()
            }
            .into(),
        );
    } else if kind.starts_with("participant.") {
        let p = crate::chat::db::participant_get_one(tx, thread, entity).await?;
        event.participant_id = p.id.to_string();
        event.detail = Some(
            types::Participant {
                id: p.id.to_string(),
                name: p.name,
                user_id: p.user_id.map(|v| v.to_string()),
                agent_id: p.agent_id.map(|v| v.to_string()),
                active: p.active,
                ..Default::default()
            }
            .into(),
        );
    } else if kind.starts_with("reasoning.") || kind.starts_with("invocation.") {
        event.invocation_id = entity.to_string();
    }
    append(pool, tx, thread, event).await
}
impl Chat {
    /// Read the immutable message behind an input event, including attachments. Free-text events have no message.
    pub async fn steering_message(
        &self,
        thread: Uuid,
        message: Uuid,
    ) -> Result<Option<types::Message>> {
        if let Some(local) = self.local() {
            return local.steering_message(thread, message).await;
        }
        flush(self.pg()?).await?;
        let row = crate::chat::db::steering_message_opt(&self.pg()?.get().await?, thread, message)
            .await?;
        if let Some(bytes) = row.and_then(|r| r.snapshot) {
            let event =
                types::Activity::decode_from_slice(&bytes).map_err(|_| ChatError::Transport)?;
            if let Some(types::activity::Detail::Message(message)) = event.detail {
                return Ok(Some(*message));
            }
        }
        Ok(None)
    }

    pub async fn message_page(
        &self,
        thread: Uuid,
        before: Option<Uuid>,
        limit: u32,
    ) -> Result<application::MessagePage> {
        if let Some(local) = self.local() {
            return local.message_page(thread, limit, before).await;
        }
        self.invocation_message_page(thread, before, limit, None)
            .await
    }
    /// Each invocation sees history through its event, plus replies it has produced itself.
    pub async fn invocation_message_page(
        &self,
        thread: Uuid,
        before: Option<Uuid>,
        limit: u32,
        invocation: Option<Uuid>,
    ) -> Result<application::MessagePage> {
        if let Some(local) = self.local() {
            return local
                .invocation_message_page(thread, limit, before, invocation)
                .await;
        }
        let size = if limit == 0 { 100 } else { limit.min(100) };
        // The first page an invocation sees is what the wake loaded moments earlier.
        let warm_key = invocation.filter(|_| before.is_none() && size == 100);
        if let Some(invocation) = warm_key
            && let Some(page) = self.warm.history(thread, invocation)
        {
            return Ok(page.as_ref().clone());
        }
        let rows = crate::chat::db::messages_page_all(
            &self.pg()?.get().await?,
            thread,
            i64::from(size) + 1,
            before,
            invocation,
        )
        .await?;
        let page = message_page_from_rows(rows, size)?;
        if let Some(invocation) = warm_key {
            self.warm
                .store_history(thread, invocation, std::sync::Arc::new(page.clone()));
        }
        Ok(page)
    }
    /// Forward cursor pagination also supplies WatchThread replay; no event class is filtered out.
    pub async fn activity_page(
        &self,
        thread: Uuid,
        after: i64,
        limit: u32,
    ) -> Result<application::ActivityPage> {
        if let Some(local) = self.local() {
            return local.activity_page(thread, after, limit).await;
        }
        if after < 0 {
            return Err(ChatError::Invalid("Invalid activity cursor".into()));
        }
        let size = if limit == 0 { 100 } else { limit.min(100) };
        let mut rows = crate::chat::db::activity_list_all(
            &self.pg()?.get().await?,
            thread,
            after,
            i64::from(size) + 1,
        )
        .await?;
        let primary_agent = rows.first().ok_or(ChatError::NotFound)?.primary_agent_id;
        rows.retain(|r| r.sequence.is_some());
        let has_more = rows.len() > size as usize;
        if has_more {
            rows.pop();
        }
        let mut events = Vec::new();
        for row in rows {
            let mut event = if let Some(bytes) = row.snapshot {
                types::Activity::decode_from_slice(&bytes).map_err(|_| ChatError::Transport)?
            } else {
                types::Activity {
                    sequence: row.sequence.ok_or(ChatError::Transport)?,
                    kind: row.kind.ok_or(ChatError::Transport)?,
                    entity_id: row.entity_id.ok_or(ChatError::Transport)?.to_string(),
                    text_delta: row.text_delta.ok_or(ChatError::Transport)?,
                    created_at: timestamp(row.created_at.ok_or(ChatError::Transport)?).into(),
                    ..Default::default()
                }
            };
            if event.origin_agent_id.is_empty() {
                event.origin_agent_id = primary_agent.to_string();
                event.origin_instance_id = Uuid::nil().to_string();
                event.origin_sequence = row.sequence.ok_or(ChatError::Transport)?;
            }
            events.push(event);
        }
        Ok(application::ActivityPage {
            next_sequence: events.last().map_or(after, |e| e.sequence),
            events,
            has_more,
        })
    }
    /// Leaving retains authorship/history. Rejoining reactivates the same participant identity.
    pub async fn add_participant(&self, r: application::AddParticipant) -> Result<types::Thread> {
        if let Some(local) = self.local() {
            return local.add_participant(r).await;
        }
        let thread = id(&r.thread_id)?;
        if let Some(agent) = self.thread_sidecar_agent(thread).await? {
            let request = crate::proto::tilde::provider::tilde::v1::AddParticipantRequest {
                thread_id: r.thread_id.clone(),
                participant: r.participant.clone().into(),
                ..Default::default()
            };
            let response: Option<crate::proto::tilde::provider::tilde::v1::AddParticipantResponse> =
                self.forward_sidecar(agent, Some(thread), "AddParticipant", &request)
                    .await?;
            if let Some(response) = response {
                return response.thread.into_option().ok_or(ChatError::Transport);
            }
        }
        let p = r.participant.ok_or(ChatError::NotFound)?;
        if p.user_id.is_some() == p.agent_id.is_some() {
            return Err(ChatError::Invalid("Specify one user or agent".into()));
        }
        let user = p.user_id.as_deref().map(id).transpose()?;
        let agent = p.agent_id.as_deref().map(id).transpose()?;
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, thread).await?;
        let existing = crate::chat::db::participant_find_opt(&tx, thread, user, agent).await?;
        match existing {
            Some(p) if p.active => {}
            Some(p) => {
                crate::chat::db::participant_active_execute(&tx, thread, p.id, true).await?;
                activity(self.pg()?, &tx, thread, "participant.joined", p.id, "").await?;
            }
            None => {
                let participant = Uuid::new_v4();
                crate::chat::db::participant_join_execute(&tx, participant, thread, user, agent)
                    .await?;
                activity(
                    self.pg()?,
                    &tx,
                    thread,
                    "participant.joined",
                    participant,
                    "",
                )
                .await?;
            }
        }
        tx.commit().await?;
        drop(tx_client);
        self.thread(thread).await
    }
    pub async fn remove_participant(
        &self,
        thread: Uuid,
        participant: Uuid,
    ) -> Result<types::Thread> {
        if let Some(local) = self.local() {
            return local.remove_participant(thread, participant).await;
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, thread).await?;
        let roster = crate::chat::db::thread_get_one(&tx, thread).await?;
        let p = crate::chat::db::participant_get_opt(&tx, thread, participant)
            .await?
            .ok_or(ChatError::NotFound)?;
        if p.agent_id == Some(roster.primary_agent_id) {
            return Err(ChatError::Invalid(
                "The primary agent must remain in the thread".into(),
            ));
        }
        if crate::chat::db::participant_active_execute(&tx, thread, participant, false).await? > 0 {
            crate::chat::db::typing_clear_execute(&tx, thread, participant).await?;
            activity(self.pg()?, &tx, thread, "participant.left", participant, "").await?;
        }
        tx.commit().await?;
        drop(tx_client);
        self.thread(thread).await
    }
    pub async fn typing(&self, thread: Uuid, participant: Uuid, typing: bool) -> Result<()> {
        if let Some(local) = self.local() {
            return local.typing(thread, participant, typing).await;
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        let (sequence, at) = lock(&tx, thread).await?;
        let expires_at = typing.then_some(at + chrono::Duration::seconds(10));
        let mut event = types::Activity {
            kind: "typing.changed".into(),
            entity_id: participant.to_string(),
            participant_id: participant.to_string(),
            detail: Some(
                types::Typing {
                    participant_id: participant.to_string(),
                    typing,
                    expires_at: expires_at.map(timestamp).into(),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        };
        prepare(thread, sequence, at, &mut event);
        let result =
            crate::chat::db::typing_write_one(&tx, thread, participant, expires_at, sequence)
                .await?;
        if !result.present {
            return Err(ChatError::NotFound);
        }
        if !result.active {
            return Err(ChatError::Denied);
        }
        if !result.recorded {
            return Err(ChatError::Conflict);
        }
        enqueue(self.pg()?, thread, result.transaction_id, at, event)?;
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    /// Persist explicit not-typing when a sender disappears; replay can render the expiry itself.
    pub(crate) async fn expire_typing(&self) -> Result<()> {
        for row in crate::chat::db::typing_expired_all(&self.pg()?.get().await?).await? {
            let mut tx_client = self.pg()?.get().await?;
            let tx = tx_client.transaction().await?;
            crate::chat::db::thread_lock_one(&tx, row.thread_id).await?;
            if crate::chat::db::typing_expire_execute(&tx, row.thread_id, row.participant_id)
                .await?
                > 0
            {
                append(
                    self.pg()?,
                    &tx,
                    row.thread_id,
                    types::Activity {
                        kind: "typing.changed".into(),
                        entity_id: row.participant_id.to_string(),
                        participant_id: row.participant_id.to_string(),
                        detail: Some(
                            types::Typing {
                                participant_id: row.participant_id.to_string(),
                                typing: false,
                                ..Default::default()
                            }
                            .into(),
                        ),
                        ..Default::default()
                    },
                )
                .await?;
            }
            tx.commit().await?;
            drop(tx_client);
        }
        Ok(())
    }
    /// Small self-hostable attachment store; encrypted bytes never appear in audit snapshots.
    pub async fn upload_attachment(
        &self,
        mut r: application::UploadAttachment,
    ) -> Result<types::Attachment> {
        if let Some(local) = self.local() {
            return local.upload_attachment(r).await;
        }
        use zeroize::Zeroize;
        let attachment = id(&r.id)?;
        let thread = id(&r.thread_id)?;
        crate::chat::db::thread_get_opt(&self.pg()?.get().await?, thread)
            .await?
            .ok_or(ChatError::NotFound)?;
        if r.filename.is_empty()
            || r.filename.len() > 255
            || r.filename.chars().any(char::is_control)
            || r.media_type.len() > 200
            || !r.media_type.contains('/')
            || r.media_type.chars().any(char::is_control)
            || r.content.len() > MAX_ATTACHMENT_BYTES
        {
            return Err(ChatError::Invalid(
                "Invalid attachment or exceeds 128 MiB".into(),
            ));
        }
        let size = r.content.len() as i64;
        let hash = hex::encode(Sha256::digest(&r.content));
        let encoded =
            SecretString::from(base64::engine::general_purpose::STANDARD.encode(&r.content));
        r.content.zeroize();
        let sealed = self
            .encryption
            .seal(attachment_binding(attachment), &encoded)
            .map_err(|_| ChatError::Transport)?
            .into_bytes();
        let row = crate::chat::db::attachment_put_opt(
            &self.pg()?.get().await?,
            attachment,
            thread,
            &(r.filename),
            &(r.media_type),
            size,
            &(hash),
            &(sealed),
        )
        .await?
        .ok_or(ChatError::Conflict)?;
        Ok(types::Attachment {
            id: attachment.to_string(),
            thread_id: thread.to_string(),
            filename: row.filename,
            media_type: row.media_type,
            size_bytes: row.size_bytes,
            sha256: row.sha256,
            ..Default::default()
        })
    }
    pub async fn download_attachment(
        &self,
        thread: Uuid,
        attachment: Uuid,
    ) -> Result<application::AttachmentContent> {
        if let Some(local) = self.local() {
            return local.download_attachment(thread, attachment).await;
        }
        let row = crate::chat::db::attachment_get_opt(&self.pg()?.get().await?, attachment, thread)
            .await?
            .ok_or(ChatError::NotFound)?;
        let content = if let Some(object) = &row.object_key {
            crate::deployment::attachments::download(
                self.objects.as_ref().ok_or(ChatError::Transport)?,
                &self.encryption,
                attachment,
                object,
            )
            .await
            .map_err(|_| ChatError::Transport)?
        } else if let Some(content) = row.content {
            let encoded = self
                .encryption
                .open(
                    attachment_binding(attachment),
                    SealedSecret::from_bytes(&content).map_err(|_| ChatError::Transport)?,
                )
                .map_err(|_| ChatError::Transport)?;
            base64::engine::general_purpose::STANDARD
                .decode(encoded.expose_secret())
                .map_err(|_| ChatError::Transport)?
        } else {
            let source = row
                .source_url
                .map(|source| {
                    self.encryption
                        .open(
                            SecretBinding {
                                resource_kind: "chat_attachment",
                                resource_id: attachment,
                                name: "source_url",
                            },
                            SealedSecret::from_bytes(&source).map_err(|_| ChatError::Transport)?,
                        )
                        .map_err(|_| ChatError::Transport)
                })
                .transpose()?;
            let channels = self.channels.as_ref().ok_or(ChatError::Transport)?;
            let content = channels
                .attachment(
                    row.connection_id.ok_or(ChatError::Transport)?,
                    row.provider_attachment_id.as_deref(),
                    source.as_ref().map(|s| s.expose_secret()),
                )
                .await
                .map_err(|_| ChatError::Transport)?;
            let encoded =
                SecretString::from(base64::engine::general_purpose::STANDARD.encode(&content));
            let sealed = self
                .encryption
                .seal(attachment_binding(attachment), &encoded)
                .map_err(|_| ChatError::Transport)?
                .into_bytes();
            crate::chat::db::attachment_store_execute(
                &self.pg()?.get().await?,
                attachment,
                &(sealed),
                content.len() as i64,
                &(hex::encode(Sha256::digest(&content))),
            )
            .await?;
            content
        };
        Ok(application::AttachmentContent {
            attachment: types::Attachment {
                id: attachment.to_string(),
                thread_id: thread.to_string(),
                filename: row.filename,
                media_type: row.media_type,
                persisted: true,
                size_bytes: content.len() as i64,
                sha256: hex::encode(Sha256::digest(&content)),
                ..Default::default()
            },
            content,
        })
    }
    /// A batch either validates every canonical message in scope and commits, or writes nothing.
    pub async fn cache_converted_messages(
        &self,
        scope: &Scope,
        messages: Vec<application::ConvertedMessage>,
    ) -> Result<()> {
        if let Some(local) = self.local() {
            return local.cache_converted_messages(scope, messages).await;
        }
        if messages.len() > 100 {
            return Err(ChatError::Invalid("At most 100 cached messages".into()));
        }
        let mut total = 0;
        let mut batch = std::collections::BTreeMap::new();
        for message in messages {
            total += message.message_json.len();
            if total > 1024 * 1024 {
                return Err(ChatError::Invalid(
                    "Conversion cache batch exceeds 1 MiB".into(),
                ));
            }
            let key = id(&message.message_id)?;
            serde_json::from_str::<serde_json::Value>(&message.message_json)
                .map_err(|_| ChatError::Invalid("Invalid converted JSON".into()))?;
            // Preserve sequential writes' last-value-wins behavior for duplicate IDs.
            batch.insert(key, message.message_json);
        }
        let (ids, representations): (Vec<_>, Vec<_>) = batch.into_iter().unzip();
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, scope.thread_id).await?;
        let result = crate::chat::db::cache_upsert_batch_one(
            &tx,
            scope.agent_id,
            scope.thread_id,
            &ids,
            &representations,
        )
        .await?;
        if result.missing != 0 {
            return Err(ChatError::NotFound);
        }
        if result.incomplete != 0 {
            return Err(ChatError::Invalid(
                "Only completed messages can be cached".into(),
            ));
        }
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    pub async fn hydrate_converted_messages(
        &self,
        agent: Uuid,
        thread: Uuid,
        ids: &[String],
    ) -> Result<Vec<application::ConvertedMessage>> {
        if let Some(local) = self.local() {
            return local.hydrate_converted_messages(agent, thread, ids).await;
        }
        if ids.len() > 100 {
            return Err(ChatError::Invalid("At most 100 message IDs".into()));
        }
        let ids = ids.iter().map(|v| id(v)).collect::<Result<Vec<_>>>()?;
        Ok(
            crate::chat::db::cache_hydrate_all(&self.pg()?.get().await?, agent, thread, &ids)
                .await?
                .into_iter()
                .map(|r| application::ConvertedMessage {
                    message_id: r.message_id.to_string(),
                    message_json: r.representation.to_string(),
                })
                .collect(),
        )
    }
}
fn attachment_binding(id: Uuid) -> SecretBinding<'static> {
    SecretBinding {
        resource_kind: "chat_attachment",
        resource_id: id,
        name: "content",
    }
}

/// Convert one SQL snapshot, shared by ordinary pagination and native UI bootstrap.
pub(crate) fn message_page_from_rows(
    mut rows: Vec<crate::chat::db::MessagesPageRow>,
    size: u32,
) -> Result<application::MessagePage> {
    if !rows.first().ok_or(ChatError::NotFound)?.cursor_valid {
        return Err(ChatError::NotFound);
    }
    rows.retain(|r| r.id.is_some());
    let more = rows.len() > size as usize;
    if more {
        rows.pop();
    }
    let next = if more {
        rows.last()
            .and_then(|r| r.id)
            .map(|id| id.to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };
    let mut cached_messages: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            Some(application::ConvertedMessage {
                message_id: r.id?.to_string(),
                message_json: r.cached_representation.as_ref()?.to_string(),
            })
        })
        .collect();
    cached_messages.sort_by(|a, b| a.message_id.cmp(&b.message_id));
    let messages = rows
        .into_iter()
        .rev()
        .map(|r| {
            Ok(types::Message {
                delivery: delivery(
                    r.connection_id,
                    r.external_message_id,
                    r.destination,
                    r.delivery_status,
                )
                .into(),
                id: r.id.ok_or(ChatError::Transport)?.to_string(),
                thread_id: r.thread_id.to_string(),
                participant_id: r.participant_id.ok_or(ChatError::Transport)?.to_string(),
                text: r.text.ok_or(ChatError::Transport)?,
                status: r.status.ok_or(ChatError::Transport)?,
                in_reply_to_message_id: r.in_reply_to_message_id.map(|v| v.to_string()),
                addressed_participant_ids: r.targets.into_iter().map(|v| v.to_string()).collect(),
                attachments: r.attachments.0.into_iter().map(Into::into).collect(),
                format: r.format.ok_or(ChatError::Transport)?,
                subject: r.subject,
                created_at: timestamp(r.created_at.ok_or(ChatError::Transport)?).into(),
                ..Default::default()
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(application::MessagePage {
        messages,
        next_page_token: next,
        cached_messages,
    })
}
