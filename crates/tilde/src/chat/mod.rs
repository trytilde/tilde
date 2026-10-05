//! Provider-neutral threads and agent-owned work. Postgres owns durable state; no inbox/event-bus abstraction.
//!
//! Humans address participating agents explicitly or fall back to the thread's primary
//! agent. Agent messages route only to explicitly addressed agents, avoiding reply loops.
//! Addressing selects who acts, not a private audience. Only completed messages route.
//! Routing receipts and pending input are persisted under the thread transaction lock.
//! Activity uses the same lock for committed cursor order; WatchThread replays it.
//! Reasoning is activity, never an implicit visible reply.
//!
//! Native and external threads share this model. Connection-backed adapters live in `providers`;
//! provider-defined tool transport lives in `tools`, execution in `runtime`.
pub mod controls;
pub mod db;
use crate::chat as application;
use crate::database::{Pool, Transaction};
use crate::proto::tilde::types::v1 as types;
pub mod access;
pub(crate) mod agent_lifecycle;
pub mod audit;
mod ingress;
pub mod providers;
pub mod rpc;
pub mod runtime;
pub mod tools;
pub mod warm;
use crate::encryption::Encryption;
use connectrpc::ConnectError;

use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("Conversation moved to gateway storage")]
    Archived,
    #[error("{0}")]
    Invalid(String),
    #[error("Record not found in this scope")]
    NotFound,
    #[error("Conflicting request or terminal state")]
    Conflict,
    #[error("Conversation access denied")]
    Forbidden,
    #[error("Invocation capability is invalid or expired")]
    Denied,
    #[error("Database operation failed")]
    Database(#[from] crate::database::DbError),
    #[error("Agent transport failed")]
    Transport,
}
impl From<ChatError> for ConnectError {
    fn from(value: ChatError) -> Self {
        match value {
            ChatError::Invalid(s) => Self::invalid_argument(s),
            ChatError::NotFound => Self::not_found("Record not found in this scope"),
            ChatError::Conflict => {
                Self::failed_precondition("Conflicting request or terminal state")
            }
            ChatError::Archived => Self::unavailable("Conversation moved to gateway storage"),
            ChatError::Forbidden => Self::permission_denied("Conversation access denied"),
            ChatError::Denied => Self::unauthenticated("Invalid or expired invocation capability"),
            ChatError::Database(ref error) if error.is_unique_violation() => {
                Self::failed_precondition("Conflicting record")
            }
            ChatError::Database(ref error)
                if error.is_foreign_key_violation() || error.is_check_violation() =>
            {
                Self::invalid_argument("Invalid scoped relationship or value")
            }
            _ => Self::internal("Chat operation failed"),
        }
    }
}
pub type Result<T> = std::result::Result<T, ChatError>;
#[derive(Clone)]
pub struct Chat {
    pub(crate) activity_notifications: Arc<crate::database::notifications::Notifications>,
    pub(crate) work_notifications: Arc<crate::database::notifications::Notifications>,
    pub(crate) control_notifications: Arc<crate::database::notifications::Notifications>,
    pub(crate) channels: Option<providers::Channels>,
    /// One instance per process: its tool hosts share a notification listener.
    pub tools: Option<crate::tools::Tools>,
    pub(crate) deployments: Option<crate::deployment::Deployments>,
    pub(crate) inference: Option<crate::inference::Gateway>,
    /// Configured with object storage and GitHub; runtime and management skill routes use it.
    pub(crate) skills: Option<crate::skills::Skills>,
    pub warm: Arc<warm::Warm>,
    pub tokens: crate::iam::tokens::Tokens,
    pub(crate) store: RuntimeStore,
    pub(crate) objects: Option<crate::agent::avatar::AvatarStore>,
    pub(crate) encryption: Arc<Encryption>,
    pub(crate) callback_url: String,
}
#[derive(Clone)]
pub enum RuntimeStore {
    Postgres(Pool),
    Sidecar(Arc<crate::deployment::runtime::Runtime>),
}
#[derive(Clone)]
pub struct Scope {
    pub capabilities: crate::iam::capabilities::Capabilities,
    pub id: Uuid,
    pub run_id: Uuid,
    pub thread_id: Uuid,
    pub agent_id: Uuid,
    pub participant_id: Uuid,
    /// Inference connections this invocation may route through, fixed at token issue/renewal.
    pub inference: Vec<Uuid>,
}
/// Parse a public UUID without accepting a missing scope.
pub fn id(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|_| ChatError::Invalid("Invalid UUID".into()))
}
/// Bound user-authored strings without echoing their values in errors.
pub fn text(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 16384 {
        return Err(ChatError::Invalid("Text must contain 1-16384 bytes".into()));
    }
    Ok(())
}
/// Append activity in commit order, locking only the affected thread.
/// Pin a (thread, agent) pair to the deployment that will execute it: the existing
/// pin while that deployment is registered, otherwise a deployment chosen by the agent's routing policy.
/// Failover stays within the deployment; moving across deployments is explicit.
pub(crate) async fn pin_deployment(
    tx: &Transaction<'_>,
    thread: Uuid,
    agent: Uuid,
) -> Result<Option<Uuid>> {
    let chosen = crate::deployment::db::choose_opt(tx, agent)
        .await?
        .map(|r| r.id);
    Ok(
        crate::chat::db::deployment_pin_opt(tx, thread, agent, chosen)
            .await?
            .and_then(|r| r.deployment_id),
    )
}
pub async fn activity(
    pool: &Pool,
    tx: &Transaction<'_>,
    thread: Uuid,
    kind: &str,
    entity: Uuid,
    delta: &str,
) -> Result<()> {
    audit::snapshot(pool, tx, thread, kind, entity, delta).await
}
impl Chat {
    pub fn with_deployments(mut self, deployments: crate::deployment::Deployments) -> Self {
        self.deployments = Some(deployments);
        self
    }
    /// Mount the inference route on this runtime surface.
    pub fn with_inference(mut self, inference: crate::inference::Gateway) -> Self {
        self.inference = Some(inference);
        self
    }
    pub fn with_skills(mut self, skills: Option<crate::skills::Skills>) -> Self {
        self.skills = skills;
        self
    }
    /// The configured skills service, or one without object storage or GitHub for tests.
    pub(crate) fn skills(&self) -> Result<crate::skills::Skills> {
        Ok(match &self.skills {
            Some(skills) => skills.clone(),
            None => crate::skills::Skills::new(self.pg()?.clone()),
        })
    }
    pub fn with_objects(mut self, objects: Option<crate::agent::avatar::AvatarStore>) -> Self {
        self.objects = objects;
        self
    }

    pub fn from_sidecar(runtime: Arc<crate::deployment::runtime::Runtime>) -> Self {
        Self {
            activity_notifications: Arc::default(),
            work_notifications: Arc::default(),
            control_notifications: Arc::default(),
            objects: None,
            channels: None,
            tools: None,
            deployments: None,
            inference: Some(runtime.inference()),
            skills: None,
            warm: Arc::default(),
            tokens: crate::iam::tokens::Tokens::sidecar(runtime.clone()),
            encryption: Arc::new(
                Encryption::from_agent_key(runtime.agent_id, crate::deployment::random_secret())
                    .expect("random agent key"),
            ),
            callback_url: runtime.callback_url.clone(),
            store: RuntimeStore::Sidecar(runtime),
        }
    }
    pub(crate) async fn has_channel(&self, thread: Uuid) -> Result<bool> {
        if let Some(local) = self.local() {
            return match local.thread(thread).await {
                Ok(thread) => Ok(thread.channel.is_set()),
                Err(ChatError::NotFound) => Ok(false),
                Err(error) => Err(error),
            };
        }
        Ok(
            crate::chat::db::channel_binding_opt(&self.pg()?.get().await?, thread)
                .await?
                .is_some(),
        )
    }
    pub(crate) fn local(&self) -> Option<&Arc<crate::deployment::runtime::Runtime>> {
        match &self.store {
            RuntimeStore::Sidecar(runtime) => Some(runtime),
            _ => None,
        }
    }
    pub(crate) fn pg(&self) -> Result<&Pool> {
        match &self.store {
            RuntimeStore::Postgres(pool) => Ok(pool),
            _ => Err(ChatError::Invalid(
                "Postgres operation reached a sidecar".into(),
            )),
        }
    }

    pub fn with_connections(
        mut self,
        connections: crate::connections::service::Connections,
    ) -> Self {
        self.tools = Some(crate::tools::Tools::new(
            connections.clone(),
            self.callback_url.clone(),
        ));
        self.channels = Some(providers::Channels::new(connections, self.warm.clone()));
        self
    }

    /// Bind the concrete service to the installation pool and encryption module.
    pub fn new(pool: Pool, encryption: Arc<Encryption>, callback_url: String) -> Self {
        Self {
            activity_notifications: Arc::default(),
            work_notifications: Arc::default(),
            control_notifications: Arc::default(),
            objects: None,
            channels: None,
            tools: None,
            deployments: None,
            inference: None,
            skills: None,
            warm: Arc::default(),
            tokens: crate::iam::tokens::Tokens::new(pool.clone(), encryption.clone()),
            store: RuntimeStore::Postgres(pool),
            encryption,
            callback_url,
        }
    }
    /// Create a human conversation identity, independent of login or provider accounts.
    pub async fn create_user(&self, name: &str) -> Result<types::User> {
        if let Some(local) = self.local() {
            return local.create_user(name).await;
        }
        text(name)?;
        if name.chars().count() > 200 {
            return Err(ChatError::Invalid("Name is too long".into()));
        }
        let row = crate::chat::db::user_create_one(&self.pg()?.get().await?, Uuid::new_v4(), name)
            .await?;
        Ok(types::User {
            id: row.id.to_string(),
            name: row.name,
            ..Default::default()
        })
    }
    /// Create native threads with one or many humans and agents. Provider bindings are not implemented yet.
    pub async fn create_thread(&self, r: application::CreateThread) -> Result<types::Thread> {
        if let Some(local) = self.local() {
            return local.create_thread(r).await;
        }
        text(&r.title)?;
        if r.participants.is_empty() || r.participants.len() > 100 {
            return Err(ChatError::Invalid(
                "Thread requires 1-100 participants".into(),
            ));
        }
        let primary = id(&r.primary_agent_id)?;
        let thread = Uuid::new_v4();
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::thread_create_execute(&tx, thread, &(r.title), primary).await?;
        let mut has_primary = false;
        for p in r.participants {
            if p.user_id.is_some() == p.agent_id.is_some() {
                return Err(ChatError::Invalid(
                    "Participant must name one user or agent".into(),
                ));
            }
            let user = p.user_id.as_deref().map(id).transpose()?;
            let agent = p.agent_id.as_deref().map(id).transpose()?;
            has_primary |= agent == Some(primary);
            let participant = Uuid::new_v4();
            crate::chat::db::participant_create_execute(&tx, participant, thread, user, agent)
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
        if !has_primary {
            return Err(ChatError::Invalid("Primary agent must participate".into()));
        }
        activity(self.pg()?, &tx, thread, "thread.created", thread, "").await?;
        tx.commit().await?;
        drop(tx_client);
        self.thread(thread).await
    }
    /// Return the thread and its complete roster without synthetic inbox instances.
    pub async fn thread(&self, thread: Uuid) -> Result<types::Thread> {
        if let Some(local) = self.local() {
            return local.thread(thread).await;
        }
        let rows = crate::chat::db::thread_detail_all(&self.pg()?.get().await?, thread).await?;
        let first = rows.first().ok_or(ChatError::NotFound)?;
        let mut result = types::Thread {
            id: first.id.to_string(),
            title: first.title.clone(),
            primary_agent_id: first.primary_agent_id.to_string(),
            channel: first
                .connection_id
                .map(|connection| types::ChannelBinding {
                    connection_id: connection.to_string(),
                    provider_id: first.provider_id.clone().unwrap_or_default(),
                    external_id: first.external_id.clone().unwrap_or_default(),
                    ..Default::default()
                })
                .into(),
            ..Default::default()
        };
        for row in rows {
            if let Some(participant) = row.participant_id {
                result.participants.push(types::Participant {
                    id: participant.to_string(),
                    active: row.active.ok_or(ChatError::Transport)?,
                    name: row.name.ok_or(ChatError::Transport)?,
                    user_id: row.user_id.map(|v| v.to_string()),
                    agent_id: row.agent_id.map(|v| v.to_string()),
                    ..Default::default()
                });
            }
        }
        Ok(result)
    }

    pub async fn messages(&self, thread: Uuid, limit: u32) -> Result<Vec<types::Message>> {
        if let Some(local) = self.local() {
            return local.messages(thread, limit).await;
        }
        self.thread(thread).await?;
        let rows = crate::chat::db::messages_all(
            &self.pg()?.get().await?,
            thread,
            limit.clamp(1, 100) as i64,
        )
        .await?;
        Ok(rows
            .into_iter()
            .rev()
            .map(|r| types::Message {
                delivery: audit::delivery(
                    r.connection_id,
                    r.external_message_id,
                    r.destination,
                    r.delivery_status,
                )
                .into(),
                id: r.id.to_string(),
                thread_id: r.thread_id.to_string(),
                participant_id: r.participant_id.to_string(),
                attachments: r.attachments.0.into_iter().map(Into::into).collect(),
                format: r.format,
                subject: r.subject,
                created_at: audit::timestamp(r.created_at).into(),
                text: r.text,
                status: r.status,
                in_reply_to_message_id: r.in_reply_to_message_id.map(|v| v.to_string()),
                addressed_participant_ids: r.targets.into_iter().map(|v| v.to_string()).collect(),
                ..Default::default()
            })
            .collect())
    }
    /// Fetch the persisted canonical message, including incomplete/aborted streams.
    pub async fn message(&self, message: Uuid) -> Result<types::Message> {
        if let Some(local) = self.local() {
            return local.message(message).await;
        }
        let r = crate::chat::db::message_get_opt(&self.pg()?.get().await?, message)
            .await?
            .ok_or(ChatError::NotFound)?;
        Ok(types::Message {
            delivery: audit::delivery(
                r.connection_id,
                r.external_message_id,
                r.destination,
                r.delivery_status,
            )
            .into(),
            id: r.id.to_string(),
            thread_id: r.thread_id.to_string(),
            participant_id: r.participant_id.to_string(),
            attachments: r.attachments.0.into_iter().map(Into::into).collect(),
            format: r.format,
            subject: r.subject,
            created_at: audit::timestamp(r.created_at).into(),
            text: r.text,
            status: r.status,
            in_reply_to_message_id: r.in_reply_to_message_id.map(|v| v.to_string()),
            addressed_participant_ids: r.targets.into_iter().map(|v| v.to_string()).collect(),
            ..Default::default()
        })
    }
    /// Persist human input. Agent authorship is available only through invocation-bound streaming.
    pub async fn post(&self, r: application::PostMessage) -> Result<types::Message> {
        if let Some(local) = self.local() {
            return local.post(r).await;
        }
        if let Some(message) = self.forward_post(&r).await? {
            return Ok(message);
        }
        if r.text.is_empty() {
            if r.attachment_ids.is_empty() {
                return Err(ChatError::Invalid(
                    "Message needs text or attachments".into(),
                ));
            }
        } else {
            text(&r.text)?;
        }
        if r.attachment_ids.len() > 20 {
            return Err(ChatError::Invalid("At most 20 attachments".into()));
        }
        let thread = id(&r.thread_id)?;
        let message = id(&r.id)?;
        let reply = r.in_reply_to_message_id.as_deref().map(id).transpose()?;
        let participant = id(&r.participant_id)?;
        let roster = self.thread(thread).await?;
        if !roster
            .participants
            .iter()
            .any(|p| p.id == r.participant_id && p.user_id.is_some() && p.active)
        {
            return Err(ChatError::Denied);
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, thread).await?;
        let author = crate::chat::db::participant_get_opt(&tx, thread, participant)
            .await?
            .ok_or(ChatError::NotFound)?;
        if !author.active || author.user_id.is_none() {
            return Err(ChatError::Denied);
        }
        if let Some(old) = crate::chat::db::message_get_opt(&tx, message).await? {
            let mut targets = r
                .addressed_participant_ids
                .iter()
                .map(|v| id(v))
                .collect::<Result<Vec<_>>>()?;
            targets.sort();
            if old.thread_id != thread
                || old.participant_id != participant
                || old.text != r.text
                || old.targets != targets
                || old.in_reply_to_message_id != reply
                || {
                    let mut actual = old
                        .attachments
                        .0
                        .iter()
                        .map(|a| a.id.to_string())
                        .collect::<Vec<_>>();
                    let mut expected = r.attachment_ids.clone();
                    actual.sort();
                    expected.sort();
                    actual != expected
                }
            {
                return Err(ChatError::Conflict);
            }
            tx.commit().await?;
            drop(tx_client);
            return self.message(message).await;
        }
        crate::chat::db::message_create_execute(
            &tx,
            message,
            thread,
            participant,
            &(r.text),
            "complete",
            None::<Uuid>,
            reply,
            &(crate::telemetry::tracing::context::capture().0),
            &(crate::telemetry::tracing::context::capture().1),
        )
        .await?;
        self.targets(&tx, thread, message, &r.addressed_participant_ids)
            .await?;
        for attachment in &r.attachment_ids {
            crate::chat::db::attachment_attach_execute(&tx, thread, message, id(attachment)?)
                .await?;
        }
        activity(self.pg()?, &tx, thread, "message.completed", message, "").await?;
        tx.commit().await?;
        drop(tx_client);
        self.message(message).await
    }
    /// Finalize an interrupted stream and record its terminal activity atomically.
    pub(crate) async fn abort_message(&self, message: Uuid, thread: Uuid) -> Result<()> {
        if let Some(local) = self.local() {
            return local.abort_message(message, thread).await;
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        if crate::chat::db::message_finish_execute(&tx, message, "aborted").await? > 0 {
            activity(self.pg()?, &tx, thread, "message.aborted", message, "").await?;
        }
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    pub(crate) async fn expire_messages(&self) -> Result<()> {
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        let rows = crate::chat::db::message_cleanup_all(&tx).await?;
        for row in rows {
            activity(
                self.pg()?,
                &tx,
                row.thread_id,
                "message.aborted",
                row.id,
                "",
            )
            .await?;
        }
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    /// Bind destinations to this thread, not arbitrary provider addresses or other threads.
    pub(crate) async fn targets(
        &self,
        tx: &Transaction<'_>,
        thread: Uuid,
        message: Uuid,
        targets: &[String],
    ) -> Result<()> {
        if targets.len() > 100 {
            return Err(ChatError::Invalid("Too many recipients".into()));
        }
        for target in targets {
            if crate::chat::db::message_target_execute(tx, message, thread, id(target)?).await? != 1
            {
                return Err(ChatError::NotFound);
            }
        }
        Ok(())
    }
    /// Resolve signed invocation identity and grants until token expiry; never accept model-authored scope.
    pub async fn scope(&self, capability: &str) -> Result<Scope> {
        self.tokens.scope(capability).await
    }
    /// Create agent-owned goals, replaying the same ID only for identical input.
    pub async fn create_goal(&self, s: &Scope, r: application::CreateGoal) -> Result<types::Goal> {
        if let Some(local) = self.local() {
            return local.create_goal(s, r).await;
        }
        text(&r.objective)?;
        let key = id(&r.id)?;
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::goal_create_execute(&tx, key, s.thread_id, s.agent_id, &(r.objective))
            .await?;
        let row = crate::chat::db::goal_get_opt(&tx, s.thread_id, s.agent_id, key)
            .await?
            .ok_or(ChatError::Conflict)?;
        if row.objective != r.objective {
            return Err(ChatError::Conflict);
        }
        activity(self.pg()?, &tx, s.thread_id, "goal.updated", key, "").await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(types::Goal {
            id: row.id.to_string(),
            objective: row.objective,
            status: row.status,
            ..Default::default()
        })
    }
    /// List only work owned by the executing agent in the executing thread.
    pub async fn goals(&self, s: &Scope) -> Result<Vec<types::Goal>> {
        if let Some(local) = self.local() {
            return local.goals(s).await;
        }
        Ok(
            crate::chat::db::goals_all(&self.pg()?.get().await?, s.thread_id, s.agent_id)
                .await?
                .into_iter()
                .map(|r| types::Goal {
                    id: r.id.to_string(),
                    objective: r.objective,
                    status: r.status,
                    ..Default::default()
                })
                .collect(),
        )
    }
    /// Terminal goals cannot be reopened by an ordinary tool call.
    pub async fn update_goal(&self, s: &Scope, r: application::UpdateGoal) -> Result<types::Goal> {
        if let Some(local) = self.local() {
            return local.update_goal(s, r).await;
        }
        if !["active", "completed", "failed", "canceled"].contains(&r.status.as_str()) {
            return Err(ChatError::Invalid("Invalid goal status".into()));
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        let key = id(&r.id)?;
        let row = crate::chat::db::goal_update_opt(&tx, s.thread_id, s.agent_id, key, &(r.status))
            .await?
            .ok_or(ChatError::Conflict)?;
        activity(self.pg()?, &tx, s.thread_id, "goal.updated", key, "").await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(types::Goal {
            id: row.id.to_string(),
            objective: row.objective,
            status: row.status,
            ..Default::default()
        })
    }
    /// Dependencies are immutable and must already exist in this scope, making cycles impossible.
    pub async fn create_task(&self, s: &Scope, r: application::CreateTask) -> Result<types::Task> {
        if let Some(local) = self.local() {
            return local.create_task(s, r).await;
        }
        text(&r.title)?;
        let key = id(&r.id)?;
        let goal = r.goal_id.as_deref().map(id).transpose()?;
        let mut deps = r
            .dependency_ids
            .iter()
            .map(|v| id(v))
            .collect::<Result<Vec<_>>>()?;
        deps.sort();
        if deps.len() > 100 || deps.contains(&key) || deps.windows(2).any(|v| v[0] == v[1]) {
            return Err(ChatError::Invalid("Invalid task dependencies".into()));
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        let (sequence, at) = audit::lock(&tx, s.thread_id).await?;
        let mut event = types::Activity {
            kind: "task.updated".into(),
            entity_id: key.to_string(),
            ..Default::default()
        };
        audit::prepare(s.thread_id, sequence, at, &mut event);
        let row = crate::chat::db::task_create_audited_opt(
            &tx,
            key,
            s.thread_id,
            s.agent_id,
            &(r.title),
            goal,
            &deps,
            sequence,
        )
        .await?
        .ok_or(ChatError::Conflict)?;
        audit::enqueue(self.pg()?, s.thread_id, row.transaction_id, at, event)?;
        tx.commit().await?;
        drop(tx_client);
        Ok(types::Task {
            id: row.id.to_string(),
            title: row.title,
            status: row.status,
            goal_id: row.goal_id.map(|v| v.to_string()),
            dependency_ids: row
                .dependencies
                .into_iter()
                .map(|v| v.to_string())
                .collect(),
            blocked_reason: row.blocked_reason,
            ..Default::default()
        })
    }
    /// Return the executing agent's durable task ledger.
    pub async fn tasks(&self, s: &Scope) -> Result<Vec<types::Task>> {
        if let Some(local) = self.local() {
            return local.tasks(s).await;
        }
        Ok(
            crate::chat::db::tasks_all(&self.pg()?.get().await?, s.thread_id, s.agent_id)
                .await?
                .into_iter()
                .map(|r| types::Task {
                    id: r.id.to_string(),
                    title: r.title,
                    status: r.status,
                    goal_id: r.goal_id.map(|v| v.to_string()),
                    dependency_ids: r.dependencies.into_iter().map(|v| v.to_string()).collect(),
                    blocked_reason: r.blocked_reason,
                    ..Default::default()
                })
                .collect(),
        )
    }
    /// Reject terminal reopening and starting/completing tasks whose dependencies are unfinished.
    pub async fn update_task(&self, s: &Scope, r: application::UpdateTask) -> Result<types::Task> {
        if let Some(local) = self.local() {
            return local.update_task(s, r).await;
        }
        if ![
            "pending",
            "working",
            "blocked",
            "completed",
            "failed",
            "canceled",
        ]
        .contains(&r.status.as_str())
        {
            return Err(ChatError::Invalid("Invalid task status".into()));
        }
        if r.blocked_reason.len() > 16384 {
            return Err(ChatError::Invalid("Reason is too long".into()));
        }
        let key = id(&r.id)?;
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, s.thread_id).await?;
        if ["working", "completed"].contains(&r.status.as_str())
            && !crate::chat::db::dependencies_ready_one(&tx, key)
                .await?
                .ready
        {
            return Err(ChatError::Conflict);
        }
        let row = crate::chat::db::task_update_opt(
            &tx,
            s.thread_id,
            s.agent_id,
            key,
            &(r.status),
            &(r.blocked_reason),
        )
        .await?
        .ok_or(ChatError::Conflict)?;
        activity(self.pg()?, &tx, s.thread_id, "task.updated", key, "").await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(types::Task {
            id: row.id.to_string(),
            title: row.title,
            status: row.status,
            goal_id: row.goal_id.map(|v| v.to_string()),
            dependency_ids: row
                .dependencies
                .into_iter()
                .map(|v| v.to_string())
                .collect(),
            blocked_reason: row.blocked_reason,
            ..Default::default()
        })
    }
    /// Run lifecycle is explicit; stopping an invocation only moves active work to waiting.
    pub async fn set_run_status(&self, s: &Scope, status: &str) -> Result<()> {
        if let Some(local) = self.local() {
            return local.set_run_status(s, status).await;
        }
        if !["waiting", "completed", "failed", "canceled"].contains(&status) {
            return Err(ChatError::Invalid("Invalid run status".into()));
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::run_status_execute(&tx, s.run_id, status).await?;
        activity(self.pg()?, &tx, s.thread_id, "run.updated", s.run_id, "").await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
}

// Shared application commands; each RPC audience validates and supplies its own scope.
#[derive(Default)]
pub struct CreateThread {
    pub title: String,
    pub participants: Vec<crate::proto::tilde::types::v1::ParticipantRef>,
    pub primary_agent_id: String,
}

#[derive(Default)]
pub struct PostMessage {
    pub id: String,
    pub thread_id: String,
    pub participant_id: String,
    pub text: String,
    pub addressed_participant_ids: Vec<String>,
    pub in_reply_to_message_id: Option<String>,
    pub attachment_ids: Vec<String>,
}

#[derive(Default)]
pub struct StartRun {
    pub thread_id: String,
    pub agent_id: String,
    pub objective: String,
    pub goal_id: Option<String>,
    pub idempotency_key: String,
}

#[derive(Default)]
pub struct SteerInvocation {
    pub invocation_id: String,
    pub input_id: String,
    pub text: String,
}

#[derive(Default)]
pub struct AddParticipant {
    pub thread_id: String,
    pub participant: Option<crate::proto::tilde::types::v1::ParticipantRef>,
}

#[derive(Default)]
pub struct UploadAttachment {
    pub id: String,
    pub thread_id: String,
    pub filename: String,
    pub media_type: String,
    pub content: Vec<u8>,
}

#[derive(Default)]
pub struct CreateGoal {
    pub id: String,
    pub objective: String,
}

#[derive(Default)]
pub struct UpdateGoal {
    pub id: String,
    pub status: String,
}

#[derive(Default)]
pub struct CreateTask {
    pub id: String,
    pub title: String,
    pub goal_id: Option<String>,
    pub dependency_ids: Vec<String>,
}

#[derive(Default)]
pub struct UpdateTask {
    pub id: String,
    pub status: String,
    pub blocked_reason: String,
}

#[derive(Default, Clone)]
pub struct MessagePage {
    pub messages: Vec<crate::proto::tilde::types::v1::Message>,
    pub next_page_token: String,
    /// Agent-specific representations for this invocation's returned messages only.
    pub cached_messages: Vec<ConvertedMessage>,
}
#[derive(Default)]
pub struct ActivityPage {
    pub events: Vec<crate::proto::tilde::types::v1::Activity>,
    pub next_sequence: i64,
    pub has_more: bool,
}
#[derive(Default)]
pub struct AttachmentContent {
    pub attachment: crate::proto::tilde::types::v1::Attachment,
    pub content: Vec<u8>,
}
#[derive(Default, Clone)]
pub struct ConvertedMessage {
    pub message_id: String,
    pub message_json: String,
}

impl From<tokio_postgres::Error> for ChatError {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Database(error.into())
    }
}
impl From<deadpool_postgres::PoolError> for ChatError {
    fn from(error: deadpool_postgres::PoolError) -> Self {
        Self::Database(error.into())
    }
}
