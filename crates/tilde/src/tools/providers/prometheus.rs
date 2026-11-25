//! Prometheus (and compatible: Mimir, Thanos, VictoriaMetrics) queries, alerts, rules and
//! targets over the HTTP API under the connection's URL, read-only. A bearer token is sent when
//! the connection has one.
use super::{
    ToolProvider,
    rest::{self, Tool, Verb, tool},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_DEVELOPER_TOOLS, model};
use crate::proto::tilde::types::v1 as types;
use futures::future::BoxFuture;
use serde_json::{Value, json};

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("Prometheus server name".into()),
        icon_url: Some("/provider-icons/prometheus.svg".into()),
        instructions: Some(
            "Connect a Prometheus-compatible server so agents can run PromQL queries and read alerts, rules and targets. Give the URL the HTTP API is served under (the part before /api/v1), and a bearer token if the server requires one."
                .into(),
        ),
        id: "prometheus".into(),
        name: "Prometheus".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Prometheus HTTP API".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "url":{"type":"string","title":"Query URL","description":"e.g. https://prometheus.example.com, or https://mimir.example.com/prometheus","format":"uri","pattern":"^https://[^?#@]+$","maxLength":2048},
                    "token":{"type":"string","title":"Bearer token","description":"Optional.","writeOnly":true}
                },"required":["url"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool],
        }],
    }
}

pub struct Prometheus;

use Verb::Get;
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("query", "Ran a PromQL query", "Evaluate a PromQL expression at one instant (default now).", Get, "/api/v1/query", "!query time timeout"),
    tool("query_range", "Ran a PromQL range query", "Evaluate a PromQL expression over a time range at `step` resolution.", Get, "/api/v1/query_range", "!query !start !end !step timeout"),
    tool("list_alerts", "Listed Prometheus alerts", "List the active (pending and firing) alerts.", Get, "/api/v1/alerts", ""),
    tool("list_rules", "Listed Prometheus rules", "List alerting and recording rules with their state and last evaluation.", Get, "/api/v1/rules", "type"),
    tool("list_targets", "Listed scrape targets", "List scrape targets with their health and last error.", Get, "/api/v1/targets", "state"),
    tool("list_label_values", "Listed label values", "List the values of a label, e.g. `job` or `__name__` for metric names.", Get, "/api/v1/label/{label}/values", "start end"),
    tool("find_series", "Found series", "Find the series matching a selector, e.g. `up{job=\"api\"}`.", Get, "/api/v1/series", "!match start end"),
];

fn property(name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    match name {
        "query" => text("PromQL expression."),
        "time" | "start" | "end" => text("RFC 3339 timestamp or Unix seconds."),
        "step" => text("Resolution as a duration (30s, 5m) or seconds."),
        "type" => json!({"enum":["alert","record"]}),
        "state" => json!({"enum":["active","dropped","any"]}),
        "match" => text("Series selector."),
        _ => json!({"type":"string"}),
    }
}

impl ToolProvider for Prometheus {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            .map(|tool| tool.definition("prometheus", property))
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
            let mut input = input;
            // The series endpoint names its selector parameter `match[]`.
            if let Some(selector) = input.as_object_mut().and_then(|o| o.remove("match")) {
                input["match[]"] = selector;
            }
            let base = rest::https(access.secret("url")?)?;
            let mut request = rest::request(access, "prometheus_api", base, &tool.rest, input)?;
            if let Some(token) = model::optional(&access.values, "token") {
                request = request.bearer_auth(token);
            }
            rest::send(request).await
        })
    }
}
