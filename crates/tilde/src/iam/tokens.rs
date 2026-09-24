//! Five-minute signed claims authorize RPCs until expiry. Issuance and renewal
//! check live invocation state; ordinary requests need no database authorization read.
use super::capabilities::Capabilities;
use crate::database::Pool;
use crate::{
    chat::{ChatError, Result, Scope},
    encryption::{Encryption, SealedSecret, SecretBinding},
};
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::OnceCell;
use uuid::Uuid;

pub const TOKEN_TTL_SECONDS: i64 = 300;
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
    pub participant_id: Uuid,
    pub capabilities: Capabilities,
    /// Ready inference connections assigned to the agent when the token was signed.
    pub inference: Vec<Uuid>,
    pub iat: i64,
    pub exp: i64,
}
#[derive(Clone)]
pub struct PostgresTokens {
    pool: Pool,
    encryption: Arc<Encryption>,
    key: Arc<OnceCell<SecretString>>,
}
impl PostgresTokens {
    pub fn new(pool: Pool, encryption: Arc<Encryption>) -> Self {
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
                crate::iam::db::key_insert_execute(&self.pool.get().await?, &(sealed)).await?;
                let row = crate::iam::db::key_get_one(&self.pool.get().await?).await?;
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
        let (participant_id, caps) = self.live_grants(agent, invocation, thread, run).await?;
        let inference = self.inference(agent, run).await?;
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            iss: ISSUER.into(),
            aud: AUDIENCE.into(),
            sub: agent,
            invocation_id: invocation,
            thread_id: thread,
            run_id: run,
            participant_id,
            capabilities: caps,
            inference,
            iat: now,
            exp: now + TOKEN_TTL_SECONDS,
        };
        self.sign(&claims).await
    }
    /// Assignment changes and exhausted budgets take effect at the next renewal. A blocked
    /// agent budget, or a blocked identity budget for the run's source identity, empties the claim.
    async fn inference(&self, agent: Uuid, run: Uuid) -> Result<Vec<Uuid>> {
        Ok(
            crate::iam::db::inference_connections_all(&self.pool.get().await?, agent, run)
                .await?
                .into_iter()
                .map(|r| r.connection_id)
                .collect(),
        )
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
        let (participant_id, current) = self
            .live_grants(
                claims.sub,
                claims.invocation_id,
                claims.thread_id,
                claims.run_id,
            )
            .await?;
        if participant_id != claims.participant_id {
            return Err(ChatError::Denied);
        }
        claims.capabilities = claims.capabilities.intersect(&current);
        claims.inference = self.inference(claims.sub, claims.run_id).await?;
        claims.iat = chrono::Utc::now().timestamp();
        claims.exp = claims.iat + TOKEN_TTL_SECONDS;
        self.sign(&claims).await
    }
    pub async fn verify(&self, token: &str) -> Result<Claims> {
        self.verify_signature(token, false).await
    }

    /// Same token, telemetry-only grace. Expired tokens are accepted for at most
    /// five minutes, and only if the invocation ended before their signed expiry.
    pub async fn verify_trace(&self, token: &str) -> Result<Claims> {
        let claims = self.verify_signature(token, true).await?;
        let live = crate::iam::db::invocation_trace_live_one(
            &self.pool.get().await?,
            claims.invocation_id,
            claims.sub,
            claims.thread_id,
            claims.run_id,
            claims.exp as f64,
        )
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

    /// Only token issuance and renewal consult current invocation authority.
    async fn live_grants(
        &self,
        agent: Uuid,
        invocation: Uuid,
        thread: Uuid,
        run: Uuid,
    ) -> Result<(Uuid, Capabilities)> {
        let row = crate::iam::db::invocation_live_opt(
            &self.pool.get().await?,
            invocation,
            agent,
            thread,
            run,
        )
        .await?
        .ok_or(ChatError::Denied)?;
        Ok((row.participant_id, row.capabilities.0))
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
    pub fn new(pool: Pool, encryption: Arc<Encryption>) -> Self {
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
    /// Resolve identity and grants from a valid token, without a live-state read.
    pub async fn scope(&self, token: &str) -> Result<Scope> {
        match self {
            Self::Postgres(tokens) => {
                let claims = tokens.verify_signature(token, false).await?;
                Ok(Scope {
                    capabilities: claims.capabilities,
                    id: claims.invocation_id,
                    run_id: claims.run_id,
                    thread_id: claims.thread_id,
                    agent_id: claims.sub,
                    participant_id: claims.participant_id,
                    inference: claims.inference,
                })
            }
            Self::Sidecar(runtime) => runtime.scope(token).await,
        }
    }
    pub async fn verify_trace(&self, token: &str) -> Result<Claims> {
        match self {
            Self::Postgres(tokens) => tokens.verify_trace(token).await,
            Self::Sidecar(runtime) => runtime.verified_claims(token, true).await,
        }
    }
}
