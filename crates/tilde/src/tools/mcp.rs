//! MCP servers as tool sources. A connection type whose `mcp` is set serves its tools from that
//! MCP server: a curated one (a catalog provider such as Notion, see `mcp_catalog`) or one an
//! admin registered as a provider of its own. The connection's credential reaches the server as
//! the type's `McpCredential` says. Tools are discovered from the server into `connection_tools`,
//! used by agents like any provider tool, and called through Tilde over MCP's
//! streamable HTTP transport. Only public addresses are dialed, with DNS pinned per request.
//! Each operation opens its own MCP session; sessions are not cached yet, which costs two extra
//! round trips per call. Managed providers that front an official MCP server (AWS) reuse this
//! client with a fixed URL and per-request SigV4 signing.
use super::db;
use crate::chat::tools::ToolResult;
use crate::connections::{
    catalog::Endpoints,
    model::{self, Values, invalid, value},
};
use crate::database::Pool;
use crate::error::Error;
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const PROTOCOL_VERSION: &str = "2025-06-18";

/// One MCP-served connection: the connection, its provider and the server its type names.
pub(crate) struct Target {
    pub connection: Uuid,
    pub provider: String,
    pub server: model::McpServer,
}
/// Bounds any one response, `tools/list` pages included; a call's output is held to
/// `tools::MAX_OUTPUT` when it is audited.
const MAX_RESPONSE: usize = 4 * 1024 * 1024;

/// An HTTP client for one MCP server: HTTPS to public addresses, pinned per request. Isolated
/// tests serve MCP from loopback (`mcp_private_origin`); production has no such override.
async fn client(url: &url::Url, endpoints: &Endpoints) -> ToolResult<reqwest::Client> {
    if endpoints.0.get("mcp_private_origin").map(String::as_str)
        == Some(url.origin().ascii_serialization().as_str())
    {
        return reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|_| ConnectError::internal("Transport failed"));
    }
    if url.scheme() != "https" {
        return Err(ConnectError::invalid_argument("MCP servers must use https"));
    }
    crate::chat::providers::files::public_client(url, std::time::Duration::from_secs(120)).await
}
struct Session {
    client: reqwest::Client,
    url: url::Url,
    auth: Option<(reqwest::header::HeaderName, reqwest::header::HeaderValue)>,
    signer: Option<client_aws_sigv4::Signer>,
    id: Option<String>,
    next: u64,
}
impl Session {
    async fn open(
        server: &model::McpServer,
        values: &Values,
        endpoints: &Endpoints,
    ) -> ToolResult<Self> {
        let mut url = url::Url::parse(&server.url)
            .map_err(|_| ConnectError::invalid_argument("Invalid MCP server URL"))?;
        let client = client(&url, endpoints).await?;
        let header = |name: &str, secret: &str| -> ToolResult<_> {
            let mut value = reqwest::header::HeaderValue::from_str(secret)
                .map_err(|_| ConnectError::invalid_argument("Invalid MCP credential"))?;
            value.set_sensitive(true);
            Ok(Some((
                name.parse()
                    .map_err(|_| ConnectError::invalid_argument("Invalid auth header name"))?,
                value,
            )))
        };
        let auth = match &server.credential {
            model::McpCredential::None => None,
            model::McpCredential::Bearer => header(
                "authorization",
                &format!(
                    "Bearer {}",
                    value(values, "access_token").or_else(|_| value(values, "api_key"))?
                ),
            )?,
            model::McpCredential::Header { name, prefix } => {
                header(name, &format!("{prefix}{}", value(values, "api_key")?))?
            }
            model::McpCredential::Query { name } => {
                url.query_pairs_mut()
                    .append_pair(name, value(values, "api_key")?);
                None
            }
        };
        Self::start(client, url, auth, None).await
    }
    async fn start(
        client: reqwest::Client,
        url: url::Url,
        auth: Option<(reqwest::header::HeaderName, reqwest::header::HeaderValue)>,
        signer: Option<client_aws_sigv4::Signer>,
    ) -> ToolResult<Self> {
        let mut session = Self {
            client,
            url,
            auth,
            signer,
            id: None,
            next: 1,
        };
        session
            .request("initialize", json!({"protocolVersion":PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"tilde","version":env!("CARGO_PKG_VERSION")}}))
            .await?;
        session
            .send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await?;
        Ok(session)
    }
    async fn send(&mut self, body: &Value) -> ToolResult<reqwest::Response> {
        let body =
            serde_json::to_vec(body).map_err(|_| ConnectError::internal("Invalid MCP message"))?;
        let mut request = self
            .client
            .post(self.url.clone())
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .header("mcp-protocol-version", PROTOCOL_VERSION)
            .body(body.clone());
        if let Some((name, value)) = &self.auth {
            request = request.header(name, value);
        }
        if let Some(id) = &self.id {
            request = request.header("mcp-session-id", id);
        }
        let mut request = request
            .build()
            .map_err(|_| ConnectError::internal("Invalid MCP request"))?;
        if let Some(signer) = &self.signer {
            signer
                .sign_request_at(&mut request, &body, std::time::SystemTime::now())
                .map_err(|_| ConnectError::invalid_argument("The request could not be signed"))?;
        }
        let response = self
            .client
            .execute(request)
            .await
            .map_err(|_| ConnectError::unavailable("MCP server is unreachable"))?;
        if matches!(response.status().as_u16(), 401 | 403) {
            return Err(Error::ConnectionAuthorizationRequired.into());
        }
        if !response.status().is_success() {
            return Err(ConnectError::unavailable("MCP server rejected the request"));
        }
        if let Some(id) = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            self.id = Some(id.to_owned());
        }
        Ok(response)
    }
    /// One JSON-RPC request. The server answers with JSON, or with an event stream in which
    /// the response is the message carrying our ID.
    async fn request(&mut self, method: &str, params: Value) -> ToolResult<Value> {
        let id = self.next;
        self.next += 1;
        let mut response = self
            .send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        let stream = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|t| t.starts_with("text/event-stream"));
        let mut bytes = Vec::new();
        let mut answer = None;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ConnectError::unavailable("MCP response was interrupted"))?
        {
            if bytes.len() + chunk.len() > MAX_RESPONSE {
                return Err(ConnectError::resource_exhausted(
                    "MCP response is too large",
                ));
            }
            bytes.extend_from_slice(&chunk);
            if stream {
                // Stop at our response instead of waiting for the server to close the stream.
                answer = String::from_utf8_lossy(&bytes)
                    .lines()
                    .filter_map(|line| line.strip_prefix("data:"))
                    .filter_map(|data| serde_json::from_str::<Value>(data.trim()).ok())
                    .find(|message| message["id"] == json!(id));
                if answer.is_some() {
                    break;
                }
            }
        }
        let message = match answer {
            Some(message) => message,
            None if !stream => serde_json::from_slice(&bytes)
                .map_err(|_| ConnectError::unknown("MCP server returned invalid JSON"))?,
            None => return Err(ConnectError::unknown("MCP server sent no response")),
        };
        if let Some(error) = message.get("error") {
            return Err(ConnectError::unknown(format!(
                "MCP error: {}",
                error["message"]
                    .as_str()
                    .unwrap_or("unknown")
                    .chars()
                    .take(1024)
                    .collect::<String>()
            )));
        }
        Ok(message["result"].clone())
    }
}

fn definitions(provider: &str, rows: Vec<db::ConnectionToolRow>) -> Vec<types::ToolDefinition> {
    rows.into_iter()
        .map(|t| types::ToolDefinition {
            name: t.name,
            provider_id: provider.into(),
            description: t.description,
            input_schema_json: t.input_schema_json,
            output_schema_json: t.output_schema_json,
            annotations: types::ToolAnnotations {
                read_only: t.read_only,
                destructive: t.destructive,
                idempotent: t.idempotent,
                open_world: t.open_world,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .collect()
}
pub(crate) async fn stored(
    pool: &Pool,
    provider: &str,
    connection: Uuid,
) -> Result<Vec<types::ToolDefinition>, Error> {
    Ok(definitions(
        provider,
        db::connection_tools_all(&pool.get().await?, connection).await?,
    ))
}
/// A registered MCP server's tools, as the installation connections discovered them. Curated servers list their advertised tools instead
/// (`mcp_catalog::advertised`), known before any connection.
pub(crate) async fn provider_tools(
    pool: &Pool,
    provider: &str,
) -> Result<Vec<types::ToolDefinition>, Error> {
    Ok(definitions(
        provider,
        db::provider_mcp_tools_all(&pool.get().await?, provider).await?,
    ))
}
/// The server's tools that Tilde can offer, sorted and deduplicated. Tools without a
/// description, or with an oversized or non-object schema, are left out, not fatal.
async fn list(session: &mut Session) -> ToolResult<Vec<db::ConnectionToolRow>> {
    let mut tools = Vec::new();
    let mut cursor = Value::Null;
    for _ in 0..20 {
        let params = if cursor.is_null() {
            json!({})
        } else {
            json!({"cursor":cursor})
        };
        let page = session.request("tools/list", params).await?;
        tools.extend(page["tools"].as_array().cloned().unwrap_or_default());
        cursor = page["nextCursor"].clone();
        if cursor.is_null() || tools.len() >= 512 {
            break;
        }
    }
    let mut rows: Vec<db::ConnectionToolRow> = tools
        .into_iter()
        .take(512)
        .filter_map(|tool| {
            let schema = tool
                .get("inputSchema")
                .filter(|s| s.is_object())?
                .to_string();
            let name = tool["name"]
                .as_str()
                .filter(|n| !n.is_empty() && n.len() <= 128)?;
            let description = tool["description"]
                .as_str()
                .or(tool["title"].as_str())
                .filter(|d| !d.is_empty())?;
            let hints = &tool["annotations"];
            (schema.len() <= 64 * 1024).then(|| db::ConnectionToolRow {
                name: name.to_owned(),
                description: description.chars().take(4096).collect(),
                input_schema_json: schema,
                output_schema_json: tool
                    .get("outputSchema")
                    .filter(|s| s.is_object())
                    .map(Value::to_string)
                    .filter(|s| s.len() <= 64 * 1024)
                    .unwrap_or_default(),
                read_only: hints["readOnlyHint"].as_bool().unwrap_or(false),
                destructive: hints["destructiveHint"].as_bool().unwrap_or(false),
                idempotent: hints["idempotentHint"].as_bool().unwrap_or(false),
                open_world: hints["openWorldHint"].as_bool().unwrap_or(false),
            })
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows.dedup_by(|a, b| a.name == b.name);
    Ok(rows)
}
/// What a server lists without credentials, for a catalog panel before any connection. Most
/// servers refuse; any failure, including a slow server, is `None`.
pub(crate) async fn probe(
    provider: &str,
    url: &str,
    endpoints: &Endpoints,
) -> Option<Vec<types::ToolDefinition>> {
    let server = model::McpServer {
        url: url.to_owned(),
        credential: model::McpCredential::None,
    };
    let rows = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let mut session = Session::open(&server, &Values::new(), endpoints)
            .await
            .ok()?;
        list(&mut session).await.ok()
    })
    .await
    .ok()??;
    (!rows.is_empty()).then(|| definitions(provider, rows))
}
/// Whether the server answers at all: an unauthenticated `initialize` below 500 within ten
/// seconds, so a server asking for credentials is up. Same address rules as every MCP call.
pub(crate) async fn reachable(url: &str, endpoints: &Endpoints) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let client = match client(&parsed, endpoints).await {
        Ok(client) => client,
        Err(_) => return false,
    };
    let request = client
        .post(parsed)
        .timeout(std::time::Duration::from_secs(10))
        .header("accept", "application/json, text/event-stream")
        .header("content-type", "application/json")
        .header("mcp-protocol-version", PROTOCOL_VERSION)
        .body(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"tilde","version":env!("CARGO_PKG_VERSION")}}}).to_string());
    request
        .send()
        .await
        .is_ok_and(|response| response.status().as_u16() < 500)
}
/// Ask the server for its tools and replace the snapshot unless it is unchanged. Tools Tilde
/// cannot offer (no description, oversized or non-object schema) are left out, not fatal.
pub(crate) async fn discover(
    pool: &Pool,
    endpoints: &Endpoints,
    target: &Target,
    values: &Values,
) -> Result<(Vec<types::ToolDefinition>, bool), Error> {
    let connection = target.connection;
    let mut session = Session::open(&target.server, values, endpoints)
        .await
        .map_err(unavailable)?;
    let rows = list(&mut session).await.map_err(unavailable)?;
    let mut hasher = Sha256::new();
    for row in &rows {
        for part in [
            &row.name,
            &row.description,
            &row.input_schema_json,
            &row.output_schema_json,
        ] {
            hasher.update((part.len() as u64).to_be_bytes());
            hasher.update(part.as_bytes());
        }
        hasher.update([
            row.read_only as u8,
            row.destructive as u8,
            row.idempotent as u8,
            row.open_world as u8,
        ]);
    }
    let hash = hasher.finalize().to_vec();
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    crate::connections::db::connection_lock_execute(&tx, &(connection.to_string())).await?;
    let changed = db::discovery_get_opt(&tx, connection).await?.as_deref() != Some(hash.as_slice());
    if changed {
        db::connection_tools_clear_execute(&tx, connection).await?;
        for row in &rows {
            db::connection_tool_insert_execute(
                &tx,
                connection,
                &row.name,
                &row.description,
                &row.input_schema_json,
                &row.output_schema_json,
                row.read_only,
                row.destructive,
                row.idempotent,
                row.open_world,
            )
            .await?;
        }
    }
    db::discovery_set_execute(&tx, connection, &hash).await?;
    tx.commit().await?;
    Ok((definitions(&target.provider, rows), changed))
}
fn unavailable(error: ConnectError) -> Error {
    invalid(&format!(
        "MCP discovery failed: {}",
        error.message.as_deref().unwrap_or("unknown error")
    ))
}
/// A tool-level failure (`isError`) is the tool's failure, with the server's own text.
pub(crate) async fn call(
    endpoints: &Endpoints,
    server: &model::McpServer,
    values: &Values,
    name: &str,
    input: Value,
) -> ToolResult<Value> {
    let mut session = Session::open(server, values, endpoints).await?;
    let result = tool_result(
        session
            .request("tools/call", json!({"name":name,"arguments":input}))
            .await?,
    )?;
    Ok(match result.get("structuredContent") {
        Some(structured) if !structured.is_null() => structured.clone(),
        _ => json!({"content": result["content"]}),
    })
}
/// One `tools/call` on a managed provider's fixed upstream, every request signed with SigV4.
/// `params` is the whole call (`name`, `arguments`, any `_meta`); returns the raw tool result.
pub(crate) async fn call_signed(
    client: reqwest::Client,
    url: url::Url,
    signer: client_aws_sigv4::Signer,
    params: Value,
) -> ToolResult<Value> {
    let mut session = Session::start(client, url, None, Some(signer)).await?;
    tool_result(session.request("tools/call", params).await?)
}
fn tool_result(result: Value) -> ToolResult<Value> {
    if result["isError"].as_bool() == Some(true) {
        let text = result["content"]
            .as_array()
            .and_then(|content| content.iter().find_map(|part| part["text"].as_str()))
            .unwrap_or("The tool reported an error");
        return Err(ConnectError::unknown(
            text.chars().take(2048).collect::<String>(),
        ));
    }
    Ok(result)
}
