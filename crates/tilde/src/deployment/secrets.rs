//! Per-agent sidecar secrets sealed in Postgres. Only the invocation signing key
//! reaches replicas; nothing here is ever written to a replica's disk.
use crate::error::Error;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use zeroize::{ZeroizeOnDrop, Zeroizing};
pub(super) struct Secrets {
    pub signing_key: SecretString,
}
#[derive(Deserialize, ZeroizeOnDrop)]
struct Stored {
    signing_key: String,
}
impl Secrets {
    pub fn generate() -> Self {
        Self {
            signing_key: random_secret(),
        }
    }
    pub fn encode(&self) -> Result<SecretString, Error> {
        #[derive(Serialize)]
        struct View<'a> {
            signing_key: &'a str,
        }
        serde_json::to_string(&View {
            signing_key: self.signing_key.expose_secret(),
        })
        .map(SecretString::from)
        .map_err(|_| Error::Encryption)
    }
    pub fn decode(encoded: SecretString) -> Result<Self, Error> {
        let value: Stored =
            serde_json::from_str(encoded.expose_secret()).map_err(|_| Error::Encryption)?;
        Ok(Self {
            signing_key: value.signing_key.clone().into(),
        })
    }
}
pub(crate) fn random_secret() -> SecretString {
    let mut bytes = Zeroizing::new([0u8; 32]);
    rand::rngs::OsRng.fill_bytes(bytes.as_mut());
    STANDARD.encode(bytes.as_slice()).into()
}
