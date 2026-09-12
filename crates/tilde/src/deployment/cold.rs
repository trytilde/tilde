//! Archived conversations use Postgres for every operation while their assigned
//! sidecar still hosts execution. Commands and reports travel over agent-event-ingress.
use super::Deployments;
use crate::{
    chat::{self, Chat},
    error::Error,
    proto::tilde::{agent_host::v1 as host, runtime::v1 as runtime, types::v1 as types},
};
use secrecy::{ExposeSecret, SecretString};
use sqlx::{Postgres, Transaction};
use std::time::Duration;
use uuid::Uuid;
impl Deployments {
    pub(crate) fn chat(&self) -> Chat {
        Chat::new(self.pool.clone(), self.encryption.clone(), String::new())
            .with_connections(self.connections.clone())
            .with_deployments(self.clone())
            .with_objects(self.agents.object_store().cloned())
    }
    async fn cold_owner(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        agent: Uuid,
        thread: Uuid,
        participant: Uuid,
    ) -> Result<Option<(Uuid, i64)>, Error> {
        sqlx::query_file!("../../queries/chat/thread_lock.sql", thread)
            .fetch_one(&mut **tx)
            .await?;
        let current = sqlx::query_file!(
            "../../queries/deployment/command_owner.sql",
            thread,
            participant
        )
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(current) = &current
            && !current.stopped
        {
            return Ok(Some((current.owner_instance_id, current.generation)));
        }
        let Some(node) = sqlx::query_file!("../../queries/deployment/choose_owner.sql", agent)
            .fetch_optional(&mut **tx)
            .await?
        else {
            return Ok(None);
        };
        let generation = current.map(|p| p.generation + 1).unwrap_or(1);
        sqlx::query_file!(
            "../../queries/deployment/project/assignment.sql",
            thread,
            participant,
            agent,
            node.instance_id,
            generation,
            false
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query_file!(
            "../../queries/deployment/reassign.sql",
            thread,
            participant,
            node.instance_id,
            generation,
            false
        )
        .execute(&mut **tx)
        .await?;
        Ok(Some((node.instance_id, generation)))
    }
    pub async fn queue_cold_commands(&self) -> Result<(), Error> {
        for row in sqlx::query_file!("../../queries/deployment/cold_pending.sql")
            .fetch_all(&self.pool)
            .await?
        {
            let mut tx = self.pool.begin().await?;
            let Some((owner, generation)) = self
                .cold_owner(&mut tx, row.agent_id, row.thread_id, row.participant_id)
                .await?
            else {
                continue;
            };
            let id = Uuid::new_v5(&row.id, b"invoke");
            let command = types::AgentCommand {
                id: id.to_string(),
                thread_id: row.thread_id.to_string(),
                participant_id: row.participant_id.to_string(),
                agent_id: row.agent_id.to_string(),
                owner_instance_id: owner.to_string(),
                generation: generation as u64,
                created_at: chrono::Utc::now().timestamp_millis(),
                action: Some(
                    types::InvokeCommand {
                        invocation_id: row.id.to_string(),
                        run_id: row.run_id.to_string(),
                        objective: row.objective,
                        ..Default::default()
                    }
                    .into(),
                ),
                ..Default::default()
            };
            let payload = self.seal_record(id, "command", &command)?;
            sqlx::query_file!(
                "../../queries/deployment/new_command.sql",
                id,
                generation,
                row.agent_id,
                row.thread_id,
                row.participant_id,
                owner,
                "invoke",
                payload,
                row.id
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query_file!(
                "../../queries/deployment/command_dispatched.sql",
                id,
                generation
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        Ok(())
    }
    pub async fn cold_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let notifications = crate::database::notifications::Notifications::default();
        let mut changed = match notifications
            .subscribe(&self.pool, "tilde_sidecar_work")
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
            if self.queue_cold_commands().await.is_err() {
                tracing::warn!("Archived conversation dispatch will retry");
                retry = Some(tokio::time::Instant::now() + Duration::from_secs(1));
            }
        }
    }
    pub async fn pending_commands(
        &self,
        agent: Uuid,
        instance: Uuid,
    ) -> Result<Vec<types::AgentCommand>, Error> {
        if sqlx::query_file!("../../queries/deployment/instance.sql", agent, instance)
            .fetch_optional(&self.pool)
            .await?
            .is_none()
        {
            return Err(Error::Denied);
        }
        sqlx::query_file!(
            "../../queries/deployment/cold_commands.sql",
            agent,
            instance
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| self.open_record(r.id, "command", &r.payload))
        .collect()
    }
    pub async fn invocation_request(
        &self,
        agent: Uuid,
        instance: Uuid,
        command: Uuid,
        generation: i64,
    ) -> Result<host::InvokeRequest, Error> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_file!(
            "../../queries/deployment/command_get.sql",
            command,
            generation,
            agent,
            instance
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::Denied)?;
        let assignment = sqlx::query_file!(
            "../../queries/deployment/command_owner.sql",
            row.thread_id,
            row.participant_id
        )
        .fetch_one(&mut *tx)
        .await?;
        if assignment.generation != generation
            || assignment.owner_instance_id != instance
            || assignment.stopped
            || row.finished_at.is_some()
        {
            return Err(Error::Denied);
        }
        let invocation = row.invocation_id.ok_or(Error::Denied)?;
        let action: types::AgentCommand = self.open_record(command, "command", &row.payload)?;
        let Some(types::agent_command::Action::Invoke(invoke)) = action.action else {
            return Err(Error::Denied);
        };
        if sqlx::query_file!("../../queries/deployment/cold_start.sql", invocation)
            .fetch_optional(&mut *tx)
            .await?
            .is_none()
        {
            return Err(Error::Conflict);
        }
        chat::activity(&mut tx, row.thread_id, "invocation.running", invocation, "").await?;
        tx.commit().await?;
        let registration =
            sqlx::query_file!("../../queries/deployment/instance.sql", agent, instance)
                .fetch_one(&self.pool)
                .await?;
        let agent_record = self.agents.get(agent).await?;
        if agent_record.paused {
            return Err(Error::Denied);
        }
        let settings = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_one(&self.pool)
            .await?;
        let secrets = self.open_secrets(
            agent,
            settings.encrypted_secrets.as_deref().ok_or(Error::Denied)?,
        )?;
        let now = chrono::Utc::now().timestamp();
        let claims = super::proxy::RuntimeClaims {
            iss: "tilde:invocation".into(),
            aud: "tilde:agent-api".into(),
            sub: agent,
            invocation_id: invocation,
            run_id: super::id(&invoke.run_id)?,
            thread_id: row.thread_id,
            capabilities: agent_record.capabilities.0,
            iat: now,
            exp: now + 900,
            assignment_generation: generation as u64,
            agent_generation: settings.generation,
        };
        let token = SecretString::from(
            jsonwebtoken::encode(
                &jsonwebtoken::Header::default(),
                &claims,
                &jsonwebtoken::EncodingKey::from_secret(
                    secrets.signing_key.expose_secret().as_bytes(),
                ),
            )
            .map_err(|_| Error::Encryption)?,
        );
        let chat = self.chat();
        let messages = chat.messages(row.thread_id, 100).await?;
        let cached = chat
            .hydrate_converted_messages(
                agent,
                row.thread_id,
                &messages.iter().map(|m| m.id.clone()).collect::<Vec<_>>(),
            )
            .await?;
        Ok(host::InvokeRequest {
            command_id: command.to_string(),
            assignment_generation: generation as u64,
            owner_instance_id: instance.to_string(),
            agent_generation: settings.generation,
            invocation_id: invocation.to_string(),
            run_id: invoke.run_id,
            thread_id: row.thread_id.to_string(),
            agent_id: agent.to_string(),
            objective: invoke.objective,
            callback_url: registration.runtime_url,
            capability: token.expose_secret().into(),
            messages,
            thread: chat.thread(row.thread_id).await?.into(),
            cached_messages: cached
                .into_iter()
                .map(|m| runtime::CachedAgentRepresentation {
                    message_id: m.message_id,
                    message_json: m.message_json,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    pub async fn acknowledge(
        &self,
        agent: Uuid,
        instance: Uuid,
        command: Uuid,
        generation: i64,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_file!(
            "../../queries/deployment/command_get.sql",
            command,
            generation,
            agent,
            instance
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::Denied)?;
        let current = sqlx::query_file!(
            "../../queries/deployment/command_owner.sql",
            row.thread_id,
            row.participant_id
        )
        .fetch_one(&mut *tx)
        .await?;
        if current.generation != generation
            || current.owner_instance_id != instance
            || current.stopped
        {
            return Err(Error::Denied);
        }
        sqlx::query_file!(
            "../../queries/deployment/ack.sql",
            command,
            generation,
            agent,
            instance
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn complete(
        &self,
        agent: Uuid,
        instance: Uuid,
        command: Uuid,
        generation: i64,
        status: &str,
        pending: &[String],
    ) -> Result<(), Error> {
        if !["stopped", "failed", "canceled"].contains(&status) {
            return Err(Error::Invalid("Invalid command outcome".into()));
        }
        let row = sqlx::query_file!(
            "../../queries/deployment/command_get.sql",
            command,
            generation,
            agent,
            instance
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::Denied)?;
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_file!(
            "../../queries/deployment/command_owner.sql",
            row.thread_id,
            row.participant_id
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query_file!(
            "../../queries/deployment/finish.sql",
            command,
            generation,
            agent,
            instance,
            if status == "failed" {
                Some("agent_execution_failed")
            } else {
                None::<&str>
            }
        )
        .execute(&mut *tx)
        .await?;
        let invocation = row.invocation_id.ok_or(Error::Denied)?;
        if row.kind == "invoke" {
            for input in pending {
                sqlx::query_file!(
                    "../../queries/chat/input_unaccept.sql",
                    invocation,
                    super::id(input)?
                )
                .execute(&mut *tx)
                .await?;
            }
        }
        let current = current.generation == generation && current.owner_instance_id == instance;
        tx.commit().await?;
        if current && row.kind == "invoke" {
            self.chat().finish(invocation, status).await?;
        }
        Ok(())
    }
    pub async fn report_reasoning(
        &self,
        agent: Uuid,
        instance: Uuid,
        command: Uuid,
        generation: i64,
        delta: &str,
    ) -> Result<(), Error> {
        if delta.len() > 65536 {
            return Err(Error::Invalid("Reasoning chunk is too large".into()));
        }
        let row = sqlx::query_file!(
            "../../queries/deployment/command_get.sql",
            command,
            generation,
            agent,
            instance
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::Denied)?;
        let mut tx = self.pool.begin().await?;
        chat::activity(
            &mut tx,
            row.thread_id,
            "reasoning.delta",
            row.invocation_id.ok_or(Error::Denied)?,
            delta,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
