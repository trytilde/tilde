//! Events published by replicas become canonical Postgres state. A batch is one
//! transaction; receipts deduplicate replays; run state is accepted only from the
//! replica holding the thread's lease. Data appends carry no ownership at all.
use super::{Deployments, Holder, id, lease_frame, liveness_secs};
use crate::proto::tilde::{
    agent_event_ingress::v1 as wire,
    types::v1::{self as types, runtime_event::State},
};
use crate::{chat, error::Error};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;
pub enum Outcome {
    Projected,
    Duplicate,
    /// Run state from a replica that does not hold the lease; carries who does.
    NotHolder(wire::ThreadLease),
}
/// Errors caused by the frame itself, as opposed to the gateway's own infrastructure.
fn rejectable(error: &Error) -> bool {
    match error {
        Error::Denied | Error::Invalid(_) | Error::NotFound | Error::Conflict => true,
        Error::Database(error) => error.as_database_error().is_some_and(|e| {
            e.is_unique_violation() || e.is_foreign_key_violation() || e.is_check_violation()
        }),
        _ => false,
    }
}
fn at(value: i64) -> Result<DateTime<Utc>, Error> {
    DateTime::from_timestamp_millis(value)
        .ok_or_else(|| Error::Invalid("Invalid event timestamp".into()))
}
/// Per-batch memory: leases already looked up, membership already verified and thread
/// routes already locked.
#[derive(Default)]
struct Batch {
    leases: HashMap<Uuid, Option<Holder>>,
    members: HashSet<Uuid>,
    locked: HashSet<Uuid>,
}
/// One frame of a batch that the projection applies in the order the replica queued it.
enum Item {
    Event(Box<types::RuntimeEvent>),
    /// The replica evicted the thread; its lease goes only after the events before it.
    Release(Uuid),
}
impl Deployments {
    /// Apply one batch in order. Events and lease releases share one transaction and
    /// keep their queue order, so a run that completed before its thread was evicted is
    /// recorded before the lease goes. A frame the gateway cannot accept is rejected on
    /// its own so the replica drops it, and only infrastructure failures fail the
    /// batch, which the replica then resends.
    pub async fn publish(
        &self,
        agent: Uuid,
        deployment: Uuid,
        instance: Uuid,
        request: wire::PublishRequest,
    ) -> Result<wire::PublishResponse, Error> {
        let mut items = vec![];
        for frame in request.frames {
            match frame.frame {
                Some(wire::upstream::Frame::Heartbeat(heartbeat)) => {
                    self.heartbeat(agent, instance, &heartbeat).await?
                }
                Some(wire::upstream::Frame::Event(event)) => {
                    if let Some(inner) = event.event.into_option() {
                        items.push(Item::Event(Box::new(inner)));
                    }
                }
                Some(wire::upstream::Frame::Release(release)) => {
                    if let Ok(thread) = id(&release.thread_id) {
                        items.push(Item::Release(thread));
                    }
                }
                Some(wire::upstream::Frame::DirectiveResult(result)) => {
                    self.directive_result(
                        agent,
                        instance,
                        id(&result.id)?,
                        &result.result.into_option().unwrap_or_default(),
                    )
                    .await?;
                }
                Some(wire::upstream::Frame::Telemetry(telemetry)) => {
                    if let Err(error) = self.accept_telemetry(agent, *telemetry).await {
                        if !rejectable(&error) {
                            return Err(error);
                        }
                        tracing::warn!(agent_id=%agent, error=%error, "Rejected sidecar telemetry");
                    }
                }
                None => {}
            }
        }
        let (rejected, lost) = match self
            .project_batch(agent, deployment, instance, &items)
            .await
        {
            Ok(outcome) => outcome,
            Err(error) if rejectable(&error) => {
                // One frame poisoned the shared transaction: isolate it by running the
                // batch frame by frame, each in its own transaction.
                let mut rejected = vec![];
                let mut lost = BTreeMap::new();
                let mut batch = Batch::default();
                for item in items {
                    let mut tx = self.pool.begin().await?;
                    batch.locked.clear();
                    let event = match item {
                        Item::Event(event) => *event,
                        Item::Release(thread) => {
                            self.release_in(&mut tx, agent, instance, thread).await?;
                            tx.commit().await?;
                            continue;
                        }
                    };
                    let event_id = event.id.clone();
                    let kind = event.kind.clone();
                    match self
                        .project_event(&mut tx, &mut batch, agent, deployment, instance, event)
                        .await
                    {
                        // Not a divergence: the lost lease tells the replica what happened.
                        Ok(Outcome::NotHolder(lease)) => {
                            lost.insert(lease.thread_id.clone(), lease);
                            tx.commit().await?;
                        }
                        Ok(_) => tx.commit().await?,
                        Err(error) if rejectable(&error) => {
                            tracing::warn!(agent_id=%agent, event_id=%event_id, kind=%kind, error=?error, "Rejected sidecar event");
                            rejected.push(event_id);
                        }
                        Err(error) => return Err(error),
                    }
                }
                (rejected, lost)
            }
            Err(error) => return Err(error),
        };
        Ok(wire::PublishResponse {
            rejected_event_ids: rejected,
            lost: lost.into_values().collect(),
            ..Default::default()
        })
    }
    async fn project_batch(
        &self,
        agent: Uuid,
        deployment: Uuid,
        instance: Uuid,
        items: &[Item],
    ) -> Result<(Vec<String>, BTreeMap<String, wire::ThreadLease>), Error> {
        let mut rejected = vec![];
        let mut lost = BTreeMap::new();
        if items.is_empty() {
            return Ok((rejected, lost));
        }
        let mut batch = Batch::default();
        let mut tx = self.pool.begin().await?;
        for item in items {
            let event = match item {
                Item::Event(event) => event,
                Item::Release(thread) => {
                    self.release_in(&mut tx, agent, instance, *thread).await?;
                    // Later events on this thread must see the lease gone.
                    batch.leases.remove(thread);
                    continue;
                }
            };
            let event_id = event.id.clone();
            let kind = event.kind.clone();
            match self
                .project_event(
                    &mut tx,
                    &mut batch,
                    agent,
                    deployment,
                    instance,
                    (**event).clone(),
                )
                .await
            {
                // Not a divergence: the lost lease tells the replica what happened.
                Ok(Outcome::NotHolder(lease)) => {
                    lost.insert(lease.thread_id.clone(), lease);
                }
                Ok(_) => {}
                // Our own checks leave the transaction usable; a database error does not.
                Err(
                    error @ (Error::Denied | Error::Invalid(_) | Error::NotFound | Error::Conflict),
                ) => {
                    tracing::warn!(agent_id=%agent, event_id=%event_id, kind=%kind, error=?error, "Rejected sidecar event");
                    rejected.push(event_id);
                }
                Err(error) => return Err(error),
            }
        }
        tx.commit().await?;
        Ok((rejected, lost))
    }
    /// Kinds of state only the lease holder may write: they drive the agent process.
    fn needs_lease(state: &Option<State>) -> bool {
        matches!(
            state,
            Some(State::Run(_) | State::Invocation(_) | State::ToolCall(_))
        )
    }
    async fn project_event(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        batch: &mut Batch,
        agent: Uuid,
        deployment: Uuid,
        instance: Uuid,
        event: types::RuntimeEvent,
    ) -> Result<Outcome, Error> {
        let event_id = id(&event.id)?;
        if id(&event.agent_id)? != agent
            || id(&event.origin_instance_id)? != instance
            || event.origin_sequence < 1
        {
            return Err(Error::Denied);
        }
        let thread = id(&event.thread_id)?;
        let created = at(event.created_at)?;
        if !thread.is_nil() {
            if batch.locked.insert(thread) {
                chat::access::lock_thread_route(tx, thread).await?;
            }
            if Self::needs_lease(&event.state) {
                let holder = match batch.leases.get(&thread) {
                    Some(holder) => holder.as_ref().map(|h| (h.instance, h.public_url.clone())),
                    None => {
                        let row = sqlx::query_file!(
                            "../../queries/deployment/lease_holder.sql",
                            thread,
                            agent,
                            liveness_secs()
                        )
                        .fetch_optional(&mut **tx)
                        .await?;
                        let holder = row.map(|r| Holder {
                            instance: r.instance_id,
                            public_url: r.public_url.unwrap_or_default(),
                            version: r.updated_at,
                        });
                        let value = holder.as_ref().map(|h| (h.instance, h.public_url.clone()));
                        batch.leases.insert(thread, holder);
                        value
                    }
                };
                if holder.as_ref().map(|(i, _)| *i) != Some(instance) {
                    let version = batch
                        .leases
                        .get(&thread)
                        .and_then(|h| h.as_ref().map(|h| h.version))
                        .unwrap_or_else(Utc::now);
                    return Ok(Outcome::NotHolder(lease_frame(
                        agent, thread, holder, version,
                    )));
                }
            }
        }
        if sqlx::query_file!(
            "../../queries/deployment/event_receipt.sql",
            event_id,
            agent,
            (!thread.is_nil()).then_some(thread),
            instance,
            event.origin_sequence,
            created
        )
        .fetch_optional(&mut **tx)
        .await?
        .is_none()
        {
            return Ok(Outcome::Duplicate);
        }
        if !thread.is_nil() && !batch.members.contains(&thread) {
            let exists =
                sqlx::query_file!("../../queries/deployment/project/has_thread.sql", thread)
                    .fetch_one(&mut **tx)
                    .await?
                    .exists;
            let member = if exists {
                // An existing conversation accepts events only from agents in it.
                sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
                    .fetch_one(&mut **tx)
                    .await?
                    .allowed
            } else {
                // Only the creating event may introduce a thread, and the agent must be in it.
                let Some(State::Thread(value)) = &event.state else {
                    return Err(Error::Denied);
                };
                value.primary_agent_id == agent.to_string()
                    || value
                        .participants
                        .iter()
                        .any(|p| p.agent_id.as_deref() == Some(&agent.to_string()))
            };
            if !member {
                return Err(Error::Denied);
            }
            batch.members.insert(thread);
        }
        match &event.state {
            Some(State::User(value)) => {
                sqlx::query_file!(
                    "../../queries/deployment/project/user.sql",
                    id(&value.id)?,
                    value.name
                )
                .execute(&mut **tx)
                .await?;
            }
            Some(State::Thread(value)) => {
                if id(&value.id)? != thread {
                    return Err(Error::Denied);
                }
                self.project_thread(tx, value).await?;
            }
            Some(State::Participant(value)) => self.project_participant(tx, thread, value).await?,
            Some(State::Message(value)) => {
                if id(&value.thread_id)? != thread {
                    return Err(Error::Denied);
                }
                let key = id(&value.id)?;
                sqlx::query_file!(
                    "../../queries/deployment/project/message.sql",
                    key,
                    thread,
                    id(&value.participant_id)?,
                    value.text,
                    value.status,
                    value
                        .in_reply_to_message_id
                        .as_deref()
                        .map(id)
                        .transpose()?,
                    created,
                    value.format,
                    value.subject
                )
                .execute(&mut **tx)
                .await?;
                if let Some(delivery) = value.delivery.as_option() {
                    sqlx::query_file!(
                        "../../queries/deployment/project/delivery.sql",
                        key,
                        id(&delivery.connection_id)?,
                        delivery.destination,
                        delivery.external_message_id,
                        delivery.status
                    )
                    .execute(&mut **tx)
                    .await?;
                }
                for target in &value.addressed_participant_ids {
                    sqlx::query_file!(
                        "../../queries/chat/message_target.sql",
                        key,
                        thread,
                        id(target)?
                    )
                    .execute(&mut **tx)
                    .await?;
                }
                // The publishing agent already routed this message locally.
                sqlx::query_file!("../../queries/deployment/project/dispatch.sql", key, agent)
                    .execute(&mut **tx)
                    .await?;
                for attachment in &value.attachments {
                    self.project_attachment(tx, thread, attachment).await?;
                    sqlx::query_file!(
                        "../../queries/chat/attachment_attach.sql",
                        thread,
                        key,
                        id(&attachment.id)?
                    )
                    .execute(&mut **tx)
                    .await?;
                }
            }
            Some(State::Goal(value)) => {
                sqlx::query_file!(
                    "../../queries/deployment/project/goal.sql",
                    id(&value.id)?,
                    thread,
                    agent,
                    value.objective,
                    value.status
                )
                .execute(&mut **tx)
                .await?;
            }
            Some(State::Task(value)) => {
                let key = id(&value.id)?;
                sqlx::query_file!(
                    "../../queries/deployment/project/task.sql",
                    key,
                    thread,
                    agent,
                    value.title,
                    value.status,
                    value.goal_id.as_deref().map(id).transpose()?,
                    value.blocked_reason
                )
                .execute(&mut **tx)
                .await?;
                for dep in &value.dependency_ids {
                    sqlx::query_file!(
                        "../../queries/chat/dependency_create.sql",
                        thread,
                        agent,
                        key,
                        id(dep)?
                    )
                    .execute(&mut **tx)
                    .await?;
                }
            }
            Some(State::Run(value)) => {
                if id(&value.thread_id)? != thread || id(&value.agent_id)? != agent {
                    return Err(Error::Denied);
                }
                sqlx::query_file!(
                    "../../queries/deployment/project/run.sql",
                    id(&value.id)?,
                    thread,
                    agent,
                    value.objective,
                    value.status,
                    value.goal_id.as_deref().map(id).transpose()?,
                    value.id
                )
                .execute(&mut **tx)
                .await?;
            }
            Some(State::Invocation(value)) => {
                if id(&value.thread_id)? != thread || id(&value.agent_id)? != agent {
                    return Err(Error::Denied);
                }
                sqlx::query_file!(
                    "../../queries/deployment/project/invocation.sql",
                    id(&value.id)?,
                    id(&value.run_id)?,
                    thread,
                    agent,
                    value.status,
                    created,
                    if value.lease_expires_at > 0 {
                        Some(at(value.lease_expires_at)?)
                    } else {
                        None
                    },
                    deployment
                )
                .execute(&mut **tx)
                .await?;
            }
            Some(State::Attachment(value)) => self.project_attachment(tx, thread, value).await?,
            Some(State::AttachmentSource(value)) => {
                self.project_attachment(tx, thread, &value.attachment)
                    .await?
            }
            Some(State::ConvertedMessage(value)) => {
                sqlx::query_file!(
                    "../../queries/chat/cache_upsert.sql",
                    agent,
                    id(&value.message_id)?,
                    serde_json::from_str::<serde_json::Value>(&value.message_json)
                        .map_err(|_| Error::Invalid("Invalid converted message".into()))?
                )
                .execute(&mut **tx)
                .await?;
            }
            Some(State::ToolCall(value)) => {
                sqlx::query_file!(
                    "../../queries/deployment/project/tool.sql",
                    id(&value.id)?,
                    thread,
                    id(&event.invocation_id)?,
                    id(&event.participant_id)?,
                    value.name,
                    value.provider_id,
                    value.status,
                    value.input_json,
                    value.output_json,
                    value.error
                )
                .execute(&mut **tx)
                .await?;
            }
            Some(State::ChannelDecision(value)) => {
                let connection = id(&value.connection_id)?;
                let identity = id(&value.identity_id)?;
                let mode = match value.access_mode.as_known() {
                    Some(types::ChannelAccessMode::Public) => "public",
                    Some(types::ChannelAccessMode::Disabled) => "disabled",
                    _ => "private",
                };
                sqlx::query_file!(
                    "../../queries/deployment/project/user.sql",
                    identity,
                    value.value
                )
                .execute(&mut **tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/channel_access/identity_upsert.sql",
                    identity,
                    connection,
                    chat::access::identity::kind_name(
                        value.identity_type.as_known().ok_or(Error::Denied)?
                    ),
                    value.value
                )
                .fetch_one(&mut **tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/channel_access/audit.sql",
                    connection,
                    value.event_id,
                    agent,
                    identity,
                    mode,
                    value.accepted
                )
                .execute(&mut **tx)
                .await?;
            }
            Some(State::Activity(_) | State::Typing(_)) => {}
            None => return Err(Error::Invalid("Event requires typed state".into())),
        }
        if !thread.is_nil() {
            let activity = super::runtime::messages::event_activity(event);
            chat::audit::append(tx, thread, activity).await?;
        }
        Ok(Outcome::Projected)
    }
    async fn project_thread(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        thread: &types::Thread,
    ) -> Result<(), Error> {
        let key = id(&thread.id)?;
        sqlx::query_file!(
            "../../queries/deployment/project/thread.sql",
            key,
            thread.title,
            id(&thread.primary_agent_id)?
        )
        .execute(&mut **tx)
        .await?;
        if let Some(channel) = thread.channel.as_option() {
            sqlx::query_file!(
                "../../queries/chat/channel_thread_bind.sql",
                key,
                id(&channel.connection_id)?,
                channel.external_id,
                id(&thread.primary_agent_id)?
            )
            .execute(&mut **tx)
            .await?;
        }
        for participant in &thread.participants {
            self.project_participant(tx, key, participant).await?;
        }
        Ok(())
    }
    async fn project_participant(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        thread: Uuid,
        value: &types::Participant,
    ) -> Result<(), Error> {
        if let Some(user) = value.user_id.as_deref() {
            sqlx::query_file!(
                "../../queries/deployment/project/user.sql",
                id(user)?,
                value.name
            )
            .execute(&mut **tx)
            .await?;
        }
        sqlx::query_file!(
            "../../queries/deployment/project/participant.sql",
            id(&value.id)?,
            thread,
            value.user_id.as_deref().map(id).transpose()?,
            value.agent_id.as_deref().map(id).transpose()?,
            value.active
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
    async fn project_attachment(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        thread: Uuid,
        value: &types::Attachment,
    ) -> Result<(), Error> {
        if id(&value.thread_id)? != thread {
            return Err(Error::Denied);
        }
        sqlx::query_file!(
            "../../queries/deployment/project/attachment.sql",
            id(&value.id)?,
            thread,
            value.filename,
            value.media_type,
            value.size_bytes,
            value.sha256
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
    /// Hand completed messages to the other sidecar agents in a room.
    pub async fn relay_pending(&self) -> Result<(bool, usize), Error> {
        let rows = sqlx::query_file!(
            "../../queries/deployment/relay_pending.sql",
            liveness_secs()
        )
        .fetch_all(&self.pool)
        .await?;
        let more = rows.len() == 50;
        let mut delivered = 0;
        let chat = self.chat();
        for row in rows {
            let Some(instance) = self.target_for(row.agent_id, Some(row.thread_id)).await? else {
                continue;
            };
            let relay = wire::RelayMessage {
                thread: chat.thread(row.thread_id).await?.into(),
                message: chat.message(row.message_id).await?.into(),
                ..Default::default()
            };
            self.direct(row.agent_id, instance, Some(row.thread_id), relay.into())
                .await?;
            sqlx::query_file!(
                "../../queries/deployment/project/dispatch.sql",
                row.message_id,
                row.agent_id
            )
            .execute(&self.pool)
            .await?;
            delivered += 1;
        }
        Ok((more, delivered))
    }
    pub async fn relay_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let notifications = crate::database::notifications::Notifications::default();
        let mut changed = match notifications
            .subscribe(&self.pool, "tilde_chat_activity")
            .await
        {
            Ok(v) => v,
            Err(_) => return,
        };
        changed.mark_changed();
        let mut retry = None;
        loop {
            tokio::select! {_=shutdown.changed()=>return,_=changed.changed()=>{},_=async{if let Some(at)=retry{tokio::time::sleep_until(at).await}else{std::future::pending().await}}=>{}}
            changed.borrow_and_update();
            retry = None;
            match self.relay_pending().await {
                Ok((true, delivered)) if delivered > 0 => changed.mark_changed(),
                Ok((true, _)) => {
                    retry = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(1));
                }
                Ok((false, _)) => {}
                Err(_) => {
                    tracing::warn!("Sidecar message relay will retry");
                    retry = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(1));
                }
            }
        }
    }
}
