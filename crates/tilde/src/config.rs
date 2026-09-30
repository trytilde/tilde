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
/// Telemetry queues its batches in the log, trace and metric buckets, so those are required.
pub struct Buckets {
    pub avatars: Option<crate::agent::avatar::ObjectStore>,
    pub logs: crate::agent::avatar::ObjectStore,
    pub traces: crate::agent::avatar::ObjectStore,
    pub metrics: crate::agent::avatar::ObjectStore,
    pub media: Option<crate::agent::avatar::ObjectStore>,
    pub skills: Option<crate::agent::avatar::ObjectStore>,
}

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
    #[envconfig(from = "ENGINE_CLICKHOUSE_URL")]
    pub clickhouse_url: Option<String>,
    #[envconfig(from = "ENGINE_CLICKHOUSE_DATABASE", default = "tilde_telemetry")]
    pub clickhouse_database: String,
    #[envconfig(from = "ENGINE_CLICKHOUSE_USER", default = "default")]
    pub clickhouse_user: String,
    #[envconfig(from = "ENGINE_CLICKHOUSE_PASSWORD")]
    pub clickhouse_password: Option<SecretEnv>,
    /// Days of log, span and metric history to keep. Unset keeps everything.
    #[envconfig(from = "ENGINE_CLICKHOUSE_RETENTION_DAYS")]
    pub clickhouse_retention_days: Option<u32>,
    #[envconfig(from = "LOGS_OTLP_ENDPOINT")]
    pub logs_otlp_endpoint: Option<String>,
    #[envconfig(from = "LOGS_OTLP_HEADERS")]
    pub logs_otlp_headers: Option<SecretEnv>,
    /// Forward every trace batch to an external OTLP/HTTP collector, independently of the span store.
    #[envconfig(from = "TRACES_OTLP_ENDPOINT")]
    pub traces_otlp_endpoint: Option<String>,
    #[envconfig(from = "TRACES_OTLP_HEADERS")]
    pub traces_otlp_headers: Option<SecretEnv>,
    /// Buckets that hold accepted telemetry until it is delivered. They share ENGINE_S3_*
    /// endpoint and credentials with avatars and are separate so each can carry its own
    /// lifecycle and access policy: the engine never deletes a queued batch, so each queue
    /// bucket needs an expiry rule of at least a day.
    #[envconfig(from = "ENGINE_LOGS_S3_BUCKET")]
    pub logs_s3_bucket: Option<String>,
    #[envconfig(from = "ENGINE_TRACES_S3_BUCKET")]
    pub traces_s3_bucket: Option<String>,
    #[envconfig(from = "ENGINE_METRICS_S3_BUCKET")]
    pub metrics_s3_bucket: Option<String>,
    /// Media and oversized span payloads, referenced by stored spans for as long as they are
    /// retained; never given a queue-style expiry. Unset keeps both inline in the span.
    #[envconfig(from = "ENGINE_MEDIA_S3_BUCKET")]
    pub media_s3_bucket: Option<String>,
    /// Skill files, content-addressed; kept as long as any skill version references them.
    /// Unset allows only text files small enough to live in Postgres.
    #[envconfig(from = "ENGINE_SKILLS_S3_BUCKET")]
    pub skills_s3_bucket: Option<String>,
    /// Git skill sources are read through the GitHub REST API. A token raises the anonymous
    /// rate limit and reaches private repositories.
    #[envconfig(from = "ENGINE_GITHUB_API_URL", default = "https://api.github.com")]
    pub github_api_url: String,
    #[envconfig(
        from = "ENGINE_GITHUB_RAW_URL",
        default = "https://raw.githubusercontent.com"
    )]
    pub github_raw_url: String,
    #[envconfig(from = "ENGINE_GITHUB_TOKEN")]
    pub github_token: Option<SecretEnv>,
    #[envconfig(from = "METRICS_OTLP_ENDPOINT")]
    pub metrics_otlp_endpoint: Option<String>,
    #[envconfig(from = "METRICS_OTLP_HEADERS")]
    pub metrics_otlp_headers: Option<SecretEnv>,
    /// Live token price sheet, refreshed daily; empty disables and keeps the vendored copy.
    #[envconfig(
        from = "ENGINE_INFERENCE_PRICES_URL",
        default = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json"
    )]
    pub inference_prices_url: String,
    #[envconfig(from = "ENGINE_CONNECTION_SETUP_PUBLIC_URL")]
    pub connection_setup_public_url: Option<String>,
    #[envconfig(from = "ENGINE_CONNECTION_UI_DEV_URL")]
    pub connection_ui_dev_url: Option<String>,
    #[envconfig(from = "ENGINE_WEB_ENABLED", default = "true")]
    pub web_enabled: bool,
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
    /// One S3 client per configured bucket, sharing endpoint and credentials. Reads only, so
    /// `validate_services` still sees every bucket name whichever order the two are called in.
    pub fn buckets(&self) -> Result<Buckets, Error> {
        let named = |value: &Option<String>| value.clone().filter(|s| !s.trim().is_empty());
        let required = |value: &Option<String>, name: &str| {
            named(value).ok_or_else(|| Error::Invalid(format!("{name} is required")))
        };
        let (avatars, media, skills) = (
            named(&self.s3_bucket),
            named(&self.media_s3_bucket),
            named(&self.skills_s3_bucket),
        );
        let logs = required(&self.logs_s3_bucket, "ENGINE_LOGS_S3_BUCKET")?;
        let traces = required(&self.traces_s3_bucket, "ENGINE_TRACES_S3_BUCKET")?;
        let metrics = required(&self.metrics_s3_bucket, "ENGINE_METRICS_S3_BUCKET")?;
        let credentials = match (&self.s3_access_key_id, &self.s3_secret_access_key) {
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
            .clone()
            .unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", self.s3_region));
        let base = crate::agent::avatar::ObjectStore::new(
            logs.clone(),
            self.s3_region.clone(),
            endpoint,
            self.s3_public_endpoint.clone(),
            credentials,
        )?;
        Ok(Buckets {
            avatars: avatars.map(|name| base.bucket(name)),
            logs: base.bucket(logs),
            traces: base.bucket(traces),
            metrics: base.bucket(metrics),
            media: media.map(|name| base.bucket(name)),
            skills: skills.map(|name| base.bucket(name)),
        })
    }
    pub fn management_enabled(&self) -> bool {
        self.serve.management
    }
    /// Static assets are served only in builds that embed the web app.
    pub fn web_served(&self) -> bool {
        self.web_enabled && cfg!(feature = "embedded-web")
    }
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
        let set = |value: &Option<String>| value.as_deref().is_some_and(|v| !v.trim().is_empty());
        if !set(&self.clickhouse_url) {
            return Err(Error::Invalid("ENGINE_CLICKHOUSE_URL is required".into()));
        }
        for (name, bucket) in [
            ("ENGINE_LOGS_S3_BUCKET", &self.logs_s3_bucket),
            ("ENGINE_METRICS_S3_BUCKET", &self.metrics_s3_bucket),
            ("ENGINE_TRACES_S3_BUCKET", &self.traces_s3_bucket),
        ] {
            if !set(bucket) {
                return Err(Error::Invalid(format!("{name} is required")));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Start-up validates services after building bucket clients; both must see the same names.
    #[test]
    fn a_configured_telemetry_stack_validates_in_either_order() {
        let config = || {
            Config::init_from_hashmap(&HashMap::from(
                [
                    ("DATABASE_URL", "postgres://unused"),
                    ("ENGINE_SERVE", "runtime"),
                    (
                        "ENGINE_ENCRYPTION_KEY",
                        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    ),
                    ("ENGINE_CLICKHOUSE_URL", "http://127.0.0.1:8123"),
                    ("TRACES_OTLP_ENDPOINT", "http://127.0.0.1:4318/v1/traces"),
                    ("ENGINE_S3_ENDPOINT", "http://127.0.0.1:9000"),
                    ("ENGINE_LOGS_S3_BUCKET", "logs"),
                    ("ENGINE_TRACES_S3_BUCKET", "traces"),
                    ("ENGINE_METRICS_S3_BUCKET", "metrics"),
                    ("ENGINE_MEDIA_S3_BUCKET", "media"),
                ]
                .map(|(k, v)| (k.to_owned(), v.to_owned())),
            ))
            .unwrap()
        };
        let mut first = config();
        first.key_protection().unwrap();
        assert!(first.buckets().unwrap().media.is_some());
        let mut second = config();
        second.buckets().unwrap();
        second.key_protection().unwrap();
        let mut missing = config();
        missing.traces_s3_bucket = None;
        assert!(missing.key_protection().is_err());
    }
}
