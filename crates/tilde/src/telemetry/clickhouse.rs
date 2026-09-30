//! The telemetry ClickHouse database: parameterized reads, schema management and Rotel's
//! RowBinary exporters. Logs, spans and metrics share it.
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
    retention_days: Option<u32>,
    client: reqwest::Client,
}
impl Store {
    /// ClickHouse is required: it holds every log, span and metric the engine keeps.
    pub fn from_config(config: &Config) -> Result<Self, Error> {
        let Some(endpoint) = config
            .clickhouse_url
            .as_ref()
            .filter(|v| !v.trim().is_empty())
        else {
            return Err(Error::Invalid("ENGINE_CLICKHOUSE_URL is required".into()));
        };
        let url = url::Url::parse(endpoint)
            .map_err(|_| Error::Invalid("Invalid ENGINE_CLICKHOUSE_URL".into()))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::Invalid(
                "ClickHouse URL must be HTTP(S) without credentials, query, or fragment".into(),
            ));
        }
        if config.clickhouse_database.is_empty()
            || !config
                .clickhouse_database
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(Error::Invalid(
                "Invalid ClickHouse database identifier".into(),
            ));
        }
        Ok(Self {
            url,
            database: config.clickhouse_database.clone(),
            user: config.clickhouse_user.clone(),
            password: config.clickhouse_password.clone(),
            retention_days: config.clickhouse_retention_days,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| Error::Invalid("Unable to configure ClickHouse client".into()))?,
        })
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
        self.query(include_str!("../../../../queries/_logs/ready.sql"), &[])
            .await
            .map(|_| ())
    }
    /// The HTTP interface takes one statement per request.
    pub(crate) async fn execute_all(&self, sql: &str) -> Result<(), Error> {
        for statement in sql.split(";\n").map(str::trim).filter(|s| !s.is_empty()) {
            self.query(statement, &[]).await?;
        }
        Ok(())
    }
    pub async fn initialize(&self) -> Result<(), Error> {
        self.query(include_str!("../../../../queries/_logs/schema.sql"), &[])
            .await?;
        self.apply_retention(&["otel_logs"]).await?;
        self.ready().await
    }
    /// Retention is an operator setting, not part of the schema, so it is applied to existing
    /// tables on every start: a TTL for a bounded window, none to keep everything. A table that
    /// already keeps everything is left alone; any other failure aborts initialization, so an
    /// old TTL cannot silently keep deleting history under an indefinite setting.
    pub(crate) async fn apply_retention(&self, tables: &[&str]) -> Result<(), Error> {
        for table in tables {
            let time = if table.starts_with("otel_metrics") {
                "TimeUnix"
            } else {
                "Timestamp"
            };
            let statement = match self.retention_days {
                Some(days) => format!(
                    "ALTER TABLE {table} MODIFY TTL toDateTime({time}) + INTERVAL {days} DAY DELETE"
                ),
                None if self.has_ttl(table).await? => format!("ALTER TABLE {table} REMOVE TTL"),
                None => continue,
            };
            self.query(&statement, &[]).await?;
        }
        Ok(())
    }
    pub(crate) async fn has_ttl(&self, table: &str) -> Result<bool, Error> {
        let rows = self
            .query(
                include_str!("../../../../queries/_logs/ttl.sql"),
                &[("param_table".into(), table.into())],
            )
            .await?;
        Ok(rows["data"][0]["has_ttl"]
            .as_u64()
            .or_else(|| rows["data"][0]["has_ttl"].as_str()?.parse().ok())
            == Some(1))
    }
    /// Rotel's ClickHouse exporter for this database; logs and spans each build their own from it.
    pub(crate) fn exporter_builder(
        &self,
    ) -> Result<rotel::exporters::clickhouse::ClickhouseExporterBuilder, Error> {
        rotel::crypto::init_crypto_provider()
            .map_err(|_| Error::Invalid("Unable to initialize exporter TLS".into()))?;
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
        builder
            .build()
            .map_err(|_| Error::Invalid("Invalid ClickHouse exporter".into()))
    }
    pub fn exporter(&self, cancel: CancellationToken) -> Result<ExporterTask, Error> {
        let (tx, rx) = bounded(2);
        let exporter = self
            .exporter_builder()?
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
/// Hands one batch to a Rotel ClickHouse exporter and waits for its acknowledgement.
pub async fn export<T>(sender: &BoundedSender<Vec<Message<T>>>, id: &str, items: Vec<T>) -> bool {
    let (tx, mut rx) = bounded(1);
    let meta = MessageMetadata::forwarder(ForwarderMetadata::new(id.to_owned(), Some(tx)));
    let send = async {
        sender
            .send(vec![Message::new(Some(meta), items, None)])
            .await
            .ok()?;
        rx.next().await
    };
    matches!(
        tokio::time::timeout(Duration::from_secs(15), send).await,
        Ok(Some(ForwarderAcknowledgement::Ack(_)))
    )
}
