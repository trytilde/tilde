//! In-memory conversation domain for the replica that owns a thread. Every
//! mutation happens under one per-thread lock, appends typed events, and hands
//! them to the shipper. Nothing is written locally; the gateway is the record.
mod attachments;
pub(crate) mod execution;
pub(crate) mod messages;
pub(crate) mod outbox;
pub mod providers;
pub(crate) mod store;
mod work;
use super::gateway;
use crate::chat::{self, ChatError, Result, Scope, id, text};
use crate::proto::tilde::{agent_event_ingress::v1 as control, types::v1 as types};
use secrecy::SecretString;
use std::{
    collections::HashSet,
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{
        Arc, Mutex as SyncMutex, RwLock,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
};
use store::ThreadState;
use tokio::sync::Mutex;
use types::runtime_event::State;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

pub(crate) type Shared = Arc<Mutex<ThreadState>>;
#[derive(Clone)]
pub struct Runtime {
    pub agent_id: Uuid,
    pub instance_id: Uuid,
    pub signing_key: SecretString,
    pub host_key: SecretString,
    pub local_endpoint: String,
    pub callback_url: String,
    pub(crate) gateway: gateway::Client,
    pub(crate) state: Arc<Memory>,
}
/// Decrypted provider configuration never leaves plaintext credentials behind.
pub struct SensitiveConfiguration(control::GetConfigurationResponse);
impl std::ops::Deref for SensitiveConfiguration {
    type Target = control::GetConfigurationResponse;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for SensitiveConfiguration {
    fn drop(&mut self) {
        providers::clear_secrets(&mut self.0);
    }
}
pub(crate) struct Memory {
    configuration: RwLock<Option<Arc<SensitiveConfiguration>>>,
    users: RwLock<HashMap<Uuid, String>>,
    threads: RwLock<HashMap<Uuid, Shared>>,
    message_threads: RwLock<HashMap<Uuid, Uuid>>,
    pub(crate) run_threads: RwLock<HashMap<Uuid, Uuid>>,
    pub(crate) invocation_threads: RwLock<HashMap<Uuid, Uuid>>,
    pub(crate) agent_healthy: AtomicBool,
    external: RwLock<HashMap<(Uuid, String), Uuid>>,
    hydrating: std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>,
    pub(crate) directives_in_flight: std::sync::Mutex<HashSet<Uuid>>,
    pub(crate) outbox: SyncMutex<VecDeque<control::Upstream>>,
    pub(crate) outbox_notify: tokio::sync::Notify,
    pub(crate) work: SyncMutex<VecDeque<(Uuid, Uuid)>>,
    pub(crate) work_notify: tokio::sync::Notify,
    changes: tokio::sync::broadcast::Sender<Uuid>,
    pub(crate) gateway_ok: AtomicI64,
    pub(crate) serving: AtomicBool,
    global_sequence: AtomicI64,
    pub(crate) attachments: Mutex<BTreeMap<Uuid, Zeroizing<Vec<u8>>>>,
}
pub struct RuntimeOptions {
    pub agent_id: Uuid,
    pub instance_id: Uuid,
    pub signing_key: SecretString,
    pub host_key: SecretString,
    pub local_endpoint: String,
    pub callback_url: String,
}
impl Runtime {
    pub fn new(gateway: gateway::Client, options: RuntimeOptions) -> Self {
        let (changes, _) = tokio::sync::broadcast::channel(4096);
        Self {
            agent_id: options.agent_id,
            instance_id: options.instance_id,
            signing_key: options.signing_key,
            host_key: options.host_key,
            local_endpoint: options.local_endpoint,
            callback_url: options.callback_url,
            gateway,
            state: Arc::new(Memory {
                configuration: RwLock::new(None),
                users: RwLock::default(),
                threads: RwLock::default(),
                message_threads: RwLock::default(),
                run_threads: RwLock::default(),
                invocation_threads: RwLock::default(),
                agent_healthy: AtomicBool::new(false),
                external: RwLock::default(),
                hydrating: std::sync::Mutex::default(),
                directives_in_flight: std::sync::Mutex::default(),
                outbox: SyncMutex::default(),
                outbox_notify: tokio::sync::Notify::new(),
                work: SyncMutex::default(),
                work_notify: tokio::sync::Notify::new(),
                changes,
                gateway_ok: AtomicI64::new(now()),
                serving: AtomicBool::new(false),
                global_sequence: AtomicI64::new(0),
                attachments: Mutex::default(),
            }),
        }
    }
    /// Replace the replicated configuration; credentials are held only in memory.
    pub fn configure(&self, configuration: control::GetConfigurationResponse) {
        let mut slot = self
            .state
            .configuration
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *slot = Some(Arc::new(SensitiveConfiguration(configuration)));
        drop(slot);
        self.state.work_notify.notify_one();
        let threads: Vec<Uuid> = self
            .state
            .threads
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .copied()
            .collect();
        for thread in threads {
            let _ = self.state.changes.send(thread);
        }
    }
    pub fn configuration(&self) -> Result<Arc<SensitiveConfiguration>> {
        self.state
            .configuration
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or(ChatError::NotFound)
    }
    pub fn paused(&self) -> bool {
        self.configuration().map(|c| c.agent.paused).unwrap_or(true)
    }
    pub(crate) fn gateway_healthy(&self) -> bool {
        now() - self.state.gateway_ok.load(Ordering::Acquire) < 30_000
    }
    pub(crate) fn changes(&self) -> tokio::sync::broadcast::Receiver<Uuid> {
        self.state.changes.subscribe()
    }
    pub(crate) fn changed(&self, thread: Uuid) {
        let _ = self.state.changes.send(thread);
    }
    pub(crate) fn owns(&self, t: &ThreadState) -> bool {
        t.assignment.owner_instance_id == self.instance_id.to_string() && !t.assignment.stopped
    }
    pub(crate) fn require_owner(&self, t: &ThreadState) -> Result<()> {
        if self.owns(t) {
            Ok(())
        } else {
            Err(ChatError::Denied)
        }
    }
    /// Queue a frame for the gateway. Capacity is checked up front by
    /// `ensure_capacity` so that state already mutated is never left unpublished.
    pub(crate) fn push(&self, frame: control::upstream::Frame) {
        let mut outbox = self.state.outbox.lock().unwrap_or_else(|e| e.into_inner());
        outbox.push_back(control::Upstream {
            frame: Some(frame),
            ..Default::default()
        });
        drop(outbox);
        self.state.outbox_notify.notify_one();
    }
    pub(crate) fn ensure_capacity(&self) -> Result<()> {
        if self
            .state
            .outbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
            >= store::OUTBOX_LIMIT
        {
            return Err(ChatError::Transport);
        }
        Ok(())
    }
    /// Append one typed event to the thread and queue it for the gateway.
    pub(crate) fn emit(
        &self,
        t: &mut ThreadState,
        kind: &str,
        state: State,
        scope: Option<&Scope>,
    ) -> Result<()> {
        t.sequence += 1;
        let event = types::RuntimeEvent {
            id: Uuid::new_v4().to_string(),
            thread_id: t.thread.id.clone(),
            agent_id: self.agent_id.to_string(),
            origin_instance_id: self.instance_id.to_string(),
            origin_agent_id: self.agent_id.to_string(),
            origin_sequence: t.sequence,
            created_at: now(),
            kind: kind.into(),
            invocation_id: scope.map(|s| s.id.to_string()).unwrap_or_default(),
            participant_id: scope
                .map(|s| s.participant_id.to_string())
                .unwrap_or_default(),
            state: Some(state),
            ..Default::default()
        };
        t.last_activity_at = event.created_at;
        self.push(
            control::Event {
                event: event.clone().into(),
                generation: t.assignment.generation,
                ..Default::default()
            }
            .into(),
        );
        t.events.push_back(event);
        t.retain_window();
        Ok(())
    }
    fn emit_global(&self, kind: &str, state: State) -> Result<()> {
        let sequence = self.state.global_sequence.fetch_add(1, Ordering::AcqRel) + 1;
        let event = types::RuntimeEvent {
            id: Uuid::new_v4().to_string(),
            thread_id: Uuid::nil().to_string(),
            agent_id: self.agent_id.to_string(),
            origin_instance_id: self.instance_id.to_string(),
            origin_agent_id: self.agent_id.to_string(),
            origin_sequence: sequence,
            created_at: now(),
            kind: kind.into(),
            state: Some(state),
            ..Default::default()
        };
        self.push(
            control::Event {
                event: event.into(),
                generation: 0,
                ..Default::default()
            }
            .into(),
        );
        Ok(())
    }
    pub(crate) fn local(&self, thread: Uuid) -> Option<Shared> {
        self.state
            .threads
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&thread)
            .cloned()
    }
    pub fn exists(&self, thread: Uuid) -> bool {
        self.local(thread).is_some()
    }
    fn insert_thread(&self, t: ThreadState) -> Shared {
        let key = id(&t.thread.id).unwrap_or_default();
        if let Some(channel) = t.thread.channel.as_option()
            && let Ok(connection) = id(&channel.connection_id)
        {
            self.state
                .external
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert((connection, channel.external_id.clone()), key);
        }
        {
            let mut index = self
                .state
                .message_threads
                .write()
                .unwrap_or_else(|e| e.into_inner());
            for message in t.message_keys.keys() {
                index.insert(*message, key);
            }
        }
        {
            let mut runs = self
                .state
                .run_threads
                .write()
                .unwrap_or_else(|e| e.into_inner());
            for run in t.runs.keys() {
                runs.insert(*run, key);
            }
        }
        let shared = Arc::new(Mutex::new(t));
        self.state
            .threads
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .entry(key)
            .or_insert(shared)
            .clone()
    }
    pub(crate) fn index_message(&self, message: Uuid, thread: Uuid) {
        self.state
            .message_threads
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(message, thread);
    }
    /// Local state, or a gateway hydration that also claims ownership.
    pub(crate) async fn load(&self, thread: Uuid) -> Result<Shared> {
        if let Some(t) = self.local(thread) {
            return Ok(t);
        }
        let lock = self.hydration_lock(thread.to_string());
        let _guard = lock.lock().await;
        if let Some(t) = self.local(thread) {
            return Ok(t);
        }
        let response = self
            .gateway
            .hydrate(control::HydrateRequest {
                key: Some(control::hydrate_request::Key::ThreadId(thread.to_string())),
                claim: true,
                instance_id: self.instance_id.to_string(),
                ..Default::default()
            })
            .await?;
        if !response.found {
            return Err(ChatError::NotFound);
        }
        Ok(self.insert_thread(self.hydrated(response)?))
    }
    pub(crate) async fn load_external(
        &self,
        connection: Uuid,
        external: &str,
    ) -> Result<Option<Shared>> {
        let known = self
            .state
            .external
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(connection, external.to_owned()))
            .copied();
        if let Some(thread) = known
            && let Some(t) = self.local(thread)
        {
            return Ok(Some(t));
        }
        let lock = self.hydration_lock(format!("{connection}:{external}"));
        let _guard = lock.lock().await;
        let known = self
            .state
            .external
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(connection, external.to_owned()))
            .copied();
        if let Some(thread) = known
            && let Some(t) = self.local(thread)
        {
            return Ok(Some(t));
        }
        let response = self
            .gateway
            .hydrate(control::HydrateRequest {
                key: Some(control::hydrate_request::Key::External(Box::new(
                    control::ExternalKey {
                        connection_id: connection.to_string(),
                        external_id: external.to_owned(),
                        ..Default::default()
                    },
                ))),
                claim: true,
                instance_id: self.instance_id.to_string(),
                ..Default::default()
            })
            .await?;
        if !response.found {
            return Ok(None);
        }
        Ok(Some(self.insert_thread(self.hydrated(response)?)))
    }
    /// One hydration per conversation at a time, without serialising unrelated ones.
    fn hydration_lock(&self, key: String) -> Arc<Mutex<()>> {
        let mut locks = self
            .state
            .hydrating
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
        locks.entry(key).or_default().clone()
    }
    fn hydrated(&self, response: control::HydrateResponse) -> Result<ThreadState> {
        let thread = response.thread.into_option().ok_or(ChatError::NotFound)?;
        let key = id(&thread.id)?;
        let assignment = response
            .assignment
            .into_option()
            .unwrap_or_else(|| self.initial_assignment(key, &thread));
        let mut t = ThreadState::new(thread, assignment);
        for message in response.messages {
            t.upsert_message(message);
        }
        for run in response.runs {
            t.runs.insert(id(&run.id)?, run);
        }
        Ok(t)
    }
    fn initial_assignment(
        &self,
        thread: Uuid,
        roster: &types::Thread,
    ) -> types::ParticipantAssignment {
        let participant = roster
            .participants
            .iter()
            .find(|p| p.agent_id.as_deref() == Some(&self.agent_id.to_string()))
            .map(|p| p.id.clone())
            .unwrap_or_else(|| {
                Uuid::new_v5(&thread, self.agent_id.to_string().as_bytes()).to_string()
            });
        types::ParticipantAssignment {
            thread_id: thread.to_string(),
            participant_id: participant,
            agent_id: self.agent_id.to_string(),
            owner_instance_id: self.instance_id.to_string(),
            generation: 1,
            ..Default::default()
        }
    }
    /// Create a thread owned here. The gateway grants ownership before any local
    /// state exists, so a replica never executes on a conversation it may lose.
    pub(crate) async fn adopt(&self, mut t: ThreadState) -> Result<Shared> {
        let key = id(&t.thread.id)?;
        t.assignment = self.initial_assignment(key, &t.thread);
        let claim = control::Claim {
            thread_id: key.to_string(),
            participant_id: t.assignment.participant_id.clone(),
            ..Default::default()
        };
        let response = self
            .gateway
            .publish(
                self.instance_id,
                vec![control::Upstream {
                    frame: Some(claim.into()),
                    ..Default::default()
                }],
            )
            .await?;
        let granted = response
            .claims
            .into_iter()
            .find(|c| c.thread_id == key.to_string() && c.granted)
            .ok_or(ChatError::Denied)?;
        t.assignment.generation = granted.generation;
        if let Some(existing) = self.local(key) {
            return Ok(existing);
        }
        Ok(self.insert_thread(t))
    }
    pub async fn create_user(&self, name: &str) -> Result<types::User> {
        text(name)?;
        if name.chars().count() > 200 {
            return Err(ChatError::Invalid("Name is too long".into()));
        }
        self.ensure_capacity()?;
        let user = types::User {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            ..Default::default()
        };
        self.state
            .users
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id(&user.id)?, name.into());
        self.emit_global("user.created", user.clone().into())?;
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
        self.ensure_capacity()?;
        let thread_id = Uuid::new_v4();
        let mut thread = types::Thread {
            id: thread_id.to_string(),
            title: r.title,
            primary_agent_id: r.primary_agent_id,
            ..Default::default()
        };
        for p in r.participants {
            let participant = self.participant(thread_id, p).await?;
            if !thread
                .participants
                .iter()
                .any(|old| old.id == participant.id)
            {
                thread.participants.push(participant);
            }
        }
        if !thread
            .participants
            .iter()
            .any(|p| p.agent_id.as_deref() == Some(&thread.primary_agent_id))
        {
            return Err(ChatError::Invalid("Primary agent must participate".into()));
        }
        let shared = self
            .adopt(ThreadState::new(thread.clone(), Default::default()))
            .await?;
        let mut t = shared.lock().await;
        let assignment = t.assignment.clone();
        self.emit(&mut t, "thread.created", thread.clone().into(), None)?;
        for p in thread.participants.clone() {
            self.emit(&mut t, "participant.joined", p.into(), None)?;
        }
        self.emit(&mut t, "participant.assigned", assignment.into(), None)?;
        drop(t);
        self.changed(thread_id);
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
        let identity_id = id(identity)?;
        let key = Uuid::new_v5(&thread, identity.as_bytes());
        let name = if p.agent_id.as_deref() == Some(&self.agent_id.to_string()) {
            self.configuration()?.agent.name.clone()
        } else if let Some(user) = &p.user_id {
            let local = self
                .state
                .users
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get(&id(user)?)
                .cloned();
            match local {
                Some(name) => name,
                None => self.gateway.resolve_user(identity_id).await?.name,
            }
        } else {
            // Cross-agent membership is a registry decision made at the gateway.
            self.gateway.resolve_agent(identity_id).await?.name
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
    /// Whether this replica currently holds a conversation, without hydrating it.
    pub async fn owns_thread(&self, key: Uuid) -> bool {
        match self.local(key) {
            Some(shared) => self.owns(&*shared.lock().await),
            None => false,
        }
    }
    pub async fn thread(&self, key: Uuid) -> Result<types::Thread> {
        let shared = self.load(key).await?;
        let t = shared.lock().await;
        let mut thread = t.thread.clone();
        thread.participants.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(thread)
    }
    pub async fn list_threads(
        &self,
        after: &str,
        limit: usize,
    ) -> Result<(Vec<types::Thread>, String)> {
        let mut keys: Vec<Uuid> = self
            .state
            .threads
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .copied()
            .collect();
        keys.sort();
        let start = if after.is_empty() {
            None
        } else {
            Some(id(after)?)
        };
        let mut threads = vec![];
        let mut next = String::new();
        for key in keys
            .into_iter()
            .filter(|k| start.is_none_or(|after| *k > after))
        {
            if threads.len() == limit {
                next = threads
                    .last()
                    .map(|t: &types::Thread| t.id.clone())
                    .unwrap_or_default();
                break;
            }
            threads.push(self.thread(key).await?);
        }
        Ok((threads, next))
    }
    pub async fn add_participant(&self, r: chat::AddParticipant) -> Result<types::Thread> {
        let thread = id(&r.thread_id)?;
        let shared = self.load(thread).await?;
        let participant = self
            .participant(thread, r.participant.ok_or(ChatError::NotFound)?)
            .await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        match t
            .thread
            .participants
            .iter_mut()
            .find(|p| p.id == participant.id)
        {
            Some(existing) => existing.active = true,
            None => t.thread.participants.push(participant.clone()),
        }
        self.emit(&mut t, "participant.joined", participant.into(), None)?;
        drop(t);
        self.changed(thread);
        self.thread(thread).await
    }
    pub async fn remove_participant(
        &self,
        thread: Uuid,
        participant: Uuid,
    ) -> Result<types::Thread> {
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let primary = t.thread.primary_agent_id.clone();
        let p = t
            .thread
            .participants
            .iter_mut()
            .find(|p| p.id == participant.to_string())
            .ok_or(ChatError::NotFound)?;
        if p.agent_id.as_deref() == Some(&primary) {
            return Err(ChatError::Conflict);
        }
        p.active = false;
        let p = p.clone();
        self.emit(&mut t, "participant.left", p.into(), None)?;
        drop(t);
        self.changed(thread);
        self.thread(thread).await
    }
    pub(crate) fn message_thread(&self, message: Uuid) -> Option<Uuid> {
        self.state
            .message_threads
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&message)
            .copied()
    }
    pub async fn message(&self, key: Uuid) -> Result<types::Message> {
        let thread = self.message_thread(key).ok_or(ChatError::NotFound)?;
        let shared = self.local(thread).ok_or(ChatError::NotFound)?;
        let t = shared.lock().await;
        t.message(key).cloned().ok_or(ChatError::NotFound)
    }
    pub async fn messages(&self, thread: Uuid, limit: u32) -> Result<Vec<types::Message>> {
        let mut page = self.message_page(thread, limit, None).await?.messages;
        page.reverse();
        Ok(page)
    }
    pub async fn message_page(
        &self,
        thread: Uuid,
        limit: u32,
        before: Option<Uuid>,
    ) -> Result<chat::MessagePage> {
        let shared = self.load(thread).await?;
        let t = shared.lock().await;
        let size = if limit == 0 { 100 } else { limit.min(100) } as usize;
        let bound = match before {
            Some(before) => Some(*t.message_keys.get(&before).ok_or(ChatError::NotFound)?),
            None => None,
        };
        let mut rows: Vec<types::Message> = t
            .messages
            .iter()
            .rev()
            .filter(|(key, _)| bound.is_none_or(|b| **key < b))
            .take(size + 1)
            .map(|(_, m)| m.clone())
            .collect();
        let more = rows.len() > size;
        if more {
            rows.pop();
        }
        let next_page_token = if more {
            rows.last().map(|m| m.id.clone()).unwrap_or_default()
        } else {
            String::new()
        };
        Ok(chat::MessagePage {
            messages: rows,
            next_page_token,
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
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        self.ensure_capacity()?;
        if !t
            .thread
            .participants
            .iter()
            .any(|p| p.id == r.participant_id && p.active)
        {
            return Err(ChatError::Denied);
        }
        for target in &r.addressed_participant_ids {
            if !t
                .thread
                .participants
                .iter()
                .any(|p| p.id == *target && p.active)
            {
                return Err(ChatError::NotFound);
            }
        }
        if let Some(reply) = &r.in_reply_to_message_id
            && t.message(id(reply)?).is_none()
        {
            return Err(ChatError::NotFound);
        }
        if let Some(old) = t.message(key) {
            return if old.participant_id == r.participant_id && old.text == r.text {
                Ok(old.clone())
            } else {
                Err(ChatError::Conflict)
            };
        }
        let mut attachments = vec![];
        for attachment in &r.attachment_ids {
            attachments.push(
                t.attachments
                    .get(&id(attachment)?)
                    .map(|a| a.metadata.clone())
                    .ok_or(ChatError::NotFound)?,
            );
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
            created_at: chat::audit::timestamp(chrono::Utc::now()).into(),
            ..Default::default()
        };
        t.upsert_message(message.clone());
        self.index_message(key, thread);
        self.emit(&mut t, "message.created", message.clone().into(), None)?;
        self.route_message_in(&mut t, &message)?;
        drop(t);
        self.changed(thread);
        Ok(message)
    }
    /// Copy a message authored elsewhere in the room into local state and route it.
    pub(crate) async fn relay(&self, roster: types::Thread, message: types::Message) -> Result<()> {
        let thread = id(&roster.id)?;
        let shared = match self.local(thread) {
            Some(shared) => shared,
            None => {
                if !roster
                    .participants
                    .iter()
                    .any(|p| p.agent_id.as_deref() == Some(&self.agent_id.to_string()))
                {
                    return Err(ChatError::Denied);
                }
                match self.load(thread).await {
                    Ok(shared) => shared,
                    Err(ChatError::NotFound) => {
                        self.adopt(ThreadState::new(roster.clone(), Default::default()))
                            .await?
                    }
                    Err(e) => return Err(e),
                }
            }
        };
        let mut t = shared.lock().await;
        for p in roster.participants {
            if !t.thread.participants.iter().any(|old| old.id == p.id) {
                t.thread.participants.push(p);
            }
        }
        let key = id(&message.id)?;
        if t.message(key).is_none() {
            t.upsert_message(message.clone());
            self.index_message(key, thread);
        }
        if self.owns(&t) && message.status == "complete" {
            self.route_message_in(&mut t, &message)?;
        }
        drop(t);
        self.changed(thread);
        Ok(())
    }
    /// Drop threads that are idle, fully shipped, and not executing.
    pub(crate) async fn evict_idle(&self) {
        let candidates: Vec<(Uuid, Shared)> = self
            .state
            .threads
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        for (key, shared) in candidates {
            let t = shared.lock().await;
            let idle = now() - t.last_activity_at > 15 * 60_000;
            let unpersisted = t.attachments.values().any(|a| !a.metadata.persisted);
            if idle && t.active_invocation(self.agent_id).is_none() && !unpersisted {
                let messages: Vec<Uuid> = t.message_keys.keys().copied().collect();
                let runs: Vec<Uuid> = t.runs.keys().copied().collect();
                let invocations: Vec<Uuid> = t.invocations.keys().copied().collect();
                let external = t
                    .thread
                    .channel
                    .as_option()
                    .and_then(|c| Some((id(&c.connection_id).ok()?, c.external_id.clone())));
                drop(t);
                self.state
                    .threads
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&key);
                let mut index = self
                    .state
                    .message_threads
                    .write()
                    .unwrap_or_else(|e| e.into_inner());
                for message in messages {
                    index.remove(&message);
                }
                drop(index);
                let mut index = self
                    .state
                    .run_threads
                    .write()
                    .unwrap_or_else(|e| e.into_inner());
                for run in runs {
                    index.remove(&run);
                }
                drop(index);
                let mut index = self
                    .state
                    .invocation_threads
                    .write()
                    .unwrap_or_else(|e| e.into_inner());
                for invocation in invocations {
                    index.remove(&invocation);
                }
                drop(index);
                if let Some(external) = external {
                    self.state
                        .external
                        .write()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&external);
                }
            }
        }
    }
}
pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
impl Drop for Memory {
    fn drop(&mut self) {
        if let Some(configuration) = self.configuration.get_mut().ok().and_then(|c| c.take()) {
            drop(configuration);
        }
        for bytes in self.attachments.get_mut().values_mut() {
            bytes.zeroize();
        }
    }
}
