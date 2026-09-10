//! Durable setup orchestration. SQL claims fence concurrent submissions and verified callbacks.
//! Reconnect stages replacement values separately; working credentials survive a failed setup.
use super::{
    catalog::{self, Endpoints},
    model::*,
    oauth::{Http, Token},
};
use crate::{
    encryption::{Encryption, SealedSecret, SecretBinding},
    error::Error,
};
use chrono::Utc;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Clone)]
pub struct Connections {
    pub(crate) pool: PgPool,
    pub(super) crypto: Arc<Encryption>,
    pub(super) public_url: String,
    pub(crate) http: Http,
    pub(crate) endpoints: Endpoints,
}
impl Connections {
    pub fn new(pool: PgPool, crypto: Arc<Encryption>, public_url: String) -> Result<Self, Error> {
        Self::with_endpoints(pool, crypto, public_url, Endpoints::default())
    }
    /// Explicit endpoint injection supports isolated provider fixtures without changing production config.
    pub fn with_endpoints(
        pool: PgPool,
        crypto: Arc<Encryption>,
        public_url: String,
        endpoints: Endpoints,
    ) -> Result<Self, Error> {
        let url = url::Url::parse(&public_url).map_err(|_| invalid("Invalid public URL"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid(
                "Public URL must be HTTP(S) without credentials or fragment",
            ));
        }
        if url.query().is_some() {
            return Err(invalid("Public URL cannot contain a query"));
        }
        Ok(Self {
            pool,
            crypto,
            public_url: public_url.trim_end_matches('/').into(),
            http: Http::new()?,
            endpoints,
        })
    }
    pub async fn seed(&self) -> Result<(), Error> {
        catalog::seed(&self.pool).await
    }
    pub async fn register_provider(
        &self,
        provider: Provider,
        backend_token: Option<SecretString>,
    ) -> Result<Provider, Error> {
        let authorization_id = uuid::Uuid::new_v4();
        let remote_authorization = match (provider.kind.remote(), backend_token) {
            (Some(_), Some(token))
                if (32..=1024).contains(&token.expose_secret().len())
                    && token.expose_secret().bytes().all(|b| b.is_ascii_graphic()) =>
            {
                Some(
                    self.crypto
                        .seal(super::remote::binding(authorization_id), &token)?
                        .into_bytes(),
                )
            }
            (None, None) => None,
            (Some(_), None)
                if !provider
                    .connection_types
                    .iter()
                    .any(|typ| typ.driver() == Driver::Custom) =>
            {
                None
            }
            _ => {
                return Err(invalid(
                    "Remote providers require a backend token of at least 32 characters",
                ));
            }
        };
        let identity = remote_authorization.as_ref().map(|_| authorization_id);
        catalog::register(&self.pool, provider, false, remote_authorization, identity).await
    }
    pub async fn providers(
        &self,
        after: &str,
        search: Option<&str>,
        size: u32,
    ) -> Result<(Vec<Provider>, String), Error> {
        catalog::list(&self.pool, after, search, size).await
    }
    pub async fn provider(&self, id: &str) -> Result<Provider, Error> {
        catalog::get(&self.pool, id).await
    }
    pub async fn get(&self, id: Uuid) -> Result<Connection, Error> {
        sqlx::query_file_as!(
            Connection,
            "../../queries/connections/connection_get.sql",
            id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| invalid("Connection not found"))
    }
    pub async fn list(
        &self,
        after: Option<(chrono::DateTime<Utc>, Uuid)>,
        size: u32,
        agent_id: Option<Uuid>,
        unassigned_channel_only: bool,
    ) -> Result<(Vec<Connection>, Option<(chrono::DateTime<Utc>, Uuid)>), Error> {
        let size = if size == 0 { 50 } else { size.min(100) };
        let mut rows = sqlx::query_file_as!(
            Connection,
            "../../queries/connections/connection_list.sql",
            after.map(|p| p.0),
            after.map(|p| p.1),
            i64::from(size) + 1,
            agent_id,
            unassigned_channel_only
        )
        .fetch_all(&self.pool)
        .await?;
        let more = rows.len() > size as usize;
        if more {
            rows.pop();
        }
        let next = if more {
            rows.last().map(|r| (r.created_at, r.id))
        } else {
            None
        };
        Ok((rows, next))
    }
    /// IDs make creation retry-safe. Initial creation never performs provider-side work.
    pub async fn start(
        &self,
        id: Uuid,
        name: &str,
        provider_id: &str,
        type_id: &str,
        assignments: &[Assignment],
    ) -> Result<Started, Error> {
        if name.trim().is_empty() || name.len() > 200 {
            return Err(invalid("Connection name is required and at most 200 bytes"));
        }
        let provider = self.provider(provider_id).await?;
        let selected = provider
            .connection_types
            .iter()
            .find(|t| t.id == type_id)
            .ok_or_else(|| invalid("Connection type not found"))?;
        let immediate = !matches!(provider.kind, ProviderKind::BuiltIn)
            && matches!(&selected.credential_source,CredentialSource::Static{schema} if schema["properties"].as_object().is_some_and(|properties|properties.is_empty()));
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            id.to_string()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_file!(
            "../../queries/connections/connection_create.sql",
            id,
            name,
            provider_id,
            type_id
        )
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query_file_as!(
            Connection,
            "../../queries/connections/connection_get.sql",
            id
        )
        .fetch_one(&mut *tx)
        .await?;
        if row.name != name || row.provider_id != provider_id || row.type_id != type_id {
            return Err(invalid(
                "Connection ID already exists with different parameters",
            ));
        }
        if assignments.len() > 1 {
            return Err(invalid("Only one channel assignment is supported"));
        }
        for assignment in assignments {
            Self::assign_in_transaction(&mut tx, &row, assignment).await?;
        }
        let row = sqlx::query_file_as!(
            Connection,
            "../../queries/connections/connection_get.sql",
            id
        )
        .fetch_one(&mut *tx)
        .await?;
        let existing =
            sqlx::query_file_as!(Setup, "../../queries/connections/setup_latest.sql", id)
                .fetch_optional(&mut *tx)
                .await?;
        let fresh = existing.is_none();
        let setup = if let Some(setup) = existing {
            setup
        } else {
            self.create_setup(&mut tx, id).await?
        };
        let brokering_url = self.brokering_url(&setup)?;
        tx.commit().await?;
        let row = if fresh && immediate {
            self.claim(&setup, setup.action_id, "fields").await?;
            self.complete(&setup, None, None).await?;
            self.get(id).await?
        } else {
            row
        };

        Ok(Started {
            connection: row,
            brokering_url,
        })
    }
    pub async fn reconnect(&self, id: Uuid) -> Result<Started, Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            id.to_string()
        )
        .execute(&mut *tx)
        .await?;
        let mut row = sqlx::query_file_as!(
            Connection,
            "../../queries/connections/connection_get.sql",
            id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| invalid("Connection not found"))?;
        sqlx::query_file!("../../queries/connections/setup_expire.sql", id)
            .execute(&mut *tx)
            .await?;
        let current = sqlx::query_file_as!(Setup, "../../queries/connections/setup_latest.sql", id)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(current) = current.filter(|setup| !terminal(&setup.step)) {
            let brokering_url = self.brokering_url(&current)?;
            tx.commit().await?;
            return Ok(Started {
                connection: row,
                brokering_url,
            });
        }
        let setup = self.create_setup(&mut tx, id).await?;
        // App connections reauthorize their existing user-owned app rather than creating another.
        if row.status == "ready" {
            let step = catalog::runtime(&row.provider_id, &row.type_id).resume_step();
            if let Some(step) = step {
                sqlx::query_file!(
                    "../../queries/connections/setup_copy_values.sql",
                    setup.id,
                    id
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/connections/setup_resume_app.sql",
                    setup.id,
                    step
                )
                .execute(&mut *tx)
                .await?;
            }
        }
        sqlx::query_file!("../../queries/connections/connection_setup_pending.sql", id)
            .execute(&mut *tx)
            .await?;
        if row.status != "ready" {
            row.status = "requires_action".into();
        }
        let brokering_url = self.brokering_url(&setup)?;
        tx.commit().await?;
        Ok(Started {
            connection: row,
            brokering_url,
        })
    }
    async fn create_setup(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        connection_id: Uuid,
    ) -> Result<Setup, Error> {
        let id = Uuid::new_v4();
        let action_id = Uuid::new_v4();
        let connection_setup_token = random_secret();
        let callback = random_secret();
        let callback_token = self.seal(id, "callback", &callback)?;
        let connection_setup_token_hash =
            Sha256::digest(connection_setup_token.expose_secret()).to_vec();
        let connection_setup_token =
            self.seal(id, "connection_setup_token", &connection_setup_token)?;
        let callback_hash = Sha256::digest(callback.expose_secret()).to_vec();
        sqlx::query_file!(
            "../../queries/connections/setup_insert.sql",
            id,
            connection_id,
            action_id,
            connection_setup_token,
            connection_setup_token_hash,
            callback_token,
            callback_hash
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query_file_as!(Setup, "../../queries/connections/setup_get.sql", id)
            .fetch_one(&mut **tx)
            .await
            .map_err(Into::into)
    }
    pub(super) fn seal(&self, id: Uuid, key: &str, value: &SecretString) -> Result<Vec<u8>, Error> {
        self.crypto
            .seal(binding(id, key), value)
            .map(SealedSecret::into_bytes)
    }
    pub(super) fn open(&self, id: Uuid, key: &str, value: &[u8]) -> Result<SecretString, Error> {
        self.crypto
            .open(binding(id, key), SealedSecret::from_bytes(value)?)
    }
    fn brokering_url(&self, setup: &Setup) -> Result<String, Error> {
        let connection_setup_token = self.open(
            setup.id,
            "connection_setup_token",
            &setup.connection_setup_token,
        )?;
        Ok(format!(
            "{}/connections/broker/{}?connection_setup_token={}",
            self.public_url,
            setup.id,
            connection_setup_token.expose_secret()
        ))
    }
    pub(super) fn callback_url(&self, _setup: &Setup) -> Result<String, Error> {
        Ok(format!("{}{}", self.public_url, super::CALLBACK_PATH))
    }
    pub(super) fn callback_state(&self, setup: &Setup) -> Result<SecretString, Error> {
        let token = self.open(setup.id, "callback", &setup.callback_token)?;
        Ok(SecretString::from(format!(
            "{}.{}",
            setup.id,
            token.expose_secret()
        )))
    }
    async fn setup(&self, id: Uuid) -> Result<Setup, Error> {
        sqlx::query_file_as!(Setup, "../../queries/connections/setup_get.sql", id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| invalid("Setup not found"))
    }
    pub(super) async fn authorize(
        &self,
        id: Uuid,
        connection_setup_token: &str,
    ) -> Result<Setup, Error> {
        let setup = self.setup(id).await?;
        if !bool::from(
            Sha256::digest(connection_setup_token)
                .as_slice()
                .ct_eq(&setup.connection_setup_token_hash),
        ) {
            return Err(invalid("Invalid connection setup token"));
        }
        if setup.expires_at <= Utc::now() {
            return Err(invalid("Setup link expired; reconnect to start again"));
        }
        Ok(setup)
    }
    pub(super) async fn connection_type(
        &self,
        connection: &Connection,
    ) -> Result<ConnectionType, Error> {
        catalog::get(&self.pool, &connection.provider_id)
            .await?
            .connection_types
            .into_iter()
            .find(|t| t.id == connection.type_id)
            .ok_or_else(|| invalid("Connection type missing"))
    }
    pub(super) async fn staged(&self, setup: &Setup) -> Result<Values, Error> {
        let mut values = Values::new();
        for row in sqlx::query_file!("../../queries/connections/setup_values_get.sql", setup.id)
            .fetch_all(&self.pool)
            .await?
        {
            values.insert(
                row.field_key.clone(),
                self.open(setup.connection_id, &row.field_key, &row.encrypted_value)?,
            );
        }
        Ok(values)
    }
    pub(super) async fn stage(&self, setup: &Setup, values: &Values) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let current =
            sqlx::query_file_as!(Setup, "../../queries/connections/setup_lock.sql", setup.id)
                .fetch_one(&mut *tx)
                .await?;
        if terminal(&current.step)
            || current.expires_at <= Utc::now()
            || current.action_id != setup.action_id
            || current.claimed_at.is_none()
        {
            return Err(invalid("Setup changed before credentials could be stored"));
        }
        for (key, value) in values {
            let sealed = self.seal(setup.connection_id, key, value)?;
            sqlx::query_file!(
                "../../queries/connections/setup_values_put.sql",
                setup.id,
                key,
                sealed
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// Reading a setup never advances it, calls a provider, or consumes its connection setup token.
    pub async fn view(&self, id: Uuid, connection_setup_token: &str) -> Result<BrokerView, Error> {
        let setup = self.authorize(id, connection_setup_token).await?;
        self.projection(setup).await
    }
    pub(super) async fn projection(&self, setup: Setup) -> Result<BrokerView, Error> {
        let connection = self.get(setup.connection_id).await?;
        let provider = catalog::get(&self.pool, &connection.provider_id).await?;
        let typ = provider
            .connection_types
            .iter()
            .find(|typ| typ.id == connection.type_id)
            .cloned()
            .ok_or_else(|| invalid("Connection type missing"))?;
        let runtime = catalog::runtime(&connection.provider_id, &connection.type_id);
        let action = if setup.claimed_at.is_some() {
            Action::Working
        } else {
            match setup.step.as_str() {
                "complete" => Action::Complete,
                "cancelled" => Action::Cancelled,
                "failed" => Action::Failed,
                "working" => Action::Working,
                "provider_redirect" => Action::Redirect {
                    url: setup
                        .provider_redirect_url
                        .clone()
                        .ok_or_else(|| invalid("Provider redirect missing"))?,
                },
                _ if typ.driver() == Driver::Custom && provider.kind.remote().is_some() => {
                    Action::Form
                }
                _ => runtime.action(self, &setup, &typ).await?,
            }
        };
        let draft = self.draft(&setup).await?;
        let auth_driver = typ.driver();
        let input_schema = typ.credential_source.input_schema();
        Ok(BrokerView {
            webhook_url: connection
                .channel_capable
                .then(|| format!("{}/connections/webhooks/{}", self.public_url, connection.id)),
            setup_id: setup.id,
            connection_id: setup.connection_id,
            connection_name: connection.name,
            type_id: typ.id.clone(),
            type_name: typ.name,
            action_id: setup.action_id,
            step: setup.step,
            error_code: setup.error_code,
            ui_path: if auth_driver == Driver::Custom {
                super::remote::ui_path(&provider, runtime.ui())
            } else {
                "/catalog/_standard/ui".into()
            },
            input_schema,
            draft,
            auth_driver,
            action,
        })
    }
    pub(super) async fn claim(&self, setup: &Setup, action: Uuid, step: &str) -> Result<(), Error> {
        if action != setup.action_id || setup.step != step {
            return Err(invalid("Stale or unexpected setup action"));
        }
        if sqlx::query_file!(
            "../../queries/connections/setup_claim.sql",
            setup.id,
            action,
            step
        )
        .fetch_optional(&self.pool)
        .await?
        .is_none()
        {
            return Err(invalid("Setup action is expired, busy or already consumed"));
        }
        Ok(())
    }
    pub(super) async fn transition(&self, setup: &Setup, step: &str) -> Result<(), Error> {
        if sqlx::query_file!(
            "../../queries/connections/setup_transition.sql",
            setup.id,
            setup.action_id,
            step,
            Uuid::new_v4()
        )
        .fetch_optional(&self.pool)
        .await?
        .is_none()
        {
            return Err(invalid("Setup was cancelled or changed while working"));
        }
        Ok(())
    }
    pub(super) async fn fail(&self, setup: &Setup, code: &str) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!(
            "../../queries/connections/setup_fail.sql",
            setup.id,
            setup.action_id,
            code
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_file!(
            "../../queries/connections/connection_fail.sql",
            setup.connection_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_file!(
            "../../queries/connections/setup_values_delete.sql",
            setup.id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
    /// Only user fields are accepted here. Provider callbacks have a distinct verified HTTP path.
    pub async fn advance(
        &self,
        id: Uuid,
        connection_setup_token: &str,
        action: Uuid,
        values: Values,
    ) -> Result<BrokerView, Error> {
        let setup = self.authorize(id, connection_setup_token).await?;
        let connection = self.get(setup.connection_id).await?;
        let typ = self.connection_type(&connection).await?;
        catalog::runtime(&connection.provider_id, &connection.type_id).validate_input(
            &typ,
            &setup.step,
            &values,
        )?;
        self.claim(&setup, action, &setup.step).await?;
        let result = self.start_method(&setup, &connection, &typ, values).await;
        if let Err(error) = result {
            self.fail(&setup, "provider_setup_failed").await?;
            return Err(error);
        }
        self.view(id, connection_setup_token).await
    }
    async fn start_method(
        &self,
        setup: &Setup,
        connection: &Connection,
        typ: &ConnectionType,
        values: Values,
    ) -> Result<(), Error> {
        catalog::runtime(&connection.provider_id, &connection.type_id)
            .start(self, setup, typ, values)
            .await
    }
    /// OAuth state is matched in constant time. Duplicate terminal callbacks only return the result URL.
    pub async fn callback(
        &self,
        id: Uuid,
        state: &str,
        parameters: &Values,
        denied: bool,
    ) -> Result<String, Error> {
        let setup = self.setup(id).await?;
        if !bool::from(Sha256::digest(state).as_slice().ct_eq(&setup.callback_hash))
            || setup.expires_at <= Utc::now()
        {
            return Err(invalid("Invalid or expired provider callback"));
        }
        if terminal(&setup.step) {
            return self.brokering_url(&setup);
        }
        let connection = self.get(setup.connection_id).await?;
        let typ = self.connection_type(&connection).await?;
        let remote_custom = typ.driver() == Driver::Custom
            && self
                .provider(&connection.provider_id)
                .await?
                .kind
                .remote()
                .is_some();
        if !(remote_custom && setup.step == "provider_redirect")
            && !catalog::runtime(&connection.provider_id, &connection.type_id).accepts_callback(
                &setup.step,
                parameters,
                denied,
            )
        {
            return Err(invalid(
                "Unexpected callback parameters for this setup step",
            ));
        }
        self.claim(&setup, setup.action_id, &setup.step).await?;
        if denied {
            self.fail(&setup, "authorization_denied").await?;
            return self.brokering_url(&setup);
        }
        let result = self.finish_callback(&setup, parameters).await;
        if result.is_err() {
            self.fail(&setup, "provider_callback_failed").await?;
        }
        self.brokering_url(&setup)
    }
    async fn finish_callback(&self, setup: &Setup, parameters: &Values) -> Result<(), Error> {
        let connection = self.get(setup.connection_id).await?;
        let typ = self.connection_type(&connection).await?;
        if typ.driver() == Driver::Custom
            && self
                .provider(&connection.provider_id)
                .await?
                .kind
                .remote()
                .is_some()
        {
            super::remote::execute(self, setup, &connection, "callback", parameters, true).await
        } else {
            catalog::runtime(&connection.provider_id, &connection.type_id)
                .callback(self, setup, &typ, parameters)
                .await
        }
    }
    pub(super) async fn complete(
        &self,
        setup: &Setup,
        token: Option<Token>,
        account: Option<String>,
    ) -> Result<(), Error> {
        let expires = token.as_ref().and_then(|t| t.expires_at);
        if let Some(token) = token {
            let mut values = token.provider_values;
            values.insert("access_token".into(), token.access);
            if let Some(refresh) = token.refresh {
                values.insert("refresh_token".into(), refresh);
            }
            if let Some(scope) = token.scope {
                values.insert("scope".into(), SecretString::from(scope));
            }
            self.stage(setup, &values).await?;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            setup.connection_id.to_string()
        )
        .execute(&mut *tx)
        .await?;
        let current =
            sqlx::query_file_as!(Setup, "../../queries/connections/setup_get.sql", setup.id)
                .fetch_one(&mut *tx)
                .await?;
        if terminal(&current.step)
            || current.expires_at <= Utc::now()
            || current.action_id != setup.action_id
            || current.claimed_at.is_none()
        {
            return Err(invalid("Setup changed while acquiring credentials"));
        }
        sqlx::query_file!(
            "../../queries/connections/values_delete.sql",
            setup.connection_id
        )
        .execute(&mut *tx)
        .await?;
        for row in sqlx::query_file!("../../queries/connections/setup_values_get.sql", setup.id)
            .fetch_all(&mut *tx)
            .await?
        {
            if row.field_key == "_pkce" {
                continue;
            }
            sqlx::query_file!(
                "../../queries/connections/values_put.sql",
                setup.connection_id,
                row.field_key,
                row.encrypted_value
            )
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query_file!(
            "../../queries/connections/connection_ready.sql",
            setup.connection_id,
            account,
            expires
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_file!(
            "../../queries/connections/setup_transition.sql",
            setup.id,
            setup.action_id,
            "complete",
            Uuid::new_v4()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| invalid("Setup transition failed"))?;
        sqlx::query_file!(
            "../../queries/connections/setup_values_delete.sql",
            setup.id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn cancel(
        &self,
        id: Uuid,
        connection_setup_token: &str,
    ) -> Result<BrokerView, Error> {
        let setup = self.authorize(id, connection_setup_token).await?;
        if terminal(&setup.step) {
            return self.projection(setup).await;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            setup.connection_id.to_string()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_file!("../../queries/connections/setup_cancel_one.sql", setup.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query_file!(
            "../../queries/connections/setup_values_delete.sql",
            setup.id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_file!(
            "../../queries/connections/connection_fail.sql",
            setup.connection_id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.view(id, connection_setup_token).await
    }
    /// Revoke local use immediately. User-owned provider apps are not deleted by this operation.
    pub async fn disconnect(&self, id: Uuid) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            id.to_string()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_file!("../../queries/connections/setup_cancel.sql", id)
            .execute(&mut *tx)
            .await?;
        sqlx::query_file!("../../queries/connections/connection_disconnect.sql", id)
            .execute(&mut *tx)
            .await?;
        sqlx::query_file!("../../queries/connections/values_delete.sql", id)
            .execute(&mut *tx)
            .await?;
        sqlx::query_file!("../../queries/connections/setup_discard.sql", id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    /// Recover expired or interrupted attempts without blindly repeating external app creation.
    pub async fn recover(&self) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query_file!("../../queries/connections/setup_recover.sql")
            .fetch_all(&mut *tx)
            .await?;
        for row in rows {
            sqlx::query_file!("../../queries/connections/setup_values_delete.sql", row.id)
                .execute(&mut *tx)
                .await?;
            sqlx::query_file!(
                "../../queries/connections/connection_fail.sql",
                row.connection_id
            )
            .execute(&mut *tx)
            .await?;
        }
        // Purge terminal sessions too: their connection setup tokens and OAuth nonces must die.
        // Cascading foreign keys remove drafts and staged secrets without deleting ready credentials.
        sqlx::query_file!("../../queries/connections/setup_purge.sql")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            tokio::select! {
                _ = tick.tick() => {if self.recover().await.is_err() {tracing::warn!("Connection setup recovery failed");}},
                changed = shutdown.changed() => {if changed.is_err() || *shutdown.borrow() {break;}},
            }
        }
    }
    /// The only runtime credential entry point. Domains share refresh and never handle OAuth themselves.
    pub async fn resolve(&self, id: Uuid) -> Result<Values, Error> {
        let connection = self.get(id).await?;
        if connection.status != "ready" {
            return Err(invalid("Connection is not ready"));
        }
        let mut values = Values::new();
        for row in sqlx::query_file!("../../queries/connections/values_get.sql", id)
            .fetch_all(&self.pool)
            .await?
        {
            values.insert(
                row.field_key.clone(),
                self.open(id, &row.field_key, &row.encrypted_value)?,
            );
        }
        if connection
            .token_expires_at
            .is_none_or(|expiry| expiry > Utc::now() + chrono::Duration::seconds(60))
        {
            return Ok(values);
        }
        if sqlx::query_file!(
            "../../queries/connections/connection_refresh_claim.sql",
            id,
            connection.credential_version
        )
        .fetch_optional(&self.pool)
        .await?
        .is_none()
        {
            return Err(invalid(
                "Credential refresh is already in progress; retry shortly",
            ));
        }
        let typ = self.connection_type(&connection).await?;
        let result = catalog::runtime(&connection.provider_id, &connection.type_id)
            .refresh(self, &typ, &values)
            .await;
        let token = match result {
            Ok(token) => token,
            Err(error) => {
                if matches!(error, Error::ConnectionAuthorizationRequired) {
                    sqlx::query_file!(
                        "../../queries/connections/connection_reauthorize.sql",
                        id,
                        connection.credential_version
                    )
                    .execute(&self.pool)
                    .await?;
                }

                sqlx::query_file!(
                    "../../queries/connections/connection_refresh_release.sql",
                    id,
                    connection.credential_version
                )
                .execute(&self.pool)
                .await?;
                return Err(error);
            }
        };
        let expires = token.expires_at;
        values.extend(token.provider_values);
        values.insert("access_token".into(), token.access);
        if let Some(refresh) = token.refresh {
            values.insert("refresh_token".into(), refresh);
        }
        if let Some(scope) = token.scope {
            values.insert("scope".into(), SecretString::from(scope));
        }
        let mut tx = self.pool.begin().await?;
        if sqlx::query_file!(
            "../../queries/connections/connection_refresh_finish.sql",
            id,
            connection.credential_version,
            expires
        )
        .fetch_optional(&mut *tx)
        .await?
        .is_none()
        {
            return Err(invalid("Connection changed during refresh"));
        }
        for (key, value) in &values {
            let encrypted = self.seal(id, key, value)?;
            sqlx::query_file!(
                "../../queries/connections/values_put.sql",
                id,
                key,
                encrypted
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(values)
    }
}
fn binding(id: Uuid, key: &str) -> SecretBinding<'_> {
    SecretBinding {
        resource_kind: "connection",
        resource_id: id,
        name: key,
    }
}
pub(super) fn terminal(step: &str) -> bool {
    matches!(step, "complete" | "failed" | "cancelled")
}

impl Connections {
    /// Assign a capability without copying credentials or enabling per-connection switches.
    /// The connection lock serializes create/assign/unassign/disconnect and guarantees one chat owner.
    pub async fn assign(&self, id: Uuid, assignment: &Assignment) -> Result<Connection, Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            id.to_string()
        )
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query_file_as!(
            Connection,
            "../../queries/connections/connection_get.sql",
            id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| invalid("Connection not found"))?;
        Self::assign_in_transaction(&mut tx, &row, assignment).await?;
        let row = sqlx::query_file_as!(
            Connection,
            "../../queries/connections/connection_get.sql",
            id
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row)
    }
    async fn assign_in_transaction(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        row: &Connection,
        assignment: &Assignment,
    ) -> Result<(), Error> {
        if !row.channel_capable {
            return Err(invalid("This connection type does not support chat"));
        }
        if sqlx::query_file!(
            "../../queries/connections/agent_exists.sql",
            assignment.agent_id
        )
        .fetch_optional(&mut **tx)
        .await?
        .is_none()
        {
            return Err(invalid("Agent not found"));
        }
        if row
            .associated_agents
            .iter()
            .any(|a| a.capability == assignment.capability && a.id != assignment.agent_id)
        {
            return Err(Error::ConnectionAssignmentConflict);
        }
        sqlx::query_file!(
            "../../queries/connections/assignment_insert.sql",
            row.id,
            "channel",
            assignment.agent_id
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
    /// Removing one capability relationship never deletes the connection or its credentials.
    pub async fn unassign(&self, id: Uuid, assignment: &Assignment) -> Result<Connection, Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            id.to_string()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_file!(
            "../../queries/connections/assignment_delete.sql",
            id,
            "channel",
            assignment.agent_id
        )
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query_file_as!(
            Connection,
            "../../queries/connections/connection_get.sql",
            id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| invalid("Connection not found"))?;
        tx.commit().await?;
        Ok(row)
    }
}
