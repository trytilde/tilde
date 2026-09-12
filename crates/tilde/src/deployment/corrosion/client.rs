use crate::chat::ChatError;
use futures::Stream;
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{pin::Pin, time::Duration};
type Result<T> = std::result::Result<T, ChatError>;
pub type Changes<T> = Pin<Box<dyn Stream<Item = Result<Change<T>>> + Send>>;
#[derive(Clone)]
pub struct Client {
    endpoint: url::Url,
    token: SecretString,
    http: reqwest::Client,
}
pub struct Change<T> {
    pub value: T,
    pub deleted: bool,
}
/// SQL transport values are parameter bindings, never persisted untyped metadata.
pub fn statement(sql: &str, parameters: Vec<Value>) -> Value {
    json!([sql, parameters])
}
impl Client {
    pub(crate) fn at(&self, endpoint: &str) -> Result<Self> {
        Self::new(endpoint, self.token.clone())
    }
    pub fn new(endpoint: &str, token: SecretString) -> Result<Self> {
        let endpoint = url::Url::parse(&format!("{}/", endpoint.trim_end_matches('/')))
            .map_err(|_| ChatError::Invalid("Invalid Corrosion endpoint".into()))?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err(ChatError::Invalid("Invalid Corrosion endpoint".into()));
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ChatError::Transport)?;
        Ok(Self {
            endpoint,
            token,
            http,
        })
    }
    fn request(&self, path: &str) -> Result<reqwest::RequestBuilder> {
        Ok(self
            .http
            .post(
                self.endpoint
                    .join(path.trim_start_matches('/'))
                    .map_err(|_| ChatError::Transport)?,
            )
            .bearer_auth(self.token.expose_secret()))
    }
    pub async fn transaction(&self, statements: Vec<Value>) -> Result<Vec<u64>> {
        let response = self
            .request("/v1/transactions")?
            .timeout(Duration::from_secs(30))
            .json(&statements)
            .send()
            .await
            .map_err(|_| ChatError::Transport)?;
        if !response.status().is_success() {
            return Err(ChatError::Transport);
        }
        let body: Value = response.json().await.map_err(|_| ChatError::Transport)?;
        let results = body
            .get("results")
            .and_then(Value::as_array)
            .ok_or(ChatError::Transport)?;
        results
            .iter()
            .map(|r| {
                r.get("rows_affected")
                    .and_then(Value::as_u64)
                    .ok_or(ChatError::Conflict)
            })
            .collect()
    }
    pub async fn query<T: DeserializeOwned>(
        &self,
        sql: &str,
        parameters: Vec<Value>,
    ) -> Result<Vec<T>> {
        let response = self
            .request("/v1/queries")?
            .timeout(Duration::from_secs(30))
            .json(&statement(sql, parameters))
            .send()
            .await
            .map_err(|_| ChatError::Transport)?;
        if !response.status().is_success() {
            return Err(ChatError::Transport);
        }
        let text = response.text().await.map_err(|_| ChatError::Transport)?;
        let mut columns = vec![];
        let mut out = vec![];
        for line in text.lines() {
            let event: Value = serde_json::from_str(line).map_err(|_| ChatError::Transport)?;
            if let Some(names) = event.get("columns") {
                columns =
                    serde_json::from_value(names.clone()).map_err(|_| ChatError::Transport)?;
            }
            if let Some(row) = event.get("row").and_then(Value::as_array) {
                out.push(decode(&columns, row.get(1).ok_or(ChatError::Transport)?)?);
            }
            if event.get("error").is_some() {
                return Err(ChatError::Transport);
            }
        }
        Ok(out)
    }
    /// Include a non-primary-key column in subscriptions: the pinned Corrosion
    /// matcher tracks changed columns and can miss primary-key-only inserts.
    /// Each new subscription includes a snapshot, making reconnect recovery an
    /// idempotent replay by domain event ID, independent of transport query IDs.
    pub async fn subscribe<T: DeserializeOwned + Send + 'static>(
        &self,
        sql: &str,
        parameters: Vec<Value>,
    ) -> Result<Changes<T>> {
        let mut response = self
            .request("/v1/subscriptions")?
            .json(&statement(sql, parameters))
            .send()
            .await
            .map_err(|_| ChatError::Transport)?;
        if !response.status().is_success() {
            return Err(ChatError::Transport);
        }
        Ok(Box::pin(async_stream::try_stream! {
            let mut pending=Vec::new();let mut columns:Vec<String>=vec![];
            while let Some(chunk)=response.chunk().await.map_err(|_|ChatError::Transport)?{
                pending.extend_from_slice(&chunk);
                if pending.len()>32*1024*1024 {Err(ChatError::Invalid("Corrosion subscription record exceeds limit".into()))?;}
                while let Some(end)=pending.iter().position(|b|*b==b'\n'){
                    let bytes:Vec<u8>=pending.drain(..=end).collect();
                    let event:Value=serde_json::from_slice(&bytes).map_err(|_|ChatError::Transport)?;
                    if let Some(names)=event.get("columns"){columns=serde_json::from_value(names.clone()).map_err(|_|ChatError::Transport)?;}
                    if let Some(row)=event.get("row").and_then(Value::as_array){yield Change{value:decode(&columns,row.get(1).ok_or(ChatError::Transport)?)?,deleted:false};}
                    if let Some(row)=event.get("change").and_then(Value::as_array){yield Change{value:decode(&columns,row.get(2).ok_or(ChatError::Transport)?)?,deleted:row.first().and_then(Value::as_str)==Some("delete")};}
                    if event.get("error").is_some(){Err(ChatError::Transport)?;}
                }
            }
        }))
    }
}
fn decode<T: DeserializeOwned>(columns: &[String], values: &Value) -> Result<T> {
    let values = values.as_array().ok_or(ChatError::Transport)?;
    if values.len() != columns.len() {
        return Err(ChatError::Transport);
    }
    serde_json::from_value(Value::Object(
        columns
            .iter()
            .cloned()
            .zip(values.iter().cloned())
            .collect(),
    ))
    .map_err(|_| ChatError::Transport)
}

impl Client {
    /// Startup barrier against a reachable peer's replication vector. The pinned
    /// Corrosion release tracks complete versions and missing ranges separately.
    /// This is a bounded readiness probe, not the runtime event delivery path.
    pub async fn catch_up(&self, peer: &Client) -> Result<()> {
        #[derive(serde::Deserialize, serde::Serialize)]
        struct Version {
            site: String,
            version: i64,
        }
        #[derive(serde::Deserialize)]
        struct Ready {
            ready: i64,
        }
        let versions = peer
            .query::<Version>(
                "SELECT hex(site_id) AS site,db_version AS version FROM crsql_db_versions",
                vec![],
            )
            .await?;
        let versions = serde_json::to_string(&versions).map_err(|_| ChatError::Transport)?;
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                let state = self.query::<Ready>(
                    "SELECT NOT EXISTS(SELECT 1 FROM json_each(?) wanted WHERE NOT EXISTS(SELECT 1 FROM crsql_db_versions local WHERE hex(local.site_id)=json_extract(wanted.value,'$.site') AND local.db_version>=json_extract(wanted.value,'$.version')) OR EXISTS(SELECT 1 FROM __corro_bookkeeping_gaps gap WHERE hex(gap.actor_id)=json_extract(wanted.value,'$.site') AND gap.start<=json_extract(wanted.value,'$.version')) OR EXISTS(SELECT 1 FROM __corro_seq_bookkeeping partial WHERE hex(partial.site_id)=json_extract(wanted.value,'$.site') AND partial.db_version<=json_extract(wanted.value,'$.version'))) AS ready", vec![json!(versions)]).await?;
                if state.first().is_some_and(|r| r.ready != 0) { return Ok(()); }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }).await.map_err(|_| ChatError::Transport)?
    }
}
