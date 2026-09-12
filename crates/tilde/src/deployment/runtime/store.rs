//! Owner-side conversation state. Everything here is plain memory: the replica
//! that owns a thread mutates it under one per-thread lock and ships typed events
//! to the gateway afterwards. Nothing is persisted locally.
use crate::proto::tilde::types::v1 as types;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use uuid::Uuid;

/// Bound on retained history per thread; older events stay durable at the gateway.
pub(crate) const EVENT_WINDOW: usize = 5000;
/// Bound on unacknowledged upstream frames before mutations start failing.
pub(crate) const OUTBOX_LIMIT: usize = 200_000;

#[derive(Clone)]
pub(crate) struct Command {
    pub attempt_id: Uuid,
    pub command: types::AgentCommand,
    pub acked_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub failure: String,
}
#[derive(Clone, Default)]
pub(crate) struct RunMeta {
    pub source_identity_id: String,
    pub channel_origin: bool,
}
#[derive(Clone)]
pub(crate) struct AttachmentEntry {
    pub metadata: types::Attachment,
    pub source: Option<types::AttachmentSource>,
}
pub(crate) struct ThreadState {
    pub thread: types::Thread,
    pub messages: BTreeMap<(i64, Uuid), types::Message>,
    pub message_keys: HashMap<Uuid, (i64, Uuid)>,
    pub runs: HashMap<Uuid, types::Run>,
    pub run_meta: HashMap<Uuid, RunMeta>,
    pub invocations: HashMap<Uuid, types::InvocationState>,
    pub commands: Vec<Command>,
    pub goals: BTreeMap<Uuid, types::Goal>,
    pub tasks: BTreeMap<Uuid, types::Task>,
    pub tool_calls: HashMap<Uuid, types::ToolCall>,
    pub attachments: HashMap<Uuid, AttachmentEntry>,
    pub converted: HashMap<Uuid, String>,
    pub typing: HashMap<Uuid, i64>,
    pub receipts: HashSet<Uuid>,
    pub dispatched: HashSet<Uuid>,
    pub control_receipts: HashSet<Uuid>,
    pub assignment: types::ParticipantAssignment,
    pub events: VecDeque<types::RuntimeEvent>,
    pub sequence: i64,
    pub last_activity_at: i64,
}
impl ThreadState {
    pub fn new(thread: types::Thread, assignment: types::ParticipantAssignment) -> Self {
        Self {
            thread,
            messages: BTreeMap::new(),
            message_keys: HashMap::new(),
            runs: HashMap::new(),
            run_meta: HashMap::new(),
            invocations: HashMap::new(),
            commands: Vec::new(),
            goals: BTreeMap::new(),
            tasks: BTreeMap::new(),
            tool_calls: HashMap::new(),
            attachments: HashMap::new(),
            converted: HashMap::new(),
            typing: HashMap::new(),
            receipts: HashSet::new(),
            dispatched: HashSet::new(),
            control_receipts: HashSet::new(),
            assignment,
            events: VecDeque::new(),
            sequence: 0,
            last_activity_at: super::now(),
        }
    }
    pub fn message(&self, key: Uuid) -> Option<&types::Message> {
        self.message_keys
            .get(&key)
            .and_then(|position| self.messages.get(position))
    }
    pub fn upsert_message(&mut self, message: types::Message) -> Option<types::Message> {
        let key = match Uuid::parse_str(&message.id) {
            Ok(key) => key,
            Err(_) => return None,
        };
        if let Some(position) = self.message_keys.get(&key).copied() {
            return self.messages.insert(position, message);
        }
        let created =
            message.created_at.seconds * 1000 + i64::from(message.created_at.nanos) / 1_000_000;
        self.message_keys.insert(key, (created, key));
        self.messages.insert((created, key), message)
    }
    pub fn command(&self, id: &str) -> Option<&Command> {
        self.commands.iter().rev().find(|c| c.command.id == id)
    }
    pub fn active_invocation(&self, agent: Uuid) -> Option<&types::InvocationState> {
        self.invocations.values().find(|v| {
            v.agent_id == agent.to_string() && matches!(v.status.as_str(), "pending" | "running")
        })
    }
    pub fn retain_window(&mut self) {
        while self.events.len() > EVENT_WINDOW {
            self.events.pop_front();
        }
    }
}
