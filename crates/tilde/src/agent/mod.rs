//! Agent registration. An agent is policy (capabilities, concurrency, connections); how it
//! runs belongs to its deployments. Tilde never calls an agent: hosts dial in with a
//! deployment token, so a registration carries no endpoint and no wake-signing key.
pub mod db;
use crate::database::Pool;
use crate::proto::tilde::types::v1 as types;
pub mod avatar;
pub mod health;
mod lifecycle;
pub mod rpc;
use crate::encryption::Encryption;
use crate::error::Error;
use crate::iam::capabilities::Capabilities;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// A concrete domain service; persistence and crypto stay inside the monolith.
#[derive(Clone)]
pub struct Agents {
    pool: Pool,
    /// Seals the application key of the Tilde connection every agent is created with.
    encryption: Arc<Encryption>,
    avatars: Option<avatar::AvatarStore>,
}

#[derive(Debug, Clone)]
pub struct Agent {
    pub concurrency_policy: ConcurrencyPolicy,
    pub avatar_seed: Uuid,
    pub avatar_key: Option<String>,
    pub paused: bool,
    pub capabilities: tokio_postgres::types::Json<crate::iam::capabilities::Capabilities>,
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct CreateAgent {
    pub concurrency_policy: ConcurrencyPolicy,
    pub capabilities: crate::iam::capabilities::Capabilities,
    pub id: Uuid,
    pub name: String,
}
pub struct UpdateAgent {
    pub concurrency_policy: Option<ConcurrencyPolicy>,
    pub capabilities: Option<crate::iam::capabilities::Capabilities>,
    pub id: Uuid,
    pub name: Option<String>,
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
    pub fn new(pool: Pool, encryption: Arc<Encryption>) -> Self {
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
    /// Retries must supply identical registration fields; conflicting
    /// input for an existing ID is rejected.
    pub async fn create(&self, input: CreateAgent) -> Result<Agent, Error> {
        self.create_as(input, None).await
    }
    /// The agent row, its system roles with the creator as owner, and the agent's Tilde chat
    /// connection commit together: a failure leaves no ownerless agent and no agent without
    /// its built-in channel.
    pub async fn create_as(
        &self,
        input: CreateAgent,
        creator: Option<&crate::iam::authz::Access>,
    ) -> Result<Agent, Error> {
        input.capabilities.validate()?;
        let name = validate_name(&input.name)?;
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        let inserted = crate::agent::db::create_opt(
            &tx,
            input.id,
            &(name),
            input.concurrency_policy.as_str(),
            &(serde_json::to_value(&input.capabilities)
                .map_err(|_| Error::Invalid("Invalid capabilities".into()))?),
        )
        .await?;
        if let Some(agent) = inserted {
            // Engine-created agents (dev registration, sidecars) get their roles with no owner.
            crate::iam::authz::create_roles(
                &tx,
                crate::iam::authz::Resource::agent(agent.id),
                creator.unwrap_or(&crate::iam::authz::Access::system()),
            )
            .await?;
            crate::connections::catalog::tilde::create(
                &tx,
                &self.encryption,
                agent.id,
                &agent.name,
            )
            .await?;
            tx.commit().await?;
            drop(tx_client);
            return Ok(agent);
        }
        drop(tx);
        drop(tx_client);
        let current = crate::agent::db::get_for_create_opt(&self.pool.get().await?, input.id)
            .await?
            .ok_or(Error::NotFound)?;
        if current.name != name
            || current.capabilities.0 != input.capabilities
            || current.concurrency_policy != input.concurrency_policy.as_str()
        {
            return Err(Error::Conflict);
        }
        Ok(Agent {
            concurrency_policy: current.concurrency_policy.parse()?,
            avatar_seed: current.avatar_seed,
            avatar_key: current.avatar_key,
            paused: current.paused,
            capabilities: current.capabilities,
            id: current.id,
            name: current.name,
            created_at: current.created_at,
            updated_at: current.updated_at,
        })
    }

    /// Return public registration metadata.
    pub async fn get(&self, id: Uuid) -> Result<Agent, Error> {
        crate::agent::db::get_opt(&self.pool.get().await?, id)
            .await?
            .ok_or(Error::NotFound)
    }

    /// Bounded keyset pagination with timestamp/ID tie-breaking.
    pub async fn list(&self, page_size: u32, page_token: &str) -> Result<AgentPage, Error> {
        self.list_filtered(
            page_size,
            page_token,
            "",
            &crate::iam::authz::Access::system(),
        )
        .await
    }

    /// Search is applied before pagination, so matches are not limited to the current page.
    pub async fn list_filtered(
        &self,
        page_size: u32,
        page_token: &str,
        search: &str,
        caller: &crate::iam::authz::Access,
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
        let mut agents = crate::agent::db::list_all(
            &self.pool.get().await?,
            cursor.as_ref().map(|c| c.created_at),
            cursor.as_ref().map(|c| c.id),
            (size + 1) as i64,
            search.trim(),
            caller,
        )
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
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::iam::db::agent_lock_opt(&tx, input.id)
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
        }
        if let Some(caps) = &input.capabilities {
            caps.validate()?;
        }
        let name = input.name.as_deref().map(validate_name).transpose()?;
        let updated = crate::agent::db::update_opt(
            &tx,
            input.id,
            name.as_deref(),
            input.concurrency_policy.map(ConcurrencyPolicy::as_str),
            input
                .capabilities
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|_| Error::Invalid("Invalid capabilities".into()))?
                .as_ref(),
        )
        .await?
        .ok_or(Error::NotFound)?;
        tx.commit().await?;
        drop(tx_client);
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
        return Err(Error::Invalid("A URL is required".into()));
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

/// Per-thread input scheduling. Queue is the default, matching the hosted harness.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyPolicy {
    #[default]
    Queue,
    Interrupt,
    QueueAndBatch,
}
impl ConcurrencyPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::Interrupt => "interrupt",
            Self::QueueAndBatch => "queue_and_batch",
        }
    }
    pub fn from_wire(value: i32) -> Result<Self, Error> {
        match value {
            0 | 1 => Ok(Self::Queue),
            2 => Ok(Self::Interrupt),
            3 => Ok(Self::QueueAndBatch),
            _ => Err(Error::Invalid("Invalid concurrency policy".into())),
        }
    }
    pub fn wire(self) -> crate::proto::tilde::types::v1::AgentConcurrencyPolicy {
        use crate::proto::tilde::types::v1::AgentConcurrencyPolicy as P;
        match self {
            Self::Queue => P::Queue,
            Self::Interrupt => P::Interrupt,
            Self::QueueAndBatch => P::QueueAndBatch,
        }
    }
}
impl std::str::FromStr for ConcurrencyPolicy {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self, Error> {
        match value {
            "queue" => Ok(Self::Queue),
            "interrupt" => Ok(Self::Interrupt),
            "queue_and_batch" => Ok(Self::QueueAndBatch),
            _ => Err(Error::Invalid("Invalid concurrency policy".into())),
        }
    }
}
