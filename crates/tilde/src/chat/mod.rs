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
use crate::chat as application;
use crate::proto::tilde::types::v1 as types;
pub mod access;
pub(crate) mod agent_lifecycle;
pub mod audit;
mod ingress;
pub mod providers;
use audit::AttachmentRecords;
pub mod rpc;
pub mod runtime;
pub mod tools;
use crate::encryption::{Encryption, SealedSecret, SecretBinding};
use connectrpc::ConnectError;
use secrecy::SecretString;

use sqlx::{PgPool, Postgres, Transaction};
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
    #[error("Invocation capability is invalid or expired")]
    Denied,
    #[error("Database operation failed")]
    Database(#[from] sqlx::Error),
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
            ChatError::Denied => Self::unauthenticated("Invalid or expired invocation capability"),
            ChatError::Database(ref error)
                if error
                    .as_database_error()
                    .is_some_and(|e| e.is_unique_violation()) =>
            {
                Self::failed_precondition("Conflicting record")
            }
            ChatError::Database(ref error)
                if error
                    .as_database_error()
                    .is_some_and(|e| e.is_foreign_key_violation() || e.is_check_violation()) =>
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
    pub(crate) deployments: Option<crate::deployment::Deployments>,
    pub tokens: crate::iam::tokens::Tokens,
    pub(crate) store: RuntimeStore,
    pub(crate) objects: Option<crate::agent::avatar::AvatarStore>,
    pub(crate) encryption: Arc<Encryption>,
    pub(crate) callback_url: String,
}
#[derive(Clone)]
pub enum RuntimeStore {
    Postgres(PgPool),
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
/// pin while that deployment is registered, otherwise the agent's serving deployment.
/// Failover stays within the deployment; moving across deployments is explicit.
pub(crate) async fn pin_deployment(
    tx: &mut Transaction<'_, Postgres>,
    thread: Uuid,
    agent: Uuid,
) -> Result<Option<Uuid>> {
    Ok(
        sqlx::query_file!("../../queries/chat/deployment_pin.sql", thread, agent)
            .fetch_optional(&mut **tx)
            .await?
            .and_then(|r| r.deployment_id),
    )
}
pub async fn activity(
    tx: &mut Transaction<'_, Postgres>,
    thread: Uuid,
    kind: &str,
    entity: Uuid,
    delta: &str,
) -> Result<()> {
    audit::snapshot(tx, thread, kind, entity, delta).await
}
impl Chat {
    pub fn with_deployments(mut self, deployments: crate::deployment::Deployments) -> Self {
        self.deployments = Some(deployments);
        self
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
            deployments: None,
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
            sqlx::query_file!("../../queries/chat/channel_binding.sql", thread)
                .fetch_optional(self.pg()?)
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
    pub(crate) fn pg(&self) -> Result<&PgPool> {
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
        self.channels = Some(providers::Channels::new(connections));
        self
    }

    /// Bind the concrete service to the installation pool and encryption module.
    pub fn new(pool: PgPool, encryption: Arc<Encryption>, callback_url: String) -> Self {
        Self {
            activity_notifications: Arc::default(),
            work_notifications: Arc::default(),
            control_notifications: Arc::default(),
            objects: None,
            channels: None,
            deployments: None,
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
        let row = sqlx::query_file!("../../queries/chat/user_create.sql", Uuid::new_v4(), name)
            .fetch_one(self.pg()?)
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
        let forwarded: Option<crate::proto::tilde::ingress::v1::CreateThreadResponse> = self
            .forward_sidecar(
                id(&r.primary_agent_id)?,
                None,
                "CreateThread",
                &crate::proto::tilde::ingress::v1::CreateThreadRequest {
                    title: r.title.clone(),
                    participants: r.participants.clone(),
                    primary_agent_id: r.primary_agent_id.clone(),
                    ..Default::default()
                },
            )
            .await?;
        if let Some(response) = forwarded {
            return response.thread.into_option().ok_or(ChatError::Transport);
        }
        text(&r.title)?;
        if r.participants.is_empty() || r.participants.len() > 100 {
            return Err(ChatError::Invalid(
                "Thread requires 1-100 participants".into(),
            ));
        }
        let primary = id(&r.primary_agent_id)?;
        let thread = Uuid::new_v4();
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!(
            "../../queries/chat/thread_create.sql",
            thread,
            r.title,
            primary
        )
        .execute(&mut *tx)
        .await?;
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
            sqlx::query_file!(
                "../../queries/chat/participant_create.sql",
                participant,
                thread,
                user,
                agent
            )
            .execute(&mut *tx)
            .await?;
            activity(&mut tx, thread, "participant.joined", participant, "").await?;
        }
        if !has_primary {
            return Err(ChatError::Invalid("Primary agent must participate".into()));
        }
        activity(&mut tx, thread, "thread.created", thread, "").await?;
        tx.commit().await?;
        self.thread(thread).await
    }
    /// Return the thread and its complete roster without synthetic inbox instances.
    pub async fn thread(&self, thread: Uuid) -> Result<types::Thread> {
        if let Some(local) = self.local() {
            return local.thread(thread).await;
        }
        let row = sqlx::query_file!("../../queries/chat/thread_get.sql", thread)
            .fetch_optional(self.pg()?)
            .await?
            .ok_or(ChatError::NotFound)?;
        let participants = sqlx::query_file!("../../queries/chat/participants.sql", thread)
            .fetch_all(self.pg()?)
            .await?
            .into_iter()
            .map(|p| types::Participant {
                id: p.id.to_string(),
                name: p.name,
                active: p.active,
                user_id: p.user_id.map(|v| v.to_string()),
                agent_id: p.agent_id.map(|v| v.to_string()),
                ..Default::default()
            })
            .collect();
        let binding = sqlx::query_file!("../../queries/chat/channel_binding.sql", thread)
            .fetch_optional(self.pg()?)
            .await?;
        Ok(types::Thread {
            channel: binding
                .map(|b| types::ChannelBinding {
                    connection_id: b.connection_id.to_string(),
                    provider_id: b.provider_id,
                    external_id: b.external_id,
                    ..Default::default()
                })
                .into(),
            id: row.id.to_string(),
            title: row.title,
            primary_agent_id: row.primary_agent_id.to_string(),
            participants,
            ..Default::default()
        })
    }
    /// Read bounded recent history. Returned messages are ordered oldest first.
    pub async fn messages(&self, thread: Uuid, limit: u32) -> Result<Vec<types::Message>> {
        if let Some(local) = self.local() {
            return local.messages(thread, limit).await;
        }
        self.thread(thread).await?;
        let rows = sqlx::query_file!(
            "../../queries/chat/messages.sql",
            thread,
            limit.clamp(1, 100) as i64
        )
        .fetch_all(self.pg()?)
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
        let r = sqlx::query_file!("../../queries/chat/message_get.sql", message)
            .fetch_optional(self.pg()?)
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
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", thread)
            .fetch_one(&mut *tx)
            .await?;
        let author = sqlx::query_file!(
            "../../queries/chat/participant_get.sql",
            thread,
            participant
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ChatError::NotFound)?;
        if !author.active || author.user_id.is_none() {
            return Err(ChatError::Denied);
        }
        if let Some(old) = sqlx::query_file!("../../queries/chat/message_get.sql", message)
            .fetch_optional(&mut *tx)
            .await?
        {
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
            return self.message(message).await;
        }
        sqlx::query_file!(
            "../../queries/chat/message_create.sql",
            message,
            thread,
            participant,
            r.text,
            "complete",
            None::<Uuid>,
            reply,
            crate::telemetry::context::capture().0,
            crate::telemetry::context::capture().1
        )
        .execute(&mut *tx)
        .await?;
        self.targets(&mut tx, thread, message, &r.addressed_participant_ids)
            .await?;
        for attachment in &r.attachment_ids {
            sqlx::query_file!(
                "../../queries/chat/attachment_attach.sql",
                thread,
                message,
                id(attachment)?
            )
            .execute(&mut *tx)
            .await?;
        }
        activity(&mut tx, thread, "message.completed", message, "").await?;
        tx.commit().await?;
        self.message(message).await
    }
    /// Finalize an interrupted stream and record its terminal activity atomically.
    pub(crate) async fn abort_message(&self, message: Uuid, thread: Uuid) -> Result<()> {
        if let Some(local) = self.local() {
            return local.abort_message(message, thread).await;
        }
        let mut tx = self.pg()?.begin().await?;
        if sqlx::query_file!("../../queries/chat/message_finish.sql", message, "aborted")
            .execute(&mut *tx)
            .await?
            .rows_affected()
            > 0
        {
            activity(&mut tx, thread, "message.aborted", message, "").await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub(crate) async fn expire_messages(&self) -> Result<()> {
        let mut tx = self.pg()?.begin().await?;
        let rows = sqlx::query_file!("../../queries/chat/message_cleanup.sql")
            .fetch_all(&mut *tx)
            .await?;
        for row in rows {
            activity(&mut tx, row.thread_id, "message.aborted", row.id, "").await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// Bind destinations to this thread, not arbitrary provider addresses or other threads.
    pub(crate) async fn targets(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        thread: Uuid,
        message: Uuid,
        targets: &[String],
    ) -> Result<()> {
        if targets.len() > 100 {
            return Err(ChatError::Invalid("Too many recipients".into()));
        }
        for target in targets {
            if sqlx::query_file!(
                "../../queries/chat/message_target.sql",
                message,
                thread,
                id(target)?
            )
            .execute(&mut **tx)
            .await?
            .rows_affected()
                != 1
            {
                return Err(ChatError::NotFound);
            }
        }
        Ok(())
    }
    /// Resolve a bearer capability to exactly one active invocation; never accept model-authored scope.
    pub async fn scope(&self, capability: &str) -> Result<Scope> {
        if let Some(local) = self.local() {
            return local.scope(capability).await;
        }
        let claims = self.tokens.verify(capability).await?;
        let r = sqlx::query_file!(
            "../../queries/chat/invocation_scope.sql",
            claims.invocation_id
        )
        .fetch_optional(self.pg()?)
        .await?
        .ok_or(ChatError::Denied)?;
        Ok(Scope {
            capabilities: claims.capabilities,
            id: claims.invocation_id,
            run_id: claims.run_id,
            thread_id: claims.thread_id,
            agent_id: claims.sub,
            participant_id: r.participant_id,
        })
    }
    /// Retrieve the outbound signing credential only inside the runtime adapter.
    pub(crate) fn signing_key(&self, agent: Uuid, sealed: &[u8]) -> Result<SecretString> {
        self.encryption
            .open(
                SecretBinding {
                    resource_kind: "agent",
                    resource_id: agent,
                    name: "webhook_signing_key",
                },
                SealedSecret::from_bytes(sealed).map_err(|_| ChatError::Transport)?,
            )
            .map_err(|_| ChatError::Transport)
    }
    /// Create agent-owned goals, replaying the same ID only for identical input.
    pub async fn create_goal(&self, s: &Scope, r: application::CreateGoal) -> Result<types::Goal> {
        if let Some(local) = self.local() {
            return local.create_goal(s, r).await;
        }
        text(&r.objective)?;
        let key = id(&r.id)?;
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!(
            "../../queries/chat/goal_create.sql",
            key,
            s.thread_id,
            s.agent_id,
            r.objective
        )
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query_file!(
            "../../queries/chat/goal_get.sql",
            s.thread_id,
            s.agent_id,
            key
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ChatError::Conflict)?;
        if row.objective != r.objective {
            return Err(ChatError::Conflict);
        }
        activity(&mut tx, s.thread_id, "goal.updated", key, "").await?;
        tx.commit().await?;
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
            sqlx::query_file!("../../queries/chat/goals.sql", s.thread_id, s.agent_id)
                .fetch_all(self.pg()?)
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
        let mut tx = self.pg()?.begin().await?;
        let key = id(&r.id)?;
        let row = sqlx::query_file!(
            "../../queries/chat/goal_update.sql",
            s.thread_id,
            s.agent_id,
            key,
            r.status
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ChatError::Conflict)?;
        activity(&mut tx, s.thread_id, "goal.updated", key, "").await?;
        tx.commit().await?;
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
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", s.thread_id)
            .fetch_one(&mut *tx)
            .await?;
        let inserted = sqlx::query_file!(
            "../../queries/chat/task_create.sql",
            key,
            s.thread_id,
            s.agent_id,
            r.title,
            goal
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        if inserted {
            for dep in &deps {
                sqlx::query_file!(
                    "../../queries/chat/dependency_create.sql",
                    s.thread_id,
                    s.agent_id,
                    key,
                    *dep
                )
                .execute(&mut *tx)
                .await?;
            }
        }
        let row = sqlx::query_file!(
            "../../queries/chat/task_get.sql",
            s.thread_id,
            s.agent_id,
            key
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ChatError::Conflict)?;
        if row.title != r.title || row.goal_id != goal || row.dependencies != deps {
            return Err(ChatError::Conflict);
        }
        activity(&mut tx, s.thread_id, "task.updated", key, "").await?;
        tx.commit().await?;
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
            sqlx::query_file!("../../queries/chat/tasks.sql", s.thread_id, s.agent_id)
                .fetch_all(self.pg()?)
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
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", s.thread_id)
            .fetch_one(&mut *tx)
            .await?;
        if ["working", "completed"].contains(&r.status.as_str())
            && !sqlx::query_file!("../../queries/chat/dependencies_ready.sql", key)
                .fetch_one(&mut *tx)
                .await?
                .ready
        {
            return Err(ChatError::Conflict);
        }
        let row = sqlx::query_file!(
            "../../queries/chat/task_update.sql",
            s.thread_id,
            s.agent_id,
            key,
            r.status,
            r.blocked_reason
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ChatError::Conflict)?;
        activity(&mut tx, s.thread_id, "task.updated", key, "").await?;
        tx.commit().await?;
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
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!("../../queries/chat/run_status.sql", s.run_id, status)
            .execute(&mut *tx)
            .await?;
        activity(&mut tx, s.thread_id, "run.updated", s.run_id, "").await?;
        tx.commit().await?;
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

#[derive(Default)]
pub struct MessagePage {
    pub messages: Vec<crate::proto::tilde::types::v1::Message>,
    pub next_page_token: String,
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
#[derive(Default)]
pub struct ConvertedMessage {
    pub message_id: String,
    pub message_json: String,
}
