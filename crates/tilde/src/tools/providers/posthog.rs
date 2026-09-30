//! PostHog product analytics over its REST and capture APIs, authenticated with a personal API
//! key against the connection's cloud region.
//!
//! Every object tool takes `project_id` and, when it names one object, `id`; list filters travel
//! in `query` and create or update fields in `data`, exactly as PostHog's API expects them.
use super::{
    ToolProvider,
    rest::{self, Rest, Verb},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_PRODUCT_ANALYTICS, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Map, Value, json};

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("PostHog account name".into()),
        icon_url: Some("/provider-icons/posthog.svg".into()),
        instructions: Some(
            "PostHog product analytics, feature flags, insights, dashboards, cohorts, surveys, experiments, persons, events, annotations, alerts, and project tools backed by direct PostHog REST APIs. Create a personal API key (phx_...) under Settings > Personal API keys and choose the cloud region your PostHog app runs in."
                .into(),
        ),
        id: "posthog".into(),
        name: "PostHog".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_PRODUCT_ANALYTICS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "PostHog API key".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "subdomain":{"type":"string","title":"Cloud region","description":"us and eu are the app hosts; us.i and eu.i the ingestion hosts.","enum":["us","us.i","eu","eu.i"],"default":"us"},
                    "api_key":{"type":"string","title":"Personal API key","minLength":1,"writeOnly":true}
                },"required":["subdomain","api_key"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool],
        }],
    }
}

pub struct Posthog;

use Verb::{Delete, Get, Patch, Post};
/// Plain object tools: summary and description follow from the name.
const TOOLS: &[(&str, Verb, &str)] = &[
    ("list_projects", Get, "/api/projects/"),
    ("get_project", Get, "/api/projects/{project_id}/"),
    ("list_persons", Get, "/api/projects/{project_id}/persons/"),
    (
        "get_person",
        Get,
        "/api/projects/{project_id}/persons/{id}/",
    ),
    (
        "update_person",
        Patch,
        "/api/projects/{project_id}/persons/{id}/",
    ),
    (
        "delete_person",
        Delete,
        "/api/projects/{project_id}/persons/{id}/",
    ),
    (
        "list_feature_flags",
        Get,
        "/api/projects/{project_id}/feature_flags/",
    ),
    (
        "get_feature_flag",
        Get,
        "/api/projects/{project_id}/feature_flags/{id}/",
    ),
    (
        "create_feature_flag",
        Post,
        "/api/projects/{project_id}/feature_flags/",
    ),
    (
        "update_feature_flag",
        Patch,
        "/api/projects/{project_id}/feature_flags/{id}/",
    ),
    (
        "delete_feature_flag",
        Delete,
        "/api/projects/{project_id}/feature_flags/{id}/",
    ),
    ("list_actions", Get, "/api/projects/{project_id}/actions/"),
    (
        "get_action",
        Get,
        "/api/projects/{project_id}/actions/{id}/",
    ),
    ("create_action", Post, "/api/projects/{project_id}/actions/"),
    (
        "update_action",
        Patch,
        "/api/projects/{project_id}/actions/{id}/",
    ),
    (
        "delete_action",
        Delete,
        "/api/projects/{project_id}/actions/{id}/",
    ),
    ("list_insights", Get, "/api/projects/{project_id}/insights/"),
    (
        "get_insight",
        Get,
        "/api/projects/{project_id}/insights/{id}/",
    ),
    (
        "create_insight",
        Post,
        "/api/projects/{project_id}/insights/",
    ),
    (
        "update_insight",
        Patch,
        "/api/projects/{project_id}/insights/{id}/",
    ),
    (
        "delete_insight",
        Delete,
        "/api/projects/{project_id}/insights/{id}/",
    ),
    (
        "list_dashboards",
        Get,
        "/api/projects/{project_id}/dashboards/",
    ),
    (
        "get_dashboard",
        Get,
        "/api/projects/{project_id}/dashboards/{id}/",
    ),
    (
        "create_dashboard",
        Post,
        "/api/projects/{project_id}/dashboards/",
    ),
    (
        "update_dashboard",
        Patch,
        "/api/projects/{project_id}/dashboards/{id}/",
    ),
    (
        "delete_dashboard",
        Delete,
        "/api/projects/{project_id}/dashboards/{id}/",
    ),
    ("list_cohorts", Get, "/api/projects/{project_id}/cohorts/"),
    (
        "get_cohort",
        Get,
        "/api/projects/{project_id}/cohorts/{id}/",
    ),
    ("create_cohort", Post, "/api/projects/{project_id}/cohorts/"),
    (
        "update_cohort",
        Patch,
        "/api/projects/{project_id}/cohorts/{id}/",
    ),
    (
        "delete_cohort",
        Delete,
        "/api/projects/{project_id}/cohorts/{id}/",
    ),
    (
        "list_annotations",
        Get,
        "/api/projects/{project_id}/annotations/",
    ),
    (
        "get_annotation",
        Get,
        "/api/projects/{project_id}/annotations/{id}/",
    ),
    (
        "create_annotation",
        Post,
        "/api/projects/{project_id}/annotations/",
    ),
    (
        "update_annotation",
        Patch,
        "/api/projects/{project_id}/annotations/{id}/",
    ),
    (
        "delete_annotation",
        Delete,
        "/api/projects/{project_id}/annotations/{id}/",
    ),
    ("list_alerts", Get, "/api/projects/{project_id}/alerts/"),
    ("get_alert", Get, "/api/projects/{project_id}/alerts/{id}/"),
    ("create_alert", Post, "/api/projects/{project_id}/alerts/"),
    (
        "update_alert",
        Patch,
        "/api/projects/{project_id}/alerts/{id}/",
    ),
    (
        "delete_alert",
        Delete,
        "/api/projects/{project_id}/alerts/{id}/",
    ),
    (
        "list_experiments",
        Get,
        "/api/projects/{project_id}/experiments/",
    ),
    (
        "get_experiment",
        Get,
        "/api/projects/{project_id}/experiments/{id}/",
    ),
    (
        "create_experiment",
        Post,
        "/api/projects/{project_id}/experiments/",
    ),
    (
        "update_experiment",
        Patch,
        "/api/projects/{project_id}/experiments/{id}/",
    ),
    ("list_surveys", Get, "/api/projects/{project_id}/surveys/"),
    (
        "get_survey",
        Get,
        "/api/projects/{project_id}/surveys/{id}/",
    ),
    ("create_survey", Post, "/api/projects/{project_id}/surveys/"),
    (
        "update_survey",
        Patch,
        "/api/projects/{project_id}/surveys/{id}/",
    ),
    (
        "delete_survey",
        Delete,
        "/api/projects/{project_id}/surveys/{id}/",
    ),
    (
        "list_early_access_features",
        Get,
        "/api/projects/{project_id}/early_access_feature/",
    ),
    (
        "get_early_access_feature",
        Get,
        "/api/projects/{project_id}/early_access_feature/{id}/",
    ),
    (
        "create_early_access_feature",
        Post,
        "/api/projects/{project_id}/early_access_feature/",
    ),
    (
        "update_early_access_feature",
        Patch,
        "/api/projects/{project_id}/early_access_feature/{id}/",
    ),
    (
        "delete_early_access_feature",
        Delete,
        "/api/projects/{project_id}/early_access_feature/{id}/",
    ),
    (
        "list_session_recordings",
        Get,
        "/api/projects/{project_id}/session_recordings/",
    ),
    ("list_events", Get, "/api/projects/{project_id}/events/"),
    ("get_event", Get, "/api/projects/{project_id}/events/{id}/"),
    (
        "list_event_definitions",
        Get,
        "/api/projects/{project_id}/event_definitions/",
    ),
    (
        "get_event_definition",
        Get,
        "/api/projects/{project_id}/event_definitions/{id}/",
    ),
    (
        "list_property_definitions",
        Get,
        "/api/projects/{project_id}/property_definitions/",
    ),
    (
        "get_property_definition",
        Get,
        "/api/projects/{project_id}/property_definitions/{id}/",
    ),
    (
        "update_property_definition",
        Patch,
        "/api/projects/{project_id}/property_definitions/{id}/",
    ),
];

const SOFT_DELETED: &[&str] = &[
    "delete_feature_flag",
    "delete_action",
    "delete_insight",
    "delete_dashboard",
    "delete_cohort",
    "delete_annotation",
];

fn key() -> Value {
    json!({"type":["string","integer"],"minLength":1})
}
fn object_tool(name: &str, verb: Verb, path: &str) -> types::ToolDefinition {
    let (action, noun) = name.split_once('_').unwrap_or((name, ""));
    let noun = noun.replace('_', " ");
    let mut properties = Map::new();
    let mut required = vec![];
    for field in ["project_id", "id"] {
        if path.contains(&format!("{{{field}}}")) {
            properties.insert(field.into(), key());
            required.push(field);
        }
    }
    let (summary, mut description) = match action {
        "list" => (format!("List {noun}"), format!("List PostHog {noun}.")),
        "get" => (
            format!("Get {noun}"),
            format!("Retrieve a PostHog {noun} by id."),
        ),
        _ => {
            let mut verb = action.to_owned();
            verb[..1].make_ascii_uppercase();
            (
                format!("{verb} {noun}"),
                format!("{verb} a PostHog {noun}."),
            )
        }
    };
    match verb {
        Get => {
            properties.insert("query".into(), json!({"type":"object","description":"Filters and pagination as PostHog query parameters, e.g. limit, offset, search."}));
            if action == "list" {
                description.push_str(" Filters and pagination go in `query`.");
            }
        }
        Post | Patch => {
            properties.insert("data".into(), json!({"type":"object","description":"Fields to set, as PostHog's API names them."}));
            required.push("data");
            description.push_str(" The fields to set go in `data`.");
        }
        _ => {}
    }
    rest::definition(
        "posthog",
        name,
        &summary,
        &description,
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
        verb.hints(),
    )
}

impl ToolProvider for Posthog {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let person = json!({
            "project_id":key(),
            "distinct_id":{"type":"string","minLength":1},
            "properties":{"type":"object","description":"Person properties to set."}
        });
        let mut tools = vec![
            rest::definition(
                "posthog",
                "capture_event",
                "Capture event",
                "Capture an event through PostHog's capture API. `api_key` is the project API token (phc_...), not the personal key.",
                json!({"type":"object","properties":{
                    "api_key":{"type":"string","minLength":1,"description":"Project API token."},
                    "event":{"type":"string","minLength":1},
                    "distinct_id":{"type":"string","minLength":1},
                    "properties":{"type":"object"},
                    "timestamp":{"type":"string","description":"ISO 8601 time of the event."}
                },"required":["api_key","event","distinct_id"],"additionalProperties":false}),
                Post.hints(),
            ),
            rest::definition(
                "posthog",
                "identify_person",
                "Identify person",
                "Identify or update a person: sends an `$identify` event with the project's own token, setting `properties` on the person.",
                json!({"type":"object","properties":person,"required":["project_id","distinct_id"],"additionalProperties":false}),
                Post.hints(),
            ),
            rest::definition(
                "posthog",
                "create_person",
                "Create person",
                "Create a person by identifying it, then return the persons PostHog holds for the distinct_id. Ingestion is asynchronous, so a brand new person may not be listed yet.",
                json!({"type":"object","properties":person,"required":["project_id","distinct_id"],"additionalProperties":false}),
                Post.hints(),
            ),
        ];
        tools.extend(
            TOOLS
                .iter()
                .map(|(name, verb, path)| object_tool(name, *verb, path)),
        );
        tools
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let mut input = match input {
                Value::Object(input) => input,
                _ => Map::new(),
            };
            match name {
                "capture_event" => {
                    // The capture API authenticates by the project token in the body.
                    let spec = spec(Post, "/capture/");
                    rest::send(request(access, &spec, Value::Object(input), false)?).await
                }
                "identify_person" => identify(access, input).await,
                "create_person" => {
                    let project = input.get("project_id").cloned();
                    let distinct = input.get("distinct_id").cloned();
                    identify(access, input).await?;
                    let spec = spec(Get, "/api/projects/{project_id}/persons/");
                    let query = json!({"project_id":project,"distinct_id":distinct});
                    rest::send(request(access, &spec, query, true)?).await
                }
                _ => {
                    let (_, verb, path) = TOOLS
                        .iter()
                        .find(|(tool, ..)| *tool == name)
                        .ok_or_else(|| ConnectError::not_found("Unsupported PostHog tool"))?;
                    // Path fields win over a `query` or `data` key of the same name.
                    let mut fields = match input.remove("query").or_else(|| input.remove("data")) {
                        Some(Value::Object(fields)) => fields,
                        _ => Map::new(),
                    };
                    fields.extend(input);
                    // PostHog answers DELETE on these with 405; they are archived by a patch.
                    let verb = if *verb == Delete && SOFT_DELETED.contains(&name) {
                        fields.insert("deleted".into(), json!(true));
                        Patch
                    } else {
                        *verb
                    };
                    let spec = spec(verb, path);
                    rest::send(request(access, &spec, Value::Object(fields), true)?).await
                }
            }
        })
    }
}

/// Only the verb and path matter to a request; the definitions above carry the rest.
fn spec(verb: Verb, path: &'static str) -> Rest {
    Rest {
        name: "",
        summary: "",
        description: "",
        verb,
        path,
        query: &[],
    }
}
fn request(
    access: &Access,
    spec: &Rest,
    input: Value,
    authenticated: bool,
) -> ToolResult<reqwest::RequestBuilder> {
    let subdomain = access.secret("subdomain")?;
    if !["us", "us.i", "eu", "eu.i"].contains(&subdomain) {
        return Err(ConnectError::failed_precondition(
            "The PostHog region must be one of us, us.i, eu or eu.i",
        ));
    }
    let base = format!("https://{subdomain}.posthog.com");
    let request = rest::request(access, "posthog_api", &base, spec, input)?;
    Ok(if authenticated {
        request.bearer_auth(access.secret("api_key")?)
    } else {
        request
    })
}
/// `$identify` needs the project token, which the personal key can read from the project.
async fn identify(access: &Access, input: Map<String, Value>) -> ToolResult<Value> {
    let project = spec(Get, "/api/projects/{project_id}/");
    let found = rest::send(request(
        access,
        &project,
        json!({"project_id":input.get("project_id")}),
        true,
    )?)
    .await?;
    let token = found["data"]["api_token"]
        .as_str()
        .ok_or_else(|| ConnectError::unknown("PostHog returned no project API token"))?;
    let event = json!({
        "api_key":token,
        "event":"$identify",
        "distinct_id":input.get("distinct_id"),
        "properties":{"$set":input.get("properties").cloned().unwrap_or_else(|| json!({}))}
    });
    rest::send(request(access, &spec(Post, "/i/v0/e/"), event, false)?).await
}
