//! Management identity grouping. A chat user remains the specific conversation identity.
//! Roots are optional, carry no permissions, and never change participant IDs or delivery.
//! Channel creation serializes with ingress on the connection lock. Association writes
//! serialize on the chat user; batch linking locks IDs in order and commits all or nothing.
pub mod db;
mod rpc;
use crate::{
    connections::service::Connections,
    database::GenericClient,
    iam::parse_id,
    proto::tilde::{management::v1 as wire, types::v1::IdentityType},
};
use connectrpc::ConnectError;
use uuid::Uuid;
type Result<T> = std::result::Result<T, ConnectError>;
#[derive(Clone)]
pub struct Identities {
    connections: Connections,
}
fn failure(error: crate::database::DbError) -> ConnectError {
    tracing::error!(?error, "Identity operation failed");
    ConnectError::internal("Identity operation failed")
}
impl Identities {
    pub fn new(connections: Connections) -> Self {
        Self { connections }
    }
    pub fn router(&self) -> axum::Router {
        rpc::router(self.clone())
    }
    pub async fn get(&self, id: Uuid) -> Result<wire::Identity> {
        Self::read(&self.connections.pool.get().await.map_err(failure)?, id).await
    }
    async fn read(client: &impl GenericClient, id: Uuid) -> Result<wire::Identity> {
        db::get(client, id)
            .await
            .map_err(failure)?
            .ok_or_else(|| ConnectError::not_found("Identity not found"))
    }
    pub async fn create(&self, request: wire::CreateIdentityRequest) -> Result<wire::Identity> {
        let root = request
            .root_identity_id
            .as_deref()
            .map(parse_id)
            .transpose()?;
        if request.create_root && root.is_some() {
            return Err(ConnectError::invalid_argument(
                "Choose create_root or root_identity_id, not both",
            ));
        }
        let selector = match (&request.agent_id, &request.provider_id) {
            (Some(agent), Some(slug)) => {
                let (provider, name) = slug
                    .split_once('/')
                    .filter(|(p, n)| !p.is_empty() && !n.is_empty() && !n.contains('/'))
                    .ok_or_else(|| {
                        ConnectError::invalid_argument("Provider ID must be provider/account-name")
                    })?;
                Some((parse_id(agent)?, provider.to_owned(), name.to_owned()))
            }
            (None, None) => None,
            _ => {
                return Err(ConnectError::invalid_argument(
                    "Supply agent_id and provider_id together",
                ));
            }
        };
        let kind = match request.identity_type.to_i32() {
            1 => IdentityType::Email,
            2 => IdentityType::PhoneNumber,
            3 => IdentityType::Username,
            _ => return Err(ConnectError::invalid_argument("Select an identity type")),
        };
        let mut client = self.connections.pool.get().await.map_err(failure)?;
        let tx = client
            .transaction()
            .await
            .map_err(crate::database::DbError::from)
            .map_err(failure)?;
        if let Some(root) = root {
            Self::require_root(&tx, root).await?;
        }
        let connection = if let Some((agent, provider, name)) = &selector {
            Some(
                db::provider_connection(&tx, *agent, provider, name)
                    .await
                    .map_err(failure)?
                    .ok_or_else(|| {
                        ConnectError::not_found("Ready chat provider not assigned to this agent")
                    })?,
            )
        } else {
            None
        };
        let identity = if let Some(connection) = connection {
            crate::connections::db::connection_lock_execute(&tx, &connection.to_string())
                .await
                .map_err(failure)?;
            if let Some((agent, provider, name)) = &selector
                && db::provider_connection(&tx, *agent, provider, name)
                    .await
                    .map_err(failure)?
                    != Some(connection)
            {
                return Err(ConnectError::failed_precondition(
                    "Provider assignment changed; retry the request",
                ));
            }
            let c = crate::connections::db::connection_get_opt(&tx, connection)
                .await
                .map_err(failure)?
                .ok_or_else(|| ConnectError::not_found("Connection not found"))?;
            let provider =
                crate::chat::providers::adapter(&c.provider_id, &c.type_id).ok_or_else(|| {
                    ConnectError::failed_precondition("Connection is not a supported chat channel")
                })?;
            provider.verification_recipient(kind, &request.value)?
        } else {
            if kind != IdentityType::Username {
                return Err(ConnectError::invalid_argument(
                    "Native identities use the username type for their application subject",
                ));
            }
            let identity = crate::chat::access::identity::Identity {
                identity_type: kind,
                value: request.value,
            };
            identity.validate()?;
            identity
        };
        let kind = crate::chat::access::identity::kind_name(identity.identity_type);
        let mut id = crate::chat::providers::ingress::stable(
            connection.unwrap_or(Uuid::nil()),
            if connection.is_some() {
                "user"
            } else {
                "native"
            },
            &format!("{kind}:{}", identity.value),
        );
        // Creation is idempotent but may never silently attach an already-existing identity.
        crate::chat::db::channel_user_execute(&tx, id, &identity.value)
            .await
            .map_err(failure)?;
        if let Some(connection) = connection {
            id = crate::chat::access::db::identity_upsert_one(
                &tx,
                id,
                connection,
                kind,
                &identity.value,
            )
            .await
            .map_err(failure)?
            .id;
        } else {
            db::native_create(&tx, id, &identity.value)
                .await
                .map_err(failure)?;
        }
        let existing = db::lock(&tx, id)
            .await
            .map_err(failure)?
            .ok_or_else(|| ConnectError::not_found("Identity not found"))?;
        let root = if request.create_root {
            let root = existing.root_identity_id.unwrap_or_else(Uuid::new_v4);
            db::root_create(&tx, root).await.map_err(failure)?;
            Some(root)
        } else {
            root
        };
        if let Some(root) = root {
            if existing
                .root_identity_id
                .is_some_and(|current| current != root)
            {
                return Err(ConnectError::failed_precondition(
                    "Identity already belongs to another root; unlink it first",
                ));
            }
            db::link(&tx, id, Some(root)).await.map_err(failure)?;
        }
        if request.skip_verification {
            db::attest(&tx, id).await.map_err(failure)?;
        }
        let result = Self::read(&tx, id).await?;
        tx.commit()
            .await
            .map_err(crate::database::DbError::from)
            .map_err(failure)?;
        Ok(result)
    }
    async fn require_root(client: &impl GenericClient, id: Uuid) -> Result<wire::RootIdentity> {
        db::root_get(client, id)
            .await
            .map_err(failure)?
            .ok_or_else(|| ConnectError::not_found("Root identity not found"))
    }
    async fn attach(client: &impl GenericClient, id: Uuid, root: Uuid) -> Result<()> {
        let existing = db::lock(client, id)
            .await
            .map_err(failure)?
            .ok_or_else(|| ConnectError::not_found("Identity not found"))?;
        if existing
            .root_identity_id
            .is_some_and(|current| current != root)
        {
            return Err(ConnectError::failed_precondition(
                "Identity already belongs to another root; unlink it first",
            ));
        }
        db::link(client, id, Some(root)).await.map_err(failure)
    }
    pub async fn link(&self, id: Uuid, root: Uuid, unlink: bool) -> Result<wire::Identity> {
        let mut client = self.connections.pool.get().await.map_err(failure)?;
        let tx = client
            .transaction()
            .await
            .map_err(crate::database::DbError::from)
            .map_err(failure)?;
        Self::require_root(&tx, root).await?;
        if unlink {
            let existing = db::lock(&tx, id)
                .await
                .map_err(failure)?
                .ok_or_else(|| ConnectError::not_found("Identity not found"))?;
            if existing
                .root_identity_id
                .is_some_and(|current| current != root)
            {
                return Err(ConnectError::failed_precondition(
                    "Identity belongs to a different root",
                ));
            }
            db::link(&tx, id, None).await.map_err(failure)?;
        } else {
            Self::attach(&tx, id, root).await?;
        }
        let result = Self::read(&tx, id).await?;
        tx.commit()
            .await
            .map_err(crate::database::DbError::from)
            .map_err(failure)?;
        Ok(result)
    }
    pub async fn create_root(
        &self,
        id: Uuid,
        mut identities: Vec<Uuid>,
    ) -> Result<wire::RootIdentity> {
        if identities.len() > 100 {
            return Err(ConnectError::invalid_argument(
                "Link at most 100 identities at a time",
            ));
        }
        identities.sort_unstable();
        identities.dedup();
        let mut client = self.connections.pool.get().await.map_err(failure)?;
        let tx = client
            .transaction()
            .await
            .map_err(crate::database::DbError::from)
            .map_err(failure)?;
        db::root_create(&tx, id).await.map_err(failure)?;
        for identity in identities {
            Self::attach(&tx, identity, id).await?;
        }
        let root = Self::require_root(&tx, id).await?;
        tx.commit()
            .await
            .map_err(crate::database::DbError::from)
            .map_err(failure)?;
        Ok(root)
    }
    pub async fn get_root(&self, id: Uuid) -> Result<wire::RootIdentity> {
        Self::require_root(&self.connections.pool.get().await.map_err(failure)?, id).await
    }
    pub async fn list(
        &self,
        after: Option<Uuid>,
        size: usize,
        root: Option<Uuid>,
        connection: Option<Uuid>,
    ) -> Result<(Vec<wire::Identity>, String)> {
        let size = size.clamp(1, 100);
        let mut rows = db::list(
            &self.connections.pool.get().await.map_err(failure)?,
            after,
            (size + 1) as i64,
            root,
            connection,
        )
        .await
        .map_err(failure)?;
        let more = rows.len() > size;
        rows.truncate(size);
        let next = if more {
            rows.last().unwrap().id.clone()
        } else {
            String::new()
        };
        Ok((rows, next))
    }
    pub async fn list_roots(
        &self,
        after: Option<Uuid>,
        size: usize,
    ) -> Result<(Vec<wire::RootIdentity>, String)> {
        let size = size.clamp(1, 100);
        let mut rows = db::root_list(
            &self.connections.pool.get().await.map_err(failure)?,
            after,
            (size + 1) as i64,
        )
        .await
        .map_err(failure)?;
        let more = rows.len() > size;
        rows.truncate(size);
        let next = if more {
            rows.last().unwrap().id.clone()
        } else {
            String::new()
        };
        Ok((rows, next))
    }
}
