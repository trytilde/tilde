//! Short-lived, agent-scoped ingress credentials. The bootstrap secret does not
//! authorize caller-selected agents, threads, or identities on operational APIs.
use super::{Deployments, id};
use crate::error::Error;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngressClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub agent_id: Uuid,
    pub thread_id: Option<Uuid>,
    pub exp: i64,
}
impl Deployments {
    pub async fn issue_ingress_token(
        &self,
        agent: Uuid,
        thread: Option<String>,
        principal: Uuid,
    ) -> Result<SecretString, Error> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_file!("../../queries/deployment/lock.sql", agent)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        let secret = match row.encrypted_secrets.as_deref() {
            Some(sealed) => self.open_secrets(agent, sealed)?,
            None => {
                let secret = super::secrets::Secrets::generate();
                let sealed = self
                    .encryption
                    .seal(super::secret_binding(agent), &secret.encode()?)?
                    .into_bytes();
                sqlx::query_file!("../../queries/deployment/store_secrets.sql", agent, sealed)
                    .execute(&mut *tx)
                    .await?;
                secret
            }
        };
        tx.commit().await?;
        let claims = IngressClaims {
            iss: "tilde:ingress".into(),
            aud: "tilde:public-event-ingress".into(),
            sub: principal.to_string(),
            agent_id: agent,
            thread_id: thread.as_deref().map(id).transpose()?,
            exp: chrono::Utc::now().timestamp() + 900,
        };
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(secret.signing_key.expose_secret().as_bytes()),
        )
        .map(SecretString::from)
        .map_err(|_| Error::Encryption)
    }
}
pub fn verify_ingress(
    token: &str,
    agent: Uuid,
    key: &SecretString,
) -> Result<IngressClaims, Error> {
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_audience(&["tilde:public-event-ingress"]);
    validation.set_issuer(&["tilde:ingress"]);
    let claims = jsonwebtoken::decode::<IngressClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(key.expose_secret().as_bytes()),
        &validation,
    )
    .map_err(|_| Error::Denied)?
    .claims;
    if claims.agent_id != agent {
        return Err(Error::Denied);
    }
    Ok(claims)
}
