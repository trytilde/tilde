//! OIDC relying party, not an identity provider. Login creates a revocable local bearer session.
//! Login also mirrors the provider's group claim into `external:` groups. Authorization reads
//! memberships from the database on every request, never from the token, so a session sees
//! local membership changes at once and provider changes at its next login.
use super::authz::Access;
use crate::database::Pool;
use crate::encryption::{Encryption, SealedSecret, SecretBinding};
use crate::error::Error;
use axum::{
    Router,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use http::{HeaderMap, StatusCode, header};
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;
type Result<T> = std::result::Result<T, AuthError>;
#[derive(Debug)]
pub struct AuthError;
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        connectrpc::ConnectError::unauthenticated("Authentication failed").into_response()
    }
}
impl From<crate::database::DbError> for AuthError {
    fn from(_: crate::database::DbError) -> Self {
        Self
    }
}
impl From<reqwest::Error> for AuthError {
    fn from(_: reqwest::Error) -> Self {
        Self
    }
}
#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}
#[derive(Clone)]
pub struct Oidc {
    inner: Arc<Inner>,
}
struct Inner {
    pool: Pool,
    api_keys: super::api_keys::ApiKeys,
    encryption: Arc<Encryption>,
    issuer: String,
    client_id: String,
    client_secret: SecretString,
    public_url: String,
    insecure: bool,
    http: reqwest::Client,
    groups: GroupMapping,
}
/// How provider claims become group memberships.
#[derive(Clone)]
pub struct GroupMapping {
    /// ID token claim holding group names: an array of strings or one string.
    pub claim: String,
    /// Scopes requested at login; providers commonly need `groups` here.
    pub scopes: String,
    /// Provider groups whose members administer the installation. When set, administrator
    /// membership follows the provider at every login; when empty it is managed locally.
    pub admin_groups: Vec<String>,
    /// Subjects that are always administrators, for bootstrap and break-glass.
    pub admin_subjects: Vec<String>,
}
impl Default for GroupMapping {
    fn default() -> Self {
        Self {
            claim: "groups".into(),
            scopes: "openid profile email".into(),
            admin_groups: Vec::new(),
            admin_subjects: Vec::new(),
        }
    }
}
/// Provider group names become stable ids: lowercase kebab-case behind the `external:` prefix,
/// so no provider value can name a system or local group.
pub fn external_group_id(name: &str) -> Option<String> {
    let mut slug = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    (!slug.is_empty() && slug.len() <= 150)
        .then(|| format!("{}{slug}", super::authz::EXTERNAL_PREFIX))
}
impl Oidc {
    pub fn new(
        pool: Pool,
        encryption: Arc<Encryption>,
        issuer: String,
        client_id: String,
        client_secret: SecretString,
        public_url: String,
        insecure: bool,
    ) -> std::result::Result<Self, Error> {
        for value in [&issuer, &public_url] {
            validate_url(value, insecure).map_err(|_| {
                Error::Invalid(
                    "IAM URLs require HTTPS (or explicitly enabled development HTTP)".into(),
                )
            })?;
        }
        if client_id.is_empty() || client_secret.expose_secret().is_empty() {
            return Err(Error::Invalid(
                "OIDC client ID and secret are required".into(),
            ));
        }
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Invalid("Cannot create OIDC client".into()))?;
        Ok(Self {
            inner: Arc::new(Inner {
                api_keys: super::api_keys::ApiKeys::new(pool.clone()),
                pool,
                encryption,
                issuer,
                client_id,
                client_secret,
                public_url: public_url.trim_end_matches('/').into(),
                insecure,
                http,
                groups: GroupMapping::default(),
            }),
        })
    }
    /// Call before the instance is cloned into routers.
    pub fn with_group_mapping(mut self, groups: GroupMapping) -> Self {
        Arc::get_mut(&mut self.inner)
            .expect("group mapping is configured before sharing")
            .groups = groups;
        self
    }
    async fn discovery(&self) -> Result<Discovery> {
        let d: Discovery = self
            .inner
            .http
            .get(format!(
                "{}/.well-known/openid-configuration",
                self.inner.issuer.trim_end_matches('/')
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if d.issuer != self.inner.issuer {
            return Err(AuthError);
        }
        for endpoint in [&d.authorization_endpoint, &d.token_endpoint, &d.jwks_uri] {
            validate_url(endpoint, self.inner.insecure)?;
        }
        Ok(d)
    }
    pub fn api_keys(&self) -> super::api_keys::ApiKeys {
        self.inner.api_keys.clone()
    }
    pub fn router(&self) -> Router {
        Router::new()
            .route("/auth/login", post(login))
            .route("/auth/exchange", post(callback))
            .route("/auth/session", get(session))
            .route("/auth/logout", post(logout))
            .with_state(self.clone())
    }
    async fn user(&self, headers: &HeaderMap) -> Result<Access> {
        let token = bearer(headers).ok_or(AuthError)?;
        let hash = digest(token);
        let row = crate::iam::db::session_get_opt(&self.inner.pool.get().await?, &(hash))
            .await?
            .ok_or(AuthError)?;
        Ok(Access::user(row.user_id, row.groups))
    }
    /// Mirror the login's claims: provider groups always, administrators when mapped.
    async fn sync_groups(&self, user: Uuid, subject: &str, claimed: &[String]) -> Result<()> {
        let mapping = &self.inner.groups;
        let mut ids = Vec::new();
        let mut names = Vec::new();
        for name in claimed.iter().take(500) {
            if let Some(id) = external_group_id(name)
                && !ids.contains(&id)
            {
                ids.push(id);
                names.push(name.trim().chars().take(100).collect::<String>());
            }
        }
        let mut client = self.inner.pool.get().await?;
        let tx = client.transaction().await.map_err(|_| AuthError)?;
        crate::iam::db::external_groups_sync(&tx, user, &ids, &names).await?;
        let listed = mapping.admin_subjects.iter().any(|s| s == subject);
        let mapped = mapping
            .admin_groups
            .iter()
            .filter_map(|name| external_group_id(name))
            .any(|id| ids.contains(&id));
        if listed || mapped {
            crate::iam::db::group_member_add(&tx, super::authz::ADMIN_GROUP, user).await?;
        } else if !mapping.admin_groups.is_empty() {
            crate::iam::db::group_member_remove(&tx, super::authz::ADMIN_GROUP, user).await?;
        }
        tx.commit().await.map_err(|_| AuthError)?;
        Ok(())
    }
}
pub async fn management_guard(
    State(oidc): State<Oidc>,
    mut request: Request,
    next: Next,
) -> Response {
    let authenticated = match bearer(request.headers()) {
        Some(token) if token.starts_with("tilde_key_") => oidc
            .inner
            .api_keys
            .authenticate(token)
            .await
            .map_err(|_| AuthError),
        _ => oidc.user(request.headers()).await,
    };
    let access = match authenticated {
        Ok(access) => access,
        Err(e) => return e.into_response(),
    };
    // Authentication only; each handler applies its own rule (`authz::require*`), and a test
    // drives every declared RPC through this guard so none ships without one.
    request.extensions_mut().insert(access);
    request
        .extensions_mut()
        .insert(super::authz::Authz::new(oidc.inner.pool.clone()));
    next.run(request).await
}
fn validate_url(value: &str, insecure: bool) -> Result<()> {
    let url = url::Url::parse(value).map_err(|_| AuthError)?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !(url.scheme() == "https" || (insecure && url.scheme() == "http"))
    {
        return Err(AuthError);
    }
    Ok(())
}
fn random() -> SecretString {
    let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
    rand::rngs::OsRng.fill_bytes(&mut *bytes);
    SecretString::from(URL_SAFE_NO_PAD.encode(bytes.as_slice()))
}
fn digest(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}
fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn binding(id: Uuid) -> SecretBinding<'static> {
    SecretBinding {
        resource_kind: "iam_oidc_state",
        resource_id: id,
        name: "pkce_verifier",
    }
}
#[derive(Deserialize)]
struct Login {
    challenge: String,
}
async fn login(State(oidc): State<Oidc>, axum::Json(input): axum::Json<Login>) -> Result<Response> {
    let browser_hash = URL_SAFE_NO_PAD
        .decode(&input.challenge)
        .map_err(|_| AuthError)?;
    if browser_hash.len() != 32 {
        return Err(AuthError);
    }
    let d = oidc.discovery().await?;
    crate::iam::db::cleanup_execute(&oidc.inner.pool.get().await?).await?;
    let state = random();
    let nonce = random();
    let verifier = random();
    let id = Uuid::new_v4();
    let sealed = oidc
        .inner
        .encryption
        .seal(binding(id), &verifier)
        .map_err(|_| AuthError)?
        .into_bytes();
    let hash = digest(state.expose_secret());
    let challenge = URL_SAFE_NO_PAD.encode(digest(verifier.expose_secret()));
    drop(verifier);
    crate::iam::db::state_create_execute(
        &oidc.inner.pool.get().await?,
        id,
        &(hash),
        nonce.expose_secret(),
        &(sealed),
        &(browser_hash),
    )
    .await?;
    let mut url = url::Url::parse(&d.authorization_endpoint).map_err(|_| AuthError)?;
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", &oidc.inner.client_id),
        (
            "redirect_uri",
            &format!("{}/auth/callback", oidc.inner.public_url),
        ),
        ("scope", &oidc.inner.groups.scopes),
        ("state", state.expose_secret()),
        ("nonce", nonce.expose_secret()),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ]);
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(
            serde_json::json!({"authorization_url":url.as_str(),"state":state.expose_secret()}),
        ),
    )
        .into_response())
}
#[derive(Deserialize)]
struct Callback {
    code: String,
    state: String,
    verifier: String,
}
#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
}
#[derive(Deserialize)]
struct IdClaims {
    sub: String,
    nonce: String,
    azp: Option<String>,
    aud: serde_json::Value,
    email: Option<String>,
    name: Option<String>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}
impl IdClaims {
    fn groups(&self, claim: &str) -> Vec<String> {
        match self.other.get(claim) {
            Some(serde_json::Value::Array(values)) => values
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            Some(serde_json::Value::String(value)) => vec![value.clone()],
            _ => Vec::new(),
        }
    }
}
async fn callback(
    State(oidc): State<Oidc>,
    axum::Json(params): axum::Json<Callback>,
) -> Result<Response> {
    if params.state.len() > 256 || !(43..=128).contains(&params.verifier.len()) {
        return Err(AuthError);
    }
    let browser_hash = digest(&params.verifier);
    let hash = digest(&params.state);
    let state =
        crate::iam::db::state_consume_opt(&oidc.inner.pool.get().await?, &(hash), &(browser_hash))
            .await?
            .ok_or(AuthError)?;
    let d = oidc.discovery().await?;
    let verifier = oidc
        .inner
        .encryption
        .open(
            binding(state.id),
            SealedSecret::from_bytes(&state.verifier).map_err(|_| AuthError)?,
        )
        .map_err(|_| AuthError)?;
    let form = [
        ("grant_type", "authorization_code"),
        ("code", &params.code),
        (
            "redirect_uri",
            &format!("{}/auth/callback", oidc.inner.public_url),
        ),
        ("code_verifier", verifier.expose_secret()),
    ];
    let result = oidc
        .inner
        .http
        .post(d.token_endpoint)
        .basic_auth(
            &oidc.inner.client_id,
            Some(oidc.inner.client_secret.expose_secret()),
        )
        .form(&form)
        .send()
        .await;
    drop(verifier);
    let tokens: TokenResponse = result?.error_for_status()?.json().await?;
    let id_token = SecretString::from(tokens.id_token);
    let header = jsonwebtoken::decode_header(id_token.expose_secret()).map_err(|_| AuthError)?;
    if header.alg != jsonwebtoken::Algorithm::RS256 {
        return Err(AuthError);
    }
    let keys: jsonwebtoken::jwk::JwkSet = oidc
        .inner
        .http
        .get(d.jwks_uri)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let jwk = keys
        .find(header.kid.as_deref().ok_or(AuthError)?)
        .ok_or(AuthError)?;
    let key = jsonwebtoken::DecodingKey::from_jwk(jwk).map_err(|_| AuthError)?;
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_issuer(&[&oidc.inner.issuer]);
    validation.set_audience(&[&oidc.inner.client_id]);
    validation.leeway = 30;
    let claims = jsonwebtoken::decode::<IdClaims>(id_token.expose_secret(), &key, &validation)
        .map_err(|_| AuthError)?
        .claims;
    if claims.sub.is_empty()
        || claims.nonce != state.nonce
        || claims
            .azp
            .as_ref()
            .is_some_and(|a| a != &oidc.inner.client_id)
        || (claims.aud.as_array().is_some_and(|a| a.len() > 1) && claims.azp.is_none())
    {
        return Err(AuthError);
    }
    let user = crate::iam::db::user_upsert_one(
        &oidc.inner.pool.get().await?,
        Uuid::new_v4(),
        &(oidc.inner.issuer),
        &(claims.sub),
        claims.email.as_deref().filter(|v| v.len() <= 320),
        claims.name.as_deref().filter(|v| v.len() <= 200),
    )
    .await?;
    oidc.sync_groups(
        user.id,
        &claims.sub,
        &claims.groups(&oidc.inner.groups.claim),
    )
    .await?;
    let session = random();
    let hash = digest(session.expose_secret());
    crate::iam::db::session_create_execute(&oidc.inner.pool.get().await?, &(hash), user.id).await?;
    Ok(([(header::CACHE_CONTROL,"no-store")],axum::Json(serde_json::json!({"access_token":session.expose_secret(),"expires_in":28800,"user_id":user.id}))).into_response())
}
async fn session(State(oidc): State<Oidc>, headers: HeaderMap) -> Result<Response> {
    let access = oidc.user(&headers).await?;
    let Some(id) = access.user else {
        return Err(AuthError);
    };
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(serde_json::json!({"id":id,"groups":access.groups,"admin":access.admin})),
    )
        .into_response())
}
async fn logout(State(oidc): State<Oidc>, headers: HeaderMap) -> Result<Response> {
    oidc.user(&headers).await?;
    if let Some(token) = bearer(&headers) {
        let hash = digest(token);
        crate::iam::db::session_delete_execute(&oidc.inner.pool.get().await?, &(hash)).await?;
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}
