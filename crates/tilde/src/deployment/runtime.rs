//! Corrosion implementation of the conversation domain. Each mutation commits its
//! typed state and immutable replication events together. Only owners execute agents.
mod attachments;
pub(crate) mod execution;
pub(crate) mod messages;
pub mod providers;
mod work;
use super::corrosion::{Client, client::statement};
use crate::proto::tilde::{agent_event_ingress::v1 as control, types::v1 as types};
use crate::{
    chat::{self, ChatError, Result, Scope, id, text},
    encryption::{Encryption, SealedSecret, SecretBinding},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use buffa::Message;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex as CounterMutex},
};
use tokio::sync::Mutex;
use types::runtime_event::State;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};
#[derive(Clone)]
pub struct Runtime {
    pub client: Client,
    pub agent_id: Uuid,
    pub instance_id: Uuid,
    pub encryption: Arc<Encryption>,
    pub signing_key: SecretString,
    pub host_key: SecretString,
    pub local_endpoint: String,
    pub callback_url: String,
    pub(crate) archive: Option<super::gateway::Client>,
    pub(crate) sequence: Arc<CounterMutex<BTreeMap<Uuid, i64>>>,
    pub(crate) mutation: Arc<Mutex<()>>,
    pub(crate) attachments: Arc<Mutex<BTreeMap<Uuid, Zeroizing<Vec<u8>>>>>,
}
/// Decrypted provider configuration never leaves plaintext credentials behind.
pub struct SensitiveConfiguration(control::GetConfigurationResponse);
impl std::ops::Deref for SensitiveConfiguration {
    type Target = control::GetConfigurationResponse;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for SensitiveConfiguration {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for SensitiveConfiguration {
    fn drop(&mut self) {
        self.0.webhook_signing_key.zeroize();
        for connection in &mut self.0.connections {
            for field in &mut connection.credentials {
                field.value.zeroize();
            }
        }
    }
}
#[derive(Deserialize)]
pub struct Payload {
    pub payload: String,
}
#[derive(Deserialize)]
pub struct EventRow {
    pub id: String,
    pub thread_id: String,
    pub agent_id: String,
    pub origin_instance_id: String,
    pub origin_sequence: i64,
    pub created_at: i64,
    pub kind: String,
    pub payload: String,
}
#[derive(Deserialize)]
pub struct IdRow {
    pub id: String,
}
pub struct RuntimeOptions {
    pub agent_id: Uuid,
    pub instance_id: Uuid,
    pub encryption: Arc<Encryption>,
    pub signing_key: SecretString,
    pub host_key: SecretString,
    pub local_endpoint: String,
    pub callback_url: String,
}
impl Runtime {
    pub fn with_archive(mut self, archive: super::gateway::Client) -> Self {
        self.archive = Some(archive);
        self
    }
    pub fn new(client: Client, options: RuntimeOptions) -> Self {
        Self {
            client,
            agent_id: options.agent_id,
            instance_id: options.instance_id,
            encryption: options.encryption,
            signing_key: options.signing_key,
            host_key: options.host_key,
            local_endpoint: options.local_endpoint,
            callback_url: options.callback_url,
            archive: None,
            sequence: Arc::default(),
            mutation: Arc::default(),
            attachments: Arc::default(),
        }
    }

    pub async fn configure(&self, configuration: &control::GetConfigurationResponse) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let payload = self.seal(self.agent_id, "configuration", configuration)?;
        self.client.transaction(vec![statement("INSERT INTO configuration(agent_id,generation,payload) VALUES(?,?,?) ON CONFLICT(agent_id) DO UPDATE SET generation=excluded.generation,payload=excluded.payload",vec![json!(self.agent_id),json!(configuration.agent_generation),json!(payload)])]).await?;
        if !configuration.logs_enabled {
            self.client
                .transaction(vec![statement("DELETE FROM logs", vec![])])
                .await?;
        }
        if !configuration.tracing_enabled {
            self.client
                .transaction(vec![statement("DELETE FROM traces", vec![])])
                .await?;
        }
        #[derive(Deserialize)]
        struct Counter {
            thread_id: String,
            sequence: i64,
        }
        let rows=self.client.query::<Counter>("SELECT thread_id,MAX(origin_sequence) AS sequence FROM events WHERE origin_instance_id=? AND (origin_agent_id=? OR origin_agent_id='') GROUP BY thread_id",vec![json!(self.instance_id),json!(self.agent_id)]).await?;
        let mut counters = self.sequence.lock().map_err(|_| ChatError::Transport)?;
        for row in rows {
            counters
                .entry(id(&row.thread_id)?)
                .and_modify(|value| *value = (*value).max(row.sequence))
                .or_insert(row.sequence);
        }
        Ok(())
    }
    pub async fn configuration(&self) -> Result<SensitiveConfiguration> {
        let row = self
            .client
            .query::<Payload>(
                "SELECT payload FROM configuration WHERE agent_id=?",
                vec![json!(self.agent_id)],
            )
            .await?
            .pop()
            .ok_or(ChatError::NotFound)?;
        self.open(self.agent_id, "configuration", &row.payload)
            .map(SensitiveConfiguration)
    }
    pub fn seal<M: Message>(&self, record: Uuid, kind: &str, value: &M) -> Result<String> {
        let bytes = Zeroizing::new(value.encode_to_vec());
        let encoded = SecretString::from(STANDARD.encode(bytes.as_slice()));
        let secret = self
            .encryption
            .seal(
                SecretBinding {
                    resource_kind: "sidecar",
                    resource_id: record,
                    name: kind,
                },
                &encoded,
            )
            .map_err(|_| ChatError::Transport)?;
        Ok(STANDARD.encode(secret.into_bytes()))
    }
    pub fn open<M: Message>(&self, record: Uuid, kind: &str, encoded: &str) -> Result<M> {
        let bytes = STANDARD.decode(encoded).map_err(|_| ChatError::Transport)?;
        let value = self
            .encryption
            .open(
                SecretBinding {
                    resource_kind: "sidecar",
                    resource_id: record,
                    name: kind,
                },
                SealedSecret::from_bytes(&bytes).map_err(|_| ChatError::Transport)?,
            )
            .map_err(|_| ChatError::Transport)?;
        let decoded = Zeroizing::new(
            STANDARD
                .decode(value.expose_secret())
                .map_err(|_| ChatError::Transport)?,
        );
        drop(value);
        M::decode_from_slice(decoded.as_slice()).map_err(|_| ChatError::Transport)
    }
    pub(crate) fn event(&self, thread: Uuid, kind: &str, state: State) -> Result<Value> {
        self.event_inner(thread, kind, state, None)
    }
    pub(crate) fn event_scoped(&self, s: &Scope, kind: &str, state: State) -> Result<Value> {
        self.event_inner(s.thread_id, kind, state, Some(s))
    }
    fn event_inner(
        &self,
        thread: Uuid,
        kind: &str,
        state: State,
        scope: Option<&Scope>,
    ) -> Result<Value> {
        let key = Uuid::new_v4();
        let sequence = {
            let mut counters = self.sequence.lock().map_err(|_| ChatError::Transport)?;
            let value = counters.entry(thread).or_default();
            *value += 1;
            *value
        };
        let event = types::RuntimeEvent {
            origin_agent_id: self.agent_id.to_string(),
            invocation_id: scope.map(|s| s.id.to_string()).unwrap_or_default(),
            participant_id: scope
                .map(|s| s.participant_id.to_string())
                .unwrap_or_default(),
            id: key.to_string(),
            thread_id: thread.to_string(),
            agent_id: self.agent_id.to_string(),
            origin_instance_id: self.instance_id.to_string(),
            origin_sequence: sequence,
            created_at: now(),
            kind: kind.into(),
            state: Some(state),
            ..Default::default()
        };
        Ok(statement(
            "INSERT INTO events(id,thread_id,agent_id,origin_instance_id,origin_agent_id,origin_sequence,created_at,kind,payload) VALUES(?,?,?,?,?,?,?,?,?)",
            vec![
                json!(key),
                json!(thread),
                json!(self.agent_id),
                json!(self.instance_id),
                json!(self.agent_id),
                json!(sequence),
                json!(event.created_at),
                json!(kind),
                json!(self.seal(key, "event", &event)?),
            ],
        ))
    }
    pub(crate) async fn commit(&self, thread: Uuid, mut statements: Vec<Value>) -> Result<()> {
        statements.push(statement("INSERT INTO write_guards(id) SELECT NULL WHERE NOT EXISTS(SELECT 1 FROM threads WHERE id=?) OR EXISTS(SELECT 1 FROM retirements WHERE thread_id=?)",vec![json!(thread),json!(thread)]));
        statements.push(statement(
            "UPDATE threads SET last_activity_at=? WHERE id=?",
            vec![json!(now()), json!(thread)],
        ));
        self.client.transaction(statements).await?;
        Ok(())
    }
    pub async fn exists(&self, thread: Uuid) -> Result<bool> {
        Ok(!self
            .client
            .query::<IdRow>("SELECT id FROM threads WHERE id=?", vec![json!(thread)])
            .await?
            .is_empty())
    }
    pub async fn create_user(&self, name: &str) -> Result<types::User> {
        text(name)?;
        if name.chars().count() > 200 {
            return Err(ChatError::Invalid("Name is too long".into()));
        }
        let user = types::User {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            ..Default::default()
        };
        self.client
            .transaction(vec![
                statement(
                    "INSERT INTO users(id,name) VALUES(?,?)",
                    vec![json!(user.id), json!(name)],
                ),
                self.event(Uuid::nil(), "user.created", user.clone().into())?,
            ])
            .await?;
        Ok(user)
    }
    pub async fn create_thread(&self, r: chat::CreateThread) -> Result<types::Thread> {
        text(&r.title)?;
        if r.participants.is_empty() || r.participants.len() > 100 {
            return Err(ChatError::Invalid(
                "Thread requires 1-100 participants".into(),
            ));
        }
        if id(&r.primary_agent_id)? != self.agent_id {
            return Err(ChatError::Denied);
        }
        let _guard = self.mutation.lock().await;
        let thread_id = Uuid::new_v4();
        let mut thread = types::Thread {
            id: thread_id.to_string(),
            title: r.title,
            primary_agent_id: r.primary_agent_id,
            ..Default::default()
        };
        let mut writes = vec![];
        for p in r.participants {
            let participant = self.participant(thread_id, p).await?;
            writes.push(self.participant_insert(thread_id, &participant)?);
            writes.push(self.event(thread_id, "participant.joined", participant.clone().into())?);
            if participant.agent_id.as_deref() == Some(&self.agent_id.to_string()) {
                let assignment = self.assignment(thread_id, id(&participant.id)?).await?;
                writes.push(self.assignment_insert(&assignment));
                writes.push(self.event(thread_id, "participant.assigned", assignment.into())?);
            }
            thread.participants.push(participant);
        }
        if !thread
            .participants
            .iter()
            .any(|p| p.agent_id.as_deref() == Some(&thread.primary_agent_id))
        {
            return Err(ChatError::Invalid("Primary agent must participate".into()));
        }
        writes.insert(0,statement("INSERT INTO threads(id,title,primary_agent_id,last_activity_at,payload) VALUES(?,?,?,?,?)",vec![json!(thread.id),json!(thread.title),json!(thread.primary_agent_id),json!(now()),json!(self.seal(thread_id,"thread",&thread)?)]));
        writes.push(self.event(thread_id, "thread.created", thread.clone().into())?);
        self.commit(thread_id, writes).await?;
        Ok(thread)
    }
    async fn participant(
        &self,
        thread: Uuid,
        p: types::ParticipantRef,
    ) -> Result<types::Participant> {
        if p.user_id.is_some() == p.agent_id.is_some() {
            return Err(ChatError::Invalid(
                "Participant names exactly one user or agent".into(),
            ));
        }
        let identity = p
            .agent_id
            .as_deref()
            .or(p.user_id.as_deref())
            .ok_or(ChatError::NotFound)?;
        id(identity)?;
        let key = Uuid::new_v5(&thread, identity.as_bytes());
        let name = if p.agent_id.as_deref() == Some(&self.agent_id.to_string()) {
            self.configuration().await?.agent.name.clone()
        } else if let Some(user) = &p.user_id {
            #[derive(Deserialize)]
            struct Name {
                name: String,
            }
            self.client
                .query::<Name>("SELECT name FROM users WHERE id=?", vec![json!(user)])
                .await?
                .pop()
                .ok_or(ChatError::NotFound)?
                .name
        } else {
            // New cross-agent membership is a registry decision. Existing
            // replicated rosters and the local agent remain usable offline.
            self.archive
                .as_ref()
                .ok_or(ChatError::Transport)?
                .agent_reference(self.agent_id, id(identity)?)
                .await?
                .name
        };
        Ok(types::Participant {
            id: key.to_string(),
            name,
            user_id: p.user_id,
            agent_id: p.agent_id,
            active: true,
            ..Default::default()
        })
    }
    pub(crate) fn participant_insert(&self, thread: Uuid, p: &types::Participant) -> Result<Value> {
        Ok(statement(
            "INSERT INTO participants(id,thread_id,agent_id,user_id,active,payload) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET active=excluded.active,payload=excluded.payload",
            vec![
                json!(p.id),
                json!(thread),
                json!(p.agent_id.as_deref().unwrap_or("")),
                json!(p.user_id.as_deref().unwrap_or("")),
                json!(i32::from(p.active)),
                json!(self.seal(id(&p.id)?, "participant", p)?),
            ],
        ))
    }
    pub async fn thread(&self, key: Uuid) -> Result<types::Thread> {
        let row = self
            .client
            .query::<Payload>("SELECT payload FROM threads WHERE id=?", vec![json!(key)])
            .await?
            .pop()
            .ok_or(ChatError::NotFound)?;
        let mut thread: types::Thread = self.open(key, "thread", &row.payload)?;
        #[derive(Deserialize)]
        struct Row {
            id: String,
            payload: String,
        }
        let rows = self
            .client
            .query::<Row>(
                "SELECT id,payload FROM participants WHERE thread_id=? ORDER BY id",
                vec![json!(key)],
            )
            .await?;
        thread.participants = rows
            .into_iter()
            .map(|r| self.open(id(&r.id)?, "participant", &r.payload))
            .collect::<Result<_>>()?;
        Ok(thread)
    }
    pub async fn add_participant(&self, r: chat::AddParticipant) -> Result<types::Thread> {
        let thread = id(&r.thread_id)?;
        self.thread(thread).await?;
        let _guard = self.mutation.lock().await;
        let participant = self
            .participant(thread, r.participant.ok_or(ChatError::NotFound)?)
            .await?;
        self.commit(
            thread,
            vec![
                self.participant_insert(thread, &participant)?,
                self.event(thread, "participant.joined", participant.into())?,
            ],
        )
        .await?;
        self.thread(thread).await
    }
    pub async fn remove_participant(
        &self,
        thread: Uuid,
        participant: Uuid,
    ) -> Result<types::Thread> {
        let mut p = self
            .thread(thread)
            .await?
            .participants
            .into_iter()
            .find(|p| p.id == participant.to_string())
            .ok_or(ChatError::NotFound)?;
        if p.agent_id.as_deref() == Some(&self.thread(thread).await?.primary_agent_id) {
            return Err(ChatError::Conflict);
        }
        p.active = false;
        let _guard = self.mutation.lock().await;
        self.commit(
            thread,
            vec![
                self.participant_insert(thread, &p)?,
                self.event(thread, "participant.left", p.into())?,
            ],
        )
        .await?;
        self.thread(thread).await
    }
    pub async fn message(&self, key: Uuid) -> Result<types::Message> {
        let row=self.client.query::<Payload>("SELECT COALESCE((SELECT payload FROM bridge_message_versions WHERE message_id=messages.id ORDER BY sequence DESC LIMIT 1),payload) AS payload FROM messages WHERE id=?",vec![json!(key)]).await?.pop().ok_or(ChatError::NotFound)?;
        self.open(key, "message", &row.payload)
    }
    pub async fn messages(&self, thread: Uuid, limit: u32) -> Result<Vec<types::Message>> {
        Ok(self.message_page(thread, limit, None).await?.messages)
    }
    pub async fn message_page(
        &self,
        thread: Uuid,
        limit: u32,
        before: Option<Uuid>,
    ) -> Result<chat::MessagePage> {
        self.thread(thread).await?;
        let size = if limit == 0 { 100 } else { limit.min(100) };
        #[derive(Deserialize)]
        struct Row {
            id: String,
            payload: String,
        }
        let mut rows=self.client.query::<Row>("SELECT id,COALESCE((SELECT payload FROM bridge_message_versions WHERE message_id=messages.id ORDER BY sequence DESC LIMIT 1),payload) AS payload FROM messages WHERE thread_id=? AND (? IS NULL OR (created_at,id)<(SELECT created_at,id FROM messages WHERE id=?)) ORDER BY created_at DESC,id DESC LIMIT ?",vec![json!(thread),json!(before),json!(before),json!(size+1)]).await?;
        let more = rows.len() > size as usize;
        if more {
            rows.pop();
        }
        let cursor = if more {
            rows.last().map(|r| r.id.clone()).unwrap_or_default()
        } else {
            String::new()
        };
        let messages = rows
            .into_iter()
            .map(|r| self.open(id(&r.id)?, "message", &r.payload))
            .collect::<Result<_>>()?;
        Ok(chat::MessagePage {
            messages,
            next_page_token: cursor,
        })
    }
    pub async fn post(&self, r: chat::PostMessage) -> Result<types::Message> {
        let key = id(&r.id)?;
        let thread = id(&r.thread_id)?;
        let participant = id(&r.participant_id)?;
        if r.text.is_empty() && r.attachment_ids.is_empty() {
            return Err(ChatError::Invalid("Message needs content".into()));
        }
        if r.text.len() > 1024 * 1024 || r.attachment_ids.len() > 20 {
            return Err(ChatError::Invalid("Message is too large".into()));
        }
        let roster = self.thread(thread).await?;
        if !roster
            .participants
            .iter()
            .any(|p| p.id == r.participant_id && p.active)
        {
            return Err(ChatError::Denied);
        }
        for target in &r.addressed_participant_ids {
            if !roster
                .participants
                .iter()
                .any(|p| p.id == *target && p.active)
            {
                return Err(ChatError::NotFound);
            }
        }
        if let Some(reply) = &r.in_reply_to_message_id
            && self.message(id(reply)?).await?.thread_id != r.thread_id
        {
            return Err(ChatError::NotFound);
        }
        let _guard = self.mutation.lock().await;
        match self.message(key).await {
            Ok(old) => {
                if old.thread_id == r.thread_id
                    && old.participant_id == r.participant_id
                    && old.text == r.text
                {
                    return Ok(old);
                }
                return Err(ChatError::Conflict);
            }
            Err(ChatError::NotFound) => {}
            Err(e) => return Err(e),
        }
        let mut attachments = vec![];
        for attachment in &r.attachment_ids {
            let value = self.attachment_metadata(thread, id(attachment)?).await?;
            attachments.push(value);
        }
        let message = types::Message {
            id: key.to_string(),
            thread_id: thread.to_string(),
            participant_id: participant.to_string(),
            text: r.text,
            status: "complete".into(),
            addressed_participant_ids: r.addressed_participant_ids,
            in_reply_to_message_id: r.in_reply_to_message_id,
            attachments,
            format: "text".into(),
            created_at: crate::chat::audit::timestamp(chrono::Utc::now()).into(),
            ..Default::default()
        };
        let mut writes = vec![
            self.message_insert(&message)?,
            self.event(thread, "message.created", message.clone().into())?,
        ];
        writes.extend(self.route_message_writes(&message).await?);
        self.commit(thread, writes).await?;
        Ok(message)
    }
    pub(crate) fn message_insert(&self, m: &types::Message) -> Result<Value> {
        Ok(statement(
            "INSERT INTO messages(id,thread_id,participant_id,status,created_at,payload) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,payload=excluded.payload",
            vec![
                json!(m.id),
                json!(m.thread_id),
                json!(m.participant_id),
                json!(m.status),
                json!(m.created_at.seconds * 1000 + i64::from(m.created_at.nanos) / 1_000_000),
                json!(self.seal(id(&m.id)?, "message", m)?),
            ],
        ))
    }
}
pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
