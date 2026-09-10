//! Small setup commands. Draft data is separate from staged credentials and OAuth internals.
use super::service::terminal;
use super::{model::*, service::Connections};
use crate::error::Error;
use uuid::Uuid;

impl Connections {
    pub(super) async fn draft(&self, setup: &Setup) -> Result<Values, Error> {
        let mut values = Values::new();
        for row in sqlx::query_file!("../../queries/connections/draft_get.sql", setup.id)
            .fetch_all(&self.pool)
            .await?
        {
            values.insert(
                row.field_key.clone(),
                self.open(
                    setup.id,
                    &format!("draft:{}", row.field_key),
                    &row.encrypted_value,
                )?,
            );
        }
        Ok(values)
    }
    pub(super) async fn write_draft(&self, setup: &Setup, values: &Values) -> Result<(), Error> {
        bounded(values)?;
        let mut tx = self.pool.begin().await?;
        let current =
            sqlx::query_file_as!(Setup, "../../queries/connections/setup_lock.sql", setup.id)
                .fetch_one(&mut *tx)
                .await?;
        if current.action_id != setup.action_id
            || current.claimed_at.is_none()
            || terminal(&current.step)
            || current.expires_at <= chrono::Utc::now()
        {
            return Err(invalid("Setup changed while saving draft"));
        }
        let mut merged = std::collections::BTreeMap::new();
        for row in sqlx::query_file!("../../queries/connections/draft_get.sql", setup.id)
            .fetch_all(&mut *tx)
            .await?
        {
            merged.insert(row.field_key, row.encrypted_value);
        }
        for (key, value) in values {
            merged.insert(
                key.clone(),
                self.seal(setup.id, &format!("draft:{key}"), value)?,
            );
        }
        if merged.len() > 100
            || merged
                .iter()
                .map(|(key, value)| key.len() + value.len())
                .sum::<usize>()
                > 256 * 1024
        {
            return Err(invalid("Setup draft exceeds 100 keys or 256 KiB"));
        }
        for (key, encrypted) in merged {
            sqlx::query_file!(
                "../../queries/connections/draft_put.sql",
                setup.id,
                key,
                encrypted
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// Merge provider-owned draft keys with optimistic concurrency; never change OAuth or lifecycle state.
    pub async fn save_draft(
        &self,
        id: Uuid,
        connection_setup_token: &str,
        action: Uuid,
        values: Values,
    ) -> Result<BrokerView, Error> {
        bounded(&values)?;
        let setup = self.authorize(id, connection_setup_token).await?;
        // The callback consumes the draft snapshot that existed when authorization began.
        if setup.step != "fields" {
            return Err(invalid("Draft can only change while waiting for input"));
        }
        self.claim(&setup, action, "fields").await?;
        if let Err(error) = self.write_draft(&setup, &values).await {
            self.fail(&setup, "draft_failed").await?;
            return Err(error);
        }
        self.transition(&setup, "fields").await?;
        self.view(id, connection_setup_token).await
    }
    /// Validate and encrypt static credentials, then atomically complete this setup.
    pub async fn save_credentials(
        &self,
        id: Uuid,
        connection_setup_token: &str,
        action: Uuid,
        values: Values,
    ) -> Result<BrokerView, Error> {
        let setup = self.authorize(id, connection_setup_token).await?;
        let connection = self.get(setup.connection_id).await?;
        if self.connection_type(&connection).await?.driver() != Driver::Static {
            return Err(invalid(
                "Use OAuth or the provider action for this connection type",
            ));
        }
        self.advance(id, connection_setup_token, action, values)
            .await
    }
    /// Start the configured OAuth flow, keeping PKCE and token exchange on the server.
    pub async fn start_oauth(
        &self,
        id: Uuid,
        connection_setup_token: &str,
        action: Uuid,
        values: Values,
    ) -> Result<BrokerView, Error> {
        let setup = self.authorize(id, connection_setup_token).await?;
        let connection = self.get(setup.connection_id).await?;
        if !matches!(
            self.connection_type(&connection).await?.driver(),
            Driver::OAuthCode | Driver::OAuthClientCredentials | Driver::OAuthJwtBearer
        ) {
            return Err(invalid("This connection type does not use generic OAuth"));
        }
        self.advance(id, connection_setup_token, action, values)
            .await
    }
    /// Invoke an installed or registered provider hook for exceptional provisioning.
    pub async fn provider_action(
        &self,
        id: Uuid,
        connection_setup_token: &str,
        action_id: Uuid,
        action: &str,
        values: Values,
    ) -> Result<BrokerView, Error> {
        bounded(&values)?;
        if action.is_empty() || action.len() > 160 || action == "callback" {
            return Err(invalid("Provider action is required"));
        }
        let setup = self.authorize(id, connection_setup_token).await?;
        let connection = self.get(setup.connection_id).await?;
        let provider = self.provider(&connection.provider_id).await?;
        match (
            self.connection_type(&connection).await?.driver(),
            &provider.kind,
        ) {
            (Driver::Custom, ProviderKind::BuiltIn) if action == "continue" => {
                self.advance(id, connection_setup_token, action_id, values)
                    .await
            }
            (Driver::Custom, ProviderKind::Remote(_)) => {
                self.claim(&setup, action_id, "fields").await?;
                if let Err(error) =
                    super::remote::execute(self, &setup, &connection, action, &values, false).await
                {
                    self.fail(&setup, "provider_action_failed").await?;
                    return Err(error);
                }
                self.view(id, connection_setup_token).await
            }
            _ => Err(invalid("Use the static credential or OAuth setup command")),
        }
    }
}
pub(super) fn bounded(values: &Values) -> Result<(), Error> {
    use secrecy::ExposeSecret;
    if values.len() > 100
        || values.iter().any(|(key, value)| {
            key.is_empty() || key.len() > 160 || value.expose_secret().len() > 65536
        })
    {
        return Err(invalid("Too many or oversized setup values"));
    }
    Ok(())
}
