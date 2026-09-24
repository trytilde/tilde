//! Persist the provider's sending identity when setup completes or a ready channel is assigned.
//! The assignment foreign key removes the identity on unassignment; labels never stand in for it.
use super::{catalog, model::*, service::Connections};
use crate::{chat::access::identity, error::Error};

impl Connections {
    pub(super) async fn sync_agent_identity(
        &self,
        tx: &crate::database::Transaction<'_>,
        connection: &Connection,
    ) -> Result<(), Error> {
        if !connection.channel_capable || connection.status != "ready" {
            return Ok(());
        }
        let mut values = Values::new();
        for row in crate::connections::db::values_get_all(tx, connection.id).await? {
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
            crate::chat::access::db::agent_identity_put_execute(
                tx,
                connection.id,
                identity::kind_name(sender.identity_type),
                &(sender.value),
            )
            .await?;
        } else {
            crate::chat::access::db::agent_identity_delete_execute(tx, connection.id).await?;
        }
        Ok(())
    }

    /// Populate identities for channels configured before this model existed, using stored
    /// provider credentials rather than editable labels. No external calls or messages are sent.
    pub async fn restore_agent_identities(&self) -> Result<(), Error> {
        for row in
            crate::chat::access::db::agent_identities_missing_all(&self.pool.get().await?).await?
        {
            self.assign(
                row.connection_id,
                &Assignment {
                    capability: Capability::Channel,
                    agent_id: row.agent_id,
                    alias: None,
                },
            )
            .await?;
        }
        Ok(())
    }
}
