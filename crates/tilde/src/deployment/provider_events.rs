//! Provider payload parsing stays in the existing adapters. This module moves
//! verified, typed events between the local runtime and archived gateway state.
use super::{
    Deployments,
    corrosion::client::statement,
    runtime::{Runtime, now},
};
use crate::{
    chat::{
        ChatError, Result,
        providers::ingress::{IncomingKind, IncomingMessage, RemoteAttachment, stable},
    },
    error::Error,
    proto::tilde::types::v1 as types,
};
use serde_json::json;
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
    pub async fn locate(&self, agent: Uuid, thread: Uuid) -> std::result::Result<String, Error> {
        if let Some(row) = sqlx::query_file!(
            "../../queries/deployment/project/location.sql",
            thread,
            agent
        )
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(row.storage);
        }
        if sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
            .fetch_one(&self.pool)
            .await?
            .allowed
        {
            return Ok("postgres".into());
        }
        Ok("not_found".into())
    }
    pub async fn ingest_archived(
        &self,
        agent: Uuid,
        connection: Uuid,
        event: types::ProviderEvent,
    ) -> std::result::Result<(), Error> {
        let owner = sqlx::query_file!("../../queries/chat/channel_owner.sql", connection)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::NotFound)?;
        if owner.agent_id != agent {
            return Err(Error::Denied);
        }
        let thread = stable(
            connection,
            "thread",
            &format!("{agent}:{}", event.thread_id),
        );
        match self.locate(agent, thread).await?.as_str() {
            "postgres" => {
                self.chat().ingest(connection, event.try_into()?).await?;
                Ok(())
            }
            "corrosion" => {
                let settings = sqlx::query_file!("../../queries/deployment/get.sql", agent)
                    .fetch_one(&self.pool)
                    .await?;
                let secrets = self.open_secrets(
                    agent,
                    settings.encrypted_secrets.as_deref().ok_or(Error::Denied)?,
                )?;
                use secrecy::ExposeSecret;
                for node in
                    sqlx::query_file!("../../queries/deployment/project/node_available.sql", agent)
                        .fetch_all(&self.pool)
                        .await?
                {
                    let peer = super::corrosion::Client::new(
                        &format!("{}/corrosion", node.agent_ingress_url.trim_end_matches('/')),
                        secrets.api_token.clone(),
                    )?;
                    if !peer
                        .query::<super::runtime::IdRow>(
                            "SELECT id FROM threads WHERE id=?",
                            vec![json!(thread)],
                        )
                        .await
                        .is_ok_and(|r| !r.is_empty())
                    {
                        continue;
                    }
                    let response = reqwest::Client::new()
                        .post(format!(
                            "{}/provider-events/{connection}",
                            node.agent_ingress_url.trim_end_matches('/')
                        ))
                        .bearer_auth(secrets.api_token.expose_secret())
                        .json(&event)
                        .send()
                        .await
                        .map_err(|_| Error::Invalid("Live peer is unavailable".into()))?;
                    if !response.status().is_success() {
                        return Err(Error::Invalid("Live peer did not accept event".into()));
                    }
                    return Ok(());
                }
                Err(Error::Invalid("Live conversation is unavailable".into()))
            }
            _ => Err(Error::Invalid("Conversation storage is unavailable".into())),
        }
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
        let configuration = self.configuration().await?;
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
        let _guard = self.mutation.lock().await;
        if !self
            .client
            .query::<super::runtime::IdRow>(
                "SELECT id FROM channel_receipts WHERE id=?",
                vec![json!(receipt)],
            )
            .await?
            .is_empty()
        {
            return Ok(());
        }
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
        if !allowed {
            self.client.transaction(vec![statement("INSERT INTO channel_receipts(id,thread_id,connection_id,external_id,message_id) VALUES(?,'',?,?,'')",vec![json!(receipt),json!(connection),json!(m.event_id)]),self.event(Uuid::nil(),"channel.access",decision.into())?]).await?;
            return Ok(());
        }
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
        let mut writes = vec![statement(
            "INSERT INTO users(id,name) VALUES(?,?) ON CONFLICT(id) DO NOTHING",
            vec![json!(user), json!(name)],
        )];
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
        let mut roster = match self.thread(thread).await {
            Ok(thread) => thread,
            Err(ChatError::NotFound) => types::Thread {
                id: thread.to_string(),
                title: name,
                primary_agent_id: self.agent_id.to_string(),
                channel: types::ChannelBinding {
                    connection_id: connection.to_string(),
                    provider_id: c.provider_id.clone(),
                    external_id: m.thread_id.clone(),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
            Err(e) => return Err(e),
        };
        for p in [&user_row, &agent_row] {
            if !roster.participants.iter().any(|old| old.id == p.id) {
                roster.participants.push(p.clone());
                writes.push(self.event(thread, "participant.joined", p.clone().into())?);
            }
            writes.push(self.participant_insert(thread, p)?);
        }
        writes.insert(0,statement("INSERT INTO threads(id,title,primary_agent_id,last_activity_at,payload) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(thread),json!(roster.title),json!(self.agent_id),json!(now()),json!(self.seal(thread,"thread",&roster)?)]));
        writes.push(statement("INSERT INTO channel_threads(id,connection_id,external_id,thread_id,agent_id) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(thread),json!(connection),json!(m.thread_id),json!(thread),json!(self.agent_id)]));
        let assignment = self.assignment(thread, agent_participant).await?;
        writes.push(self.assignment_insert(&assignment));
        writes.push(self.event(thread, "participant.assigned", assignment.into())?);
        writes.push(self.event(thread, "channel.access", decision.into())?);
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
                    writes.push(statement("INSERT INTO attachments(id,thread_id,holder_instance_id,payload,source) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(key),json!(thread),json!(self.instance_id),json!(self.seal(key,"attachment",&metadata)?),json!(self.seal(key,"attachment_source",&source)?)]));
                    writes.push(self.event(thread, "attachment.source", source.into())?);
                    message.attachments.push(metadata);
                }
                writes.push(self.message_insert(&message)?);
                writes.push(self.event(thread, "message.completed", message.clone().into())?);
                writes.extend(self.route_message_in(&message, &roster).await?);
            }
            IncomingKind::Updated | IncomingKind::Deleted => {
                let mut message = self.message(message_id).await?;
                message.text = m.text;
                message.status = if matches!(m.kind, IncomingKind::Deleted) {
                    "deleted"
                } else {
                    "complete"
                }
                .into();
                writes.push(self.message_insert(&message)?);
                writes.push(statement(
                    "DELETE FROM converted_messages WHERE message_id=?",
                    vec![json!(message_id)],
                ));
                writes.push(self.event(thread, "message.updated", message.into())?);
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
                writes.push(self.event(thread, "typing.changed", typing.into())?);
            }
            IncomingKind::Membership(active) => {
                let mut user = user_row;
                user.active = active;
                writes.push(self.participant_insert(thread, &user)?);
                writes.push(self.event(
                    thread,
                    if active {
                        "participant.joined"
                    } else {
                        "participant.left"
                    },
                    user.into(),
                )?);
            }
        }
        writes.push(statement("INSERT INTO channel_receipts(id,thread_id,connection_id,external_id,message_id) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(receipt),json!(thread),json!(connection),json!(m.event_id),json!(message_id)]));
        self.commit(thread, writes).await
    }
    pub async fn provider_sent(&self, mut message: types::Message) -> Result<serde_json::Value> {
        let call_id = crate::chat::id(&message.id)?;
        let connection = crate::chat::id(&message.delivery.connection_id)?;
        let destination = message.delivery.destination.clone();
        let external = message.delivery.external_message_id.clone();
        let configuration = self.configuration().await?;
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
        message.thread_id = thread.to_string();
        message.participant_id = participant.to_string();
        message.status = "complete".into();
        message.delivery.get_or_insert_default().status = "accepted".into();
        message.created_at = crate::chat::audit::timestamp(chrono::Utc::now()).into();
        let _guard = self.mutation.lock().await;
        self.commit(thread,vec![statement("INSERT INTO threads(id,title,primary_agent_id,last_activity_at,payload) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(thread),json!(destination),json!(self.agent_id),json!(now()),json!(self.seal(thread,"thread",&root)?)]),self.participant_insert(thread,&p)?,statement("INSERT INTO channel_threads(id,connection_id,external_id,thread_id,agent_id) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(thread),json!(connection),json!(destination),json!(thread),json!(self.agent_id)]),self.message_insert(&message)?,self.event(thread,"message.completed",message.into())?]).await?;
        Ok(
            json!({"messageId":call_id,"threadId":thread,"externalMessageId":external,"accepted":true}),
        )
    }
}
