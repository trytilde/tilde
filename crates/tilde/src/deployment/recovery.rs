//! Postgres serializes reassignment. A durable outbox separates that decision
//! from Corrosion publication; retries never elect a second owner for a generation.
use super::{
    Deployments,
    corrosion::client::statement,
    runtime::{Runtime, execution::CommandRow},
};
use crate::{
    encryption::{SealedSecret, SecretBinding},
    error::Error,
    proto::tilde::types::v1 as types,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use buffa::Message;
use futures::StreamExt;
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;
use zeroize::Zeroizing;
impl Deployments {
    pub(crate) fn seal_record<M: Message>(
        &self,
        key: Uuid,
        kind: &str,
        value: &M,
    ) -> Result<Vec<u8>, Error> {
        let bytes = Zeroizing::new(value.encode_to_vec());
        let text = SecretString::from(STANDARD.encode(bytes.as_slice()));
        Ok(self
            .encryption
            .seal(
                SecretBinding {
                    resource_kind: "sidecar_control",
                    resource_id: key,
                    name: kind,
                },
                &text,
            )?
            .into_bytes())
    }
    pub(crate) fn open_record<M: Message>(
        &self,
        key: Uuid,
        kind: &str,
        sealed: &[u8],
    ) -> Result<M, Error> {
        let text = self.encryption.open(
            SecretBinding {
                resource_kind: "sidecar_control",
                resource_id: key,
                name: kind,
            },
            SealedSecret::from_bytes(sealed)?,
        )?;
        let bytes = Zeroizing::new(
            STANDARD
                .decode(text.expose_secret())
                .map_err(|_| Error::Encryption)?,
        );
        M::decode_from_slice(bytes.as_slice()).map_err(|_| Error::Encryption)
    }
    pub async fn project_command(&self, source: &Runtime, row: CommandRow) -> Result<(), Error> {
        let agent = super::id(&row.agent_id)?;
        if agent != source.agent_id {
            return Err(Error::Denied);
        }
        let command: types::AgentCommand =
            source.open(super::id(&row.id)?, "command", &row.payload)?;
        if command.agent_id != row.agent_id
            || command.owner_instance_id != row.owner_instance_id
            || command.generation != row.generation as u64
        {
            return Err(Error::Denied);
        }
        let thread = super::id(&row.thread_id)?;
        let participant = super::id(&row.participant_id)?;
        let owner = super::id(&row.owner_instance_id)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!(
            "../../queries/deployment/project/assignment.sql",
            thread,
            participant,
            agent,
            owner,
            row.generation,
            false
        )
        .execute(&mut *tx)
        .await?;
        let invocation = match &command.action {
            Some(types::agent_command::Action::Invoke(v)) => super::id(&v.invocation_id)?,
            Some(types::agent_command::Action::Steer(v)) => super::id(&v.invocation_id)?,
            None => return Err(Error::Denied),
        };
        let input_id = match &command.action {
            Some(types::agent_command::Action::Steer(v)) => Some(super::id(&v.input_id)?),
            _ => None,
        };
        let key = super::id(&row.id)?;
        let payload = self.seal_record(key, "command", &command)?;
        sqlx::query_file!(
            "../../queries/deployment/project/command.sql",
            key,
            row.generation,
            agent,
            thread,
            participant,
            owner,
            row.kind,
            payload,
            chrono::DateTime::from_timestamp_millis(row.created_at).ok_or(Error::Denied)?,
            row.acked_at
                .and_then(chrono::DateTime::from_timestamp_millis),
            row.finished_at
                .and_then(chrono::DateTime::from_timestamp_millis),
            row.failure,
            invocation,
            input_id
        )
        .execute(&mut *tx)
        .await?;
        let input = match &command.action {
            Some(types::agent_command::Action::Steer(v)) => super::id(&v.input_id)?,
            _ => key,
        };
        sqlx::query_file!(
            "../../queries/deployment/project/dispatch_ready.sql",
            thread,
            agent,
            input
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn replicate_commands(
        &self,
        source: &Runtime,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), Error> {
        let mut commands = source
            .client
            .subscribe::<CommandRow>("SELECT * FROM commands", vec![])
            .await?;
        loop {
            tokio::select! {_=shutdown.changed()=>return Ok(()),row=commands.next()=>match row{Some(Ok(row)) if !row.deleted=>self.project_command(source,row.value).await?,Some(Ok(_))=>{},_=>return Ok(())}}
        }
    }
    pub async fn recover(&self) -> Result<usize, Error> {
        let rows = sqlx::query_file!("../../queries/deployment/due_commands.sql")
            .fetch_all(&self.pool)
            .await?;
        let mut count = 0;
        for row in rows {
            let mut tx = self.pool.begin().await?;
            // Follow the thread -> participant lock order used by cold dispatch.
            if sqlx::query_file!("../../queries/chat/thread_lock.sql", row.thread_id)
                .fetch_optional(&mut *tx)
                .await?
                .is_none()
            {
                continue;
            }

            let Some(current) = sqlx::query_file!(
                "../../queries/deployment/command_owner.sql",
                row.thread_id,
                row.participant_id
            )
            .fetch_optional(&mut *tx)
            .await?
            else {
                continue;
            };
            if current.generation != row.generation || current.stopped {
                continue;
            }
            let still_due = sqlx::query_file!("../../queries/deployment/due_commands.sql")
                .fetch_all(&mut *tx)
                .await?
                .iter()
                .any(|c| c.id == row.id && c.generation == row.generation);
            if !still_due {
                continue;
            }
            let original: types::AgentCommand =
                self.open_record(row.id, "command", &row.payload)?;
            let stopped = row.failure_mode == "stop";
            let owner = if stopped {
                row.owner_instance_id
            } else {
                let Some(node) = sqlx::query_file!(
                    "../../queries/deployment/replacement.sql",
                    row.agent_id,
                    row.owner_instance_id
                )
                .fetch_optional(&mut *tx)
                .await?
                else {
                    continue;
                };
                node.instance_id
            };
            let generation = current.generation.checked_add(1).ok_or(Error::Conflict)?;
            let old_invocation = match &original.action {
                Some(types::agent_command::Action::Invoke(v)) => super::id(&v.invocation_id)?,
                Some(types::agent_command::Action::Steer(v)) => super::id(&v.invocation_id)?,
                None => return Err(Error::Denied),
            };
            let mut change = types::AssignmentChange {
                assignment: types::ParticipantAssignment {
                    thread_id: row.thread_id.to_string(),
                    participant_id: row.participant_id.to_string(),
                    agent_id: row.agent_id.to_string(),
                    owner_instance_id: owner.to_string(),
                    generation: generation as u64,
                    stopped,
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            };
            sqlx::query_file!(
                "../../queries/deployment/reassign.sql",
                row.thread_id,
                row.participant_id,
                owner,
                generation,
                stopped
            )
            .execute(&mut *tx)
            .await?;
            let failed = sqlx::query_file!(
                "../../queries/deployment/fail_old_invocations.sql",
                row.thread_id,
                row.agent_id
            )
            .fetch_all(&mut *tx)
            .await?;
            for invocation in failed {
                crate::chat::activity(
                    &mut tx,
                    row.thread_id,
                    "invocation.ended",
                    invocation.id,
                    "",
                )
                .await?;
            }
            crate::chat::activity(
                &mut tx,
                row.thread_id,
                "participant.assigned",
                row.participant_id,
                "",
            )
            .await?;
            if stopped {
                let record =
                    sqlx::query_file!("../../queries/deployment/recovery_run.sql", old_invocation)
                        .fetch_one(&mut *tx)
                        .await?;
                let mut run = self.chat().run(record.run_id).await?;
                run.status = "failed".into();
                run.invocation_status = "failed".into();
                sqlx::query_file!("../../queries/deployment/stop_run.sql", record.run_id)
                    .execute(&mut *tx)
                    .await?;
                crate::chat::activity(&mut tx, row.thread_id, "run.updated", record.run_id, "")
                    .await?;
                change.run = run.into();
            }
            if !stopped {
                let (run_id, objective) = match original.action {
                    Some(types::agent_command::Action::Invoke(invoke)) => {
                        (super::id(&invoke.run_id)?, invoke.objective)
                    }
                    Some(types::agent_command::Action::Steer(steer)) => {
                        let run = sqlx::query_file!(
                            "../../queries/deployment/recovery_run.sql",
                            super::id(&steer.invocation_id)?
                        )
                        .fetch_one(&mut *tx)
                        .await?;
                        (run.run_id, format!("{}\n{}", run.objective, steer.text))
                    }
                    None => return Err(Error::Denied),
                };
                let invocation = Uuid::new_v4();
                let command = Uuid::new_v4();
                let run = types::Run {
                    id: run_id.to_string(),
                    thread_id: row.thread_id.to_string(),
                    agent_id: row.agent_id.to_string(),
                    objective: objective.clone(),
                    status: "active".into(),
                    invocation_id: invocation.to_string(),
                    invocation_status: "pending".into(),
                    ..Default::default()
                };
                change.run = run.clone().into();
                change.invocation = types::InvocationState {
                    id: invocation.to_string(),
                    thread_id: row.thread_id.to_string(),
                    run_id: run_id.to_string(),
                    agent_id: row.agent_id.to_string(),
                    participant_id: row.participant_id.to_string(),
                    owner_instance_id: owner.to_string(),
                    generation: generation as u64,
                    status: "pending".into(),
                    objective: objective.clone(),
                    ..Default::default()
                }
                .into();
                let action = types::AgentCommand {
                    id: command.to_string(),
                    thread_id: row.thread_id.to_string(),
                    participant_id: row.participant_id.to_string(),
                    agent_id: row.agent_id.to_string(),
                    owner_instance_id: owner.to_string(),
                    generation: generation as u64,
                    created_at: chrono::Utc::now().timestamp_millis(),
                    action: Some(
                        types::InvokeCommand {
                            invocation_id: invocation.to_string(),
                            run_id: run_id.to_string(),
                            objective,
                            ..Default::default()
                        }
                        .into(),
                    ),
                    ..Default::default()
                };
                let sealed = self.seal_record(command, "command", &action)?;
                sqlx::query_file!(
                    "../../queries/deployment/new_command.sql",
                    command,
                    generation,
                    row.agent_id,
                    row.thread_id,
                    row.participant_id,
                    owner,
                    "invoke",
                    sealed,
                    invocation
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/deployment/project/run.sql",
                    run_id,
                    row.thread_id,
                    row.agent_id,
                    run.objective,
                    "active",
                    None::<Uuid>,
                    run.id
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/deployment/project/invocation.sql",
                    invocation,
                    run_id,
                    row.thread_id,
                    row.agent_id,
                    "pending",
                    chrono::Utc::now(),
                    None::<chrono::DateTime<chrono::Utc>>
                )
                .execute(&mut *tx)
                .await?;
                change.command = action.into();
            }
            let outbox = Uuid::new_v4();
            let payload = self.seal_record(outbox, "assignment", &change)?;
            sqlx::query_file!(
                "../../queries/deployment/outbox_insert.sql",
                outbox,
                row.agent_id,
                "assignment",
                payload
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            count += 1;
        }
        Ok(count)
    }
    pub async fn publish_outbox(&self) -> Result<bool, Error> {
        let rows = sqlx::query_file!("../../queries/deployment/outbox_pending.sql")
            .fetch_all(&self.pool)
            .await?;
        let more = rows.len() == 50;
        for row in rows {
            let result = async {
                if row.kind == "bridge" {
                    let snapshot: types::BridgeSnapshot =
                        self.open_record(row.id, "bridge", &row.payload)?;
                    self.deliver_bridge(row.agent_id, snapshot).await?;
                    sqlx::query_file!("../../queries/deployment/outbox_done.sql", row.id)
                        .execute(&self.pool)
                        .await?;
                    return Ok::<_, Error>(());
                }
                if row.kind == "attachment" {
                    let transfer: types::AttachmentTransfer =
                        self.open_record(row.id, "attachment", &row.payload)?;
                    self.transfer_attachment(row.agent_id, &transfer).await?;
                    sqlx::query_file!("../../queries/deployment/outbox_done.sql", row.id)
                        .execute(&self.pool)
                        .await?;
                    return Ok::<_, Error>(());
                }
                if row.kind != "assignment" {
                    return Ok::<_, Error>(());
                }
                let change: types::AssignmentChange =
                    self.open_record(row.id, "assignment", &row.payload)?;
                let thread = super::id(&change.assignment.thread_id)?;
                let location = sqlx::query_file!(
                    "../../queries/deployment/project/location.sql",
                    thread,
                    row.agent_id
                )
                .fetch_optional(&self.pool)
                .await?;
                if !location.is_some_and(|r| r.storage == "postgres") {
                    let peer = self.peer(row.agent_id).await?.ok_or_else(|| {
                        Error::Invalid("No Corrosion peer available for reassignment".into())
                    })?;
                    peer.apply_assignment(&change).await?;
                }
                let mut tx = self.pool.begin().await?;
                sqlx::query_file!("../../queries/deployment/outbox_done.sql", row.id)
                    .execute(&mut *tx)
                    .await?;
                if let Some(command) = change.command.as_option() {
                    sqlx::query_file!(
                        "../../queries/deployment/command_dispatched.sql",
                        super::id(&command.id)?,
                        command.generation as i64
                    )
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
                Ok::<_, Error>(())
            }
            .await;
            if result.is_err() {
                sqlx::query_file!("../../queries/deployment/outbox_retry.sql", row.id)
                    .execute(&self.pool)
                    .await?;
                tracing::warn!(agent_id=%row.agent_id,kind=%row.kind,"Sidecar outbox delivery will retry");
            }
        }
        Ok(more)
    }
    pub async fn outbox_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let notifications = crate::database::notifications::Notifications::default();
        let mut changes = match notifications
            .subscribe(&self.pool, "tilde_sidecar_outbox")
            .await
        {
            Ok(value) => value,
            Err(_) => return,
        };
        changes.mark_changed();
        let mut retry = None;
        loop {
            tokio::select! {
                _=shutdown.changed()=>break,
                _=changes.changed()=>{},
                _=async{if let Some(deadline)=retry{tokio::time::sleep_until(deadline).await}else{std::future::pending().await}}=>{},
            }
            changes.borrow_and_update();
            retry = None;
            match self.publish_outbox().await {
                Ok(true) => changes.mark_changed(),
                Ok(false) => {
                    let next = sqlx::query_file!("../../queries/deployment/outbox_deadline.sql")
                        .fetch_one(&self.pool)
                        .await;
                    retry = match next {
                        Ok(row) => row.deadline.map(|at| {
                            tokio::time::Instant::now()
                                + (at - chrono::Utc::now())
                                    .to_std()
                                    .unwrap_or(Duration::from_millis(100))
                        }),
                        Err(_) => Some(tokio::time::Instant::now() + Duration::from_secs(1)),
                    };
                }
                Err(_) => {
                    tracing::warn!("Sidecar control publication will retry");
                    retry = Some(tokio::time::Instant::now() + Duration::from_secs(1));
                }
            }
        }
    }
}
impl Runtime {
    pub async fn apply_assignment(
        &self,
        change: &types::AssignmentChange,
    ) -> crate::chat::Result<()> {
        let assignment = &change.assignment;
        let thread = crate::chat::id(&assignment.thread_id)?;
        let participant = crate::chat::id(&assignment.participant_id)?;
        let _guard = self.mutation.lock().await;
        let current = self.assignment(thread, participant).await?;
        if current.generation > assignment.generation {
            return Ok(());
        }
        let mut writes = vec![statement(
            "INSERT INTO assignments(id,thread_id,participant_id,agent_id,owner_instance_id,generation,stopped) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO NOTHING",
            vec![
                json!(format!(
                    "{}:{}",
                    assignment.participant_id, assignment.generation
                )),
                json!(thread),
                json!(participant),
                json!(assignment.agent_id),
                json!(assignment.owner_instance_id),
                json!(assignment.generation),
                json!(i32::from(assignment.stopped)),
            ],
        )];
        let old = self.client.query::<super::runtime::IdRow>("SELECT id FROM invocations WHERE thread_id=? AND participant_id=? AND generation<? AND status IN ('pending','running')",vec![json!(thread),json!(participant),json!(assignment.generation)]).await?;
        for row in old {
            let mut invocation = self.invocation(crate::chat::id(&row.id)?).await?;
            invocation.status = "failed".into();
            invocation.ended_at = chrono::Utc::now().timestamp_millis();
            writes.push(self.invocation_insert(&invocation)?);
        }
        writes.push(statement("UPDATE commands SET finished_at=?,failure='ownership_changed' WHERE thread_id=? AND participant_id=? AND generation<? AND finished_at IS NULL", vec![json!(chrono::Utc::now().timestamp_millis()),json!(thread),json!(participant),json!(assignment.generation)]));
        if let Some(run) = change.run.as_option() {
            writes.push(statement(
                "UPDATE runs SET status=?,payload=? WHERE id=?",
                vec![
                    json!(run.status),
                    json!(self.seal(crate::chat::id(&run.id)?, "run", run)?),
                    json!(run.id),
                ],
            ));
        }
        if let Some(invocation) = change.invocation.as_option() {
            writes.push(self.invocation_insert(invocation)?);
        }
        if let Some(command) = change.command.as_option() {
            writes.push(self.command_insert(command)?);
        }
        writes.push(statement("INSERT INTO write_guards(id,reason) SELECT NULL,'stale assignment' WHERE EXISTS(SELECT 1 FROM assignments WHERE thread_id=? AND participant_id=? AND generation>?)", vec![json!(thread),json!(participant),json!(assignment.generation)]));
        self.client.transaction(writes).await?;
        Ok(())
    }
}

impl Deployments {
    pub async fn recovery_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let notifications = crate::database::notifications::Notifications::default();
        let mut changes = match notifications
            .subscribe(&self.pool, "tilde_sidecar_recovery")
            .await
        {
            Ok(v) => v,
            Err(_) => return,
        };
        changes.mark_changed();
        let mut deadline = None;
        loop {
            tokio::select! {
                _=shutdown.changed()=>return,
                _=changes.changed()=>{},
                _=async { if let Some(at)=deadline { tokio::time::sleep_until(at).await } else { std::future::pending().await } }=>{},
            }
            changes.borrow_and_update();
            let result = async {
                self.recover().await?;
                let next = sqlx::query_file!("../../queries/deployment/recovery_deadline.sql")
                    .fetch_one(&self.pool)
                    .await?
                    .deadline;
                Ok::<_, Error>(next.map(|at| {
                    tokio::time::Instant::now()
                        + (at - chrono::Utc::now()).to_std().unwrap_or_default()
                }))
            };
            tokio::select! { _=shutdown.changed()=>return, result=result=>{
                deadline=match result { Ok(next)=>next,Err(_)=>{tracing::warn!("Sidecar recovery will retry");Some(tokio::time::Instant::now()+Duration::from_secs(2))} };
            }}
        }
    }
}
