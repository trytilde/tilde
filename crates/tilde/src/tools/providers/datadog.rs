//! Datadog monitors, metrics, events and logs over its REST API, read-only. The connection names
//! its Datadog site (the API host is `api.<site>`) and holds an API key and an application key,
//! sent as `DD-API-KEY` and `DD-APPLICATION-KEY`.
use super::{
    ToolProvider,
    rest::{self, Tool, Verb, tool},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_DEVELOPER_TOOLS, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

pub(crate) const SITES: &[&str] = &[
    "datadoghq.com",
    "us3.datadoghq.com",
    "us5.datadoghq.com",
    "datadoghq.eu",
    "ap1.datadoghq.com",
    "ap2.datadoghq.com",
    "ddog-gov.com",
];

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("Datadog organization".into()),
        icon_url: Some("/provider-icons/datadog.svg".into()),
        instructions: Some(
            "Connect Datadog so agents can read monitors, query metrics and search events and logs. Create an API key (Organization Settings > API Keys) and an application key (Organization Settings > Application Keys) scoped to monitors_read, timeseries_query, events_read and logs_read_data, and choose your Datadog site."
                .into(),
        ),
        id: "datadog".into(),
        name: "Datadog".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Datadog API and application keys".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "site":{"type":"string","title":"Datadog site","enum":SITES,"default":"datadoghq.com","description":"The site in your Datadog URL, e.g. datadoghq.eu for app.datadoghq.eu."},
                    "api_key":{"type":"string","title":"API key","minLength":1,"writeOnly":true},
                    "app_key":{"type":"string","title":"Application key","minLength":1,"writeOnly":true}
                },"required":["site","api_key","app_key"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool, model::Capability::Egress],
        }],
    }
}

pub struct Datadog;

use Verb::{Get, Post};
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("list_monitors", "Listed Datadog monitors", "List monitors, optionally by name, tags or group state (alert, warn, no data).", Get, "/api/v1/monitor", "name tags monitor_tags group_states page page_size"),
    tool("search_monitors", "Searched Datadog monitors", "Search monitors with Datadog's monitor query syntax, e.g. `status:alert service:checkout`.", Get, "/api/v1/monitor/search", "query page per_page sort"),
    tool("get_monitor", "Read a Datadog monitor", "Get a monitor with its query, thresholds and state.", Get, "/api/v1/monitor/{monitor_id}", "group_states"),
    tool("query_metrics", "Queried Datadog metrics", "Query timeseries points, e.g. `avg:system.cpu.user{service:checkout} by {host}`, between two Unix times in seconds.", Get, "/api/v1/query", "!query !from !to"),
    tool("search_events", "Searched Datadog events", "Search events (deploys, alerts, changes) with the event query syntax over a time range.", Post, "/api/v2/events/search", "query from to limit sort cursor"),
    tool("search_logs", "Searched Datadog logs", "Search logs with the log query syntax, e.g. `service:checkout status:error`, over a time range.", Post, "/api/v2/logs/events/search", "query from to limit sort cursor indexes"),
];

fn property(name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    match name {
        "monitor_id" => json!({"type":"integer","minimum":1}),
        "tags" | "monitor_tags" => text("Comma-separated tags, e.g. env:prod,team:payments."),
        "group_states" => text("Comma-separated: all, alert, warn, no data."),
        "page" => json!({"type":"integer","minimum":0}),
        "page_size" | "per_page" => json!({"type":"integer","minimum":1,"maximum":1000}),
        "limit" => json!({"type":"integer","minimum":1,"maximum":1000}),
        "indexes" => json!({"type":"array","items":{"type":"string"}}),
        "sort" => text("timestamp (oldest first) or -timestamp."),
        "cursor" => text("`meta.page.after` of the previous result."),
        "from" | "to" => {
            json!({"type":["string","integer"],"description":"Unix seconds for query_metrics; for searches a time such as now-15m, now or an ISO 8601 timestamp."})
        }
        _ => json!({"type":"string"}),
    }
}

impl ToolProvider for Datadog {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            .map(|tool| tool.definition("datadog", property))
            .collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let tool = rest::find(TOOLS, name)?;
            let site = access.secret("site")?;
            if !SITES.contains(&site) {
                return Err(ConnectError::failed_precondition("Unknown Datadog site"));
            }
            // The search endpoints nest their filter and paging.
            let input = if tool.rest.verb == Post {
                rest::without_nulls(json!({
                    "filter": {
                        "query": input.get("query"),
                        "from": input.get("from"),
                        "to": input.get("to"),
                        "indexes": input.get("indexes"),
                    },
                    "page": {"limit": input.get("limit"), "cursor": input.get("cursor")},
                    "sort": input.get("sort"),
                }))
            } else {
                input
            };
            let base = format!("https://api.{site}");
            let request = rest::request(access, "datadog_api", &base, &tool.rest, input)?
                .header("DD-API-KEY", access.secret("api_key")?)
                .header("DD-APPLICATION-KEY", access.secret("app_key")?)
                .header("accept", "application/json");
            rest::send(request).await
        })
    }
}
