//! Bootstrap keys are scoped to one agent. Temporary plaintext is zeroized on drop.
use crate::error::Error;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use zeroize::{ZeroizeOnDrop, Zeroizing};

pub(super) struct Secrets {
    pub encryption_key: SecretString,
    pub api_token: SecretString,
    pub signing_key: SecretString,
    pub ca_certificate: SecretString,
    pub ca_key: SecretString,
}
#[derive(Deserialize, ZeroizeOnDrop)]
struct Stored {
    api_token: String,
    encryption_key: String,
    signing_key: String,
    ca_certificate: String,
    ca_key: String,
}
impl Secrets {
    pub fn generate() -> Result<Self, Error> {
        let mut params = rcgen::CertificateParams::default();
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "Tilde agent replication");
        let ca = rcgen::Certificate::from_params(params).map_err(|_| Error::Encryption)?;
        Ok(Self {
            api_token: random_secret(),
            encryption_key: random_secret(),
            signing_key: random_secret(),
            ca_certificate: ca.serialize_pem().map_err(|_| Error::Encryption)?.into(),
            ca_key: ca.serialize_private_key_pem().into(),
        })
    }
    pub fn encode(&self) -> Result<SecretString, Error> {
        #[derive(Serialize)]
        struct View<'a> {
            api_token: &'a str,
            encryption_key: &'a str,
            signing_key: &'a str,
            ca_certificate: &'a str,
            ca_key: &'a str,
        }
        serde_json::to_string(&View {
            api_token: self.api_token.expose_secret(),
            encryption_key: self.encryption_key.expose_secret(),
            signing_key: self.signing_key.expose_secret(),
            ca_certificate: self.ca_certificate.expose_secret(),
            ca_key: self.ca_key.expose_secret(),
        })
        .map(SecretString::from)
        .map_err(|_| Error::Encryption)
    }
    pub fn decode(encoded: SecretString) -> Result<Self, Error> {
        let value: Stored =
            serde_json::from_str(encoded.expose_secret()).map_err(|_| Error::Encryption)?;
        Ok(Self {
            api_token: value.api_token.clone().into(),
            encryption_key: value.encryption_key.clone().into(),
            signing_key: value.signing_key.clone().into(),
            ca_certificate: value.ca_certificate.clone().into(),
            ca_key: value.ca_key.clone().into(),
        })
    }
    pub fn certificate(&self, address: std::net::IpAddr) -> Result<(String, SecretString), Error> {
        let key =
            rcgen::KeyPair::from_pem(self.ca_key.expose_secret()).map_err(|_| Error::Encryption)?;
        let params =
            rcgen::CertificateParams::from_ca_cert_pem(self.ca_certificate.expose_secret(), key)
                .map_err(|_| Error::Encryption)?;
        let ca = rcgen::Certificate::from_params(params).map_err(|_| Error::Encryption)?;
        let params = rcgen::CertificateParams::new(vec![address.to_string()]);
        let cert = rcgen::Certificate::from_params(params).map_err(|_| Error::Encryption)?;
        Ok((
            cert.serialize_pem_with_signer(&ca)
                .map_err(|_| Error::Encryption)?,
            cert.serialize_private_key_pem().into(),
        ))
    }
}
pub(super) fn random_secret() -> SecretString {
    let mut bytes = Zeroizing::new([0u8; 32]);
    rand::rngs::OsRng.fill_bytes(bytes.as_mut());
    STANDARD.encode(bytes.as_slice()).into()
}
