//! Internal encryption for database secrets, with seed or AWS KMS key protection.
//! Resource identity is authenticated with each ciphertext; no tenant model is involved.
//! The legacy context labels are persisted cryptographic identifiers, not product branding.
use crate::error::Error;
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use client_aws_kms::Client;
use rand::{RngCore, rngs::OsRng};
use secrecy::{ExposeSecret, SecretString};
use sqlx::{FromRow, PgPool};
use std::collections::BTreeMap;
use uuid::Uuid;
use zeroize::Zeroizing;

const FORMAT_VERSION: i16 = 1;
const KEY_LOCK: i64 = 0x4147454e544b4559;

/// Configured once at startup. Deliberately does not implement Debug or Serialize.
pub enum KeyProtection {
    Seed(Zeroizing<Vec<u8>>),
    AwsKms { client: Client, key_id: String },
}

impl KeyProtection {
    /// The seed is base64-encoded random key material, not a password or passphrase.
    pub fn seed(encoded: SecretString) -> Result<Self, Error> {
        let decoded = Zeroizing::new(
            STANDARD
                .decode(encoded.expose_secret())
                .map_err(|_| Error::Invalid("Encryption seed must be base64".into()))?,
        );
        if decoded.len() != 32 {
            return Err(Error::Invalid(
                "Encryption seed must decode to exactly 32 bytes".into(),
            ));
        }
        Ok(Self::Seed(decoded))
    }

    /// Accept the focused, project-owned KMS client.
    pub fn kms(client: Client, key_id: String) -> Result<Self, Error> {
        if key_id.trim().is_empty() {
            return Err(Error::Invalid("AWS KMS key ID is required".into()));
        }
        Ok(Self::AwsKms { client, key_id })
    }

    fn backend(&self) -> &'static str {
        match self {
            Self::Seed(_) => "seed",
            Self::AwsKms { .. } => "aws_kms",
        }
    }

    async fn generate(&self, id: Uuid) -> Result<(KeyRow, Zeroizing<Vec<u8>>), Error> {
        match self {
            Self::Seed(seed) => {
                let mut key = Zeroizing::new(vec![0; 32]);
                OsRng.fill_bytes(&mut key);
                let nonce = random_nonce();
                let wrapped = encrypt(seed, &nonce, &wrap_context(id), &key)?;
                Ok((
                    KeyRow {
                        id,
                        backend: "seed".into(),
                        wrapping_key_id: "seed-v1".into(),
                        wrap_nonce: Some(nonce.to_vec()),
                        wrapped_key: wrapped,
                    },
                    key,
                ))
            }
            Self::AwsKms { client, key_id } => {
                let result = client
                    .generate_data_key(key_id, &kms_context(id))
                    .await
                    .map_err(|_| Error::Kms)?;
                let key = result.plaintext;
                let wrapped_key = result.ciphertext_blob;
                let canonical_id = result.key_id;
                Ok((
                    KeyRow {
                        id,
                        backend: "aws_kms".into(),
                        wrapping_key_id: canonical_id,
                        wrap_nonce: None,
                        wrapped_key,
                    },
                    key,
                ))
            }
        }
    }

    async fn unwrap(&self, row: &KeyRow) -> Result<Zeroizing<Vec<u8>>, Error> {
        if row.backend != self.backend() {
            return Err(Error::BackendMismatch);
        }
        let key = match self {
            Self::Seed(seed) => {
                if row.wrapping_key_id != "seed-v1" {
                    return Err(Error::Encryption);
                }
                let nonce: [u8; 12] = row
                    .wrap_nonce
                    .as_deref()
                    .ok_or(Error::Encryption)?
                    .try_into()
                    .map_err(|_| Error::Encryption)?;
                decrypt(seed, &nonce, &wrap_context(row.id), &row.wrapped_key)?
            }
            Self::AwsKms { client, key_id } => {
                let result = client
                    .decrypt(key_id, &row.wrapped_key, &kms_context(row.id))
                    .await
                    .map_err(|_| Error::Kms)?;
                let key = result.plaintext;
                if result.key_id != row.wrapping_key_id {
                    return Err(Error::Kms);
                }
                key
            }
        };
        if key.len() != 32 {
            return Err(Error::Encryption);
        }
        Ok(key)
    }
}

#[derive(FromRow)]
struct KeyRow {
    id: Uuid,
    backend: String,
    wrapping_key_id: String,
    wrap_nonce: Option<Vec<u8>>,
    wrapped_key: Vec<u8>,
}

/// The only long-lived plaintext key. Cleared on drop; never serialized or debug-printed.
pub struct Encryption {
    key_id: Uuid,
    key: Zeroizing<Vec<u8>>,
}

/// Non-secret, caller-derived identity of the record that will store the ciphertext.
#[derive(Clone, Copy)]
pub struct SecretBinding<'a> {
    pub resource_kind: &'a str,
    pub resource_id: Uuid,
    pub name: &'a str,
}

/// Database representation of a sealed value; the format and binding are authenticated.
#[derive(Clone, FromRow)]
pub struct SealedSecret {
    pub key_id: Uuid,
    pub format_version: i16,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

impl SealedSecret {
    /// Store an envelope in one BYTEA column: version (2), key ID (16), nonce (12), ciphertext/tag.
    pub fn into_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(30 + self.ciphertext.len());
        bytes.extend_from_slice(&self.format_version.to_be_bytes());
        bytes.extend_from_slice(self.key_id.as_bytes());
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.ciphertext);
        bytes
    }

    /// Decode only the envelope framing; `Encryption::open` authenticates its contents.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 46 {
            return Err(Error::Encryption);
        }
        Ok(Self {
            format_version: i16::from_be_bytes([bytes[0], bytes[1]]),
            key_id: Uuid::from_slice(&bytes[2..18]).map_err(|_| Error::Encryption)?,
            nonce: bytes[18..30].to_vec(),
            ciphertext: bytes[30..].to_vec(),
        })
    }
}

impl Encryption {
    /// Unlock the persisted data key or initialize it once under a database lock.
    pub async fn initialize(pool: &PgPool, protection: KeyProtection) -> Result<Self, Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_file!("../../queries/encryption/lock.sql", KEY_LOCK)
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query_file_as!(KeyRow, "../../queries/encryption/get_key.sql")
            .fetch_optional(&mut *tx)
            .await?;
        let (key_id, key) = if let Some(row) = row {
            let key = protection.unwrap(&row).await?;
            (row.id, key)
        } else {
            let id = Uuid::new_v4();
            let (row, key) = protection.generate(id).await?;
            sqlx::query_file!(
                "../../queries/encryption/create_key.sql",
                row.id,
                row.backend,
                row.wrapping_key_id,
                row.wrap_nonce,
                row.wrapped_key
            )
            .execute(&mut *tx)
            .await?;
            (id, key)
        };
        tx.commit().await?;
        Ok(Self { key_id, key })
    }

    /// Encrypt a named resource secret with a fresh nonce and exact record binding.
    pub fn seal(
        &self,
        binding: SecretBinding<'_>,
        value: &SecretString,
    ) -> Result<SealedSecret, Error> {
        let nonce = random_nonce();
        let aad = secret_context(self.key_id, binding)?;
        let ciphertext = encrypt(&self.key, &nonce, &aad, value.expose_secret().as_bytes())?;
        Ok(SealedSecret {
            key_id: self.key_id,
            format_version: FORMAT_VERSION,
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }

    /// Load plaintext only for an internal consumer of the exact resource/name slot.
    pub fn open(
        &self,
        binding: SecretBinding<'_>,
        sealed: SealedSecret,
    ) -> Result<SecretString, Error> {
        if sealed.key_id != self.key_id || sealed.format_version != FORMAT_VERSION {
            return Err(Error::Encryption);
        }
        let nonce: [u8; 12] = sealed
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| Error::Encryption)?;
        let plaintext = decrypt(
            &self.key,
            &nonce,
            &secret_context(self.key_id, binding)?,
            &sealed.ciphertext,
        )?;
        let text = std::str::from_utf8(&plaintext).map_err(|_| Error::Encryption)?;
        Ok(SecretString::from(text.to_owned()))
    }
}

fn random_nonce() -> [u8; 12] {
    let mut nonce = [0; 12];
    OsRng.fill_bytes(&mut nonce);
    nonce
}
fn wrap_context(id: Uuid) -> Vec<u8> {
    format!("agent-engine:data-key:v1:{id}").into_bytes()
}
fn secret_context(key: Uuid, binding: SecretBinding<'_>) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(&(
        "agent-engine:secret",
        FORMAT_VERSION,
        key,
        binding.resource_kind,
        binding.resource_id,
        binding.name,
    ))
    .map_err(|_| Error::Encryption)
}
fn encrypt(key: &[u8], nonce: &[u8; 12], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, Error> {
    Aes256Gcm::new_from_slice(key)
        .map_err(|_| Error::Encryption)?
        .encrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| Error::Encryption)
}
fn decrypt(
    key: &[u8],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, Error> {
    Aes256Gcm::new_from_slice(key)
        .map_err(|_| Error::Encryption)?
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::Encryption)
}

fn kms_context(id: Uuid) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("application".into(), "agent-engine".into()),
        ("data_key_id".into(), id.to_string()),
    ])
}
