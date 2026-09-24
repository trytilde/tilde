//! The gateway process keeps every ready inference connection decrypted in memory, so a
//! forwarded call never reads Postgres. The map is rebuilt on the same notification that
//! pushes configuration to sidecars (connections, assignments) and on a slow timer as a net.
use super::{Candidate, Provider, Upstreams};
use crate::{connections::service::Connections, database::Pool, error::Error};
use std::{sync::Arc, time::Duration};

#[derive(Clone)]
pub struct Loader {
    pool: Pool,
    connections: Connections,
    pub upstreams: Arc<Upstreams>,
    notifications: Arc<crate::database::notifications::Notifications>,
}
impl Loader {
    pub fn new(pool: Pool, connections: Connections) -> Self {
        Self {
            pool,
            connections,
            upstreams: Arc::default(),
            notifications: Arc::default(),
        }
    }
    /// Decrypt every ready inference connection and swap the routing map.
    pub async fn load(&self) -> Result<(), Error> {
        let rows = super::db::upstreams_all(&self.pool.get().await?).await?;
        let aliases = super::db::aliases_all(&self.pool.get().await?).await?;
        let mut candidates = Vec::with_capacity(rows.len());
        for row in rows {
            let Some(provider) = Provider::parse(&row.provider_id) else {
                continue;
            };
            let connection = self.connections.get(row.id).await?;
            match self.connections.resolve(row.id).await {
                Ok(values) => candidates.push(Candidate {
                    slug: format!("{}/{}", row.provider_id, row.name),
                    connection: row.id,
                    version: connection.credential_version,
                    provider,
                    values,
                    agents: row.agents,
                    aliases: aliases
                        .iter()
                        .filter(|a| a.connection_id == row.id)
                        .map(|a| (a.agent_id, a.alias.clone()))
                        .collect(),
                }),
                Err(error) => {
                    tracing::warn!(connection=%row.id, %error, "Inference credentials unavailable")
                }
            }
        }
        self.upstreams.replace(candidates);
        Ok(())
    }
    pub async fn worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut changes = match self
            .notifications
            .subscribe(&self.pool, "tilde_sidecar_configuration")
            .await
        {
            Ok(receiver) => receiver,
            Err(_) => {
                tracing::warn!(
                    "Inference configuration listener unavailable; using the timer only"
                );
                tokio::sync::watch::channel(()).1
            }
        };
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = tick.tick() => {}
                changed = changes.changed() => {
                    if changed.is_err() {
                        // The listener is gone; the timer carries on.
                        changes = tokio::sync::watch::channel(()).1;
                        continue;
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { break; }
                    continue;
                }
            }
            if let Err(error) = self.load().await {
                tracing::warn!(%error, "Inference connection reload failed");
            }
        }
    }
}
