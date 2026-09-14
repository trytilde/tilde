//! Provider payload parsing stays in the existing adapters. This module moves
//! verified, typed events into the memory of the replica holding the thread, or
//! hands them to a live replica when they arrive at the gateway.
use super::{Deployments, runtime::Runtime, runtime::store};
use crate::{
    chat::{
        ChatError, Result,
        providers::ingress::{IncomingKind, IncomingMessage, RemoteAttachment, stable},
    },
    error::Error,
    proto::tilde::{agent_event_ingress::v1 as wire, types::v1 as types},
};
use uuid::Uuid;
impl From<IncomingMessage> for types::ProviderEvent {
    fn from(m: IncomingMessage) -> Self {
        let (kind, active) = match m.kind {
            IncomingKind::Message => (types::ProviderEventKind::Message, false),
            IncomingKind::Updated => (types::ProviderEventKind::Updated, false),
            IncomingKind::Deleted => (types::ProviderEventKind::Deleted, false),
            IncomingKind::Typing(v) => (types::ProviderEventKind::Typing, v),
            IncomingKind::Membership(v) => (types::ProviderEventKind::Membership, v),
        };
        Self {
            kind: kind.into(),
            event_id: m.event_id,
            message_id: m.message_id,
            thread_id: m.thread_id,
            identity_type: m.sender.identity_type.into(),
            identity_value: m.sender.value,
            sender_name: m.sender_name,
            text: m.text,
            format: m.format.into(),
            reply_to: m.reply_to,
            subject: m.subject,
            active,
            attachments: m
                .attachments
                .into_iter()
                .map(|a| types::ProviderAttachment {
                    id: a.id,
                    filename: a.filename,
                    media_type: a.media_type,
                    size: a.size,
                    url: a.url,
                    provider_reference: a.provider_id,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }
}
impl TryFrom<types::ProviderEvent> for IncomingMessage {
    type Error = Error;
    fn try_from(m: types::ProviderEvent) -> std::result::Result<Self, Error> {
        let kind = match m.kind.as_known() {
            Some(types::ProviderEventKind::Message) => IncomingKind::Message,
            Some(types::ProviderEventKind::Updated) => IncomingKind::Updated,
            Some(types::ProviderEventKind::Deleted) => IncomingKind::Deleted,
            Some(types::ProviderEventKind::Typing) => IncomingKind::Typing(m.active),
            Some(types::ProviderEventKind::Membership) => IncomingKind::Membership(m.active),
            _ => return Err(Error::Invalid("Invalid provider event kind".into())),
        };
        let format = match m.format.as_str() {
            "text" => "text",
            "html" => "html",
            "markdown" => "markdown",
            "slack_mrkdwn" => "slack_mrkdwn",
            "whatsapp" => "whatsapp",
            _ => return Err(Error::Invalid("Invalid provider message format".into())),
        };
        Ok(Self {
            kind,
            event_id: m.event_id,
            message_id: m.message_id,
            thread_id: m.thread_id,
            sender: crate::chat::access::identity::Identity {
                identity_type: m.identity_type.as_known().ok_or(Error::Denied)?,
                value: m.identity_value,
            },
            sender_name: m.sender_name,
            text: m.text,
            format,
            reply_to: m.reply_to,
            subject: m.subject,
            attachments: m
                .attachments
                .into_iter()
                .map(|a| RemoteAttachment {
                    id: a.id,
                    filename: a.filename,
                    media_type: a.media_type,
                    size: a.size,
                    url: a.url,
                    provider_id: a.provider_reference,
                })
                .collect(),
        })
    }
}
impl Deployments {
    /// A provider event that reached the gateway for a sidecar agent goes to the
    /// replica owning its conversation, or to any live replica for a new one.
    pub async fn forward_provider_event(
        &self,
        agent: Uuid,
        connection: Uuid,
        event: types::ProviderEvent,
    ) -> Result<()> {
        let thread = stable(
            connection,
            "thread",
            &format!("{agent}:{}", event.thread_id),
        );
        let known = sqlx::query_file!(
            "../../queries/deployment/external_thread.sql",
            connection,
            event.thread_id,
            agent
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(ChatError::from)?
        .map(|r| r.thread_id)
        .unwrap_or(thread);
        let instance = self
            .target_for(agent, Some(known))
            .await
            .map_err(|_| ChatError::Transport)?
            .ok_or_else(|| {
                ChatError::Invalid("No sidecar replica is available for this agent".into())
            })?;
        let directive = wire::ProviderEventDirective {
            connection_id: connection.to_string(),
            event: event.into(),
            ..Default::default()
        };
        self.direct(agent, instance, Some(known), directive.into())
            .await
            .map_err(|_| ChatError::Transport)?;
        Ok(())
    }
}
impl Runtime {
    pub async fn ingest_provider(&self, connection: Uuid, m: IncomingMessage) -> Result<()> {
        m.sender
            .validate()
            .map_err(|_| ChatError::Invalid("Invalid sender identity".into()))?;
        if m.text.len() > 1024 * 1024 || m.message_id.is_empty() || m.thread_id.is_empty() {
            return Err(ChatError::Invalid("Invalid provider message".into()));
        }
        let configuration = self.configuration()?;
        let c = configuration
            .connections
            .iter()
            .find(|c| c.id == connection.to_string() && c.status == "ready")
            .ok_or(ChatError::Denied)?;
        let allowed = match c.access_mode.as_known() {
            Some(types::ChannelAccessMode::Public) => true,
            Some(types::ChannelAccessMode::Private) => c.identities.iter().any(|i| {
                i.identity_type.as_known() == Some(m.sender.identity_type)
                    && i.value == m.sender.value
                    && i.verified
                    && i.allowed
            }),
            _ => false,
        };
        let kind = crate::chat::access::identity::kind_name(m.sender.identity_type);
        let user = stable(connection, "user", &format!("{kind}:{}", m.sender.value));
        let receipt = stable(connection, "receipt", &m.event_id);
        let decision = types::ChannelDecision {
            connection_id: connection.to_string(),
            event_id: m.event_id.clone(),
            identity_id: user.to_string(),
            identity_type: m.sender.identity_type.into(),
            value: m.sender.value.clone(),
            accepted: allowed,
            access_mode: c.access_mode,
            ..Default::default()
        };
        let thread = stable(
            connection,
            "thread",
            &format!("{}:{}", self.agent_id, m.thread_id),
        );
        let user_participant = stable(connection, "participant", &format!("{thread}:{user}"));
        let agent_participant = stable(connection, "agent", &thread.to_string());
        let message_id = stable(connection, "message", &format!("{thread}:{}", m.message_id));
        let name = if m.sender_name.is_empty() {
            m.sender.value.clone()
        } else {
            m.sender_name.clone()
        }
        .chars()
        .take(200)
        .collect::<String>();
        let user_row = types::Participant {
            id: user_participant.to_string(),
            user_id: Some(user.to_string()),
            name: name.clone(),
            active: true,
            ..Default::default()
        };
        let agent_row = types::Participant {
            id: agent_participant.to_string(),
            agent_id: Some(self.agent_id.to_string()),
            name: configuration.agent.name.clone(),
            active: true,
            ..Default::default()
        };
        let (shared, created) = match self.load_external(connection, &m.thread_id).await? {
            Some(shared) => (shared, false),
            None => {
                if !allowed {
                    return Ok(());
                }
                let roster = types::Thread {
                    id: thread.to_string(),
                    title: name.clone(),
                    primary_agent_id: self.agent_id.to_string(),
                    participants: vec![user_row.clone(), agent_row.clone()],
                    channel: types::ChannelBinding {
                        connection_id: connection.to_string(),
                        provider_id: c.provider_id.clone(),
                        external_id: m.thread_id.clone(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                };
                (self.adopt(store::ThreadState::new(roster)).await?, true)
            }
        };
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        if !t.receipts.insert(receipt) {
            return Ok(());
        }
        if created {
            let roster = t.thread.clone();
            self.emit(&mut t, "thread.created", roster.into(), None)?;
            self.emit(&mut t, "participant.joined", user_row.clone().into(), None)?;
            self.emit(&mut t, "participant.joined", agent_row.clone().into(), None)?;
        } else {
            for p in [&user_row, &agent_row] {
                if !t.thread.participants.iter().any(|old| old.id == p.id) {
                    t.thread.participants.push(p.clone());
                    self.emit(&mut t, "participant.joined", p.clone().into(), None)?;
                }
            }
        }
        self.emit(&mut t, "channel.access", decision.into(), None)?;
        if !allowed {
            return Ok(());
        }
        match m.kind {
            IncomingKind::Message => {
                let mut message = types::Message {
                    id: message_id.to_string(),
                    thread_id: thread.to_string(),
                    participant_id: user_participant.to_string(),
                    text: m.text,
                    status: "complete".into(),
                    format: m.format.into(),
                    subject: m.subject,
                    in_reply_to_message_id: m.reply_to.map(|key| {
                        stable(connection, "message", &format!("{thread}:{key}")).to_string()
                    }),
                    delivery: types::MessageDelivery {
                        connection_id: connection.to_string(),
                        external_message_id: m.message_id,
                        destination: m.thread_id.clone(),
                        status: "accepted".into(),
                        ..Default::default()
                    }
                    .into(),
                    created_at: crate::chat::audit::timestamp(chrono::Utc::now()).into(),
                    ..Default::default()
                };
                for a in m.attachments {
                    let key = stable(connection, "attachment", &a.id);
                    let metadata = types::Attachment {
                        id: key.to_string(),
                        thread_id: thread.to_string(),
                        filename: a.filename,
                        media_type: a.media_type,
                        size_bytes: a.size,
                        ..Default::default()
                    };
                    let source = types::AttachmentSource {
                        attachment: metadata.clone().into(),
                        connection_id: connection.to_string(),
                        provider_reference: a.provider_id,
                        url: a.url,
                        ..Default::default()
                    };
                    self.record_attachment_source(&mut t, source)?;
                    message.attachments.push(metadata);
                }
                t.upsert_message(message.clone());
                self.index_message(message_id, thread);
                self.emit(&mut t, "message.completed", message.clone().into(), None)?;
                self.route_message_in(&mut t, &message)?;
            }
            IncomingKind::Updated | IncomingKind::Deleted => {
                let mut message = t.message(message_id).cloned().ok_or(ChatError::NotFound)?;
                message.text = m.text;
                message.status = if matches!(m.kind, IncomingKind::Deleted) {
                    "deleted"
                } else {
                    "complete"
                }
                .into();
                t.upsert_message(message.clone());
                t.converted.remove(&message_id);
                self.emit(&mut t, "message.updated", message.into(), None)?;
            }
            IncomingKind::Typing(typing) => {
                let typing = types::Typing {
                    participant_id: user_participant.to_string(),
                    typing,
                    expires_at: crate::chat::audit::timestamp(
                        chrono::Utc::now() + chrono::Duration::seconds(10),
                    )
                    .into(),
                    ..Default::default()
                };
                self.emit(&mut t, "typing.changed", typing.into(), None)?;
            }
            IncomingKind::Membership(active) => {
                let mut user = user_row;
                user.active = active;
                if let Some(p) = t.thread.participants.iter_mut().find(|p| p.id == user.id) {
                    p.active = active;
                }
                self.emit(
                    &mut t,
                    if active {
                        "participant.joined"
                    } else {
                        "participant.left"
                    },
                    user.into(),
                    None,
                )?;
            }
        }
        drop(t);
        self.changed(thread);
        Ok(())
    }
    /// Record an outbound provider message the agent already sent.
    pub async fn provider_sent(&self, mut message: types::Message) -> Result<serde_json::Value> {
        let call_id = crate::chat::id(&message.id)?;
        let connection = crate::chat::id(&message.delivery.connection_id)?;
        let destination = message.delivery.destination.clone();
        let external = message.delivery.external_message_id.clone();
        let configuration = self.configuration()?;
        let c = configuration
            .connections
            .iter()
            .find(|c| c.id == connection.to_string())
            .ok_or(ChatError::Denied)?;
        let thread = stable(
            connection,
            "thread",
            &format!("{}:{destination}", self.agent_id),
        );
        let participant = stable(connection, "agent", &thread.to_string());
        let p = types::Participant {
            id: participant.to_string(),
            agent_id: Some(self.agent_id.to_string()),
            name: configuration.agent.name.clone(),
            active: true,
            ..Default::default()
        };
        let (shared, created) = match self.load_external(connection, &destination).await? {
            Some(shared) => (shared, false),
            None => {
                let root = types::Thread {
                    id: thread.to_string(),
                    title: destination.clone(),
                    primary_agent_id: self.agent_id.to_string(),
                    participants: vec![p.clone()],
                    channel: types::ChannelBinding {
                        connection_id: connection.to_string(),
                        provider_id: c.provider_id.clone(),
                        external_id: destination.clone(),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                };
                (self.adopt(store::ThreadState::new(root)).await?, true)
            }
        };
        message.thread_id = thread.to_string();
        message.participant_id = participant.to_string();
        message.status = "complete".into();
        message.delivery.get_or_insert_default().status = "accepted".into();
        message.created_at = crate::chat::audit::timestamp(chrono::Utc::now()).into();
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        if created {
            let roster = t.thread.clone();
            self.emit(&mut t, "thread.created", roster.into(), None)?;
            self.emit(&mut t, "participant.joined", p.clone().into(), None)?;
        } else if !t.thread.participants.iter().any(|old| old.id == p.id) {
            t.thread.participants.push(p.clone());
            self.emit(&mut t, "participant.joined", p.into(), None)?;
        }
        t.upsert_message(message.clone());
        self.index_message(call_id, thread);
        self.emit(&mut t, "message.completed", message.into(), None)?;
        drop(t);
        self.changed(thread);
        Ok(
            serde_json::json!({"messageId":call_id,"threadId":thread,"externalMessageId":external,"accepted":true}),
        )
    }
}
