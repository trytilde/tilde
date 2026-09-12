//! Signed claims avoid identity/config reads. Live invocation state remains authoritative.
use super::capabilities::Capabilities;
use crate::{
    chat::{ChatError, Result},
    encryption::{Encryption, SealedSecret, SecretBinding},
};
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;
use std::sync::Arc;
use tokio::sync::OnceCell;
use uuid::Uuid;

pub const TOKEN_TTL_SECONDS: i64 = 900;
const ISSUER: &str = "tilde:invocation";
const AUDIENCE: &str = "tilde:agent-api";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    pub iss: String,
    pub aud: String,
    pub sub: Uuid,
    pub invocation_id: Uuid,
    pub thread_id: Uuid,
    pub run_id: Uuid,
    pub capabilities: Capabilities,
    pub iat: i64,
    pub exp: i64,
}
#[derive(Clone)]
pub struct PostgresTokens {
    pool: PgPool,
    encryption: Arc<Encryption>,
    key: Arc<OnceCell<SecretString>>,
}
impl PostgresTokens {
    pub fn new(pool: PgPool, encryption: Arc<Encryption>) -> Self {
        Self {
            pool,
            encryption,
            key: Arc::new(OnceCell::new()),
        }
    }
    async fn key(&self) -> Result<&SecretString> {
        self.key
            .get_or_try_init(|| async {
                let binding = || SecretBinding {
                    resource_kind: "iam",
                    resource_id: Uuid::nil(),
                    name: "agent_token_signing_key",
                };
                let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
                rand::rngs::OsRng.fill_bytes(&mut *bytes);
                let secret = SecretString::from(hex::encode(bytes.as_slice()));
                let sealed = self
                    .encryption
                    .seal(binding(), &secret)
                    .map_err(|_| ChatError::Transport)?
                    .into_bytes();
                drop(secret);
                sqlx::query_file!("../../queries/iam/key_insert.sql", sealed)
                    .execute(&self.pool)
                    .await?;
                let row = sqlx::query_file!("../../queries/iam/key_get.sql")
                    .fetch_one(&self.pool)
                    .await?;
                self.encryption
                    .open(
                        binding(),
                        SealedSecret::from_bytes(&row.sealed).map_err(|_| ChatError::Transport)?,
                    )
                    .map_err(|_| ChatError::Transport)
            })
            .await
    }
    pub async fn issue(
        &self,
        agent: Uuid,
        invocation: Uuid,
        thread: Uuid,
        run: Uuid,
    ) -> Result<SecretString> {
        let caps = sqlx::query_file!("../../queries/iam/agent_caps.sql", agent)
            .fetch_one(&self.pool)
            .await?
            .capabilities
            .0;
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            iss: ISSUER.into(),
            aud: AUDIENCE.into(),
            sub: agent,
            invocation_id: invocation,
            thread_id: thread,
            run_id: run,
            capabilities: caps,
            iat: now,
            exp: now + TOKEN_TTL_SECONDS,
        };
        self.sign(&claims).await
    }
    async fn sign(&self, claims: &Claims) -> Result<SecretString> {
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        header.typ = Some("tilde-agent-connect+jwt".into());
        let token = jsonwebtoken::encode(
            &header,
            claims,
            &jsonwebtoken::EncodingKey::from_secret(self.key().await?.expose_secret().as_bytes()),
        )
        .map_err(|_| ChatError::Transport)?;
        Ok(SecretString::from(token))
    }
    pub async fn renew(&self, token: &str) -> Result<SecretString> {
        let mut claims = self.verify(token).await?;
        let current = sqlx::query_file!("../../queries/iam/agent_caps.sql", claims.sub)
            .fetch_one(&self.pool)
            .await?
            .capabilities
            .0;
        claims.capabilities = claims.capabilities.intersect(&current);
        claims.iat = chrono::Utc::now().timestamp();
        claims.exp = claims.iat + TOKEN_TTL_SECONDS;
        self.sign(&claims).await
    }
    pub async fn verify(&self, token: &str) -> Result<Claims> {
        let claims = self.verify_signature(token, false).await?;
        self.verify_live(claims).await
    }

    /// Same token, telemetry-only grace. Expired tokens are accepted for at most
    /// five minutes, and only if the invocation ended before their signed expiry.
    pub async fn verify_trace(&self, token: &str) -> Result<Claims> {
        let claims = self.verify_signature(token, true).await?;
        let live = sqlx::query_file!(
            "../../queries/iam/invocation_trace_live.sql",
            claims.invocation_id,
            claims.sub,
            claims.thread_id,
            claims.run_id,
            claims.exp as f64
        )
        .fetch_one(&self.pool)
        .await?
        .live;
        if !live {
            return Err(ChatError::Denied);
        }
        Ok(claims)
    }

    async fn verify_signature(&self, token: &str, telemetry: bool) -> Result<Claims> {
        if token.len() > 32768 {
            return Err(ChatError::Denied);
        }
        let header = jsonwebtoken::decode_header(token).map_err(|_| ChatError::Denied)?;
        if header.typ.as_deref() != Some("tilde-agent-connect+jwt") {
            return Err(ChatError::Denied);
        }
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_issuer(&[ISSUER]);
        validation.set_audience(&[AUDIENCE]);
        validation.leeway = if telemetry { 300 } else { 0 };
        let claims = jsonwebtoken::decode::<Claims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(self.key().await?.expose_secret().as_bytes()),
            &validation,
        )
        .map_err(|_| ChatError::Denied)?
        .claims;
        if claims.iat > chrono::Utc::now().timestamp()
            || claims.exp - claims.iat > TOKEN_TTL_SECONDS
            || claims.exp <= claims.iat
        {
            return Err(ChatError::Denied);
        }
        Ok(claims)
    }

    async fn verify_live(&self, claims: Claims) -> Result<Claims> {
        let live = sqlx::query_file!(
            "../../queries/iam/invocation_live.sql",
            claims.invocation_id,
            claims.sub,
            claims.thread_id,
            claims.run_id
        )
        .fetch_one(&self.pool)
        .await?
        .live;
        if !live {
            return Err(ChatError::Denied);
        }
        Ok(claims)
    }
}

/// Concrete runtime token authorities share the public API without giving a
/// sidecar a Postgres pool or the installation signing key.
#[derive(Clone)]
pub enum Tokens {
    Postgres(PostgresTokens),
    Sidecar(Arc<crate::deployment::runtime::Runtime>),
}
impl Tokens {
    pub fn new(pool: PgPool, encryption: Arc<Encryption>) -> Self {
        Self::Postgres(PostgresTokens::new(pool, encryption))
    }
    pub fn sidecar(runtime: Arc<crate::deployment::runtime::Runtime>) -> Self {
        Self::Sidecar(runtime)
    }
    pub async fn issue(
        &self,
        agent: Uuid,
        invocation: Uuid,
        thread: Uuid,
        run: Uuid,
    ) -> Result<SecretString> {
        match self {
            Self::Postgres(tokens) => tokens.issue(agent, invocation, thread, run).await,
            Self::Sidecar(runtime) => {
                let state = runtime.invocation(invocation).await?;
                if state.agent_id != agent.to_string()
                    || state.thread_id != thread.to_string()
                    || state.run_id != run.to_string()
                {
                    return Err(ChatError::Denied);
                }
                runtime.issue_token(&state).await
            }
        }
    }
    pub async fn renew(&self, token: &str) -> Result<SecretString> {
        match self {
            Self::Postgres(tokens) => tokens.renew(token).await,
            Self::Sidecar(runtime) => runtime.renew_token(token).await,
        }
    }
    pub async fn verify(&self, token: &str) -> Result<Claims> {
        match self {
            Self::Postgres(tokens) => tokens.verify(token).await,
            Self::Sidecar(runtime) => runtime.verified_claims(token, false).await,
        }
    }
    pub async fn verify_trace(&self, token: &str) -> Result<Claims> {
        match self {
            Self::Postgres(tokens) => tokens.verify_trace(token).await,
            Self::Sidecar(runtime) => runtime.verified_claims(token, true).await,
        }
    }
}
