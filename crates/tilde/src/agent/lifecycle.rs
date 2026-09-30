//! Registry lifecycle is independent of health. Commit revocation before executions observe it.
use super::*;

impl Agents {
    /// Pause dispatch and revoke active invocations even when the host is unreachable.
    /// Running executions observe revocation through their control subscriptions.
    pub async fn pause(&self, id: Uuid) -> Result<Agent, Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        let _row = crate::agent::db::lifecycle_opt(&tx, id, true)
            .await?
            .ok_or(Error::NotFound)?;
        crate::chat::agent_lifecycle::stop_invocations(&self.pool, &tx, id, false).await?;
        tx.commit().await?;
        drop(tx_client);
        self.get(id).await
    }

    /// Resume queued work with a new generation; old invocations remain revoked.
    pub async fn resume(&self, id: Uuid) -> Result<Agent, Error> {
        crate::agent::db::lifecycle_opt(&self.pool.get().await?, id, false)
            .await?
            .ok_or(Error::NotFound)?;
        self.get(id).await
    }

    /// Retire a paused registry entry and erase credentials, preserving conversation history.
    pub async fn delete(&self, id: Uuid) -> Result<(), Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        let Some(agent) = crate::iam::db::agent_lock_opt(&tx, id).await? else {
            return Ok(());
        };
        if !agent.paused {
            return Err(Error::AgentNotPaused);
        }
        crate::chat::agent_lifecycle::stop_invocations(&self.pool, &tx, id, true).await?;
        crate::chat::agent_lifecycle::retire(&self.pool, &tx, id).await?;
        // The system-owned Tilde connection goes with the agent; other connections only lose
        // their assignment. Asserted identities are forgotten, their chat users remain.
        let tilde = crate::connections::db::tilde_connection_opt(&tx, id).await?;
        crate::agent::db::unassign_execute(&tx, id).await?;
        if let Some(tilde) = tilde {
            crate::connections::db::tilde_identities_delete_execute(&tx, tilde.id).await?;
            crate::connections::db::tilde_delete_execute(&tx, tilde.id).await?;
        }
        crate::agent::db::delete_execute(&tx, id).await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
}
