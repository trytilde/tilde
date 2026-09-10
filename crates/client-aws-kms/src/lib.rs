//! Focused KMS JSON client adapted from Tilde API's client-aws-kms.
//! Every operation uses real signed HTTP; there is no identity/no-op wrapping mode.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use client_aws_config::Config;
use client_aws_sigv4::{Credentials, Signer};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime},
};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

/// An unwrapped data key and its canonical KMS key identity. No Debug or Clone.
pub struct DecryptOutput {
    pub key_id: String,
    pub plaintext: Zeroizing<Vec<u8>>,
}

/// A generated data key, returned once in plaintext and in persistable wrapped form.
pub struct GenerateDataKeyOutput {
    pub key_id: String,
    pub plaintext: Zeroizing<Vec<u8>>,
    pub ciphertext_blob: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("AWS credentials are unavailable")]
    Config(#[from] client_aws_config::Error),
    #[error("AWS KMS HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("AWS KMS request signing failed")]
    Signing(#[from] client_aws_sigv4::Error),
    #[error("invalid AWS KMS response")]
    InvalidResponse,
    #[error("invalid AWS KMS region or endpoint")]
    InvalidConfig,
    #[error("AWS KMS rejected the request (HTTP {0})")]
    Service(u16),
}

#[derive(Clone)]
pub struct Client {
    region: String,
    endpoint: String,
    http: reqwest::Client,
    credentials: Option<Credentials>,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "PascalCase")]
struct WireKey {
    key_id: String,
    plaintext: String,
    ciphertext_blob: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct GenerateRequest<'a> {
    key_id: &'a str,
    key_spec: &'static str,
    encryption_context: &'a BTreeMap<String, String>,
}
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct DecryptRequest<'a> {
    key_id: &'a str,
    ciphertext_blob: String,
    encryption_context: &'a BTreeMap<String, String>,
}

impl Client {
    /// Use the regional KMS endpoint and resolve fresh credentials for each operation.
    pub fn new(region: impl Into<String>) -> Result<Self, Error> {
        let region = region.into();
        if region.is_empty()
            || !region
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(Error::InvalidConfig);
        }
        let suffix = if region.starts_with("cn-") {
            "amazonaws.com.cn"
        } else {
            "amazonaws.com"
        };
        Ok(Self {
            endpoint: format!("https://kms.{region}.{suffix}/"),
            region,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            credentials: None,
        })
    }

    /// Inject explicit credentials and an endpoint for local fixtures or compatible KMS services.
    pub fn with_config(config: Config, endpoint: &str) -> Result<Self, Error> {
        let mut client = Self::new(config.region)?;
        let url = reqwest::Url::parse(endpoint).map_err(|_| Error::InvalidConfig)?;
        let loopback = url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.host_str().is_none()
        {
            return Err(Error::InvalidConfig);
        }
        client.endpoint = url.to_string();
        client.credentials = Some(config.credentials);
        Ok(client)
    }

    /// Request an AES-256 data key bound to the supplied encryption context.
    pub async fn generate_data_key(
        &self,
        key_id: &str,
        context: &BTreeMap<String, String>,
    ) -> Result<GenerateDataKeyOutput, Error> {
        let mut wire = self
            .call(
                "GenerateDataKey",
                &GenerateRequest {
                    key_id,
                    key_spec: "AES_256",
                    encryption_context: context,
                },
            )
            .await?;
        let plaintext = decode_plaintext(&wire.plaintext)?;
        let ciphertext_blob = STANDARD
            .decode(
                wire.ciphertext_blob
                    .as_deref()
                    .ok_or(Error::InvalidResponse)?,
            )
            .map_err(|_| Error::InvalidResponse)?;
        if ciphertext_blob.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(GenerateDataKeyOutput {
            key_id: std::mem::take(&mut wire.key_id),
            plaintext,
            ciphertext_blob,
        })
    }

    /// Unwrap a data key under the specified KMS key and exact original context.
    pub async fn decrypt(
        &self,
        key_id: &str,
        ciphertext: &[u8],
        context: &BTreeMap<String, String>,
    ) -> Result<DecryptOutput, Error> {
        let mut wire = self
            .call(
                "Decrypt",
                &DecryptRequest {
                    key_id,
                    ciphertext_blob: STANDARD.encode(ciphertext),
                    encryption_context: context,
                },
            )
            .await?;
        Ok(DecryptOutput {
            plaintext: decode_plaintext(&wire.plaintext)?,
            key_id: std::mem::take(&mut wire.key_id),
        })
    }

    async fn call(&self, operation: &str, payload: &impl Serialize) -> Result<WireKey, Error> {
        let credentials = match &self.credentials {
            Some(credentials) => credentials.clone(),
            None => {
                client_aws_config::load(self.region.clone())
                    .await?
                    .credentials
            }
        };
        let body = serde_json::to_vec(payload).map_err(|_| Error::InvalidResponse)?;
        let mut request = self
            .http
            .post(&self.endpoint)
            .header("content-type", "application/x-amz-json-1.1")
            .header("x-amz-target", format!("TrentService.{operation}"))
            .body(body.clone())
            .build()?;
        Signer::new(credentials, &self.region, "kms").sign_request_at(
            &mut request,
            &body,
            SystemTime::now(),
        )?;
        let response = self.http.execute(request).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Service(status.as_u16()));
        }
        let text = Zeroizing::new(response.text().await?);
        let wire: WireKey = serde_json::from_str(&text).map_err(|_| Error::InvalidResponse)?;
        if wire.key_id.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(wire)
    }
}

fn decode_plaintext(encoded: &str) -> Result<Zeroizing<Vec<u8>>, Error> {
    let bytes = Zeroizing::new(
        STANDARD
            .decode(encoded)
            .map_err(|_| Error::InvalidResponse)?,
    );
    if bytes.len() != 32 {
        return Err(Error::InvalidResponse);
    }
    Ok(bytes)
}
