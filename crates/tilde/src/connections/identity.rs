//! Persist the provider's sending identity when setup completes or a ready channel is assigned.
//! The assignment foreign key removes the identity on unassignment; labels never stand in for it.
use super::{catalog, model::*, service::Connections};
use crate::{chat::access::identity, error::Error};

impl Connections {
    pub(super) async fn sync_agent_identity(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        connection: &Connection,
    ) -> Result<(), Error> {
        if !connection.channel_capable || connection.status != "ready" {
            return Ok(());
        }
        let mut values = Values::new();
        for row in sqlx::query_file!("../../queries/connections/values_get.sql", connection.id)
            .fetch_all(&mut **tx)
            .await?
        {
            values.insert(
                row.field_key.clone(),
                self.open(connection.id, &row.field_key, &row.encrypted_value)?,
            );
        }
        let sender = catalog::runtime(&connection.provider_id, &connection.type_id)
            .channel_identity(&values, connection.account_label.as_deref());
        drop(values); // Decrypted credentials are SecretStrings and zeroize on drop.
        if let Some(sender) = sender {
            sender
                .validate()
                .map_err(|_| invalid("Invalid provider channel identity"))?;
            sqlx::query_file!(
                "../../queries/channel_access/agent_identity_put.sql",
                connection.id,
                identity::kind_name(sender.identity_type),
                sender.value
            )
            .execute(&mut **tx)
            .await?;
        } else {
            sqlx::query_file!(
                "../../queries/channel_access/agent_identity_delete.sql",
                connection.id
            )
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    /// Populate identities for channels configured before this model existed, using stored
    /// provider credentials rather than editable labels. No external calls or messages are sent.
    pub async fn restore_agent_identities(&self) -> Result<(), Error> {
        for row in sqlx::query_file!("../../queries/channel_access/agent_identities_missing.sql")
            .fetch_all(&self.pool)
            .await?
        {
            self.assign(
                row.connection_id,
                &Assignment {
                    capability: Capability::Channel,
                    agent_id: row.agent_id,
                },
            )
            .await?;
        }
        Ok(())
    }
}
