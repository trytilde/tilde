//! Sentry issue triage and the Sentry REST API, ported from trytilde/api's Sentry toolkit.
//!
//! One deduplicated surface is assembled once from the generated `catalog.json` (Sentry's OpenAPI
//! document, the official Sentry MCP tool definitions and Composio's Sentry actions):
//! hand-written issue tools under official MCP names and schemas, the remaining official MCP tools
//! mapped onto REST operations, Composio's DSN ingestion, and a generated
//! `sentry_<snake_case(operationId)>` tool for every other non-deprecated operation.
//!
//! The bearer token only goes to Sentry: a caller's `regionUrl` must be an HTTPS sentry.io origin.
//! The `sentry_api` endpoint override (tests) replaces whichever region was chosen.
mod official;
mod openapi;

use super::{ToolProvider, rest};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_DEVELOPER_TOOLS, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

const DEFAULT_REGION_URL: &str = "https://sentry.io";

pub fn definition() -> model::Provider {
    let mut oauth = model::OAuth::standard("https://sentry.io/oauth/token/");
    oauth.authorization_url = Some("https://sentry.io/oauth/authorize/".into());
    oauth.scopes = [
        "alerts:read",
        "alerts:write",
        "event:admin",
        "event:read",
        "event:write",
        "member:admin",
        "member:invite",
        "member:read",
        "member:write",
        "org:admin",
        "org:ci",
        "org:integrations",
        "org:read",
        "org:write",
        "project:admin",
        "project:distribution",
        "project:read",
        "project:releases",
        "project:write",
        "team:admin",
        "team:read",
        "team:write",
    ]
    .map(String::from)
    .to_vec();
    model::Provider {
        account_name_label: Some("Sentry connection name".into()),
        icon_url: Some("/provider-icons/sentry.svg".into()),
        instructions: Some(
            "Connect Sentry so agents can investigate, triage and resolve issues. Use an organization auth token from a Sentry internal integration, a personal auth token, or a Sentry OAuth application."
                .into(),
        ),
        id: "sentry".into(),
        name: "Sentry".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![
            model::ConnectionType {
                mcp: None,
                id: "token".into(),
                name: "Sentry auth token".into(),
                credential_source: model::CredentialSource::Static {
                    schema: json!({"type":"object","properties":{"auth_token":{"type":"string","title":"Auth token","description":"An organization or personal auth token, e.g. sntrys_...","minLength":1,"writeOnly":true},"client_secret":{"type":"string","title":"Integration client secret","description":"Optional. To receive issue events as signals, create an internal integration with this connection's webhook URL, subscribe it to issue events and paste its client secret here.","writeOnly":true}},"required":["auth_token"],"additionalProperties":false}),
                },
                // Issue webhooks of an internal integration, signed with its client secret.
                capabilities: vec![model::Capability::Tool, model::Capability::Signal],
            },
            model::ConnectionType {
                mcp: None,
                id: "oauth".into(),
                name: "Sentry OAuth app".into(),
                credential_source: model::CredentialSource::OAuth {
                    grant: model::OAuthGrant::AuthorizationCode,
                    configuration: Box::new(oauth),
                    additional_schema: None,
                },
                capabilities: vec![model::Capability::Tool],
            },
        ],
    }
}

/// The connection value holding the bearer token differs by connection type.
pub struct Sentry(&'static str);
pub static TOKEN: Sentry = Sentry("auth_token");
pub static OAUTH: Sentry = Sentry("access_token");

impl ToolProvider for Sentry {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS.clone()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let client = Client {
                access,
                token: access.secret(self.0)?,
            };
            let name = name
                .strip_prefix("sentry_")
                .ok_or_else(|| ConnectError::not_found("Unsupported Sentry tool"))?;
            invoke(&client, name, input).await
        })
    }
}

/// Dispatch by tool name without its `sentry_` prefix; `execute_sentry_tool` re-enters here.
fn invoke<'a>(
    client: &'a Client<'a>,
    name: &'a str,
    input: Value,
) -> BoxFuture<'a, ToolResult<Value>> {
    Box::pin(async move {
        if !input.is_object() {
            return Err(ConnectError::invalid_argument(
                "Sentry tool input must be an object",
            ));
        }
        if CURATED.iter().any(|(tool, _, _)| *tool == name) {
            return curated(client, name, &input).await;
        }
        if CATALOG.official_mcp_tools.iter().any(|t| t.name == name) {
            return official::invoke(client, name, input).await;
        }
        if name == "ingest_event_via_dsn" {
            return ingest(client.access, input).await;
        }
        match openapi::generated(name) {
            Some(operation) => openapi::invoke(client, operation, input).await,
            None => Err(ConnectError::not_found("Unsupported Sentry tool")),
        }
    })
}

#[derive(Deserialize)]
struct Catalog {
    official_mcp_tools: Vec<Official>,
    composio_tools: Vec<Composio>,
    openapi_operations: Vec<openapi::Operation>,
}
#[derive(Deserialize)]
struct Official {
    name: String,
    description: String,
    input_schema: Value,
    output_schema: Value,
}
#[derive(Deserialize)]
struct Composio {
    slug: String,
    name: String,
    description: String,
    input_schema: Value,
    output_schema: Value,
}
static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    serde_json::from_str(include_str!("catalog.json"))
        .expect("the generated Sentry catalog is valid")
});

/// Official MCP tools implemented by hand: name, summary and description.
const CURATED: &[(&str, &str, &str)] = &[
    (
        "whoami",
        "Who am I",
        "Return the authenticated Sentry user.",
    ),
    (
        "find_organizations",
        "Find organizations",
        "Find Sentry organizations accessible to the authenticated user.",
    ),
    (
        "find_projects",
        "Find projects",
        "Find projects in a Sentry organization.",
    ),
    (
        "search_issues",
        "Search issues",
        "Search Sentry issues using Sentry issue-search syntax. Defaults to unresolved issues.",
    ),
    (
        "get_issue_details",
        "Get issue details",
        "Get strongly typed details for one Sentry issue.",
    ),
    (
        "get_issue_activity",
        "Get issue activity",
        "Get activity and human comments for one Sentry issue.",
    ),
    (
        "search_issue_events",
        "Search issue events",
        "Search the error events grouped into one Sentry issue.",
    ),
    (
        "get_event_stacktrace",
        "Get event stacktrace",
        "Retrieve a full issue event, including stacktrace entries when Sentry provides them.",
    ),
    (
        "update_issue",
        "Update issue",
        "Assign an issue or update its status. This mutates visible Sentry state.",
    ),
    (
        "add_issue_note",
        "Add issue note",
        "Add a human-visible comment to a Sentry issue activity feed.",
    ),
];
const INGEST: &str = "SENTRY_INGEST_EVENT_VIA_DSN";

static TOOLS: LazyLock<Vec<types::ToolDefinition>> = LazyLock::new(|| {
    let official = |name: &str| {
        CATALOG
            .official_mcp_tools
            .iter()
            .find(|tool| tool.name == name)
            .expect("curated tools have official definitions")
    };
    let mut tools = Vec::new();
    for (name, summary, description) in CURATED {
        let tool = official(name);
        tools.push(tool_definition(
            name,
            summary,
            description,
            &tool.input_schema,
            &tool.output_schema,
            official_hints(name),
        ));
    }
    for tool in &CATALOG.official_mcp_tools {
        if !CURATED.iter().any(|(name, _, _)| *name == tool.name) {
            tools.push(tool_definition(
                &tool.name,
                &tool.name.replace('_', " "),
                &tool.description,
                &tool.input_schema,
                &tool.output_schema,
                official_hints(&tool.name),
            ));
        }
    }
    let ingest = CATALOG
        .composio_tools
        .iter()
        .find(|tool| tool.slug == INGEST)
        .expect("the catalog has Composio's DSN ingestion");
    tools.push(tool_definition(
        "ingest_event_via_dsn",
        &ingest.name,
        &ingest.description,
        &ingest.input_schema,
        &ingest.output_schema,
        rest::Hints::default(),
    ));
    for operation in openapi::operations() {
        tools.push(tool_definition(
            &openapi::snake_case(&operation.operation_id),
            &operation.summary,
            &format!(
                "{}\n\nSentry REST operation `{}`: `{} {}`.",
                operation.description, operation.operation_id, operation.method, operation.path
            ),
            &operation.input_schema,
            &operation.output_schema,
            operation.hints(),
        ));
    }
    tools
});
fn tool_definition(
    name: &str,
    summary: &str,
    description: &str,
    input: &Value,
    output: &Value,
    hints: rest::Hints,
) -> types::ToolDefinition {
    let mut tool = rest::definition(
        "sentry",
        &format!("sentry_{name}"),
        summary,
        description,
        input.clone(),
        hints,
    );
    // Sentry often answers with arrays; only object output schemas describe the result.
    tool.output_schema_json = if output["type"] == "object" {
        output.to_string()
    } else {
        String::new()
    };
    tool
}
fn official_hints(name: &str) -> rest::Hints {
    rest::Hints {
        // The generic dispatcher can reach any Sentry tool.
        read_only: !is_mutating_name(name) && name != "execute_sentry_tool",
        destructive: is_destructive_name(name),
    }
}
fn is_destructive_name(name: &str) -> bool {
    name.starts_with("delete_") || name.starts_with("remove_")
}
fn is_mutating_name(name: &str) -> bool {
    is_destructive_name(name)
        || ["add_", "create_", "update_", "analyze_"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// A connection's authenticated view of the Sentry API.
struct Client<'a> {
    access: &'a Access,
    token: &'a str,
}
impl Client<'_> {
    fn base(&self, region: Option<&str>) -> ToolResult<String> {
        let region = normalize_region_url(region)?;
        Ok(match self.access.endpoints.0.get("sentry_api") {
            Some(base) => base.trim_end_matches('/').to_owned(),
            None => region,
        })
    }
    /// `path` is already percent-encoded; arrays in `query` repeat their key.
    fn request(
        &self,
        method: Method,
        base: &str,
        path: &str,
        query: &[(String, Value)],
    ) -> ToolResult<reqwest::RequestBuilder> {
        let mut url = url::Url::parse(&format!("{base}{path}"))
            .map_err(|_| ConnectError::invalid_argument("Invalid Sentry request URL"))?;
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                match value {
                    Value::Null => {}
                    Value::Array(items) => {
                        for item in items {
                            pairs.append_pair(key, &scalar(item));
                        }
                    }
                    value => {
                        pairs.append_pair(key, &scalar(value));
                    }
                }
            }
        }
        Ok(self
            .access
            .http
            .client
            .request(method, url)
            .bearer_auth(self.token))
    }
    async fn call(
        &self,
        method: Method,
        base: &str,
        path: &str,
        query: &[(String, Value)],
        body: Option<Value>,
    ) -> ToolResult<Value> {
        let mut request = self.request(method, base, path, query)?;
        if let Some(body) = body {
            request = request.json(&body);
        }
        send(request).await
    }
    async fn get(&self, base: &str, path: &str, query: &[(String, Value)]) -> ToolResult<Value> {
        self.call(Method::GET, base, path, query, None).await
    }
}

/// Read a Sentry response: JSON, text, or base64 for binary bodies such as attachments. A non-2xx
/// status is the tool's error with Sentry's `detail`, which the agent needs to correct its call.
async fn send(request: reqwest::RequestBuilder) -> ToolResult<Value> {
    let mut response = request
        .send()
        .await
        .map_err(|_| ConnectError::unavailable("Sentry could not be reached"))?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ConnectError::unavailable("The Sentry response was interrupted"))?
    {
        if bytes.len() + chunk.len() > crate::tools::MAX_OUTPUT {
            return Err(ConnectError::resource_exhausted(
                crate::tools::OUTPUT_TOO_LARGE,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let text = String::from_utf8_lossy(&bytes);
        let detail = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|body| body["detail"].as_str().map(str::to_owned))
            .unwrap_or_else(|| text.into_owned());
        return Err(ConnectError::unknown(format!(
            "Sentry API returned {status}: {}",
            detail.chars().take(2048).collect::<String>()
        )));
    }
    Ok(if bytes.is_empty() {
        Value::Null
    } else if content_type.contains("json") {
        serde_json::from_slice(&bytes)
            .map_err(|_| ConnectError::unknown("Sentry returned invalid JSON"))?
    } else if content_type.starts_with("text/") || content_type.is_empty() {
        Value::String(String::from_utf8_lossy(&bytes).into_owned())
    } else {
        use base64::Engine;
        json!({"contentType": content_type, "bodyBase64": base64::engine::general_purpose::STANDARD.encode(bytes)})
    })
}

/// Only HTTPS sentry.io origins may receive the token; defaults to https://sentry.io.
fn normalize_region_url(region: Option<&str>) -> ToolResult<String> {
    let region = region.unwrap_or(DEFAULT_REGION_URL).trim_end_matches('/');
    let url = url::Url::parse(region)
        .map_err(|_| ConnectError::invalid_argument("Invalid Sentry regionUrl"))?;
    if url.scheme() != "https"
        || !url
            .host_str()
            .is_some_and(|host| host == "sentry.io" || host.ends_with(".sentry.io"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ConnectError::invalid_argument(
            "Sentry regionUrl must be a trusted sentry.io HTTPS origin",
        ));
    }
    Ok(region.to_owned())
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}
fn scalar(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
fn text<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input[key].as_str()
}
fn required<'a>(input: &'a Value, key: &str) -> ToolResult<&'a str> {
    text(input, key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ConnectError::invalid_argument(format!("Missing required parameter {key}")))
}
fn invalid(message: impl Into<String>) -> ConnectError {
    ConnectError::invalid_argument(message.into())
}
fn query(pairs: &[(&str, Option<Value>)]) -> Vec<(String, Value)> {
    pairs
        .iter()
        .filter_map(|(key, value)| value.clone().map(|value| ((*key).to_owned(), value)))
        .collect()
}
fn limit(input: &Value, default: u64) -> Value {
    json!(input["limit"].as_u64().unwrap_or(default).clamp(1, 100))
}

/// An issue from `issueUrl` (organization from the path or subdomain) or explicit fields.
fn resolve_issue_locator(
    organization: Option<&str>,
    issue_id: Option<&str>,
    issue_url: Option<&str>,
) -> ToolResult<(String, String)> {
    let Some(issue_url) = issue_url else {
        let organization = organization
            .ok_or_else(|| invalid("organizationSlug is required when issueUrl is not provided"))?;
        let issue_id =
            issue_id.ok_or_else(|| invalid("Either issueId or issueUrl must be provided"))?;
        return Ok((organization.to_owned(), clean_issue_id(issue_id)?));
    };
    let url = url::Url::parse(issue_url).map_err(|_| invalid("Invalid Sentry issueUrl"))?;
    let parts = url
        .path_segments()
        .map(|segments| segments.filter(|part| !part.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();
    let issue_index = parts.iter().position(|part| *part == "issues");
    let parsed_issue = issue_index
        .and_then(|index| parts.get(index + 1))
        .ok_or_else(|| invalid("Sentry issueUrl must contain /issues/{issue_id}"))?;
    let path_org = parts
        .iter()
        .position(|part| *part == "organizations")
        .and_then(|index| parts.get(index + 1))
        .copied()
        .or_else(|| {
            issue_index
                .filter(|index| *index > 0 && parts[0] != "issues")
                .map(|_| parts[0])
        });
    let subdomain_org = url
        .host_str()
        .and_then(|host| host.split('.').next())
        .filter(|part| !matches!(*part, "sentry" | "us" | "eu" | "de" | "www"));
    let parsed_org = path_org
        .or(subdomain_org)
        .ok_or_else(|| invalid("Could not determine organization from Sentry issueUrl"))?;
    if organization.is_some_and(|organization| organization != parsed_org) {
        return Err(invalid("organizationSlug does not match issueUrl"));
    }
    Ok((parsed_org.to_owned(), clean_issue_id(parsed_issue)?))
}
fn clean_issue_id(issue_id: &str) -> ToolResult<String> {
    let issue_id = issue_id
        .trim()
        .trim_end_matches(|character: char| !character.is_alphanumeric());
    if issue_id.is_empty() {
        return Err(invalid("Sentry issueId cannot be empty"));
    }
    Ok(issue_id.to_owned())
}
fn locate(input: &Value) -> ToolResult<(String, String)> {
    resolve_issue_locator(
        text(input, "organizationSlug"),
        text(input, "issueId"),
        text(input, "issueUrl"),
    )
}
fn issue_path(organization: &str, issue: &str, rest: &str) -> String {
    format!(
        "/api/0/organizations/{}/issues/{}/{rest}",
        encode(organization),
        encode(issue)
    )
}

async fn curated(client: &Client<'_>, name: &str, input: &Value) -> ToolResult<Value> {
    let base = client.base(text(input, "regionUrl"))?;
    let optional = |key: &str| input.get(key).filter(|value| !value.is_null()).cloned();
    match name {
        "whoami" => client.get(&base, "/api/0/", &[]).await,
        "find_organizations" => {
            client
                .get(
                    &base,
                    "/api/0/organizations/",
                    &query(&[("query", optional("query"))]),
                )
                .await
        }
        "find_projects" => {
            let organization = required(input, "organizationSlug")?;
            client
                .get(
                    &base,
                    &format!("/api/0/organizations/{}/projects/", encode(organization)),
                    &query(&[("query", optional("query"))]),
                )
                .await
        }
        "search_issues" => search_issues(client, &base, input).await,
        "get_issue_details" => match text(input, "eventId") {
            Some(event) => issue_event(client, &base, input, event).await,
            None => {
                let (organization, issue) = locate(input)?;
                client
                    .get(&base, &issue_path(&organization, &issue, ""), &[])
                    .await
            }
        },
        "get_issue_activity" => {
            let (organization, issue) = locate(input)?;
            let activity = client
                .get(
                    &base,
                    &issue_path(&organization, &issue, "activities/"),
                    &[],
                )
                .await?;
            let comments = if input["includeComments"].as_bool().unwrap_or(true) {
                client
                    .get(
                        &base,
                        &issue_path(&organization, &issue, "notes/"),
                        &query(&[("per_page", Some(limit(input, 10)))]),
                    )
                    .await?
            } else {
                json!([])
            };
            Ok(json!({"activity": activity, "comments": comments}))
        }
        "search_issue_events" => {
            let (organization, issue) = locate(input)?;
            client
                .get(
                    &base,
                    &issue_path(&organization, &issue, "events/"),
                    &query(&[
                        ("per_page", Some(limit(input, 50))),
                        ("statsPeriod", optional("period")),
                        ("query", optional("query")),
                        ("project", optional("projectSlug")),
                        ("sort", optional("sort")),
                    ]),
                )
                .await
        }
        "get_event_stacktrace" => {
            let (organization, issue) = locate(input)?;
            let event = text(input, "eventId").unwrap_or("latest");
            client
                .get(
                    &base,
                    &issue_path(&organization, &issue, &format!("events/{}/", encode(event))),
                    &[],
                )
                .await
        }
        "update_issue" => update_issue(client, &base, input).await,
        "add_issue_note" => {
            let (organization, issue) = locate(input)?;
            let note = text(input, "text").unwrap_or_default();
            if note.trim().is_empty() || note.len() > 4096 {
                return Err(invalid(
                    "Sentry issue note text must contain 1-4096 characters",
                ));
            }
            client
                .call(
                    Method::POST,
                    &base,
                    &issue_path(&organization, &issue, "notes/"),
                    &[],
                    Some(json!({"text": note})),
                )
                .await
        }
        _ => Err(ConnectError::not_found("Unsupported Sentry tool")),
    }
}

async fn search_issues(client: &Client<'_>, base: &str, input: &Value) -> ToolResult<Value> {
    let organization = required(input, "organizationSlug")?;
    let mut pairs = query(&[
        (
            "query",
            Some(json!(text(input, "query").unwrap_or("is:unresolved"))),
        ),
        (
            "statsPeriod",
            Some(json!(text(input, "period").unwrap_or("30d"))),
        ),
        ("per_page", Some(limit(input, 10))),
    ]);
    if let Some(project) = text(input, "projectSlugOrId") {
        // Issue search filters by numeric project ID.
        let id = if project.chars().all(|c| c.is_ascii_digit()) {
            project.to_owned()
        } else {
            let found = client
                .get(
                    base,
                    &format!(
                        "/api/0/projects/{}/{}/",
                        encode(organization),
                        encode(project)
                    ),
                    &[],
                )
                .await?;
            found["id"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid(format!("Sentry project {project} has no id")))?
        };
        pairs.push(("project".into(), json!(id)));
    }
    if let Some(sort) = text(input, "sort") {
        pairs.push(("sort".into(), json!(sort)));
    }
    client
        .get(
            base,
            &format!("/api/0/organizations/{}/issues/", encode(organization)),
            &pairs,
        )
        .await
}

async fn issue_event(
    client: &Client<'_>,
    base: &str,
    input: &Value,
    event: &str,
) -> ToolResult<Value> {
    let organization = text(input, "organizationSlug")
        .ok_or_else(|| invalid("organizationSlug is required when eventId is provided"))?;
    let issue = match text(input, "issueId") {
        Some(issue) => clean_issue_id(issue)?,
        None => {
            let resolved = client
                .get(
                    base,
                    &format!(
                        "/api/0/organizations/{}/eventids/{}/",
                        encode(organization),
                        encode(event)
                    ),
                    &[],
                )
                .await?;
            resolved
                .get("groupId")
                .or_else(|| resolved.get("groupID"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| {
                    invalid(format!("Sentry did not resolve event {event} to an issue"))
                })?
        }
    };
    client
        .get(
            base,
            &issue_path(organization, &issue, &format!("events/{}/", encode(event))),
            &[],
        )
        .await
}

async fn update_issue(client: &Client<'_>, base: &str, input: &Value) -> ToolResult<Value> {
    let (organization, issue) = locate(input)?;
    let status = text(input, "status");
    let assigned = text(input, "assignedTo");
    if status.is_none() && assigned.is_none() {
        return Err(invalid("At least one of status or assignedTo is required"));
    }
    let number = |key: &str| input[key].as_u64();
    let duration = number("ignoreDurationMinutes").is_some();
    let occurrence = number("ignoreCount").is_some() || number("ignoreWindowMinutes").is_some();
    let users = number("ignoreUserCount").is_some() || number("ignoreUserWindowMinutes").is_some();
    let mode = text(input, "ignoreMode");
    if (mode.is_some() || duration || occurrence || users) && status != Some("ignored") {
        return Err(invalid("Ignore options require status=ignored"));
    }
    let mut body = Map::new();
    if let Some(status) = status {
        body.insert("status".into(), json!(status));
    }
    if let Some(assigned) = assigned {
        body.insert("assignedTo".into(), json!(assigned));
    }
    if status == Some("ignored") {
        if [duration, occurrence, users].iter().filter(|x| **x).count() > 1 {
            return Err(invalid("Choose only one ignore condition family"));
        }
        let mode = mode.unwrap_or(if duration {
            "forDuration"
        } else if occurrence {
            "untilOccurrenceCount"
        } else if users {
            "untilUserCount"
        } else {
            "untilEscalating"
        });
        let (incompatible, substatus) = match mode {
            "untilEscalating" => (duration || occurrence || users, "archived_until_escalating"),
            "forever" => (duration || occurrence || users, "archived_forever"),
            "forDuration" => (occurrence || users, "archived_until_condition_met"),
            "untilOccurrenceCount" => (duration || users, "archived_until_condition_met"),
            _ => (duration || occurrence, "archived_until_condition_met"),
        };
        if incompatible {
            return Err(invalid(format!(
                "ignoreMode {mode} cannot be combined with those ignore conditions"
            )));
        }
        body.insert("substatus".into(), json!(substatus));
        let positive = |key: &str| {
            number(key).filter(|n| *n > 0).ok_or_else(|| {
                invalid(format!(
                    "{key} must be a positive integer for the selected ignore mode"
                ))
            })
        };
        match mode {
            "forDuration" => {
                body.insert(
                    "ignoreDuration".into(),
                    json!(positive("ignoreDurationMinutes")?),
                );
            }
            "untilOccurrenceCount" => {
                body.insert("ignoreCount".into(), json!(positive("ignoreCount")?));
                if let Some(window) = number("ignoreWindowMinutes") {
                    body.insert("ignoreWindow".into(), json!(window));
                }
            }
            "untilUserCount" => {
                body.insert(
                    "ignoreUserCount".into(),
                    json!(positive("ignoreUserCount")?),
                );
                if let Some(window) = number("ignoreUserWindowMinutes") {
                    body.insert("ignoreUserWindow".into(), json!(window));
                }
            }
            _ => {}
        }
    }
    let updated = client
        .call(
            Method::PUT,
            base,
            &issue_path(&organization, &issue, ""),
            &[],
            Some(Value::Object(body)),
        )
        .await?;
    // The reason is a follow-up note; failing to add it does not undo the update.
    let (note, note_error) = match text(input, "reason") {
        None => (Value::Null, Value::Null),
        Some(reason) if reason.trim().is_empty() || reason.len() > 4096 => {
            (Value::Null, json!("reason must contain 1-4096 characters"))
        }
        Some(reason) => match client
            .call(
                Method::POST,
                base,
                &issue_path(&organization, &issue, "notes/"),
                &[],
                Some(json!({"text": reason})),
            )
            .await
        {
            Ok(note) => (note, Value::Null),
            Err(error) => (Value::Null, json!(error.message)),
        },
    };
    Ok(json!({"issue": updated, "reasonNote": note, "reasonNoteError": note_error}))
}

/// Composio's DSN ingestion: an event envelope posted to the DSN's project. The DSN is its own
/// credential, so the connection token is not sent.
async fn ingest(access: &Access, input: Value) -> ToolResult<Value> {
    let dsn = required(&input, "dsn")?;
    let parsed = url::Url::parse(dsn).map_err(|_| invalid("Invalid Sentry DSN"))?;
    let host = parsed
        .host_str()
        .filter(|host| *host == "sentry.io" || host.ends_with(".sentry.io"));
    let (Some(host), "https", false, None, None, None) = (
        host,
        parsed.scheme(),
        parsed.username().is_empty(),
        parsed.password(),
        parsed.query(),
        parsed.fragment(),
    ) else {
        return Err(invalid(
            "Sentry DSN must use a trusted sentry.io HTTPS host and contain a public key",
        ));
    };
    let project = parsed
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("Sentry DSN must end with a project ID"))?;
    let origin = match access.endpoints.0.get("sentry_ingest") {
        Some(origin) => origin.trim_end_matches('/').to_owned(),
        None => format!("https://{host}"),
    };
    let event_id = text(&input, "event_id")
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    let mut event = Map::new();
    event.insert("event_id".into(), json!(event_id));
    event.insert("timestamp".into(), json!(chrono::Utc::now().to_rfc3339()));
    for key in [
        "level",
        "logger",
        "message",
        "release",
        "environment",
        "platform",
        "fingerprint",
        "tags",
        "extra",
    ] {
        if let Some(value) = input.get(key) {
            event.insert(key.into(), value.clone());
        }
    }
    let mut user = Map::new();
    for (from, to) in [
        ("user_id", "id"),
        ("user_email", "email"),
        ("user_username", "username"),
        ("user_ip_address", "ip_address"),
    ] {
        if let Some(value) = input.get(from) {
            user.insert(to.into(), value.clone());
        }
    }
    if !user.is_empty() {
        event.insert("user".into(), Value::Object(user));
    }
    if let Some(kind) = input.get("exception_type") {
        event.insert(
            "exception".into(),
            json!({"values": [{"type": kind, "value": input.get("exception_value")}]}),
        );
    }
    let envelope = format!(
        "{}\n{}\n{}",
        json!({"event_id": event_id, "dsn": dsn}),
        json!({"type": "event"}),
        Value::Object(event)
    );
    let url = url::Url::parse(&format!("{origin}/api/{}/envelope/", encode(project)))
        .map_err(|_| invalid("Invalid Sentry DSN"))?;
    let response = send(
        access
            .http
            .client
            .post(url)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-sentry-envelope",
            )
            .body(envelope),
    )
    .await?;
    Ok(json!({"successful": true, "data": {"eventId": event_id, "response": response}}))
}
