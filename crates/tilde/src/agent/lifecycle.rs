//! Registry lifecycle is independent of health. Commit revocation before contacting the host.
use super::*;
use crate::proto::tilde::agent_host::v1 as host;

impl Agents {
    /// Pause dispatch and revoke active invocations even when the host is unreachable.
    /// The acknowledgement reports cooperative host cancellation, not process termination.
    pub async fn pause(&self, id: Uuid) -> Result<(Agent, bool), Error> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_file!("../../queries/agent/lifecycle.sql", id, true)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        crate::chat::agent_lifecycle::stop_invocations(&mut tx, id, false).await?;
        tx.commit().await?;
        let acknowledged = if let Some(endpoint) = row.endpoint_url {
            let key = self.encryption.open(
                binding(id),
                SealedSecret::from_bytes(&row.webhook_signing_key)?,
            )?;
            let request = host::StopRequest {
                agent_id: id.to_string(),
                through_generation: row.generation,
                ..Default::default()
            };
            let client =
                crate::chat::runtime::client(&endpoint, key.expose_secret(), "Stop", &request);
            drop(key);
            let result = async {
                client?
                    .stop(request)
                    .await
                    .map_err(|_| crate::chat::ChatError::Transport)
            };
            matches!(
                tokio::time::timeout(std::time::Duration::from_secs(5), result).await,
                Ok(Ok(_))
            )
        } else {
            true
        };
        Ok((self.get(id).await?, acknowledged))
    }

    /// Resume queued work with a new generation that late Stop requests cannot cancel.
    pub async fn resume(&self, id: Uuid) -> Result<Agent, Error> {
        sqlx::query_file!("../../queries/agent/lifecycle.sql", id, false)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::NotFound)?;
        self.get(id).await
    }

    /// Retire a paused registry entry and erase credentials, preserving conversation history.
    pub async fn delete(&self, id: Uuid) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let Some(agent) = sqlx::query_file_as!(Agent, "../../queries/iam/agent_lock.sql", id)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(());
        };
        if !agent.paused {
            return Err(Error::AgentNotPaused);
        }
        crate::chat::agent_lifecycle::stop_invocations(&mut tx, id, true).await?;
        crate::chat::agent_lifecycle::retire(&mut tx, id).await?;
        sqlx::query_file!("../../queries/agent/unassign.sql", id)
            .execute(&mut *tx)
            .await?;
        sqlx::query_file!("../../queries/agent/delete.sql", id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
