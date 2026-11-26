//! PagerDuty incidents, services and on-calls over its REST API: reads, plus adding a note to an
//! incident. An API key goes in `Authorization: Token token=…`, an OAuth access token as a
//! bearer token; accounts in the EU service region use `api.eu.pagerduty.com`.
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
    let mut oauth = model::OAuth::standard("https://identity.pagerduty.com/oauth/token");
    oauth.authorization_url = Some("https://identity.pagerduty.com/oauth/authorize".into());
    oauth.scopes = [
        "incidents.read",
        "incidents.write",
        "services.read",
        "oncalls.read",
        "users.read",
    ]
    .map(String::from)
    .to_vec();
    let region =
        json!({"type":"string","title":"Service region","enum":["us","eu"],"default":"us"});
    model::Provider {
        account_name_label: Some("PagerDuty account".into()),
        icon_url: Some("/provider-icons/pagerduty.svg".into()),
        instructions: Some(
            "Connect PagerDuty so agents can read incidents, services and who is on call, and add notes to incidents. Use a REST API key (Integrations > API Access Keys; read-only is enough without notes) or a PagerDuty OAuth app with scoped OAuth."
                .into(),
        ),
        id: "pagerduty".into(),
        name: "PagerDuty".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![
            model::ConnectionType {
                mcp: None,
                id: "api_token".into(),
                name: "PagerDuty API key".into(),
                credential_source: model::CredentialSource::Static {
                    schema: json!({"type":"object","properties":{
                        "api_token":{"type":"string","title":"API key","minLength":1,"writeOnly":true},
                        "region":region
                    },"required":["api_token"],"additionalProperties":false}),
                },
                capabilities: vec![model::Capability::Tool, model::Capability::Egress],
            },
            model::ConnectionType {
                mcp: None,
                id: "oauth".into(),
                name: "PagerDuty OAuth app".into(),
                credential_source: model::CredentialSource::OAuth {
                    grant: model::OAuthGrant::AuthorizationCode,
                    configuration: Box::new(oauth),
                    additional_schema: Some(json!({"type":"object","properties":{
                        "region":region
                    },"required":[],"additionalProperties":false})),
                },
                capabilities: vec![model::Capability::Tool, model::Capability::Egress],
            },
        ],
    }
}

pub struct Pagerduty {
    oauth: bool,
}
pub static API_TOKEN: Pagerduty = Pagerduty { oauth: false };
pub static OAUTH: Pagerduty = Pagerduty { oauth: true };

use Verb::{Get, Post};
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("list_incidents", "Listed PagerDuty incidents", "List incidents, newest first by default, filtered by status, service, urgency or time range.", Get, "/incidents", "statuses service_ids urgencies since until sort_by limit offset"),
    tool("get_incident", "Read a PagerDuty incident", "Get an incident by ID or number.", Get, "/incidents/{incident_id}", ""),
    tool("list_incident_alerts", "Listed incident alerts", "List the alerts grouped into an incident, with the events that raised them.", Get, "/incidents/{incident_id}/alerts", "limit offset"),
    tool("list_incident_notes", "Listed incident notes", "List the notes on an incident.", Get, "/incidents/{incident_id}/notes", ""),
    tool("create_incident_note", "Added an incident note", "Add a note to an incident. `from` is the email of the PagerDuty user the note is written as.", Post, "/incidents/{incident_id}/notes", "!content !from"),
    tool("list_services", "Listed PagerDuty services", "List services, optionally by name.", Get, "/services", "query team_ids limit offset"),
    tool("list_oncalls", "Listed who is on call", "List on-call entries: who is on call for which escalation policy and schedule, now or in a time range.", Get, "/oncalls", "schedule_ids escalation_policy_ids user_ids since until earliest limit offset"),
];

fn property(name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    let ids = || json!({"type":"array","items":{"type":"string"}});
    match name {
        "incident_id" => text("Incident ID (e.g. PT4KHLK) or number."),
        "statuses" => {
            json!({"type":"array","items":{"enum":["triggered","acknowledged","resolved"]}})
        }
        "urgencies" => json!({"type":"array","items":{"enum":["high","low"]}}),
        "service_ids" | "team_ids" | "schedule_ids" | "escalation_policy_ids" | "user_ids" => ids(),
        "since" | "until" => text("ISO 8601 timestamp."),
        "sort_by" => text("e.g. created_at:desc."),
        "limit" => json!({"type":"integer","minimum":1,"maximum":100}),
        "offset" => json!({"type":"integer","minimum":0}),
        "earliest" => json!({"type":"boolean"}),
        "from" => json!({"type":"string","format":"email"}),
        _ => json!({"type":"string"}),
    }
}

impl ToolProvider for Pagerduty {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            .map(|tool| tool.definition("pagerduty", property))
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
            let Value::Object(mut input) = input else {
                return Err(ConnectError::invalid_argument("Input must be an object"));
            };
            let from = input.remove("from");
            let input = if name == "create_incident_note" {
                json!({"note": {"content": input.remove("content")}, "incident_id": input.remove("incident_id")})
            } else {
                // PagerDuty names list filters `statuses[]`.
                Value::Object(
                    input
                        .into_iter()
                        .map(|(key, value)| match value {
                            Value::Array(_) => (format!("{key}[]"), value),
                            _ => (key, value),
                        })
                        .collect(),
                )
            };
            let base = match model::optional(&access.values, "region") {
                Some("eu") => "https://api.eu.pagerduty.com",
                _ => "https://api.pagerduty.com",
            };
            let authorization = zeroize::Zeroizing::new(if self.oauth {
                format!("Bearer {}", access.secret("access_token")?)
            } else {
                format!("Token token={}", access.secret("api_token")?)
            });
            let mut request = rest::request(access, "pagerduty_api", base, &tool.rest, input)?
                .header("authorization", authorization.as_str())
                .header("accept", "application/vnd.pagerduty+json;version=2");
            if let Some(Value::String(from)) = from {
                request = request.header("from", from);
            }
            rest::send(request).await
        })
    }
}
