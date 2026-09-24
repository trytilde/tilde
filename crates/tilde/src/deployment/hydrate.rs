//! Conversation state from the projection for a replica that does not hold it in
//! memory. The lease is taken first, in its own transaction, and the state is read
//! after it: once this replica holds the thread nobody else writes run state for it,
//! and appends that land in between are projected the same way as any later ones.
//! Everything the executor needs to resume travels here, so a failover restores the
//! same goals, tasks, cached conversions and run origins the previous holder had.
use super::{Deployments, id, lease_frame};
use crate::{error::Error, proto::tilde::agent_event_ingress::v1 as wire};
use uuid::Uuid;
impl Deployments {
    pub async fn hydrate(
        &self,
        agent: Uuid,
        deployment: Uuid,
        instance: Option<Uuid>,
        r: wire::HydrateRequest,
    ) -> Result<wire::HydrateResponse, Error> {
        let thread = match r.key {
            Some(wire::hydrate_request::Key::ThreadId(thread)) => id(&thread)?,
            Some(wire::hydrate_request::Key::External(key)) => {
                match crate::deployment::db::external_thread_opt(
                    &self.pool.get().await?,
                    id(&key.connection_id)?,
                    &(key.external_id),
                    agent,
                )
                .await?
                {
                    Some(row) => row.thread_id,
                    None => return Ok(wire::HydrateResponse::default()),
                }
            }
            None => {
                return Err(Error::Invalid(
                    "Hydrate requires a thread or external key".into(),
                ));
            }
        };
        let exists = crate::deployment::db::project_has_thread_one(&self.pool.get().await?, thread)
            .await?
            .exists;
        if exists
            && !crate::deployment::db::thread_agent_one(&self.pool.get().await?, thread, agent)
                .await?
                .allowed
        {
            return Ok(wire::HydrateResponse::default());
        }
        let lease = match (r.lease, instance) {
            (true, Some(instance)) => {
                let mut tx_client = self.pool.get().await?;
                let tx = tx_client.transaction().await?;
                let lease = self
                    .lease_in(&tx, agent, deployment, instance, thread)
                    .await?;
                tx.commit().await?;
                drop(tx_client);
                lease
            }
            _ => match self.holder(agent, thread).await? {
                Some(h) => lease_frame(agent, thread, Some(h.instance), h.version),
                None => lease_frame(agent, thread, None, chrono::Utc::now()),
            },
        };
        if !exists {
            // A brand-new conversation: the replica creates it and now holds it.
            return Ok(wire::HydrateResponse {
                lease: lease.into(),
                ..Default::default()
            });
        }
        let chat = self.chat();
        let roster = chat.thread(thread).await?;
        let mut messages = chat.messages(thread, 100).await?;
        // A run may be projected before its first invocation frame lands; the replica
        // still needs it, so runs are read here rather than through the chat API.
        let runs = crate::deployment::db::agent_runs_all(&self.pool.get().await?, thread, agent)
            .await?
            .into_iter()
            .map(|r| crate::proto::tilde::types::v1::Run {
                id: r.id.to_string(),
                thread_id: r.thread_id.to_string(),
                agent_id: r.agent_id.to_string(),
                objective: r.objective,
                status: r.status,
                invocation_id: r.invocation_id.map(|v| v.to_string()).unwrap_or_default(),
                invocation_status: r.invocation_status.unwrap_or_default(),
                goal_id: r.goal_id.map(|v| v.to_string()),
                ..Default::default()
            })
            .collect();
        let run_origins =
            crate::deployment::db::run_origins_all(&self.pool.get().await?, thread, agent)
                .await?
                .into_iter()
                .map(|r| wire::RunOrigin {
                    run_id: r.id.to_string(),
                    source_identity_id: r
                        .source_identity_id
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    channel_origin: r.channel_origin,
                    ..Default::default()
                })
                .collect();
        let goals = crate::chat::db::goals_all(&self.pool.get().await?, thread, agent)
            .await?
            .into_iter()
            .map(|g| crate::proto::tilde::types::v1::Goal {
                id: g.id.to_string(),
                objective: g.objective,
                status: g.status,
                ..Default::default()
            })
            .collect();
        let tasks = crate::chat::db::tasks_all(&self.pool.get().await?, thread, agent)
            .await?
            .into_iter()
            .map(|t| crate::proto::tilde::types::v1::Task {
                id: t.id.to_string(),
                title: t.title,
                status: t.status,
                goal_id: t.goal_id.map(|v| v.to_string()),
                dependency_ids: t.dependencies.iter().map(|d| d.to_string()).collect(),
                blocked_reason: t.blocked_reason,
                ..Default::default()
            })
            .collect();
        let converted =
            crate::deployment::db::converted_messages_all(&self.pool.get().await?, agent, thread)
                .await?
                .into_iter()
                .map(|c| crate::proto::tilde::types::v1::ConvertedMessageState {
                    message_id: c.message_id.to_string(),
                    message_json: c.representation.to_string(),
                    ..Default::default()
                })
                .collect();
        let queued_inputs: Vec<crate::proto::tilde::types::v1::QueuedInput> =
            crate::chat::providers::tilde::db::sidecar_queue(
                &self.pool.get().await?,
                thread,
                agent,
            )
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        // A queued message can be older than the normal history window. Retain its
        // attachments and delivery identity when rebuilding the input command.
        for input in &queued_inputs {
            if !messages.iter().any(|message| message.id == input.id) {
                match chat.message(id(&input.id)?).await {
                    Ok(message) if message.thread_id == thread.to_string() => {
                        messages.push(message)
                    }
                    Ok(_) => return Err(Error::Denied),
                    Err(crate::chat::ChatError::NotFound) => {} // Free-text steering has no message.
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(wire::HydrateResponse {
            found: true,
            thread: roster.into(),
            messages,
            runs,
            lease: lease.into(),
            goals,
            tasks,
            run_origins,
            converted,
            queued_inputs,
            ..Default::default()
        })
    }
}
