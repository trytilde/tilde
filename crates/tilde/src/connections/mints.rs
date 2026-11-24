//! Mints: the credential a connection's consumer sends upstream, as narrow as the consumer asks.
//!
//! A connection type mints one of two ways. `Stored` sends the stored credential as it is (a
//! personal access token or API key, or an OAuth access token the connection keeps refreshed);
//! nothing narrower exists, so the scope is ignored and consumers narrow in their own checks.
//! `Custom` is provider code: GitHub App installation tokens limited to the scope's repositories
//! and permissions, AWS STS sessions in the scope's account, and Entire service account tokens
//! exchanged from the connection's secret.
//!
//! Minted credentials are cached in memory by connection, a fingerprint of its stored values
//! (any credential change misses) and the scope, until five minutes before they expire; one mint
//! per key runs at a time. The cache empties with the connection credential cache on every
//! `tilde_sidecar_configuration` notification. Cached values zeroize when they are dropped.
//! Each network mint is a `credential.mint` span (connection, scope digest, expiry, consumer).
use crate::chat::{providers::Access, tools::ToolResult};
use chrono::{DateTime, Utc};
use client_aws_sigv4::Credentials;
use connectrpc::ConnectError;
use http::HeaderName;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use tracing::Instrument;

/// How a connection type turns its stored credential into what is sent upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mint {
    Stored,
    Custom,
}

/// The connection types whose credentials provider code mints.
pub fn strategy(provider: &str, typ: &str) -> Mint {
    match (provider, typ) {
        ("github", "github_app") | ("aws", _) | ("entire", _) => Mint::Custom,
        _ => Mint::Stored,
    }
}

/// What a consumer needs: provider resources (GitHub `owner/repo`, an AWS account ID) and
/// permissions (GitHub `("contents", "write")`). Empty: everything the connection reaches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MintScope {
    pub resources: Vec<String>,
    pub permissions: Vec<(String, String)>,
}
impl MintScope {
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty() && self.permissions.is_empty()
    }
    /// A non-secret name for the scope: cache keys and audit.
    pub fn digest(&self) -> String {
        let mut hash = Sha256::new();
        for resource in &self.resources {
            hash.update(resource.as_bytes());
            hash.update([0]);
        }
        hash.update([1]);
        for (name, level) in &self.permissions {
            hash.update(name.as_bytes());
            hash.update([0]);
            hash.update(level.as_bytes());
            hash.update([0]);
        }
        hex::encode(&hash.finalize()[..8])
    }
}

#[derive(Clone)]
pub enum Credential {
    /// Request headers, e.g. `authorization: Bearer …`.
    Headers(Vec<(HeaderName, SecretString)>),
    /// SigV4 signing credentials.
    Aws(Credentials),
}
#[derive(Clone)]
pub struct Minted {
    pub credential: Credential,
    /// `None`: the stored credential, valid until the connection changes.
    pub expires_at: Option<DateTime<Utc>>,
}
impl Minted {
    pub fn bearer(token: &str, expires_at: Option<DateTime<Utc>>) -> Self {
        Self {
            credential: Credential::Headers(vec![(
                http::header::AUTHORIZATION,
                SecretString::from(format!("Bearer {token}")),
            )]),
            expires_at,
        }
    }
    pub fn headers(&self) -> &[(HeaderName, SecretString)] {
        match &self.credential {
            Credential::Headers(headers) => headers,
            Credential::Aws(_) => &[],
        }
    }
    pub fn aws(self) -> ToolResult<Credentials> {
        match self.credential {
            Credential::Aws(credentials) => Ok(credentials),
            Credential::Headers(_) => Err(ConnectError::internal("Not an AWS credential")),
        }
    }
}

/// Refresh this long before a minted credential expires.
const MARGIN: chrono::TimeDelta = chrono::TimeDelta::minutes(5);
static CACHE: LazyLock<Mutex<HashMap<String, Minted>>> = LazyLock::new(Default::default);
static FLIGHTS: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(Default::default);

/// A cached credential still comfortably valid.
pub(crate) fn cached(key: &str) -> Option<Minted> {
    let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let minted = cache.get(key)?;
    minted
        .expires_at
        .is_some_and(|at| at - MARGIN > Utc::now())
        .then(|| minted.clone())
}
pub(crate) fn remember(key: String, minted: Minted) {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let now = Utc::now();
    cache.retain(|_, m| m.expires_at.is_some_and(|at| at > now));
    cache.insert(key, minted);
}
/// Empties the cache; the connection credential cache calls this whenever it empties.
pub(crate) fn forget() {
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// A fingerprint of the connection's stored values: changes whenever a credential does.
fn fingerprint(access: &Access) -> String {
    let mut hash = Sha256::new();
    for (key, value) in &access.values {
        hash.update(key.as_bytes());
        hash.update([0]);
        hash.update(value.expose_secret().as_bytes());
        hash.update([0]);
    }
    hex::encode(&hash.finalize()[..12])
}

/// The credential `consumer` (`tool`, `chat`, `egress`) sends to the connection's provider.
pub async fn mint(
    access: &Access,
    provider: &str,
    typ: &str,
    scope: &MintScope,
    consumer: &str,
) -> ToolResult<Minted> {
    match (strategy(provider, typ), provider) {
        (Mint::Stored, _) => stored(access),
        // AWS sessions are cached per identity and role inside sts, in this same cache.
        (Mint::Custom, "aws") => {
            let account = scope.resources.first().map(String::as_str);
            let credentials =
                crate::tools::providers::aws::sts::credentials(access, typ, account).await?;
            Ok(Minted {
                credential: Credential::Aws(credentials),
                expires_at: None,
            })
        }
        // The installation token the connection keeps refreshed serves unscoped consumers.
        (Mint::Custom, "github") if scope.is_empty() => stored(access),
        (Mint::Custom, "github") => {
            networked(access, provider, scope, consumer, async {
                crate::connections::catalog::github::installation_token(access, scope).await
            })
            .await
        }
        // A service account token reaches what the service account does; no scope narrows it.
        (Mint::Custom, "entire") => {
            let scope = MintScope::default();
            networked(access, provider, &scope, consumer, async {
                crate::connections::catalog::entire::service_token(access).await
            })
            .await
        }
        (Mint::Custom, _) => Err(ConnectError::internal(format!(
            "{provider} {typ} connections mint no credentials"
        ))),
    }
}

/// A mint that calls the provider: cached per connection, credential fingerprint and scope,
/// one at a time per key, and traced as a `credential.mint` span.
async fn networked(
    access: &Access,
    provider: &str,
    scope: &MintScope,
    consumer: &str,
    mint: impl Future<Output = ToolResult<Minted>>,
) -> ToolResult<Minted> {
    let key = format!(
        "{provider}|{}|{}|{}",
        access.connection_id,
        fingerprint(access),
        scope.digest()
    );
    single_flight(key, || {
        let span = tracing::info_span!(
            "credential.mint",
            connection_id = %access.connection_id,
            scope = %scope.digest(),
            consumer,
            expires_at = tracing::field::Empty,
        );
        async {
            let minted = mint.await?;
            if let Some(at) = minted.expires_at {
                tracing::Span::current().record("expires_at", at.to_rfc3339());
            }
            Ok(minted)
        }
        .instrument(span)
    })
    .await
}

/// The stored OAuth access token, else a personal token or API key, as a bearer token.
fn stored(access: &Access) -> ToolResult<Minted> {
    ["access_token", "token", "api_key"]
        .iter()
        .find_map(|key| access.values.get(*key))
        .map(|token| Minted::bearer(token.expose_secret(), None))
        .ok_or_else(|| ConnectError::failed_precondition("The connection holds no credential"))
}

async fn single_flight<F, Fut>(key: String, mint: F) -> ToolResult<Minted>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = ToolResult<Minted>>,
{
    if let Some(minted) = cached(&key) {
        return Ok(minted);
    }
    let flight = FLIGHTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(key.clone())
        .or_default()
        .clone();
    let _turn = flight.lock().await;
    // A concurrent mint for the same key may have finished while this one waited.
    let result = match cached(&key) {
        Some(minted) => Ok(minted),
        None => mint().await.inspect(|minted| {
            if minted.expires_at.is_some() {
                remember(key.clone(), minted.clone());
            }
        }),
    };
    FLIGHTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&key);
    result
}
