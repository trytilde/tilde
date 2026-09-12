//! Parameterized ClickHouse reads and the existing Rotel RowBinary logs exporter.
use crate::{
    config::{Config, SecretEnv},
    error::Error,
};
use opentelemetry_proto::tonic::logs::v1::ResourceLogs;
use rotel::{
    bounded_channel::{BoundedSender, bounded},
    exporters::clickhouse::{ClickhouseExporterConfigBuilder, RetryConfig},
    topology::payload::{ForwarderAcknowledgement, ForwarderMetadata, Message, MessageMetadata},
};
use secrecy::ExposeSecret;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
pub type ExporterTask = (
    BoundedSender<Vec<Message<ResourceLogs>>>,
    tokio::task::JoinHandle<()>,
);
#[derive(Clone)]
pub struct Store {
    pub url: url::Url,
    pub database: String,
    user: String,
    password: Option<SecretEnv>,
    client: reqwest::Client,
}
impl Store {
    pub fn from_config(config: &Config) -> Result<Option<Self>, Error> {
        let Some(endpoint) = config
            .logs_clickhouse_url
            .as_ref()
            .filter(|v| !v.trim().is_empty())
        else {
            return Ok(None);
        };
        let url = url::Url::parse(endpoint)
            .map_err(|_| Error::Invalid("Invalid LOGS_CLICKHOUSE_URL".into()))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::Invalid(
                "ClickHouse logs URL must be HTTP(S) without credentials, query, or fragment"
                    .into(),
            ));
        }
        if config.logs_clickhouse_database.is_empty()
            || !config
                .logs_clickhouse_database
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(Error::Invalid("Invalid logs database identifier".into()));
        }
        Ok(Some(Self {
            url,
            database: config.logs_clickhouse_database.clone(),
            user: config.logs_clickhouse_user.clone(),
            password: config.logs_clickhouse_password.clone(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| Error::Invalid("Unable to configure ClickHouse client".into()))?,
        }))
    }
    pub fn identity(&self) -> String {
        format!("{}|{}|{}", self.url, self.database, self.user)
    }
    pub async fn query(
        &self,
        sql: &str,
        parameters: &[(String, String)],
    ) -> Result<serde_json::Value, Error> {
        let response = self
            .client
            .post(self.url.clone())
            .basic_auth(
                &self.user,
                self.password.as_ref().map(|p| p.0.expose_secret()),
            )
            .query(&[("database", &self.database)])
            .query(parameters)
            .body(sql.to_owned())
            .send()
            .await
            .map_err(|_| Error::Invalid("ClickHouse logs unavailable".into()))?;
        if !response.status().is_success() {
            return Err(Error::Invalid("ClickHouse logs query failed".into()));
        }
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| Error::Invalid("ClickHouse logs response failed".into()))?
        {
            if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                return Err(Error::Invalid(
                    "Log query response limit exceeded; narrow the time range".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| Error::Invalid("Invalid ClickHouse logs response".into()))
    }
    pub async fn ready(&self) -> Result<(), Error> {
        self.query(include_str!("../../../../queries/logs/ready.sql"), &[])
            .await
            .map(|_| ())
    }
    pub async fn initialize(&self) -> Result<(), Error> {
        self.query(include_str!("../../../../queries/logs/schema.sql"), &[])
            .await?;
        self.ready().await
    }
    pub fn exporter(&self, cancel: CancellationToken) -> Result<ExporterTask, Error> {
        rotel::crypto::init_crypto_provider()
            .map_err(|_| Error::Invalid("Unable to initialize exporter TLS".into()))?;
        let (tx, rx) = bounded(2);
        let mut builder = ClickhouseExporterConfigBuilder::new(
            self.url.to_string(),
            self.database.clone(),
            "otel".into(),
            RetryConfig::new(
                Duration::from_millis(200),
                Duration::from_secs(1),
                Duration::from_secs(3),
                false,
            ),
        )
        .with_user(self.user.clone())
        .with_async_insert(true);
        if let Some(password) = &self.password {
            builder = builder.with_password(password.0.expose_secret().to_owned());
        }
        let exporter = builder
            .build()
            .map_err(|_| Error::Invalid("Invalid ClickHouse exporter".into()))?
            .build_logs_exporter(rx, None)
            .map_err(|_| Error::Invalid("Invalid logs exporter".into()))?;
        Ok((
            tx,
            tokio::spawn(async move {
                let _ = exporter.start(cancel).await;
            }),
        ))
    }
}
pub async fn export(
    sender: &BoundedSender<Vec<Message<ResourceLogs>>>,
    id: &str,
    logs: Vec<ResourceLogs>,
) -> bool {
    let (tx, mut rx) = bounded(1);
    let meta = MessageMetadata::forwarder(ForwarderMetadata::new(id.to_owned(), Some(tx)));
    let send = async {
        sender
            .send(vec![Message::new(Some(meta), logs, None)])
            .await
            .ok()?;
        rx.next().await
    };
    matches!(
        tokio::time::timeout(Duration::from_secs(15), send).await,
        Ok(Some(ForwarderAcknowledgement::Ack(_)))
    )
}
