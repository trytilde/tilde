//! Typed application environment. Development loads .env before launching this process.
use crate::{encryption::KeyProtection, error::Error};
use clap::ValueEnum;
use envconfig::Envconfig;
use secrecy::{ExposeSecret, SecretString};
use std::{convert::Infallible, net::SocketAddr, str::FromStr};

/// Environment secret with no Debug implementation and zeroizing owned storage.
pub struct SecretEnv(pub SecretString);
impl FromStr for SecretEnv {
    type Err = Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(SecretString::from(value)))
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Backend {
    Seed,
    AwsKms,
}
impl FromStr for Backend {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(value, false)
    }
}

/// Comma-separated browser origins; validated by the network boundary at startup.
pub struct WebOrigins(pub Vec<String>);
impl FromStr for WebOrigins {
    type Err = Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            value
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        ))
    }
}

/// One typed environment schema. Never derive Debug or serialize this configuration.
#[derive(Envconfig)]
pub struct Config {
    /// Exact OTLP/HTTP traces URL, including /v1/traces.
    #[envconfig(from = "ENGINE_TRACING_EXPORT_ENDPOINT")]
    pub tracing_export_endpoint: Option<String>,
    #[envconfig(from = "ENGINE_TRACING_EXPORT_HEADERS")]
    pub tracing_export_headers: Option<SecretEnv>,
    #[envconfig(from = "ENGINE_TRACING_RETENTION_DAYS", default = "7")]
    pub tracing_retention_days: i32,
    #[envconfig(from = "ENGINE_CONNECTION_SETUP_PUBLIC_URL")]
    pub connection_setup_public_url: Option<String>,
    #[envconfig(from = "ENGINE_CONNECTION_UI_DEV_URL")]
    pub connection_ui_dev_url: Option<String>,
    #[envconfig(from = "ENGINE_MANAGEMENT_ENABLED", default = "true")]
    pub management_enabled: bool,
    #[envconfig(from = "ENGINE_WEB_ENABLED", default = "true")]
    pub web_enabled: bool,
    #[envconfig(from = "ENGINE_AGENT_RUNTIME_LISTEN", default = "127.0.0.1:8081")]
    pub agent_runtime_listen: SocketAddr,
    #[envconfig(from = "ENGINE_AGENT_RUNTIME_PUBLIC_URL")]
    pub agent_runtime_public_url: Option<String>,
    #[envconfig(from = "ENGINE_OIDC_ISSUER")]
    pub oidc_issuer: Option<String>,
    #[envconfig(from = "ENGINE_OIDC_CLIENT_ID")]
    pub oidc_client_id: Option<String>,
    #[envconfig(from = "ENGINE_OIDC_CLIENT_SECRET")]
    pub oidc_client_secret: Option<SecretEnv>,
    #[envconfig(from = "ENGINE_OIDC_ALLOW_HTTP", default = "false")]
    pub oidc_allow_http: bool,
    #[envconfig(from = "ENGINE_MANAGEMENT_PUBLIC_URL")]
    pub management_public_url: Option<String>,
    #[envconfig(from = "DATABASE_URL")]
    pub database_url: SecretEnv,
    #[envconfig(from = "ENGINE_MANAGEMENT_LISTEN", default = "127.0.0.1:8080")]
    pub management_listen: SocketAddr,
    #[envconfig(from = "ENGINE_ALLOW_NETWORK", default = "false")]
    pub allow_network: bool,
    #[envconfig(from = "ENGINE_WEB_ORIGINS", default = "")]
    pub web_origins: WebOrigins,
    #[envconfig(from = "ENGINE_ENCRYPTION_BACKEND", default = "seed")]
    pub encryption_backend: Backend,
    #[envconfig(from = "ENGINE_ENCRYPTION_KEY")]
    pub encryption_key: Option<SecretEnv>,
    #[envconfig(from = "ENGINE_KMS_KEY_ID")]
    pub kms_key_id: Option<String>,
    #[envconfig(from = "AWS_REGION")]
    pub aws_region: Option<String>,
    #[envconfig(from = "AWS_DEFAULT_REGION")]
    pub aws_default_region: Option<String>,
    #[envconfig(from = "RUST_LOG", default = "tilde=info")]
    pub log_filter: tracing_subscriber::EnvFilter,
    #[envconfig(from = "API_PORT", default = "8080")]
    pub api_port: u16,
    #[envconfig(from = "WEB_PORT", default = "5173")]
    pub web_port: u16,
}

impl Config {
    /// Static assets share the management bind only in builds that embed the web app.
    pub fn management_listener_enabled(&self) -> bool {
        self.management_enabled || (self.web_enabled && cfg!(feature = "embedded-web"))
    }
    /// OIDC is required only by the enabled management API, never by the agent runtime API.
    pub fn validate_services(&self) -> Result<(), Error> {
        if !(1..=365).contains(&self.tracing_retention_days) {
            return Err(Error::Invalid(
                "ENGINE_TRACING_RETENTION_DAYS must be between 1 and 365".into(),
            ));
        }
        if self.management_enabled {
            for (name, value) in [
                ("ENGINE_OIDC_ISSUER", self.oidc_issuer.as_deref()),
                ("ENGINE_OIDC_CLIENT_ID", self.oidc_client_id.as_deref()),
                (
                    "ENGINE_OIDC_CLIENT_SECRET",
                    self.oidc_client_secret
                        .as_ref()
                        .map(|s| s.0.expose_secret()),
                ),
            ] {
                if value.is_none_or(|v| v.trim().is_empty()) {
                    return Err(Error::Invalid(format!(
                        "{name} is required when ENGINE_MANAGEMENT_ENABLED=true"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Validate configuration and consume wrapping-key material without making network calls.
    pub fn key_protection(&mut self) -> Result<KeyProtection, Error> {
        self.validate_services()?;
        if self.database_url.0.expose_secret().trim().is_empty() {
            return Err(Error::Invalid("DATABASE_URL is required".into()));
        }
        if (self.management_enabled && self.api_port == 0)
            || (self.web_enabled && self.web_port == 0)
            || (self.management_enabled && self.web_enabled && self.api_port == self.web_port)
        {
            return Err(Error::Invalid(
                "API_PORT and WEB_PORT must be distinct ports from 1 to 65535".into(),
            ));
        }
        match self.encryption_backend {
            Backend::Seed => {
                if self
                    .kms_key_id
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
                {
                    return Err(Error::Invalid(
                        "ENGINE_KMS_KEY_ID is set but seed mode is selected".into(),
                    ));
                }
                let seed = self.encryption_key.take().ok_or_else(|| Error::Invalid(
                    "ENGINE_ENCRYPTION_KEY is required in seed mode; use generate-key once and retain it".into()))?;
                KeyProtection::seed(seed.0)
            }
            Backend::AwsKms => {
                if self
                    .encryption_key
                    .as_ref()
                    .is_some_and(|value| !value.0.expose_secret().is_empty())
                {
                    return Err(Error::Invalid(
                        "ENGINE_ENCRYPTION_KEY must not be set in aws-kms mode".into(),
                    ));
                }
                let key_id = self.kms_key_id.take().ok_or_else(|| {
                    Error::Invalid("ENGINE_KMS_KEY_ID is required in aws-kms mode".into())
                })?;
                let region = self
                    .aws_region
                    .take()
                    .or_else(|| self.aws_default_region.take())
                    .ok_or_else(|| {
                        Error::Invalid(
                            "AWS_REGION or AWS_DEFAULT_REGION is required in aws-kms mode".into(),
                        )
                    })?;
                KeyProtection::kms(
                    client_aws_kms::Client::new(region).map_err(|_| {
                        Error::Invalid("Invalid AWS KMS region or client configuration".into())
                    })?,
                    key_id,
                )
            }
        }
    }
}
