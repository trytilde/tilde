//! Agent registration with an encrypted, caller-supplied webhook signing key.
//!
//! Callers generate the shared key (for example, `openssl rand -hex 32`) and configure
//! the same value in the agent runtime. Keys are opaque 32–1024 printable non-whitespace
//! ASCII characters, encrypted through the encryption module and never returned by
//! registry APIs. They are separate from the installation's encryption key.
//! Registration stores an endpoint without invoking it; chat dispatches work through
//! that endpoint's ConnectRPC runtime. Updates cannot rotate the signing key.
use crate::proto::tilde::types::v1 as types;
pub mod avatar;
pub mod health;
mod lifecycle;
pub mod rpc;
use crate::iam::capabilities::Capabilities;
use crate::{
    encryption::{Encryption, SealedSecret, SecretBinding},
    error::Error,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// A concrete domain service; persistence and crypto stay inside the monolith.
#[derive(Clone)]
pub struct Agents {
    pool: PgPool,
    encryption: Arc<Encryption>,
    avatars: Option<avatar::AvatarStore>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Agent {
    pub avatar_seed: Uuid,
    pub avatar_key: Option<String>,
    pub paused: bool,
    pub capabilities: sqlx::types::Json<crate::iam::capabilities::Capabilities>,
    pub id: Uuid,
    pub name: String,
    pub endpoint_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct CreateAgent {
    pub capabilities: crate::iam::capabilities::Capabilities,
    pub id: Uuid,
    pub name: String,
    pub webhook_signing_key: SecretString,
    pub endpoint_url: String,
}
pub struct UpdateAgent {
    pub capabilities: Option<crate::iam::capabilities::Capabilities>,
    pub id: Uuid,
    pub name: Option<String>,
    pub endpoint_url: Option<String>,
}
pub struct AgentPage {
    pub agents: Vec<Agent>,
    pub next_page_token: String,
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    created_at: DateTime<Utc>,
    id: Uuid,
}

impl Agents {
    /// Bind this service to one database and its already-unlocked encryption key.
    pub fn new(pool: PgPool, encryption: Arc<Encryption>) -> Self {
        Self {
            pool,
            encryption,
            avatars: None,
        }
    }

    /// Read health and activity metrics for a bounded registry page.
    pub async fn metrics(
        &self,
        ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, types::AgentMetrics>, Error> {
        crate::agent::health::metrics(&self.pool, ids, Utc::now()).await
    }

    /// Caller-generated IDs make creation retryable without a second idempotency model.
    /// Retries must supply the original key and identical registration fields; conflicting
    /// input for an existing ID is rejected.
    pub async fn create(&self, input: CreateAgent) -> Result<Agent, Error> {
        input.capabilities.validate()?;
        let name = validate_name(&input.name)?;
        validate_webhook_signing_key(input.webhook_signing_key.expose_secret())?;
        let endpoint = validate_endpoint(input.endpoint_url)?;
        let encrypted_key = self
            .encryption
            .seal(binding(input.id), &input.webhook_signing_key)?
            .into_bytes();
        let inserted = sqlx::query_file_as!(
            Agent,
            "../../queries/agent/create.sql",
            input.id,
            name,
            endpoint,
            encrypted_key,
            serde_json::to_value(&input.capabilities)
                .map_err(|_| Error::Invalid("Invalid capabilities".into()))?
        )
        .fetch_optional(&self.pool)
        .await?;
        if let Some(agent) = inserted {
            return Ok(agent);
        }
        let current = sqlx::query_file!("../../queries/agent/get_for_create.sql", input.id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::NotFound)?;
        let stored_key = self.encryption.open(
            binding(input.id),
            SealedSecret::from_bytes(&current.webhook_signing_key)?,
        )?;
        let same_key = bool::from(
            stored_key
                .expose_secret()
                .as_bytes()
                .ct_eq(input.webhook_signing_key.expose_secret().as_bytes()),
        );
        drop(stored_key);
        if current.name != name
            || current.endpoint_url.as_deref() != Some(endpoint.as_str())
            || !same_key
            || current.capabilities.0 != input.capabilities
        {
            return Err(Error::Conflict);
        }
        Ok(Agent {
            avatar_seed: current.avatar_seed,
            avatar_key: current.avatar_key,
            paused: current.paused,
            capabilities: current.capabilities,
            id: current.id,
            name: current.name,
            endpoint_url: current.endpoint_url,
            created_at: current.created_at,
            updated_at: current.updated_at,
        })
    }

    /// Return public metadata, never the signing key or its ciphertext.
    pub async fn get(&self, id: Uuid) -> Result<Agent, Error> {
        sqlx::query_file_as!(Agent, "../../queries/agent/get.sql", id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::NotFound)
    }

    /// Bounded keyset pagination with timestamp/ID tie-breaking.
    pub async fn list(&self, page_size: u32, page_token: &str) -> Result<AgentPage, Error> {
        self.list_filtered(page_size, page_token, "").await
    }

    /// Search is applied before pagination, so matches are not limited to the current page.
    pub async fn list_filtered(
        &self,
        page_size: u32,
        page_token: &str,
        search: &str,
    ) -> Result<AgentPage, Error> {
        if search.len() > 200 {
            return Err(Error::Invalid("Agent search is too long".into()));
        }
        let size = if page_size == 0 {
            50
        } else {
            page_size.min(100)
        } as usize;
        let cursor: Option<Cursor> = if page_token.is_empty() {
            None
        } else {
            if page_token.len() > 1024 {
                return Err(Error::Invalid("Invalid page token".into()));
            }
            let bytes = URL_SAFE_NO_PAD
                .decode(page_token)
                .map_err(|_| Error::Invalid("Invalid page token".into()))?;
            Some(
                serde_json::from_slice(&bytes)
                    .map_err(|_| Error::Invalid("Invalid page token".into()))?,
            )
        };
        let mut agents = sqlx::query_file_as!(
            Agent,
            "../../queries/agent/list.sql",
            cursor.as_ref().map(|c| c.created_at),
            cursor.as_ref().map(|c| c.id),
            (size + 1) as i64,
            search.trim()
        )
        .fetch_all(&self.pool)
        .await?;
        let more = agents.len() > size;
        if more {
            agents.pop();
        }
        let token = if more {
            let last = agents.last().expect("bounded nonempty page");
            URL_SAFE_NO_PAD.encode(
                serde_json::to_vec(&Cursor {
                    created_at: last.created_at,
                    id: last.id,
                })
                .map_err(|_| Error::Invalid("Cannot encode page token".into()))?,
            )
        } else {
            String::new()
        };
        Ok(AgentPage {
            agents,
            next_page_token: token,
        })
    }

    /// Patch supplied fields. Endpoints cannot be cleared; omitted fields stay unchanged.
    pub async fn update(&self, input: UpdateAgent) -> Result<Agent, Error> {
        self.update_as(input, None).await
    }
    /// Lock authorization-bearing fields through the write so concurrent grants cannot redirect authority.
    pub async fn update_as(
        &self,
        input: UpdateAgent,
        caller: Option<&Capabilities>,
    ) -> Result<Agent, Error> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_file_as!(Agent, "../../queries/iam/agent_lock.sql", input.id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        if let Some(caller) = caller {
            use crate::iam::capabilities::Capability;
            if !caller.permits(Capability::AgentsUpdate, &input.id.to_string()) {
                return Err(Error::Denied);
            }
            if let Some(caps) = &input.capabilities
                && (!caller.permits(Capability::AgentsGrant, &input.id.to_string())
                    || !caps.subset_of(caller))
            {
                return Err(Error::Denied);
            }
            if input.endpoint_url.is_some() && !current.capabilities.subset_of(caller) {
                return Err(Error::Denied);
            }
        }
        if let Some(caps) = &input.capabilities {
            caps.validate()?;
        }
        let name = input.name.as_deref().map(validate_name).transpose()?;
        let change_endpoint = input.endpoint_url.is_some();
        let endpoint = input.endpoint_url.map(validate_endpoint).transpose()?;
        let updated = sqlx::query_file_as!(
            Agent,
            "../../queries/agent/update.sql",
            input.id,
            name,
            change_endpoint,
            endpoint,
            input
                .capabilities
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|_| Error::Invalid("Invalid capabilities".into()))?
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound)?;
        tx.commit().await?;
        Ok(updated)
    }
}

pub(crate) fn validate_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(Error::Invalid(
            "Agent name must contain 1-200 characters".into(),
        ));
    }
    Ok(name.into())
}
pub(crate) fn validate_endpoint(value: String) -> Result<String, Error> {
    if value.trim().is_empty() {
        return Err(Error::Invalid("An agent endpoint is required".into()));
    }
    let url = url::Url::parse(&value).map_err(|_| Error::Invalid("Invalid endpoint URL".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Invalid(
            "Endpoint must be HTTP(S); credentials must not appear in the URL".into(),
        ));
    }
    Ok(url.to_string())
}
pub(crate) fn validate_webhook_signing_key(value: &str) -> Result<(), Error> {
    if !(32..=1024).contains(&value.len()) || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(Error::Invalid(
            "Webhook signing key is required: supply 32-1024 printable non-whitespace ASCII characters generated securely by the caller".into()
        ));
    }
    Ok(())
}

fn binding(id: Uuid) -> SecretBinding<'static> {
    SecretBinding {
        resource_kind: "agent",
        resource_id: id,
        name: "webhook_signing_key",
    }
}
