//! OIDC relying party, not an identity provider. Login creates a revocable local bearer session.
//! All users admitted by the configured provider have unrestricted installation access.
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
use sqlx::PgPool;
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
impl From<sqlx::Error> for AuthError {
    fn from(_: sqlx::Error) -> Self {
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
    pool: PgPool,
    encryption: Arc<Encryption>,
    issuer: String,
    client_id: String,
    client_secret: SecretString,
    public_url: String,
    insecure: bool,
    http: reqwest::Client,
}
impl Oidc {
    pub fn new(
        pool: PgPool,
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
                pool,
                encryption,
                issuer,
                client_id,
                client_secret,
                public_url: public_url.trim_end_matches('/').into(),
                insecure,
                http,
            }),
        })
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
    pub fn router(&self) -> Router {
        Router::new()
            .route("/auth/login", post(login))
            .route("/auth/exchange", post(callback))
            .route("/auth/session", get(session))
            .route("/auth/logout", post(logout))
            .with_state(self.clone())
    }
    pub async fn principal(&self, headers: &HeaderMap) -> Result<super::Principal> {
        let token = bearer(headers).ok_or(AuthError)?;
        let hash = digest(token);
        let row = sqlx::query_file!("../../queries/iam/session_get.sql", hash)
            .fetch_optional(&self.inner.pool)
            .await?
            .ok_or(AuthError)?;
        Ok(super::Principal::User { id: row.user_id })
    }
}
pub async fn management_guard(
    State(oidc): State<Oidc>,
    mut request: Request,
    next: Next,
) -> Response {
    match oidc.principal(request.headers()).await {
        Ok(principal) => {
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        Err(e) => e.into_response(),
    }
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
    sqlx::query_file!("../../queries/iam/cleanup.sql")
        .execute(&oidc.inner.pool)
        .await?;
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
    sqlx::query_file!(
        "../../queries/iam/state_create.sql",
        id,
        hash,
        nonce.expose_secret(),
        sealed,
        browser_hash
    )
    .execute(&oidc.inner.pool)
    .await?;
    let mut url = url::Url::parse(&d.authorization_endpoint).map_err(|_| AuthError)?;
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", &oidc.inner.client_id),
        (
            "redirect_uri",
            &format!("{}/auth/callback", oidc.inner.public_url),
        ),
        ("scope", "openid profile email"),
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
    let state = sqlx::query_file!("../../queries/iam/state_consume.sql", hash, browser_hash)
        .fetch_optional(&oidc.inner.pool)
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
    let user = sqlx::query_file!(
        "../../queries/iam/user_upsert.sql",
        Uuid::new_v4(),
        oidc.inner.issuer,
        claims.sub
    )
    .fetch_one(&oidc.inner.pool)
    .await?;
    let session = random();
    let hash = digest(session.expose_secret());
    sqlx::query_file!("../../queries/iam/session_create.sql", hash, user.id)
        .execute(&oidc.inner.pool)
        .await?;
    Ok(([(header::CACHE_CONTROL,"no-store")],axum::Json(serde_json::json!({"access_token":session.expose_secret(),"expires_in":28800,"user_id":user.id}))).into_response())
}
async fn session(State(oidc): State<Oidc>, headers: HeaderMap) -> Result<Response> {
    let super::Principal::User { id } = oidc.principal(&headers).await? else {
        return Err(AuthError);
    };
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(serde_json::json!({"id":id})),
    )
        .into_response())
}
async fn logout(State(oidc): State<Oidc>, headers: HeaderMap) -> Result<Response> {
    oidc.principal(&headers).await?;
    if let Some(token) = bearer(&headers) {
        let hash = digest(token);
        sqlx::query_file!("../../queries/iam/session_delete.sql", hash)
            .execute(&oidc.inner.pool)
            .await?;
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}
