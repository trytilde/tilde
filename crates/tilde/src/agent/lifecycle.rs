//! Registry lifecycle is independent of health. Commit revocation before executions observe it.
use super::*;

impl Agents {
    /// Pause dispatch and revoke active invocations even when the host is unreachable.
    /// Running executions observe revocation through their control subscriptions.
    pub async fn pause(&self, id: Uuid) -> Result<Agent, Error> {
        let mut tx = self.pool.begin().await?;
        let _row = sqlx::query_file!("../../queries/agent/lifecycle.sql", id, true)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        crate::chat::agent_lifecycle::stop_invocations(&mut tx, id, false).await?;
        tx.commit().await?;
        self.get(id).await
    }

    /// Resume queued work with a new generation; old invocations remain revoked.
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
