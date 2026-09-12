//! Typed application environment. Development loads .env before launching this process.
//! One listener serves every route group; ENGINE_SERVE selects which groups a process mounts.
use crate::{encryption::KeyProtection, error::Error};
use clap::ValueEnum;
use envconfig::Envconfig;
use secrecy::{ExposeSecret, SecretString};
use std::{convert::Infallible, net::SocketAddr, str::FromStr};

/// Environment secret with no Debug implementation and zeroizing owned storage.
#[derive(Clone)]
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

/// Route groups mounted by this process. Each group keeps its own authentication;
/// operators who want network isolation run separate processes per group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Serve {
    pub management: bool,
    pub ingress: bool,
    pub runtime: bool,
    pub sidecar: bool,
}
impl Default for Serve {
    fn default() -> Self {
        Self {
            management: true,
            ingress: true,
            runtime: true,
            sidecar: true,
        }
    }
}
impl FromStr for Serve {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut serve = Self {
            management: false,
            ingress: false,
            runtime: false,
            sidecar: false,
        };
        for group in value.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            match group {
                "management" => serve.management = true,
                "ingress" => serve.ingress = true,
                "runtime" => serve.runtime = true,
                "sidecar" => serve.sidecar = true,
                "all" => serve = Self::default(),
                other => {
                    return Err(format!(
                        "ENGINE_SERVE contains unknown group {other:?}; use management, ingress, runtime, sidecar"
                    ));
                }
            }
        }
        Ok(serve)
    }
}
impl std::fmt::Display for Serve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let groups = [
            ("management", self.management),
            ("ingress", self.ingress),
            ("runtime", self.runtime),
            ("sidecar", self.sidecar),
        ];
        let names: Vec<&str> = groups
            .iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| *name)
            .collect();
        write!(f, "{}", names.join(","))
    }
}

/// One typed environment schema. Never derive Debug or serialize this configuration.
#[derive(Envconfig)]
pub struct Config {
    #[envconfig(from = "ENGINE_LISTEN", default = "127.0.0.1:8080")]
    pub listen: SocketAddr,
    #[envconfig(from = "ENGINE_SERVE", default = "all")]
    pub serve: Serve,
    /// Browser-facing origin for management, OAuth callbacks and setup links.
    #[envconfig(from = "ENGINE_PUBLIC_URL")]
    pub public_url: Option<String>,
    /// Origin providers deliver webhooks to when it differs from the public URL.
    #[envconfig(from = "ENGINE_INGRESS_PUBLIC_URL")]
    pub ingress_public_url: Option<String>,
    /// Origin agent hosts call back when it differs from the public URL.
    #[envconfig(from = "ENGINE_RUNTIME_PUBLIC_URL")]
    pub runtime_public_url: Option<String>,
    #[envconfig(from = "ENGINE_S3_BUCKET")]
    pub s3_bucket: Option<String>,
    #[envconfig(from = "ENGINE_S3_REGION", default = "us-east-1")]
    pub s3_region: String,
    #[envconfig(from = "ENGINE_S3_ENDPOINT")]
    pub s3_endpoint: Option<String>,
    #[envconfig(from = "ENGINE_S3_PUBLIC_ENDPOINT")]
    pub s3_public_endpoint: Option<String>,
    #[envconfig(from = "ENGINE_S3_ACCESS_KEY_ID")]
    pub s3_access_key_id: Option<SecretEnv>,
    #[envconfig(from = "ENGINE_S3_SECRET_ACCESS_KEY")]
    pub s3_secret_access_key: Option<SecretEnv>,
    #[envconfig(from = "LOGS_CLICKHOUSE_URL")]
    pub logs_clickhouse_url: Option<String>,
    #[envconfig(from = "LOGS_CLICKHOUSE_DATABASE", default = "tilde_logs")]
    pub logs_clickhouse_database: String,
    #[envconfig(from = "LOGS_CLICKHOUSE_USER", default = "default")]
    pub logs_clickhouse_user: String,
    #[envconfig(from = "LOGS_CLICKHOUSE_PASSWORD")]
    pub logs_clickhouse_password: Option<SecretEnv>,
    #[envconfig(from = "LOGS_OTLP_ENDPOINT")]
    pub logs_otlp_endpoint: Option<String>,
    #[envconfig(from = "LOGS_OTLP_HEADERS")]
    pub logs_otlp_headers: Option<SecretEnv>,
    #[envconfig(from = "LOGS_QUEUE_DIR", default = ".run/log-queue")]
    pub logs_queue_dir: String,
    #[envconfig(from = "LANGFUSE_BASE_URL")]
    pub langfuse_base_url: Option<String>,
    #[envconfig(from = "LANGFUSE_PUBLIC_URL")]
    pub langfuse_public_url: Option<String>,
    #[envconfig(from = "LANGFUSE_PUBLIC_KEY")]
    pub langfuse_public_key: Option<SecretEnv>,
    #[envconfig(from = "LANGFUSE_SECRET_KEY")]
    pub langfuse_secret_key: Option<SecretEnv>,
    #[envconfig(from = "ENGINE_CONNECTION_SETUP_PUBLIC_URL")]
    pub connection_setup_public_url: Option<String>,
    #[envconfig(from = "ENGINE_CONNECTION_UI_DEV_URL")]
    pub connection_ui_dev_url: Option<String>,
    #[envconfig(from = "ENGINE_WEB_ENABLED", default = "true")]
    pub web_enabled: bool,
    #[envconfig(from = "ENGINE_OIDC_ISSUER")]
    pub oidc_issuer: Option<String>,
    #[envconfig(from = "ENGINE_OIDC_CLIENT_ID")]
    pub oidc_client_id: Option<String>,
    #[envconfig(from = "ENGINE_OIDC_CLIENT_SECRET")]
    pub oidc_client_secret: Option<SecretEnv>,
    #[envconfig(from = "ENGINE_OIDC_ALLOW_HTTP", default = "false")]
    pub oidc_allow_http: bool,
    #[envconfig(from = "DATABASE_URL")]
    pub database_url: SecretEnv,
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
    /// Dedicated credentials keep local MinIO configuration separate from AWS KMS credentials.
    pub fn avatar_store(&mut self) -> Result<Option<crate::agent::avatar::AvatarStore>, Error> {
        let Some(bucket) = self.s3_bucket.take().filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        let credentials = match (
            self.s3_access_key_id.take(),
            self.s3_secret_access_key.take(),
        ) {
            (Some(id), Some(secret)) => Some(client_aws_sigv4::Credentials {
                access_key_id: id.0.expose_secret().to_owned(),
                secret_access_key: secret.0.expose_secret().to_owned(),
                session_token: None,
            }),
            (None, None) => None,
            _ => {
                return Err(Error::Invalid(
                    "Supply both S3 access key and secret, or use the AWS credential chain".into(),
                ));
            }
        };
        let endpoint = self
            .s3_endpoint
            .take()
            .unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", self.s3_region));
        crate::agent::avatar::AvatarStore::new(
            bucket,
            self.s3_region.clone(),
            endpoint,
            self.s3_public_endpoint.take(),
            credentials,
        )
        .map(Some)
    }
    pub fn management_enabled(&self) -> bool {
        self.serve.management
    }
    /// Static assets are served only in builds that embed the web app.
    pub fn web_served(&self) -> bool {
        self.web_enabled && cfg!(feature = "embedded-web")
    }
    /// OIDC is required only by the management group, never by agent-facing groups.
    pub fn validate_services(&self) -> Result<(), Error> {
        for origin in [
            &self.public_url,
            &self.ingress_public_url,
            &self.runtime_public_url,
        ]
        .into_iter()
        .flatten()
        {
            crate::network::Boundary::new(self.listen, self.allow_network, vec![origin.clone()])?;
        }
        if self.serve.management {
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
                        "{name} is required when ENGINE_SERVE includes management"
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
        if self.api_port == 0
            || (self.web_enabled && (self.web_port == 0 || self.api_port == self.web_port))
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
                let seed = self.encryption_key.take().ok_or_else(|| Error::Invalid("ENGINE_ENCRYPTION_KEY is required in seed mode; use generate-key once and retain it".into()))?;
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
