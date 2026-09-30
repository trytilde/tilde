//! Events published by replicas become canonical Postgres state. A batch is one
//! transaction; receipts deduplicate replays; run state is accepted only from the
//! replica holding the thread's lease. Data appends carry no ownership at all.
use super::{Deployments, Holder, id, lease_frame, liveness_secs};
use crate::database::Transaction;
use crate::proto::tilde::{
    agent_event_ingress::v1 as wire,
    types::v1::{self as types, runtime_event::State},
};
use crate::{chat, error::Error};
use chrono::{DateTime, Utc};
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
        Error::Database(error) => {
            error.is_unique_violation()
                || error.is_foreign_key_violation()
                || error.is_check_violation()
        }
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
        let mut usage = vec![];
        for frame in request.frames {
            match frame.frame {
                Some(wire::upstream::Frame::Inference(record)) => {
                    match crate::inference::audit::Record::from_wire(agent, *record) {
                        Ok(record) => usage.push(record),
                        Err(error) => {
                            tracing::warn!(agent_id=%agent, error=%error, "Rejected sidecar inference record")
                        }
                    }
                }
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
                    if let Err(error) = self.accept_telemetry(agent, deployment, *telemetry).await {
                        if !rejectable(&error) {
                            return Err(error);
                        }
                        tracing::warn!(agent_id=%agent, error=%error, "Rejected sidecar telemetry");
                    }
                }
                None => {}
            }
        }
        if !usage.is_empty() {
            // Only connections assigned to this agent for inference may be billed to it: one
            // read for the whole batch, however many frames it carries.
            let assigned =
                crate::inference::db::assigned_connections_all(&self.pool.get().await?, agent)
                    .await?;
            let before = usage.len();
            usage.retain(|record| assigned.contains(&record.connection_id));
            if usage.len() != before {
                tracing::warn!(agent_id=%agent, dropped = before - usage.len(), "Rejected inference records for unassigned connections");
            }
            if !usage.is_empty() {
                let db = self.pool.get().await?;
                let records: Vec<&crate::inference::audit::Record> = usage.iter().collect();
                crate::inference::db::requests_insert_execute(&db, &records).await?;
                // Sidecar records carry stamps only; see inference::audit.
                let stamped: Vec<_> = usage.iter().map(|r| (r, None)).collect();
                let mut linker = crate::prompts::matching::Linker::new(
                    crate::prompts::Prompts::new(self.pool.clone()),
                );
                crate::inference::audit::link_prompts(&db, &mut linker, &stamped).await;
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
                    let mut tx_client = self.pool.get().await?;
                    let tx = tx_client.transaction().await?;
                    batch.locked.clear();
                    let event = match item {
                        Item::Event(event) => *event,
                        Item::Release(thread) => {
                            self.release_in(&tx, agent, instance, thread).await?;
                            tx.commit().await?;
                            drop(tx_client);
                            continue;
                        }
                    };
                    let event_id = event.id.clone();
                    let kind = event.kind.clone();
                    match self
                        .project_event(&tx, &mut batch, agent, deployment, instance, event)
                        .await
                    {
                        // Not a divergence: the lost lease tells the replica what happened.
                        Ok(Outcome::NotHolder(lease)) => {
                            lost.insert(lease.thread_id.clone(), lease);
                            tx.commit().await?;
                            drop(tx_client);
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
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        for item in items {
            let event = match item {
                Item::Event(event) => event,
                Item::Release(thread) => {
                    self.release_in(&tx, agent, instance, *thread).await?;
                    // Later events on this thread must see the lease gone.
                    batch.leases.remove(thread);
                    continue;
                }
            };
            let event_id = event.id.clone();
            let kind = event.kind.clone();
            match self
                .project_event(
                    &tx,
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
        drop(tx_client);
        Ok((rejected, lost))
    }
    /// Kinds of state only the lease holder may write: they drive the agent process.
    fn needs_lease(state: &Option<State>) -> bool {
        matches!(
            state,
            Some(State::Run(_) | State::Invocation(_) | State::ToolCall(_) | State::Queue(_))
        )
    }
    async fn project_event(
        &self,
        tx: &Transaction<'_>,
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
            if Self::needs_lease(&event.state) || event.kind == "thread.updated" {
                let holder = match batch.leases.get(&thread) {
                    Some(holder) => holder.as_ref().map(|h| h.instance),
                    None => {
                        let row = crate::deployment::db::lease_holder_opt(
                            tx,
                            thread,
                            agent,
                            liveness_secs(),
                        )
                        .await?;
                        let holder = row.map(|r| Holder {
                            instance: r.instance_id,
                            deployment: r.deployment_id,
                            version: r.updated_at,
                        });
                        let value = holder.as_ref().map(|h| h.instance);
                        batch.leases.insert(thread, holder);
                        value
                    }
                };
                if holder != Some(instance) {
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
        if crate::deployment::db::event_receipt_opt(
            tx,
            event_id,
            agent,
            (!thread.is_nil()).then_some(thread),
            instance,
            event.origin_sequence,
            created,
        )
        .await?
        .is_none()
        {
            return Ok(Outcome::Duplicate);
        }
        if !thread.is_nil() && !batch.members.contains(&thread) {
            let exists = crate::deployment::db::project_has_thread_one(tx, thread)
                .await?
                .exists;
            let member = if exists {
                // An existing conversation accepts events only from agents in it.
                crate::deployment::db::thread_agent_one(tx, thread, agent)
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
                crate::deployment::db::project_user_execute(tx, id(&value.id)?, &(value.name))
                    .await?;
            }
            Some(State::Thread(value)) => {
                if id(&value.id)? != thread {
                    return Err(Error::Denied);
                }
                self.project_thread(tx, value).await?;
                crate::deployment::db::project_lease_pin_execute(tx, thread, agent).await?;
            }
            Some(State::Participant(value)) => {
                self.project_participant(tx, thread, value).await?;
                crate::deployment::db::project_lease_pin_execute(tx, thread, agent).await?;
            }
            Some(State::Message(value)) => {
                if id(&value.thread_id)? != thread {
                    return Err(Error::Denied);
                }
                let key = id(&value.id)?;
                crate::deployment::db::project_message_execute(
                    tx,
                    key,
                    thread,
                    id(&value.participant_id)?,
                    &(value.text),
                    &(value.status),
                    value
                        .in_reply_to_message_id
                        .as_deref()
                        .map(id)
                        .transpose()?,
                    created,
                    &(value.format),
                    value.subject.as_deref(),
                )
                .await?;
                if let Some(delivery) = value.delivery.as_option() {
                    crate::deployment::db::project_delivery_execute(
                        tx,
                        key,
                        id(&delivery.connection_id)?,
                        &(delivery.destination),
                        &(delivery.external_message_id),
                        &(delivery.status),
                    )
                    .await?;
                }
                for target in &value.addressed_participant_ids {
                    crate::deployment::db::project_message_target(tx, key, thread, id(target)?)
                        .await?;
                }
                // The publishing agent already routed this message locally.
                crate::deployment::db::project_dispatch_execute(tx, key, agent).await?;
                for attachment in &value.attachments {
                    self.project_attachment(tx, thread, attachment).await?;
                    crate::deployment::db::project_message_attachment(
                        tx,
                        thread,
                        key,
                        id(&attachment.id)?,
                    )
                    .await?;
                }
            }
            Some(State::Goal(value)) => {
                crate::deployment::db::project_goal_execute(
                    tx,
                    id(&value.id)?,
                    thread,
                    agent,
                    &(value.objective),
                    &(value.status),
                )
                .await?;
            }
            Some(State::Task(value)) => {
                let key = id(&value.id)?;
                crate::deployment::db::project_task_execute(
                    tx,
                    key,
                    thread,
                    agent,
                    &(value.title),
                    &(value.status),
                    value.goal_id.as_deref().map(id).transpose()?,
                    &(value.blocked_reason),
                )
                .await?;
                for dep in &value.dependency_ids {
                    crate::chat::db::dependency_create_execute(tx, thread, agent, key, id(dep)?)
                        .await?;
                }
            }
            Some(State::Run(value)) => {
                if id(&value.thread_id)? != thread || id(&value.agent_id)? != agent {
                    return Err(Error::Denied);
                }
                crate::deployment::db::project_run_execute(
                    tx,
                    id(&value.id)?,
                    thread,
                    agent,
                    &(value.objective),
                    &(value.status),
                    value.goal_id.as_deref().map(id).transpose()?,
                    &(value.id),
                )
                .await?;
            }
            Some(State::Invocation(value)) => {
                if id(&value.thread_id)? != thread || id(&value.agent_id)? != agent {
                    return Err(Error::Denied);
                }
                crate::deployment::db::project_invocation_execute(
                    tx,
                    id(&value.id)?,
                    id(&value.run_id)?,
                    thread,
                    agent,
                    &(value.status),
                    created,
                    if value.lease_expires_at > 0 {
                        Some(at(value.lease_expires_at)?)
                    } else {
                        None
                    },
                    Some(deployment),
                    if value.history_through_message_id.is_empty() {
                        None
                    } else {
                        Some(id(&value.history_through_message_id)?)
                    },
                )
                .await?;
            }
            Some(State::Attachment(value)) => self.project_attachment(tx, thread, value).await?,
            Some(State::AttachmentSource(value)) => {
                self.project_attachment(tx, thread, &value.attachment)
                    .await?
            }
            Some(State::ConvertedMessage(value)) => {
                crate::chat::db::cache_upsert_execute(
                    tx,
                    agent,
                    id(&value.message_id)?,
                    &(serde_json::from_str::<serde_json::Value>(&value.message_json)
                        .map_err(|_| Error::Invalid("Invalid converted message".into()))?),
                )
                .await?;
            }
            Some(State::ToolCall(value)) => {
                crate::deployment::db::project_tool_execute(
                    tx,
                    id(&value.id)?,
                    thread,
                    id(&event.invocation_id)?,
                    id(&event.participant_id)?,
                    &(value.name),
                    &(value.provider_id),
                    &(value.status),
                    &(value.input_json),
                    &(value.output_json),
                    &(value.error),
                    &(value.summary),
                )
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
                crate::deployment::db::project_user_execute(tx, identity, &(value.value)).await?;
                crate::chat::access::db::identity_upsert_one(
                    tx,
                    identity,
                    connection,
                    chat::access::identity::kind_name(
                        value.identity_type.as_known().ok_or(Error::Denied)?,
                    ),
                    &(value.value),
                )
                .await?;
                crate::chat::access::db::audit_execute(
                    tx,
                    connection,
                    &(value.event_id),
                    agent,
                    identity,
                    mode,
                    value.accepted,
                )
                .await?;
            }
            Some(State::Queue(queue)) => {
                if id(&queue.agent_id)? != agent {
                    return Err(Error::Denied);
                }
                crate::chat::providers::tilde::db::replace_queue(tx, thread, agent, &queue.items)
                    .await?;
            }
            Some(State::Activity(_) | State::Typing(_)) => {}
            None => return Err(Error::Invalid("Event requires typed state".into())),
        }
        if !thread.is_nil() {
            let activity = super::runtime::messages::event_activity(event);
            chat::audit::append(&self.pool, tx, thread, activity).await?;
        }
        Ok(Outcome::Projected)
    }
    async fn project_thread(
        &self,
        tx: &Transaction<'_>,
        thread: &types::Thread,
    ) -> Result<(), Error> {
        let key = id(&thread.id)?;
        crate::deployment::db::project_thread_execute(
            tx,
            key,
            &(thread.title),
            id(&thread.primary_agent_id)?,
        )
        .await?;
        if let Some(channel) = thread.channel.as_option() {
            crate::chat::db::channel_thread_bind_execute(
                tx,
                key,
                id(&channel.connection_id)?,
                &(channel.external_id),
                id(&thread.primary_agent_id)?,
            )
            .await?;
        }
        for participant in &thread.participants {
            self.project_participant(tx, key, participant).await?;
        }
        Ok(())
    }
    async fn project_participant(
        &self,
        tx: &Transaction<'_>,
        thread: Uuid,
        value: &types::Participant,
    ) -> Result<(), Error> {
        if let Some(user) = value.user_id.as_deref() {
            crate::deployment::db::project_user_execute(tx, id(user)?, &(value.name)).await?;
        }
        crate::deployment::db::project_participant_execute(
            tx,
            id(&value.id)?,
            thread,
            value.user_id.as_deref().map(id).transpose()?,
            value.agent_id.as_deref().map(id).transpose()?,
            value.active,
        )
        .await?;
        Ok(())
    }
    async fn project_attachment(
        &self,
        tx: &Transaction<'_>,
        thread: Uuid,
        value: &types::Attachment,
    ) -> Result<(), Error> {
        if id(&value.thread_id)? != thread {
            return Err(Error::Denied);
        }
        crate::deployment::db::project_attachment_execute(
            tx,
            id(&value.id)?,
            thread,
            &(value.filename),
            &(value.media_type),
            value.size_bytes,
            &(value.sha256),
        )
        .await?;
        Ok(())
    }
    /// Hand completed messages to the other sidecar agents in a room.
    pub async fn relay_pending(&self) -> Result<(bool, usize), Error> {
        let rows =
            crate::deployment::db::relay_pending_all(&self.pool.get().await?, liveness_secs())
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
            crate::deployment::db::project_dispatch_execute(
                &self.pool.get().await?,
                row.message_id,
                row.agent_id,
            )
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
