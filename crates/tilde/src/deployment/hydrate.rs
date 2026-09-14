//! Conversation state from the projection for a replica that does not hold it in
//! memory. Taking the lease happens in the same transaction, so a replica that
//! starts executing has one round trip behind it, not two.
use super::{Deployments, id, lease_frame};
use crate::{error::Error, proto::tilde::agent_event_ingress::v1 as wire};
use uuid::Uuid;
impl Deployments {
    pub async fn hydrate(
        &self,
        agent: Uuid,
        instance: Option<Uuid>,
        r: wire::HydrateRequest,
    ) -> Result<wire::HydrateResponse, Error> {
        let thread = match r.key {
            Some(wire::hydrate_request::Key::ThreadId(thread)) => id(&thread)?,
            Some(wire::hydrate_request::Key::External(key)) => {
                match sqlx::query_file!(
                    "../../queries/deployment/external_thread.sql",
                    id(&key.connection_id)?,
                    key.external_id,
                    agent
                )
                .fetch_optional(&self.pool)
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
        let exists = sqlx::query_file!("../../queries/deployment/project/has_thread.sql", thread)
            .fetch_one(&self.pool)
            .await?
            .exists;
        if exists
            && !sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
                .fetch_one(&self.pool)
                .await?
                .allowed
        {
            return Ok(wire::HydrateResponse::default());
        }
        let lease = match (r.lease, instance) {
            (true, Some(instance)) => {
                let mut tx = self.pool.begin().await?;
                let lease = self.lease_in(&mut tx, agent, instance, thread).await?;
                tx.commit().await?;
                lease
            }
            _ => lease_frame(
                agent,
                thread,
                self.holder(agent, thread)
                    .await?
                    .map(|h| (h.instance, h.public_url)),
            ),
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
        let messages = chat.messages(thread, 100).await?;
        // A run may be projected before its first invocation frame lands; the replica
        // still needs it, so runs are read here rather than through the chat API.
        let runs = sqlx::query_file!("../../queries/deployment/agent_runs.sql", thread, agent)
            .fetch_all(&self.pool)
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
        Ok(wire::HydrateResponse {
            found: true,
            thread: roster.into(),
            messages,
            runs,
            lease: lease.into(),
            ..Default::default()
        })
    }
}
