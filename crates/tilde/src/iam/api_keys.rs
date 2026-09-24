//! Management credentials for automation. A key holds roles exactly as a user does, joins no
//! groups and is never an administrator. Its roles are read on every request, so only the
//! key's identity is cached. Plaintext exists only in
//! the creation response. Positive authentication is cached by digest for at most 30 seconds.
//! PostgreSQL NOTIFY invalidates other listeners; local revocation clears synchronously. A
//! generation prevents an in-flight lookup from repopulating a cache invalidated during it.
use super::{
    authz::{Access, Grantee},
    db,
};
use crate::database::{Pool, notifications::Notifications};
use crate::proto::tilde::management::v1 as wire;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OnceCell, watch};
use uuid::Uuid;

const PREFIX: &str = "tilde_key_";
const TTL: Duration = Duration::from_secs(30);
const CAPACITY: usize = 4096;
#[derive(Clone)]
pub struct ApiKeys(Arc<Inner>);
struct Inner {
    pool: Pool,
    notifications: Notifications,
    cache: OnceCell<Mutex<Cache>>,
}
struct Cache {
    entries: HashMap<[u8; 32], (Uuid, Instant)>,
    generation: u64,
    changes: watch::Receiver<()>,
}
impl Cache {
    fn clear(&mut self) {
        self.entries.clear();
        self.generation = self.generation.wrapping_add(1);
        self.changes.borrow_and_update();
    }
    fn synchronize(&mut self) {
        if self.changes.has_changed().unwrap_or(true) {
            self.clear();
        }
    }
}
fn failure(error: crate::database::DbError) -> ConnectError {
    tracing::error!(?error, "Management key operation failed");
    ConnectError::internal("Management key operation failed")
}
impl ApiKeys {
    pub fn new(pool: Pool) -> Self {
        Self(Arc::new(Inner {
            pool,
            notifications: Notifications::default(),
            cache: OnceCell::new(),
        }))
    }
    async fn cache(&self) -> Result<&Mutex<Cache>, crate::database::DbError> {
        self.0
            .cache
            .get_or_try_init(|| async {
                let changes = self
                    .0
                    .notifications
                    .subscribe(&self.0.pool, "tilde_management_keys")
                    .await?;
                Ok(Mutex::new(Cache {
                    entries: HashMap::new(),
                    generation: 0,
                    changes,
                }))
            })
            .await
    }
    pub async fn authenticate(&self, token: &str) -> Result<Access, ConnectError> {
        let secret = token
            .strip_prefix(PREFIX)
            .ok_or_else(|| ConnectError::unauthenticated("Invalid API key"))?;
        if secret.len() != 43
            || !secret
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(ConnectError::unauthenticated("Invalid API key"));
        }
        let hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let cache = self.cache().await.map_err(failure)?;
        let started = Instant::now();
        let generation = {
            let mut cache = cache.lock().await;
            cache.synchronize();
            if let Some((id, at)) = cache.entries.get(&hash)
                && at.elapsed() < TTL
            {
                return Ok(Access::api_key(*id));
            }
            cache.generation
        };
        let id = db::api_key_authenticate(&self.0.pool.get().await.map_err(failure)?, &hash)
            .await
            .map_err(failure)?
            .ok_or_else(|| ConnectError::unauthenticated("Invalid or revoked API key"))?;
        let mut cache = cache.lock().await;
        cache.synchronize();
        if cache.generation == generation && started.elapsed() < TTL {
            if cache.entries.len() >= CAPACITY {
                cache.entries.retain(|_, (_, at)| at.elapsed() < TTL);
            }
            if cache.entries.len() < CAPACITY {
                cache.entries.insert(hash, (id, started));
            }
        }
        Ok(Access::api_key(id))
    }
    /// The caller has already been checked to reach every role, so a key never holds more
    /// than its creator could assign.
    pub async fn create(
        &self,
        name: &str,
        roles: &[String],
        creator: &Access,
    ) -> Result<(wire::ApiKey, SecretString), ConnectError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(ConnectError::invalid_argument(
                "Name must contain 1 to 100 characters",
            ));
        }
        let user = creator
            .user
            .ok_or_else(|| ConnectError::permission_denied("Keys are created by users"))?;
        let random = crate::connections::model::random_secret();
        let secret = SecretString::from(format!("{PREFIX}{}", random.expose_secret()));
        drop(random);
        let hash = Sha256::digest(secret.expose_secret().as_bytes()).to_vec();
        let prefix = &secret.expose_secret()[..PREFIX.len() + 8];
        let mut client = self.0.pool.get().await.map_err(failure)?;
        let tx = client.transaction().await.map_err(|e| failure(e.into()))?;
        let mut key = db::api_key_create(&tx, Uuid::new_v4(), name, prefix, &hash, user)
            .await
            .map_err(failure)?;
        let principal = Grantee::ApiKey(super::parse_id(&key.id)?);
        for role in roles {
            db::role_member_put(&tx, role, &principal, creator)
                .await
                .map_err(failure)?;
            key.roles
                .extend(db::role_get(&tx, role).await.map_err(failure)?);
        }
        tx.commit().await.map_err(|e| failure(e.into()))?;
        Ok((key, secret))
    }
    pub async fn list(
        &self,
        after: Option<Uuid>,
        size: usize,
        caller: &Access,
    ) -> Result<(Vec<wire::ApiKey>, String), ConnectError> {
        let size = size.clamp(1, 100);
        let mut rows = db::api_key_list(
            &self.0.pool.get().await.map_err(failure)?,
            after,
            (size + 1) as i64,
            caller,
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
    pub async fn revoke(&self, id: Uuid, caller: &Access) -> Result<(), ConnectError> {
        if db::api_key_revoke(&self.0.pool.get().await.map_err(failure)?, id, caller)
            .await
            .map_err(failure)?
            == 0
        {
            return Err(ConnectError::not_found("API key not found"));
        }
        if let Some(cache) = self.0.cache.get() {
            cache.lock().await.clear();
        }
        Ok(())
    }
    pub fn router(&self) -> axum::Router {
        crate::rpc::mount(
            connectrpc::Router::new().add_service(Arc::new(self.clone())),
            32 * 1024,
        )
    }
}
impl crate::services::tilde::management::v1::ApiKeysService for ApiKeys {
    async fn create_api_key<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::CreateApiKeyRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::CreateApiKeyResponse> + Send + use<'a>>
    {
        let creator = super::authz::require_user(&ctx)?;
        let body = request.to_owned_message();
        if body.role_ids.len() > 200 {
            return Err(ConnectError::invalid_argument("At most 200 roles"));
        }
        let mut roles: Vec<String> = Vec::new();
        for role in body.role_ids {
            if roles.contains(&role) {
                continue;
            }
            super::authz::authz(&ctx)?
                .require_reach(creator, &role)
                .await?;
            roles.push(role);
        }
        let (api_key, secret) = self.create(&body.name, &roles, creator).await?;
        let mut response = Response::ok(wire::CreateApiKeyResponse {
            api_key: api_key.into(),
            secret: secret.expose_secret().to_owned(),
            ..Default::default()
        })?;
        response.headers.insert(
            http::header::CACHE_CONTROL,
            http::HeaderValue::from_static("no-store"),
        );
        Ok(response)
    }
    async fn list_api_keys<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::ListApiKeysRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListApiKeysResponse> + Send + use<'a>> {
        let after = if request.page_token.is_empty() {
            None
        } else {
            Some(super::parse_id(request.page_token)?)
        };
        let (api_keys, next_page_token) = self
            .list(
                after,
                if request.page_size == 0 {
                    50
                } else {
                    request.page_size as usize
                },
                super::authz::require_user(&ctx)?,
            )
            .await?;
        Response::ok(wire::ListApiKeysResponse {
            api_keys,
            next_page_token,
            ..Default::default()
        })
    }
    async fn revoke_api_key<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::RevokeApiKeyRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::RevokeApiKeyResponse> + Send + use<'a>>
    {
        let caller = super::authz::require_user(&ctx)?;
        self.revoke(super::parse_id(request.id)?, caller).await?;
        Response::ok(wire::RevokeApiKeyResponse::default())
    }
}
