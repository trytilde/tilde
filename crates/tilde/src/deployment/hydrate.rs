//! Conversation state for a replica that does not hold it in memory, with the
//! ownership decision made in the same call so no second round trip is needed.
use super::{Deployments, id};
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
        if !sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
            .fetch_one(&self.pool)
            .await?
            .allowed
        {
            return Ok(wire::HydrateResponse::default());
        }
        let chat = self.chat();
        let roster = chat.thread(thread).await?;
        let participant = roster
            .participants
            .iter()
            .find(|p| p.agent_id.as_deref() == Some(&agent.to_string()))
            .map(|p| id(&p.id))
            .transpose()?
            .ok_or(Error::NotFound)?;
        let messages = chat.messages(thread, 100).await?;
        let mut runs = vec![];
        for row in sqlx::query_file!("../../queries/deployment/agent_runs.sql", thread, agent)
            .fetch_all(&self.pool)
            .await?
        {
            runs.push(chat.run(row.id).await?);
        }
        let assignment = if r.claim
            && let Some(instance) = instance
        {
            let claim = self.claim(agent, instance, thread, participant).await?;
            Some((
                id(&claim.owner_instance_id)?,
                claim.generation as i64,
                false,
            ))
        } else {
            sqlx::query_file!("../../queries/deployment/current_owner.sql", thread, agent)
                .fetch_optional(&self.pool)
                .await?
                .map(|c| (c.owner_instance_id, c.generation, c.stopped))
        };
        let (assignment, owner_live) = match assignment {
            Some((owner, generation, stopped)) => (
                Some(crate::proto::tilde::types::v1::ParticipantAssignment {
                    thread_id: thread.to_string(),
                    participant_id: participant.to_string(),
                    agent_id: agent.to_string(),
                    owner_instance_id: owner.to_string(),
                    generation: generation as u64,
                    stopped,
                    ..Default::default()
                }),
                !stopped && self.instance_live(agent, owner).await?,
            ),
            None => (None, false),
        };
        Ok(wire::HydrateResponse {
            found: true,
            thread: roster.into(),
            messages,
            runs,
            assignment: assignment.into(),
            owner_live,
            ..Default::default()
        })
    }
}
