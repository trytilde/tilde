//! Typed ConnectRPC clients for the management API.
//!
//! These come from `tilde-contracts`, the same generated code the engine serves, so field names
//! and enum values are checked by the compiler instead of being written out by hand.
use anyhow::{Context, Result, bail};
use connectrpc::client::{CallOptions, ClientConfig, HttpClient};
use std::{sync::Arc, time::Duration};
use tilde_contracts::services::tilde::management::v1::{
    AgentAccessServiceClient, AgentServiceClient, DeploymentServiceClient,
    TildeChatProviderServiceClient,
};

pub struct Client {
    base: String,
    pub agents: AgentServiceClient<HttpClient>,
    pub deployments: DeploymentServiceClient<HttpClient>,
    pub access: AgentAccessServiceClient<HttpClient>,
    pub chat: TildeChatProviderServiceClient<HttpClient>,
}

/// Reject anything that could send a credential somewhere unintended before the first request.
pub fn base_url(value: &str) -> Result<String> {
    let url = url::Url::parse(value).with_context(|| format!("{value} is not an absolute URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("{value} must be HTTP(S) without credentials, a query or a fragment");
    }
    Ok(value.trim_end_matches('/').to_string())
}

/// Every call carries this timeout; a gateway that is not answering should fail, not hang.
pub fn options() -> CallOptions {
    CallOptions::default().with_timeout(Duration::from_secs(30))
}

/// HTTPS with the bundled web roots, or plain HTTP/1.1 for a local gateway. `base` must have
/// passed [`base_url`].
pub fn transport(base: &str) -> Result<HttpClient> {
    let url = url::Url::parse(base).context("base_url validated this")?;
    if url.scheme() != "https" {
        // Plain HTTP/1.1: a quickstart gateway behind no proxy does not advertise h2c.
        return Ok(HttpClient::plaintext());
    }
    let roots = connectrpc::rustls::RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
    );
    let tls = connectrpc::rustls::ClientConfig::builder_with_provider(Arc::new(
        connectrpc::rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .context("Could not build a TLS configuration")?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(HttpClient::with_tls(Arc::new(tls)))
}

impl Client {
    pub fn new(base: &str, api_key: Option<String>) -> Result<Self> {
        let base = base_url(base)?;
        let transport = transport(&base)?;
        let mut headers = http::HeaderMap::new();
        if let Some(key) = api_key {
            let mut value = http::HeaderValue::from_str(&format!("Bearer {key}"))
                .context("The API key is not a valid header value")?;
            value.set_sensitive(true);
            headers.insert(http::header::AUTHORIZATION, value);
        }
        let config = ClientConfig::new(
            base.parse()
                .with_context(|| format!("{base} is not a usable endpoint"))?,
        )
        .with_default_headers(headers);
        Ok(Self {
            agents: AgentServiceClient::new(transport.clone(), config.clone()),
            deployments: DeploymentServiceClient::new(transport.clone(), config.clone()),
            access: AgentAccessServiceClient::new(transport.clone(), config.clone()),
            chat: TildeChatProviderServiceClient::new(transport, config),
            base,
        })
    }

    pub fn gateway(&self) -> &str {
        &self.base
    }
}

/// Connect failures carry a code and a message; neither the URL nor the credential is echoed.
pub fn failed(method: &str, error: connectrpc::ConnectError) -> anyhow::Error {
    anyhow::anyhow!(
        "{method} failed ({}): {}",
        format!("{:?}", error.code).to_lowercase(),
        error.message.unwrap_or_else(|| "no detail".into())
    )
}
