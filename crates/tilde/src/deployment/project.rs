//! Continuous Corrosion -> Postgres projection. Archive insert, canonical state
//! and activity commit together. Corrosion deletes are retention, never PG deletes.
use super::{
    Deployments,
    corrosion::Client,
    runtime::{EventRow, Runtime, RuntimeOptions},
};
use crate::proto::tilde::types::v1::{self as types, runtime_event::State};
use crate::{
    chat::{self},
    encryption::{Encryption, SecretBinding},
    error::Error,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use buffa::Message;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use secrecy::SecretString;
use sqlx::{Postgres, Transaction};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use zeroize::Zeroizing;
fn uuid(value: &str) -> Result<Uuid, Error> {
    super::id(value)
}
fn at(value: i64) -> Result<DateTime<Utc>, Error> {
    DateTime::from_timestamp_millis(value)
        .ok_or_else(|| Error::Invalid("Invalid event timestamp".into()))
}
impl Deployments {
    /// The remote handle is an adapter for queries/decryption only; it never
    /// dispatches agent work or becomes a participant owner.
    pub async fn peer(&self, agent: Uuid) -> Result<Option<Runtime>, Error> {
        let row = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_one(&self.pool)
            .await?;
        let secrets = self.open_secrets(
            agent,
            row.encrypted_secrets.as_deref().ok_or(Error::Denied)?,
        )?;
        for node in sqlx::query_file!("../../queries/deployment/project/node_available.sql", agent)
            .fetch_all(&self.pool)
            .await?
        {
            let client = Client::new(
                &format!("{}/corrosion", node.agent_ingress_url.trim_end_matches('/')),
                secrets.api_token.clone(),
            )?;
            if client
                .query::<serde_json::Value>("SELECT 1 AS ready", vec![])
                .await
                .is_err()
            {
                continue;
            }
            return Ok(Some(Runtime::new(
                client,
                RuntimeOptions {
                    agent_id: agent,
                    instance_id: node.instance_id,
                    encryption: Arc::new(Encryption::from_agent_key(
                        agent,
                        secrets.encryption_key.clone(),
                    )?),
                    signing_key: secrets.signing_key,
                    host_key: SecretString::from(""),
                    local_endpoint: String::new(),
                    callback_url: String::new(),
                },
            )));
        }
        Ok(None)
    }
    pub async fn project_event(
        &self,
        source: &Runtime,
        event: types::RuntimeEvent,
    ) -> Result<(), Error> {
        if event.archive_origin {
            return Ok(());
        }
        let event_id = uuid(&event.id)?;
        let agent = uuid(&event.agent_id)?;
        let origin = uuid(&event.origin_instance_id)?;
        if agent != source.agent_id || event.origin_sequence < 1 {
            return Err(Error::Denied);
        }
        // Replays remain valid after the conversation has been retired and
        // source dependencies removed. The transactional insert still resolves
        // races between simultaneous projectors below.
        let archived_id = [event_id];
        if sqlx::query_file!(
            "../../queries/deployment/retention/archived_ids.sql",
            agent,
            &archived_id
        )
        .fetch_one(&self.pool)
        .await?
        .count
            != 0
        {
            return Ok(());
        }
        let thread = uuid(&event.thread_id)?;
        let created = at(event.created_at)?;
        // Initial dependency hydration precedes the transaction. Peers commit a
        // complete thread aggregate before exposing its existence row.
        let bootstrap = if !thread.is_nil()
            && !sqlx::query_file!("../../queries/deployment/project/has_thread.sql", thread)
                .fetch_one(&self.pool)
                .await?
                .exists
        {
            Some(source.thread(thread).await?)
        } else {
            None
        };
        #[derive(serde::Deserialize)]
        struct RunOrigin {
            source_identity_id: String,
            channel_origin: i64,
            idempotency_key: String,
        }
        let run_origin = if let Some(State::Run(run)) = &event.state {
            source
                .client
                .query::<RunOrigin>(
                    "SELECT source_identity_id,channel_origin,idempotency_key FROM runs WHERE id=?",
                    vec![serde_json::json!(run.id)],
                )
                .await?
                .pop()
        } else {
            None
        };
        let execution = match &event.state {
            Some(State::Invocation(value)) => Some((**value).clone()),
            Some(State::Run(value)) if !value.invocation_id.is_empty() => {
                Some(source.invocation(uuid(&value.invocation_id)?).await?)
            }
            Some(State::Goal(_) | State::Task(_) | State::ToolCall(_))
                if !event.invocation_id.is_empty() =>
            {
                Some(source.invocation(uuid(&event.invocation_id)?).await?)
            }
            _ => None,
        };
        let bytes = Zeroizing::new(event.encode_to_vec());
        let encoded = SecretString::from(STANDARD.encode(bytes.as_slice()));
        let sealed = self
            .encryption
            .seal(
                SecretBinding {
                    resource_kind: "sidecar_archive",
                    resource_id: event_id,
                    name: "event",
                },
                &encoded,
            )?
            .into_bytes();
        let mut tx = self.pool.begin().await?;
        if sqlx::query_file!(
            "../../queries/deployment/project/archive.sql",
            event_id,
            agent,
            (!thread.is_nil()).then_some(thread),
            origin,
            event.origin_sequence,
            event.kind,
            sealed,
            created
        )
        .fetch_optional(&mut *tx)
        .await?
        .is_none()
        {
            return Ok(());
        }
        if let Some(root) = bootstrap {
            self.project_thread(&mut tx, &root).await?;
        }
        if !thread.is_nil() {
            sqlx::query_file!("../../queries/chat/thread_lock.sql", thread)
                .fetch_one(&mut *tx)
                .await?;
        }
        let assignment = if thread.is_nil() {
            None
        } else {
            sqlx::query_file!("../../queries/deployment/current_owner.sql", thread, agent)
                .fetch_optional(&mut *tx)
                .await?
        };
        // The receiving replica is not necessarily the execution owner. Fence
        // using the invocation's immutable assignment generation, never the
        // replica that happened to accept and replicate this callback.
        let current = execution.as_ref().is_none_or(|v| {
            assignment.as_ref().is_none_or(|a| {
                v.owner_instance_id == a.owner_instance_id.to_string()
                    && v.generation == a.generation as u64
                    && !a.stopped
            })
        });
        let entity = match &event.state {
            Some(State::User(v)) => Some(("user", v.id.as_str())),
            Some(State::Thread(v)) => Some(("thread", v.id.as_str())),
            Some(State::Participant(v)) => Some(("participant", v.id.as_str())),
            Some(State::Message(v)) => Some(("message", v.id.as_str())),
            Some(State::Run(v)) => Some(("run", v.id.as_str())),
            Some(State::Invocation(v)) => Some(("invocation", v.id.as_str())),
            Some(State::Goal(v)) => Some(("goal", v.id.as_str())),
            Some(State::Task(v)) => Some(("task", v.id.as_str())),
            Some(State::ToolCall(v)) => Some(("tool", v.id.as_str())),
            Some(State::Attachment(v)) => Some(("attachment", v.id.as_str())),
            Some(State::ConvertedMessage(v)) => Some(("converted", v.message_id.as_str())),
            _ => None,
        };
        let current = if current {
            if let Some((kind, key)) = entity {
                sqlx::query_file!(
                    "../../queries/deployment/project/version.sql",
                    agent,
                    kind,
                    uuid(key)?,
                    origin,
                    event.origin_sequence,
                    created
                )
                .fetch_optional(&mut *tx)
                .await?
                .is_some()
            } else {
                true
            }
        } else {
            false
        };
        if current {
            match &event.state {
                Some(State::User(value)) => {
                    sqlx::query_file!(
                        "../../queries/deployment/project/user.sql",
                        uuid(&value.id)?,
                        value.name
                    )
                    .execute(&mut *tx)
                    .await?;
                }
                Some(State::Thread(value)) => {
                    if uuid(&value.id)? != thread {
                        return Err(Error::Denied);
                    }
                    self.project_thread(&mut tx, value).await?;
                }
                Some(State::Participant(value)) => {
                    self.project_participant(&mut tx, thread, value).await?
                }
                Some(State::Message(value)) => {
                    if uuid(&value.thread_id)? != thread {
                        return Err(Error::Denied);
                    }
                    let key = uuid(&value.id)?;
                    sqlx::query_file!(
                        "../../queries/deployment/project/message.sql",
                        key,
                        thread,
                        uuid(&value.participant_id)?,
                        value.text,
                        value.status,
                        value
                            .in_reply_to_message_id
                            .as_deref()
                            .map(uuid)
                            .transpose()?,
                        created,
                        value.format,
                        value.subject
                    )
                    .execute(&mut *tx)
                    .await?;
                    if let Some(delivery) = value.delivery.as_option() {
                        sqlx::query_file!(
                            "../../queries/deployment/project/delivery.sql",
                            key,
                            uuid(&delivery.connection_id)?,
                            delivery.destination,
                            delivery.external_message_id,
                            delivery.status
                        )
                        .execute(&mut *tx)
                        .await?;
                    }
                    // Imported messages already belong to the sidecar's durable
                    // dispatch path. Archival must never re-invoke historical input.
                    sqlx::query_file!(
                        "../../queries/deployment/project/dispatch_ready.sql",
                        thread,
                        agent,
                        key
                    )
                    .execute(&mut *tx)
                    .await?;
                    for attachment in &value.attachments {
                        self.project_attachment(&mut tx, thread, attachment).await?;
                        sqlx::query_file!(
                            "../../queries/chat/attachment_attach.sql",
                            thread,
                            key,
                            uuid(&attachment.id)?
                        )
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                Some(State::Goal(value)) => {
                    sqlx::query_file!(
                        "../../queries/deployment/project/goal.sql",
                        uuid(&value.id)?,
                        thread,
                        agent,
                        value.objective,
                        value.status
                    )
                    .execute(&mut *tx)
                    .await?;
                }
                Some(State::Task(value)) => {
                    let key = uuid(&value.id)?;
                    sqlx::query_file!(
                        "../../queries/deployment/project/task.sql",
                        key,
                        thread,
                        agent,
                        value.title,
                        value.status,
                        value.goal_id.as_deref().map(uuid).transpose()?,
                        value.blocked_reason
                    )
                    .execute(&mut *tx)
                    .await?;
                    for dep in &value.dependency_ids {
                        sqlx::query_file!(
                            "../../queries/chat/dependency_create.sql",
                            thread,
                            agent,
                            key,
                            uuid(dep)?
                        )
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                Some(State::Run(value)) => {
                    if uuid(&value.thread_id)? != thread || uuid(&value.agent_id)? != agent {
                        return Err(Error::Denied);
                    }
                    sqlx::query_file!(
                        "../../queries/deployment/project/run.sql",
                        uuid(&value.id)?,
                        thread,
                        agent,
                        value.objective,
                        value.status,
                        value.goal_id.as_deref().map(uuid).transpose()?,
                        value.id
                    )
                    .execute(&mut *tx)
                    .await?;
                    if let Some(origin) = &run_origin {
                        sqlx::query_file!(
                            "../../queries/deployment/project/run_origin.sql",
                            uuid(&value.id)?,
                            if origin.source_identity_id.is_empty() {
                                None
                            } else {
                                Some(uuid(&origin.source_identity_id)?)
                            },
                            origin.channel_origin != 0,
                            origin.idempotency_key
                        )
                        .execute(&mut *tx)
                        .await?;
                        if let Ok(message) = uuid(&origin.idempotency_key) {
                            sqlx::query_file!(
                                "../../queries/deployment/project/dispatch_ready.sql",
                                thread,
                                agent,
                                message
                            )
                            .execute(&mut *tx)
                            .await?;
                        }
                    }
                }
                Some(State::Invocation(value)) => {
                    if uuid(&value.thread_id)? != thread || uuid(&value.agent_id)? != agent {
                        return Err(Error::Denied);
                    }
                    sqlx::query_file!(
                        "../../queries/deployment/project/invocation.sql",
                        uuid(&value.id)?,
                        uuid(&value.run_id)?,
                        thread,
                        agent,
                        value.status,
                        created,
                        if value.lease_expires_at > 0 {
                            Some(at(value.lease_expires_at)?)
                        } else {
                            None
                        }
                    )
                    .execute(&mut *tx)
                    .await?;
                }
                Some(State::Assignment(value)) => {
                    if uuid(&value.thread_id)? != thread || uuid(&value.agent_id)? != agent {
                        return Err(Error::Denied);
                    }
                    sqlx::query_file!(
                        "../../queries/deployment/project/assignment.sql",
                        thread,
                        uuid(&value.participant_id)?,
                        agent,
                        uuid(&value.owner_instance_id)?,
                        value.generation as i64,
                        value.stopped
                    )
                    .execute(&mut *tx)
                    .await?;
                }
                Some(State::Attachment(value)) => {
                    self.project_attachment(&mut tx, thread, value).await?;
                    self.queue_attachment(&mut tx, agent, thread, value).await?;
                }
                Some(State::AttachmentSource(value)) => {
                    self.project_attachment(&mut tx, thread, &value.attachment)
                        .await?;
                    self.queue_attachment(&mut tx, agent, thread, &value.attachment)
                        .await?;
                }
                Some(State::ConvertedMessage(value)) => {
                    sqlx::query_file!(
                        "../../queries/chat/cache_upsert.sql",
                        agent,
                        uuid(&value.message_id)?,
                        serde_json::from_str::<serde_json::Value>(&value.message_json)
                            .map_err(|_| Error::Invalid("Invalid converted message".into()))?
                    )
                    .execute(&mut *tx)
                    .await?;
                }
                Some(State::ToolCall(value)) => {
                    let invocation = uuid(&event.invocation_id)?;
                    let participant = uuid(&event.participant_id)?;
                    sqlx::query_file!(
                        "../../queries/deployment/project/tool.sql",
                        uuid(&value.id)?,
                        thread,
                        invocation,
                        participant,
                        value.name,
                        value.provider_id,
                        value.status,
                        value.input_json,
                        value.output_json,
                        value.error
                    )
                    .execute(&mut *tx)
                    .await?;
                }
                Some(State::ChannelDecision(value)) => {
                    let connection = uuid(&value.connection_id)?;
                    let identity = uuid(&value.identity_id)?;
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
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query_file!(
                        "../../queries/channel_access/identity_upsert.sql",
                        identity,
                        connection,
                        crate::chat::access::identity::kind_name(
                            value.identity_type.as_known().ok_or(Error::Denied)?
                        ),
                        value.value
                    )
                    .fetch_one(&mut *tx)
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
                    .execute(&mut *tx)
                    .await?;
                }
                Some(State::Activity(_) | State::Typing(_)) => {}
                None => return Err(Error::Invalid("Event requires typed state".into())),
            }
        }
        if !thread.is_nil() {
            sqlx::query_file!(
                "../../queries/deployment/project/placement.sql",
                thread,
                agent,
                created
            )
            .execute(&mut *tx)
            .await?;
            let activity = super::runtime::messages::event_activity(event);
            chat::audit::append(&mut tx, thread, activity).await?;
        }
        tx.commit().await?;
        Ok(())
    }
    async fn project_thread(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        thread: &types::Thread,
    ) -> Result<(), Error> {
        let key = uuid(&thread.id)?;
        sqlx::query_file!(
            "../../queries/deployment/project/thread.sql",
            key,
            thread.title,
            uuid(&thread.primary_agent_id)?
        )
        .execute(&mut **tx)
        .await?;
        if let Some(channel) = thread.channel.as_option() {
            sqlx::query_file!(
                "../../queries/chat/channel_thread_bind.sql",
                key,
                uuid(&channel.connection_id)?,
                channel.external_id,
                uuid(&thread.primary_agent_id)?
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
                uuid(user)?,
                value.name
            )
            .execute(&mut **tx)
            .await?;
        }
        sqlx::query_file!(
            "../../queries/deployment/project/participant.sql",
            uuid(&value.id)?,
            thread,
            value.user_id.as_deref().map(uuid).transpose()?,
            value.agent_id.as_deref().map(uuid).transpose()?,
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
        if uuid(&value.thread_id)? != thread {
            return Err(Error::Denied);
        }
        sqlx::query_file!(
            "../../queries/deployment/project/attachment.sql",
            uuid(&value.id)?,
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
    async fn queue_attachment(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        agent: Uuid,
        thread: Uuid,
        value: &types::Attachment,
    ) -> Result<(), Error> {
        let key = uuid(&value.id)?;
        let outbox = Uuid::new_v5(&key, b"s3-upload");
        let transfer = types::AttachmentTransfer {
            attachment_id: value.id.clone(),
            thread_id: thread.to_string(),
            ..Default::default()
        };
        let payload = self.seal_record(outbox, "attachment", &transfer)?;
        sqlx::query_file!(
            "../../queries/deployment/attachment_outbox.sql",
            outbox,
            agent,
            payload,
            key
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
    pub async fn replicate_agent(
        &self,
        agent: Uuid,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        loop {
            if *shutdown.borrow() {
                return;
            }
            if let Ok(Some(source)) = self.peer(agent).await {
                let command_source = source.clone();
                let command_service = self.clone();
                let command_shutdown = shutdown.clone();
                let mut command_task = tokio::spawn(async move {
                    command_service
                        .replicate_commands(&command_source, command_shutdown)
                        .await
                });
                let telemetry_service = self.clone();
                let telemetry_source = source.clone();
                let telemetry_shutdown = shutdown.clone();
                let mut telemetry_task = tokio::spawn(async move {
                    telemetry_service
                        .replicate_telemetry(&telemetry_source, telemetry_shutdown)
                        .await
                });
                if let Ok(mut events) = source
                    .client
                    .subscribe::<EventRow>(
                        "SELECT * FROM events ORDER BY origin_instance_id,origin_sequence",
                        vec![],
                    )
                    .await
                {
                    let mut deferred = std::collections::VecDeque::new();
                    let mut retry = tokio::time::interval(Duration::from_secs(1));
                    loop {
                        tokio::select! {
                            _=shutdown.changed()=>{command_task.abort();telemetry_task.abort();return;},
                            _=&mut command_task=>break,
                            _=&mut telemetry_task=>break,
                            _=retry.tick(),if !deferred.is_empty()=>{
                                let batch=deferred.iter().take(25).copied().collect::<Vec<Uuid>>();
                                for key in batch {
                                    deferred.pop_front();
                                    let result=async {
                                        let row=source.client.query::<super::runtime::Payload>("SELECT payload FROM events WHERE id=?",vec![serde_json::json!(key)]).await?.pop().ok_or(Error::NotFound)?;
                                        let event=source.open(key,"event",&row.payload)?;
                                        self.project_event(&source,event).await
                                    }.await;
                                    if result.is_err(){deferred.push_back(key);}
                                }
                            },
                            event=events.next()=>match event{
                                Some(Ok(row)) if !row.deleted=>{
                                    let result=async{let event=source.open::<types::RuntimeEvent>(uuid(&row.value.id)?,"event",&row.value.payload)?;self.project_event(&source,event).await}.await;
                                    if let Err(error)=result{tracing::warn!(agent_id=%agent,error=%error,"Sidecar projection awaits dependencies or retry");if let Ok(key)=uuid(&row.value.id)&& !deferred.contains(&key){deferred.push_back(key);}}
                                },Some(Ok(_))=>{},_=>break,
                            }
                        }
                    }
                }
                command_task.abort();
                telemetry_task.abort();
            }
            tokio::select! {_=shutdown.changed()=>return,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        }
    }
    pub async fn worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let notifications = crate::database::notifications::Notifications::default();
        let mut tasks = tokio::task::JoinSet::new();
        let mut agents = std::collections::BTreeSet::new();
        let mut changes = match notifications
            .subscribe(&self.pool, "tilde_sidecar_configuration")
            .await
        {
            Ok(v) => v,
            Err(_) => return,
        };
        changes.mark_changed();
        loop {
            tokio::select! {
                _=shutdown.changed()=>break,
                _=tasks.join_next(),if !tasks.is_empty()=>{},
                value=changes.changed()=>{
                    if value.is_err(){break;}
                    if let Ok(rows)=sqlx::query_file!("../../queries/deployment/project/candidates.sql").fetch_all(&self.pool).await{for row in rows{if let Ok(Some(peer))=self.peer(row.id).await
                        && let Ok(mut configuration)=self.configuration(row.id).await {
                            let _=peer.configure(&configuration).await;
                            super::runtime::providers::clear_secrets(&mut configuration);
                        }
                    if agents.insert(row.id){let service=self.clone();let rx=shutdown.clone();tasks.spawn(async move{service.replicate_agent(row.id,rx).await;});}}}
                }
            }
        }
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
}
