//! Grafana dashboards, alerting and data source queries over its HTTP API, authenticated with a
//! service account token. Reads only, apart from `query_datasource`, which runs a query through
//! Grafana's data source proxy (`/api/ds/query`).
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

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("Grafana instance name".into()),
        icon_url: Some("/provider-icons/grafana.svg".into()),
        instructions: Some(
            "Connect Grafana (Cloud or self-hosted) so agents can read alert rules and their state, find dashboards and query data sources. Create a service account with the Viewer role (Administration > Users and access > Service accounts) and add a token."
                .into(),
        ),
        id: "grafana".into(),
        name: "Grafana".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Grafana service account token".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "url":{"type":"string","title":"Grafana URL","description":"e.g. https://acme.grafana.net","format":"uri","pattern":"^https://[^?#@]+$","maxLength":2048},
                    "token":{"type":"string","title":"Service account token","description":"e.g. glsa_...","minLength":1,"writeOnly":true}
                },"required":["url","token"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool, model::Capability::Egress],
        }],
    }
}

pub struct Grafana;

use Verb::{Get, Post};
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("search_dashboards", "Searched Grafana dashboards", "Search dashboards and folders by title or tag.", Get, "/api/search", "query tag type folderUIDs limit page"),
    tool("get_dashboard", "Read a Grafana dashboard", "Get a dashboard's panels and queries by UID.", Get, "/api/dashboards/uid/{uid}", ""),
    tool("list_alert_rules", "Listed Grafana alert rules", "List Grafana-managed alert rule definitions.", Get, "/api/v1/provisioning/alert-rules", ""),
    tool("get_alert_rule", "Read a Grafana alert rule", "Get one alert rule definition by UID.", Get, "/api/v1/provisioning/alert-rules/{uid}", ""),
    tool("list_alert_states", "Read Grafana alert states", "List Grafana-managed alert rules with their current state (firing, pending, inactive) and active alerts.", Get, "/api/prometheus/grafana/api/v1/rules", "state rule_name rule_group folder_uid dashboard_uid"),
    tool("list_firing_alerts", "Listed firing Grafana alerts", "List alert instances in Grafana's Alertmanager, optionally by label matchers.", Get, "/api/alertmanager/grafana/api/v2/alerts", "filter active silenced inhibited"),
    tool("list_datasources", "Listed Grafana data sources", "List data sources with their UID and type.", Get, "/api/datasources", ""),
    tool("query_datasource", "Queried a Grafana data source", "Run one query against a data source. `query` is the data source's own query model, e.g. {\"expr\":\"up\"} for Prometheus, {\"expr\":\"{app=\\\"api\\\"} |= \\\"error\\\"\"} for Loki or {\"rawSql\":\"select 1\",\"format\":\"table\"} for SQL.", Post, "/api/ds/query", "!datasource_uid !query from to"),
];

fn property(name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    match name {
        "uid" | "datasource_uid" => text("UID."),
        "query" => {
            json!({"type":["object","string"],"description":"Search text, or for query_datasource the query model object."})
        }
        "from" | "to" => {
            text("Epoch milliseconds or a relative time such as now-1h; default the last hour.")
        }
        "type" => json!({"enum":["dash-db","dash-folder"]}),
        "state" => text("firing, pending, inactive or error."),
        "filter" => text("A label matcher, e.g. alertname=\"HighErrorRate\"."),
        "limit" | "page" => json!({"type":"integer","minimum":1}),
        "active" | "silenced" | "inhibited" => json!({"type":"boolean"}),
        _ => json!({"type":"string"}),
    }
}

impl ToolProvider for Grafana {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            .map(|tool| tool.definition("grafana", property))
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
            let input = if name == "query_datasource" {
                let Some(mut query) = input["query"].as_object().cloned() else {
                    return Err(ConnectError::invalid_argument("`query` must be an object"));
                };
                query.insert("refId".into(), "A".into());
                query.insert("datasource".into(), json!({"uid": input["datasource_uid"]}));
                json!({
                    "queries": [query],
                    "from": input["from"].as_str().unwrap_or("now-1h"),
                    "to": input["to"].as_str().unwrap_or("now"),
                })
            } else {
                input
            };
            let base = rest::https(access.secret("url")?)?;
            let request = rest::request(access, "grafana_api", base, &tool.rest, input)?
                .bearer_auth(access.secret("token")?)
                .header("accept", "application/json");
            rest::send(request).await
        })
    }
}
