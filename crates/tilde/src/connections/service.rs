//! Durable setup orchestration. SQL claims fence concurrent submissions and verified callbacks.
//! Reconnect stages replacement values separately; working credentials survive a failed setup.
use super::{
    catalog::{self, Endpoints},
    model::*,
    oauth::{Http, Token},
};
use crate::database::Pool;
use crate::{
    encryption::{Encryption, SealedSecret, SecretBinding},
    error::Error,
};
use chrono::Utc;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Clone)]
pub struct Connections {
    pub(crate) pool: Pool,
    pub(super) crypto: Arc<Encryption>,
    /// Browser-facing setup pages and OAuth redirects on management.
    pub(crate) public_url: String,
    /// Provider webhook delivery only, independently exposed from management.
    pub(crate) public_event_ingress_url: String,
    pub(crate) http: Http,
    pub(crate) endpoints: Endpoints,
    /// Decrypted credentials of recently used connections. Every credential, status or
    /// assignment change notifies `tilde_sidecar_configuration`, which empties it; the TTL
    /// bounds staleness if that listener is down. `SecretString` values zeroize on eviction.
    credentials: Arc<std::sync::RwLock<std::collections::HashMap<Uuid, CachedCredentials>>>,
    notifications: Arc<crate::database::notifications::Notifications>,
}
struct CachedCredentials {
    values: Values,
    until: std::time::Instant,
}
const CREDENTIAL_TTL: std::time::Duration = std::time::Duration::from_secs(300);
impl Connections {
    pub fn new(
        pool: Pool,
        crypto: Arc<Encryption>,
        public_url: String,
        public_event_ingress_url: String,
    ) -> Result<Self, Error> {
        Self::with_endpoints(
            pool,
            crypto,
            public_url,
            public_event_ingress_url,
            Endpoints::default(),
        )
    }
    /// Explicit endpoint injection supports isolated provider fixtures without changing production config.
    pub fn with_endpoints(
        pool: Pool,
        crypto: Arc<Encryption>,
        public_url: String,
        public_event_ingress_url: String,
        endpoints: Endpoints,
    ) -> Result<Self, Error> {
        for origin in [&public_url, &public_event_ingress_url] {
            let url = url::Url::parse(origin).map_err(|_| invalid("Invalid public URL"))?;
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
        }
        Ok(Self {
            pool,
            crypto,
            public_url: public_url.trim_end_matches('/').into(),
            public_event_ingress_url: public_event_ingress_url.trim_end_matches('/').into(),
            http: Http::new()?,
            endpoints,
            credentials: Arc::default(),
            notifications: Arc::default(),
        })
    }
    fn forget_credential(&self, id: Uuid) {
        self.credentials
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
    }
    /// Drop every cached credential; the next resolve decrypts again.
    pub fn forget_credentials(&self) {
        self.credentials
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
    fn cached_credentials(&self, id: Uuid) -> Option<Values> {
        let cache = self.credentials.read().unwrap_or_else(|e| e.into_inner());
        let entry = cache.get(&id)?;
        (entry.until > std::time::Instant::now()).then(|| entry.values.clone())
    }
    fn remember_credentials(&self, connection: &Connection, values: &Values) {
        // Refreshable tokens are cached only while the refresh window is comfortably away.
        let mut until = std::time::Instant::now() + CREDENTIAL_TTL;
        if let Some(expiry) = connection.token_expires_at {
            let remaining = (expiry - Utc::now() - chrono::Duration::seconds(120))
                .to_std()
                .unwrap_or_default();
            if remaining.is_zero() {
                return;
            }
            until = until.min(std::time::Instant::now() + remaining);
        }
        self.credentials
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                connection.id,
                CachedCredentials {
                    values: values.clone(),
                    until,
                },
            );
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
        crate::connections::db::connection_get_opt(&self.pool.get().await?, id)
            .await?
            .ok_or_else(|| invalid("Connection not found"))
    }
    pub async fn list(
        &self,
        after: Option<(chrono::DateTime<Utc>, Uuid)>,
        size: u32,
        agent_id: Option<Uuid>,
        capability: Option<Capability>,
    ) -> Result<(Vec<Connection>, Option<(chrono::DateTime<Utc>, Uuid)>), Error> {
        self.list_as(
            after,
            size,
            agent_id,
            capability,
            &crate::iam::authz::Access::system(),
        )
        .await
    }
    /// Every connection; attached agents are listed only when the caller can view them.
    pub async fn list_as(
        &self,
        after: Option<(chrono::DateTime<Utc>, Uuid)>,
        size: u32,
        agent_id: Option<Uuid>,
        capability: Option<Capability>,
        caller: &crate::iam::authz::Access,
    ) -> Result<(Vec<Connection>, Option<(chrono::DateTime<Utc>, Uuid)>), Error> {
        let size = if size == 0 { 50 } else { size.min(100) };
        let mut rows = crate::connections::db::connection_list_all(
            &self.pool.get().await?,
            after.map(|p| p.0),
            after.map(|p| p.1),
            i64::from(size) + 1,
            agent_id,
            capability.map(Capability::as_str),
            caller,
        )
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
        if provider_id == catalog::tilde::PROVIDER_ID {
            return Err(invalid(
                "Tilde connections are created with their agent and managed by the system",
            ));
        }
        let provider = self.provider(provider_id).await?;
        let selected = provider
            .connection_types
            .iter()
            .find(|t| t.id == type_id)
            .ok_or_else(|| invalid("Connection type not found"))?;
        let immediate = !matches!(provider.kind, ProviderKind::BuiltIn)
            && matches!(&selected.credential_source,CredentialSource::Static{schema} if schema["properties"].as_object().is_some_and(|properties|properties.is_empty()));
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::connections::db::connection_lock_execute(&tx, &(id.to_string())).await?;
        crate::connections::db::connection_create_execute(&tx, id, name, provider_id, type_id)
            .await?;
        let row = crate::connections::db::connection_get_one(&tx, id).await?;
        if row.name != name || row.provider_id != provider_id || row.type_id != type_id {
            return Err(invalid(
                "Connection ID already exists with different parameters",
            ));
        }
        if assignments.len() > 1 {
            return Err(invalid("Only one channel assignment is supported"));
        }
        for assignment in assignments {
            Self::assign_in_transaction(&tx, &row, assignment).await?;
            self.sync_agent_identity(&tx, &row).await?;
        }
        let row = crate::connections::db::connection_get_one(&tx, id).await?;
        let existing = crate::connections::db::setup_latest_opt(&tx, id).await?;
        let fresh = existing.is_none();
        let setup = if let Some(setup) = existing {
            setup
        } else {
            self.create_setup(&tx, id).await?
        };
        let brokering_url = self.brokering_url(&setup)?;
        tx.commit().await?;
        drop(tx_client);
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
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::connections::db::connection_lock_execute(&tx, &(id.to_string())).await?;
        let mut row = crate::connections::db::connection_get_opt(&tx, id)
            .await?
            .ok_or_else(|| invalid("Connection not found"))?;
        crate::connections::db::setup_expire_execute(&tx, id).await?;
        let current = crate::connections::db::setup_latest_opt(&tx, id).await?;
        if let Some(current) = current.filter(|setup| !terminal(&setup.step)) {
            let brokering_url = self.brokering_url(&current)?;
            tx.commit().await?;
            drop(tx_client);
            return Ok(Started {
                connection: row,
                brokering_url,
            });
        }
        let setup = self.create_setup(&tx, id).await?;
        // App connections reauthorize their existing user-owned app rather than creating another.
        if row.status == "ready" {
            let step = catalog::runtime(&row.provider_id, &row.type_id).resume_step();
            if let Some(step) = step {
                crate::connections::db::setup_copy_values_execute(&tx, setup.id, id).await?;
                crate::connections::db::setup_resume_app_execute(&tx, setup.id, step).await?;
            }
        }
        crate::connections::db::connection_setup_pending_execute(&tx, id).await?;
        if row.status != "ready" {
            row.status = "requires_action".into();
        }
        let brokering_url = self.brokering_url(&setup)?;
        tx.commit().await?;
        drop(tx_client);
        Ok(Started {
            connection: row,
            brokering_url,
        })
    }
    async fn create_setup(
        &self,
        tx: &crate::database::Transaction<'_>,
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
        crate::connections::db::setup_insert_execute(
            tx,
            id,
            connection_id,
            action_id,
            &(connection_setup_token),
            &(connection_setup_token_hash),
            &(callback_token),
            &(callback_hash),
        )
        .await?;
        crate::connections::db::setup_get_one(tx, id)
            .await
            .map_err(Into::into)
    }
    pub(super) fn seal(&self, id: Uuid, key: &str, value: &SecretString) -> Result<Vec<u8>, Error> {
        seal_with(&self.crypto, id, key, value)
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
        crate::connections::db::setup_get_opt(&self.pool.get().await?, id)
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
        for row in
            crate::connections::db::setup_values_get_all(&self.pool.get().await?, setup.id).await?
        {
            values.insert(
                row.field_key.clone(),
                self.open(setup.connection_id, &row.field_key, &row.encrypted_value)?,
            );
        }
        Ok(values)
    }
    pub(super) async fn stage(&self, setup: &Setup, values: &Values) -> Result<(), Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        let current = crate::connections::db::setup_lock_one(&tx, setup.id).await?;
        if terminal(&current.step)
            || current.expires_at <= Utc::now()
            || current.action_id != setup.action_id
            || current.claimed_at.is_none()
        {
            return Err(invalid("Setup changed before credentials could be stored"));
        }
        for (key, value) in values {
            let sealed = self.seal(setup.connection_id, key, value)?;
            crate::connections::db::setup_values_put_execute(&tx, setup.id, key, &(sealed)).await?;
        }
        tx.commit().await?;
        drop(tx_client);
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
        let mut input_schema = typ.credential_source.input_schema();
        if let (Some(field), Some(schema)) = (runtime.account_name_field(&typ), &mut input_schema) {
            if let Some(properties) = schema.get_mut("properties").and_then(|v| v.as_object_mut()) {
                properties.remove(field);
            }
            if let Some(required) = schema.get_mut("required").and_then(|v| v.as_array_mut()) {
                required.retain(|value| value.as_str() != Some(field));
            }
        }
        Ok(BrokerView {
            setup_instructions: runtime
                .instructions(&typ)
                .iter()
                .map(|step| (*step).to_owned())
                .collect(),
            provider_name: provider.name.clone(),
            account_name_label: provider
                .account_name_label
                .clone()
                .unwrap_or_else(|| "Account name".into()),
            icon_url: provider.icon_url.clone(),
            instructions: provider.instructions.clone().or_else(|| {
                input_schema
                    .as_ref()?
                    .get("description")?
                    .as_str()
                    .map(str::to_owned)
            }),
            webhook_url: connection.channel_capable.then(|| {
                format!(
                    "{}/connections/webhooks/{}",
                    self.public_event_ingress_url, connection.id
                )
            }),
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
        if crate::connections::db::setup_claim_opt(&self.pool.get().await?, setup.id, action, step)
            .await?
            .is_none()
        {
            return Err(invalid("Setup action is expired, busy or already consumed"));
        }
        Ok(())
    }
    pub(super) async fn transition(&self, setup: &Setup, step: &str) -> Result<(), Error> {
        if crate::connections::db::setup_transition_opt(
            &self.pool.get().await?,
            setup.id,
            setup.action_id,
            step,
            Uuid::new_v4(),
        )
        .await?
        .is_none()
        {
            return Err(invalid("Setup was cancelled or changed while working"));
        }
        Ok(())
    }
    pub(super) async fn fail(&self, setup: &Setup, code: &str) -> Result<(), Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::connections::db::setup_fail_execute(&tx, setup.id, setup.action_id, code).await?;
        self.forget_credential(setup.connection_id);
        crate::connections::db::connection_fail_execute(&tx, setup.connection_id).await?;
        crate::connections::db::setup_values_delete_execute(&tx, setup.id).await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    /// Only user fields are accepted here. Provider callbacks have a distinct verified HTTP path.
    pub async fn advance(
        &self,
        id: Uuid,
        connection_setup_token: &str,
        action: Uuid,
        mut values: Values,
    ) -> Result<BrokerView, Error> {
        let setup = self.authorize(id, connection_setup_token).await?;
        let connection = self.get(setup.connection_id).await?;
        let typ = self.connection_type(&connection).await?;
        let runtime = catalog::runtime(&connection.provider_id, &connection.type_id);
        if setup.step == "fields"
            && let Some(field) = runtime.account_name_field(&typ)
        {
            if values.contains_key(field) {
                return Err(invalid(
                    "Account name must be supplied through SetConnectionName",
                ));
            }
            values.insert(field.into(), SecretString::from(connection.name.clone()));
        }
        // Validate against the full schema after applying the server-owned account-name binding.
        runtime.validate_input(&typ, &setup.step, &values)?;
        self.claim(&setup, action, &setup.step).await?;
        let result = self.start_method(&setup, &connection, &typ, values).await;
        if let Err(error) = result {
            // The broker only shows the generic failure; keep the provider's reason server-side.
            tracing::warn!(
                connection = %connection.id,
                provider = %connection.provider_id,
                step = %setup.step,
                ?error,
                "Provider setup failed"
            );
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
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::connections::db::connection_lock_execute(&tx, &(setup.connection_id.to_string()))
            .await?;
        let current = crate::connections::db::setup_get_one(&tx, setup.id).await?;
        if terminal(&current.step)
            || current.expires_at <= Utc::now()
            || current.action_id != setup.action_id
            || current.claimed_at.is_none()
        {
            return Err(invalid("Setup changed while acquiring credentials"));
        }
        crate::connections::db::values_delete_execute(&tx, setup.connection_id).await?;
        for row in crate::connections::db::setup_values_get_all(&tx, setup.id).await? {
            if row.field_key == "_pkce" {
                continue;
            }
            crate::connections::db::values_put_execute(
                &tx,
                setup.connection_id,
                &(row.field_key),
                &(row.encrypted_value),
            )
            .await?;
        }
        self.forget_credential(setup.connection_id);
        crate::connections::db::connection_ready_execute(
            &tx,
            setup.connection_id,
            account.as_deref(),
            expires,
        )
        .await?;
        let ready = crate::connections::db::connection_get_one(&tx, setup.connection_id).await?;
        self.sync_agent_identity(&tx, &ready).await?;
        crate::connections::db::setup_transition_opt(
            &tx,
            setup.id,
            setup.action_id,
            "complete",
            Uuid::new_v4(),
        )
        .await?
        .ok_or_else(|| invalid("Setup transition failed"))?;
        crate::connections::db::setup_values_delete_execute(&tx, setup.id).await?;
        tx.commit().await?;
        drop(tx_client);
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
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::connections::db::connection_lock_execute(&tx, &(setup.connection_id.to_string()))
            .await?;
        crate::connections::db::setup_cancel_one_execute(&tx, setup.id).await?;
        crate::connections::db::setup_values_delete_execute(&tx, setup.id).await?;
        self.forget_credential(setup.connection_id);
        crate::connections::db::connection_fail_execute(&tx, setup.connection_id).await?;
        tx.commit().await?;
        drop(tx_client);
        self.view(id, connection_setup_token).await
    }
    /// Revoke local use immediately. User-owned provider apps are not deleted by this operation.
    pub async fn disconnect(&self, id: Uuid) -> Result<(), Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::connections::db::connection_lock_execute(&tx, &(id.to_string())).await?;
        let row = crate::connections::db::connection_get_opt(&tx, id)
            .await?
            .ok_or_else(|| invalid("Connection not found"))?;
        system_managed(&row)?;
        crate::connections::db::setup_cancel_execute(&tx, id).await?;
        self.forget_credential(id);
        crate::connections::db::connection_disconnect_execute(&tx, id).await?;
        crate::connections::db::values_delete_execute(&tx, id).await?;
        crate::connections::db::setup_discard_execute(&tx, id).await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    /// Recover expired or interrupted attempts without blindly repeating external app creation.
    pub async fn recover(&self) -> Result<(), Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        let rows = crate::connections::db::setup_recover_all(&tx).await?;
        for row in rows {
            crate::connections::db::setup_values_delete_execute(&tx, row.id).await?;
            self.forget_credential(row.connection_id);
            crate::connections::db::connection_fail_execute(&tx, row.connection_id).await?;
        }
        // Purge terminal sessions too: their connection setup tokens and OAuth nonces must die.
        // Cascading foreign keys remove drafts and staged secrets without deleting ready credentials.
        crate::connections::db::setup_purge_execute(&tx).await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    pub async fn worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(15));
        let mut changes = self
            .notifications
            .subscribe(&self.pool, "tilde_sidecar_configuration")
            .await
            .unwrap_or_else(|_| tokio::sync::watch::channel(()).1);
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    if self.recover().await.is_err() {tracing::warn!("Connection setup recovery failed");}
                    let now = std::time::Instant::now();
                    self.credentials.write().unwrap_or_else(|e| e.into_inner()).retain(|_, c| c.until > now);
                }
                changed = changes.changed() => {
                    if changed.is_err() { changes = tokio::sync::watch::channel(()).1; }
                    self.forget_credentials();
                }
                changed = shutdown.changed() => {if changed.is_err() || *shutdown.borrow() {break;}},
            }
        }
    }
    /// The only runtime credential entry point. Domains share refresh and never handle OAuth themselves.
    pub async fn resolve(&self, id: Uuid) -> Result<Values, Error> {
        if let Some(values) = self.cached_credentials(id) {
            return Ok(values);
        }
        let connection = self.get(id).await?;
        if connection.status != "ready" {
            return Err(invalid("Connection is not ready"));
        }
        let mut values = Values::new();
        for row in crate::connections::db::values_get_all(&self.pool.get().await?, id).await? {
            values.insert(
                row.field_key.clone(),
                self.open(id, &row.field_key, &row.encrypted_value)?,
            );
        }
        if connection
            .token_expires_at
            .is_none_or(|expiry| expiry > Utc::now() + chrono::Duration::seconds(60))
        {
            self.remember_credentials(&connection, &values);
            return Ok(values);
        }
        if crate::connections::db::connection_refresh_claim_opt(
            &self.pool.get().await?,
            id,
            connection.credential_version,
        )
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
                    crate::connections::db::connection_reauthorize_execute(
                        &self.pool.get().await?,
                        id,
                        connection.credential_version,
                    )
                    .await?;
                }

                crate::connections::db::connection_refresh_release_execute(
                    &self.pool.get().await?,
                    id,
                    connection.credential_version,
                )
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
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        self.forget_credential(id);
        if crate::connections::db::connection_refresh_finish_opt(
            &tx,
            id,
            connection.credential_version,
            expires,
        )
        .await?
        .is_none()
        {
            return Err(invalid("Connection changed during refresh"));
        }
        for (key, value) in &values {
            let encrypted = self.seal(id, key, value)?;
            crate::connections::db::values_put_execute(&tx, id, key, &(encrypted)).await?;
        }
        tx.commit().await?;
        drop(tx_client);
        Ok(values)
    }
}
fn seal_with(
    crypto: &Encryption,
    id: Uuid,
    key: &str,
    value: &SecretString,
) -> Result<Vec<u8>, Error> {
    crypto
        .seal(binding(id, key), value)
        .map(SealedSecret::into_bytes)
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
/// Tilde connections live and die with their agent; clients cannot disconnect or unassign them.
fn system_managed(row: &Connection) -> Result<(), Error> {
    if row.provider_id == catalog::tilde::PROVIDER_ID {
        return Err(invalid("Tilde connections are managed by the system"));
    }
    Ok(())
}
impl Connections {
    /// Create a ready connection from known static values with one channel assignment, all in
    /// the caller's transaction. No setup row exists and no provider is contacted, so this takes
    /// the encryption module rather than a service instance: agent creation uses it before any
    /// `Connections` exists. The catalog runtime supplies no sending identity for such
    /// connections, so none is synced.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn create_ready(
        tx: &crate::database::Transaction<'_>,
        crypto: &Encryption,
        id: Uuid,
        name: &str,
        provider_id: &str,
        type_id: &str,
        values: &Values,
        agent: Uuid,
    ) -> Result<(), Error> {
        crate::connections::db::connection_lock_execute(tx, &(id.to_string())).await?;
        crate::connections::db::connection_create_execute(tx, id, name, provider_id, type_id)
            .await?;
        for (key, value) in values {
            let sealed = seal_with(crypto, id, key, value)?;
            crate::connections::db::values_put_execute(tx, id, key, &(sealed)).await?;
        }
        crate::connections::db::connection_ready_execute(tx, id, None, None).await?;
        let row = crate::connections::db::connection_get_one(tx, id).await?;
        Self::assign_in_transaction(
            tx,
            &row,
            &Assignment {
                capability: Capability::Channel,
                agent_id: agent,
                alias: None,
            },
        )
        .await
    }
    /// Replace one static value of a ready connection; every cached copy is invalidated.
    pub async fn replace_value(
        &self,
        id: Uuid,
        key: &str,
        value: &SecretString,
    ) -> Result<(), Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::connections::db::connection_lock_execute(&tx, &(id.to_string())).await?;
        let row = crate::connections::db::connection_get_opt(&tx, id)
            .await?
            .ok_or_else(|| invalid("Connection not found"))?;
        if row.status != "ready" {
            return Err(invalid("Connection is not ready"));
        }
        let sealed = self.seal(id, key, value)?;
        crate::connections::db::values_put_execute(&tx, id, key, &(sealed)).await?;
        crate::connections::db::connection_bump_version_execute(&tx, id).await?;
        self.forget_credential(id);
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    /// Give agents created before Tilde was a catalog provider their connection.
    pub async fn backfill_tilde(&self) -> Result<(), Error> {
        for row in crate::connections::db::tilde_missing_all(&self.pool.get().await?).await? {
            let mut tx_client = self.pool.get().await?;
            let tx = tx_client.transaction().await?;
            let Some(agent) = crate::iam::db::agent_lock_opt(&tx, row.id).await? else {
                continue;
            };
            if crate::connections::db::tilde_connection_opt(&tx, agent.id)
                .await?
                .is_none()
            {
                catalog::tilde::create(&tx, &self.crypto, agent.id, &agent.name).await?;
            }
            // Role rows are idempotent; agents from before roles existed get theirs here.
            crate::iam::authz::create_roles(
                &tx,
                crate::iam::authz::Resource::agent(agent.id),
                &crate::iam::authz::Access::system(),
            )
            .await?;
            tx.commit().await?;
            drop(tx_client);
        }
        Ok(())
    }
}

impl Connections {
    /// Assign a capability without copying credentials or enabling per-connection switches.
    /// The connection lock serializes create/assign/unassign/disconnect and guarantees one chat owner.
    pub async fn assign(&self, id: Uuid, assignment: &Assignment) -> Result<Connection, Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::connections::db::connection_lock_execute(&tx, &(id.to_string())).await?;
        let row = crate::connections::db::connection_get_opt(&tx, id)
            .await?
            .ok_or(Error::NotFound)?;
        Self::assign_in_transaction(&tx, &row, assignment).await?;
        self.sync_agent_identity(&tx, &row).await?;
        let row = crate::connections::db::connection_get_one(&tx, id).await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(row)
    }
    async fn assign_in_transaction(
        tx: &crate::database::Transaction<'_>,
        row: &Connection,
        assignment: &Assignment,
    ) -> Result<(), Error> {
        if !row.capable(assignment.capability) {
            return Err(invalid(match assignment.capability {
                Capability::Channel => "This connection type does not support chat",
                Capability::Inference => "This connection type does not support inference",
            }));
        }
        if crate::connections::db::agent_exists_opt(tx, assignment.agent_id)
            .await?
            .is_none()
        {
            return Err(invalid("Agent not found"));
        }
        // Chat has one owner per connection; an inference key may serve any number of agents.
        if assignment.capability == Capability::Channel
            && row
                .associated_agents
                .0
                .iter()
                .any(|a| a.capability == assignment.capability && a.id != assignment.agent_id)
        {
            return Err(Error::ConnectionAssignmentConflict);
        }
        match (&assignment.alias, assignment.capability) {
            (Some(alias), Capability::Inference) => validate_alias(alias)?,
            (Some(_), Capability::Channel) => {
                return Err(invalid("Only inference assignments take an alias"));
            }
            (None, _) => {}
        }
        crate::connections::db::assignment_insert_execute(
            tx,
            row.id,
            assignment.capability.as_str(),
            assignment.agent_id,
            assignment.alias.as_deref(),
        )
        .await?;
        Ok(())
    }
    /// Removing one capability relationship never deletes the connection or its credentials.
    pub async fn unassign(&self, id: Uuid, assignment: &Assignment) -> Result<Connection, Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::connections::db::connection_lock_execute(&tx, &(id.to_string())).await?;
        let row = crate::connections::db::connection_get_opt(&tx, id)
            .await?
            .ok_or_else(|| invalid("Connection not found"))?;
        system_managed(&row)?;
        crate::connections::db::assignment_delete_execute(
            &tx,
            id,
            assignment.capability.as_str(),
            assignment.agent_id,
        )
        .await?;
        let row = crate::connections::db::connection_get_one(&tx, id).await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(row)
    }
}
