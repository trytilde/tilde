//! The registry's OpenID Connect issuer: one issuer URL and one signing key set, from which any
//! Tilde component mints tokens for a relying party. Its consumers are workload federation from
//! connections (`aws`/`oidc`: audience `sts.amazonaws.com`, subject `connection:<connection id>`,
//! [`Issuer::mint`]). Key storage is not tied to any consumer.
//!
//! The issuer is a public URL (`ENGINE_TOKEN_ISSUER_URL`, by default the event ingress origin
//! plus `/oidc`) that serves the discovery document at `{issuer}/.well-known/openid-configuration`
//! and the JWKS at `{issuer}/jwks`; both routes live on the public event ingress under `/oidc`,
//! so a custom issuer URL must reach those paths. Tokens are RS256 JWTs for one audience, valid
//! for five minutes.
//!
//! Signing keys are 2048-bit RSA, generated on first use, sealed by the encryption crate and
//! stored in `oidc_signing_keys`. The newest key signs. A key older than [`ROTATE_DAYS`] is
//! replaced at the next signing ([`Issuer::rotate`] forces it); superseded keys stay in the JWKS
//! until they are [`PUBLISHED_DAYS`] old, longer than any verifier caches them, then are pruned.
pub mod db;

use crate::database::Pool;
use crate::encryption::{Encryption, SealedSecret, SecretBinding};
use crate::error::Error;
use aws_lc_rs::{rand::SystemRandom, rsa, signature};
use base64::Engine;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const ROTATE_DAYS: i32 = 30;
pub const PUBLISHED_DAYS: i32 = 37;
/// How long a token is valid.
pub const TOKEN_SECONDS: i64 = 300;
/// How long a replica trusts its loaded keys before rereading (another replica may rotate).
const RELOAD: Duration = Duration::from_secs(300);
/// How stale the published key set may be. A verifier that meets an unknown key id fetches the
/// set again, and another process (a replica, say) may have signed with a key it created a
/// moment ago: the set is reread for a request unless it was read within
/// this second (which bounds the reads, not the freshness a verifier's refetch gets).
const PUBLISH_RELOAD: Duration = Duration::from_secs(1);
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

struct Key {
    id: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    pair: rsa::KeyPair,
}
/// The published keys and when this replica read them.
type Loaded = (Instant, Arc<Vec<Key>>);
#[derive(Clone)]
pub struct Issuer {
    pool: Pool,
    crypto: Arc<Encryption>,
    url: String,
    keys: Arc<Mutex<Option<Loaded>>>,
}
fn binding(id: Uuid) -> SecretBinding<'static> {
    SecretBinding {
        resource_kind: "oidc_signing_key",
        resource_id: id,
        name: "private_key",
    }
}
impl Issuer {
    pub fn new(pool: Pool, crypto: Arc<Encryption>, url: String) -> Self {
        Self {
            pool,
            crypto,
            url: url.trim_end_matches('/').into(),
            keys: Arc::default(),
        }
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn discovery(&self) -> Value {
        json!({
            "issuer": self.url,
            "jwks_uri": format!("{}/jwks", self.url),
            "response_types_supported": ["id_token"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
            "claims_supported": ["iss", "sub", "aud", "iat", "nbf", "exp", "jti",
                "act", "org", "groups", "thread", "routine", "signal"],
        })
    }
    /// Every published key's public half.
    pub async fn jwks(&self) -> Result<Value, Error> {
        let keys = self.keys(PUBLISH_RELOAD).await?;
        let mut out = Vec::with_capacity(keys.len());
        for key in keys.iter() {
            use aws_lc_rs::signature::KeyPair;
            let (n, e) = rsa_components(key.pair.public_key().as_ref()).ok_or(Error::Encryption)?;
            out.push(json!({
                "kty": "RSA", "use": "sig", "alg": "RS256", "kid": key.id.to_string(),
                "n": B64.encode(n), "e": B64.encode(e),
            }));
        }
        Ok(json!({ "keys": out }))
    }
    /// A short-lived token for `subject` and `audience`, signed with the current key.
    pub async fn mint(&self, subject: &str, audience: &str) -> Result<SecretString, Error> {
        self.sign(subject, audience, serde_json::Map::new()).await
    }
    pub(crate) async fn sign(
        &self,
        subject: &str,
        audience: &str,
        extra: serde_json::Map<String, Value>,
    ) -> Result<SecretString, Error> {
        let mut keys = self.keys(RELOAD).await?;
        if keys.first().is_none_or(|k| {
            k.created_at < chrono::Utc::now() - chrono::Duration::days(ROTATE_DAYS as i64)
        }) {
            self.add_key(ROTATE_DAYS).await?;
            keys = self.keys(Duration::ZERO).await?;
        }
        let key = keys
            .first()
            .ok_or_else(|| Error::Invalid("No OIDC signing key".into()))?;
        let now = chrono::Utc::now().timestamp();
        let header = json!({"alg": "RS256", "typ": "JWT", "kid": key.id.to_string()});
        let mut claims = extra;
        // The registered claims are written last: a consumer's claims never replace them.
        for (name, value) in [
            ("iss", json!(self.url)),
            ("sub", json!(subject)),
            ("aud", json!(audience)),
            ("iat", json!(now)),
            ("nbf", json!(now - 30)),
            ("exp", json!(now + TOKEN_SECONDS)),
            ("jti", json!(Uuid::new_v4().to_string())),
        ] {
            claims.insert(name.into(), value);
        }
        let claims = Value::Object(claims);
        let signed = format!(
            "{}.{}",
            B64.encode(header.to_string()),
            B64.encode(claims.to_string())
        );
        let mut signature = vec![0; key.pair.public_modulus_len()];
        key.pair
            .sign(
                &signature::RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                signed.as_bytes(),
                &mut signature,
            )
            .map_err(|_| Error::Encryption)?;
        Ok(SecretString::from(format!(
            "{signed}.{}",
            B64.encode(signature)
        )))
    }
    /// The claims of a token this issuer signed for `audience`, if its signature, issuer,
    /// audience and lifetime check out against the published keys (reread once for an unknown
    /// key id: another process may have just rotated).
    pub async fn verify(&self, token: &str, audience: &str) -> Result<Value, Error> {
        let refused = || Error::Denied;
        let kid = jsonwebtoken::decode_header(token)
            .ok()
            .and_then(|h| h.kid)
            .and_then(|k| Uuid::parse_str(&k).ok())
            .ok_or_else(refused)?;
        let mut keys = self.keys(RELOAD).await?;
        if !keys.iter().any(|k| k.id == kid) {
            keys = self.keys(PUBLISH_RELOAD).await?;
        }
        let key = keys.iter().find(|k| k.id == kid).ok_or_else(refused)?;
        use aws_lc_rs::signature::KeyPair;
        let decoding = jsonwebtoken::DecodingKey::from_rsa_der(key.pair.public_key().as_ref());
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_issuer(&[&self.url]);
        validation.set_audience(&[audience]);
        validation.leeway = 30;
        jsonwebtoken::decode::<Value>(token, &decoding, &validation)
            .map(|data| data.claims)
            .map_err(|_| refused())
    }
    /// Starts signing with a new key now; the previous ones stay published.
    pub async fn rotate(&self) -> Result<(), Error> {
        self.add_key(0).await?;
        self.keys(Duration::ZERO).await?;
        Ok(())
    }
    async fn add_key(&self, fresh_days: i32) -> Result<(), Error> {
        let id = Uuid::new_v4();
        let pair = rsa::KeyPair::generate(rsa::KeySize::Rsa2048).map_err(|_| Error::Encryption)?;
        let der = {
            use aws_lc_rs::encoding::AsDer;
            let der: aws_lc_rs::encoding::Pkcs8V1Der =
                pair.as_der().map_err(|_| Error::Encryption)?;
            SecretString::from(B64.encode(der.as_ref()))
        };
        let store = async {
            let sealed = self.crypto.seal(binding(id), &der)?.into_bytes();
            let client = self.pool.get().await?;
            db::key_insert_execute(&client, id, &sealed, fresh_days).await?;
            db::key_prune_execute(&client, PUBLISHED_DAYS).await?;
            Ok(())
        };
        store.await
    }
    /// The published keys, newest first, from memory unless loaded longer than `stale` ago.
    async fn keys(&self, stale: Duration) -> Result<Arc<Vec<Key>>, Error> {
        if let Some((at, keys)) = self.keys.lock().ok().and_then(|k| k.clone())
            && at.elapsed() < stale
        {
            return Ok(keys);
        }
        let load = async {
            let rows = db::key_list_all(&self.pool.get().await?, PUBLISHED_DAYS).await?;
            let mut keys = Vec::with_capacity(rows.len());
            for row in rows {
                let pkcs8 = self
                    .crypto
                    .open(binding(row.id), SealedSecret::from_bytes(&row.sealed)?)?;
                let der = zeroize::Zeroizing::new(
                    B64.decode(pkcs8.expose_secret())
                        .map_err(|_| Error::Encryption)?,
                );
                keys.push(Key {
                    id: row.id,
                    created_at: row.created_at,
                    pair: rsa::KeyPair::from_pkcs8(&der).map_err(|_| Error::Encryption)?,
                });
            }
            Ok::<_, Error>(keys)
        };
        let keys = Arc::new(load.await?);
        if let Ok(mut cache) = self.keys.lock() {
            *cache = Some((Instant::now(), keys.clone()));
        }
        Ok(keys)
    }
    /// The discovery document and JWKS, mounted on the public event ingress.
    pub fn router(self) -> axum::Router {
        use axum::{extract::State, response::IntoResponse, routing::get};
        async fn discovery(State(issuer): State<Issuer>) -> axum::response::Response {
            axum::Json(issuer.discovery()).into_response()
        }
        async fn jwks(State(issuer): State<Issuer>) -> axum::response::Response {
            match issuer.jwks().await {
                Ok(keys) => (
                    // Verifiers keep the set themselves and refetch on an unknown key id.
                    [(http::header::CACHE_CONTROL, "no-cache")],
                    axum::Json(keys),
                )
                    .into_response(),
                Err(error) => {
                    tracing::warn!(%error, "OIDC keys unavailable");
                    http::StatusCode::SERVICE_UNAVAILABLE.into_response()
                }
            }
        }
        axum::Router::new()
            .route("/oidc/.well-known/openid-configuration", get(discovery))
            .route("/oidc/jwks", get(jwks))
            .with_state(self)
    }
}

/// `n` and `e` of a DER `RSAPublicKey` (RFC 8017: a SEQUENCE of two INTEGERs), without the
/// sign-padding zero JWK forbids.
fn rsa_components(der: &[u8]) -> Option<(&[u8], &[u8])> {
    fn element(input: &[u8], tag: u8) -> Option<(&[u8], &[u8])> {
        let (&first, rest) = input.split_first()?;
        if first != tag {
            return None;
        }
        let (&length, rest) = rest.split_first()?;
        let (length, rest) = if length < 0x80 {
            (length as usize, rest)
        } else {
            let bytes = (length & 0x7f) as usize;
            if bytes == 0 || bytes > 4 || rest.len() < bytes {
                return None;
            }
            let length = rest[..bytes]
                .iter()
                .fold(0usize, |n, b| n << 8 | *b as usize);
            (length, &rest[bytes..])
        };
        (rest.len() >= length).then(|| (&rest[..length], &rest[length..]))
    }
    let (sequence, _) = element(der, 0x30)?;
    let (n, rest) = element(sequence, 0x02)?;
    let (e, _) = element(rest, 0x02)?;
    fn strip(v: &[u8]) -> &[u8] {
        let zeros = v.iter().take_while(|b| **b == 0).count();
        &v[zeros.min(v.len().saturating_sub(1))..]
    }
    Some((strip(n), strip(e)))
}
