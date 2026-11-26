//! Honeycomb triggers, SLOs, columns and queries over its API, read-only apart from the saved
//! query a run creates. The configuration API key goes in `X-Honeycomb-Team`; EU teams use
//! `api.eu1.honeycomb.io`.
//!
//! Running a query is three calls (create the query, start a result, poll it), made inside
//! `run_query`; Honeycomb offers query results on Enterprise plans.
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
        account_name_label: Some("Honeycomb environment".into()),
        icon_url: Some("/provider-icons/honeycomb.svg".into()),
        instructions: Some(
            "Connect Honeycomb so agents can read triggers and SLOs and run queries. Create a configuration API key for the environment (Environment settings > API keys) with the Manage Queries and Columns, Run Queries, Manage Triggers and Manage SLOs permissions."
                .into(),
        ),
        id: "honeycomb".into(),
        name: "Honeycomb".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Honeycomb API key".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "api_key":{"type":"string","title":"API key","minLength":1,"writeOnly":true},
                    "region":{"type":"string","title":"Region","enum":["us","eu"],"default":"us"}
                },"required":["api_key"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool, model::Capability::Egress],
        }],
    }
}

pub struct Honeycomb;

use Verb::{Get, Post};
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("list_datasets", "Listed Honeycomb datasets", "List the environment's datasets.", Get, "/1/datasets", ""),
    tool("list_columns", "Listed dataset columns", "List a dataset's columns, to write queries against.", Get, "/1/columns/{dataset}", ""),
    tool("list_triggers", "Listed Honeycomb triggers", "List a dataset's triggers with their thresholds and whether they are triggered. Use `__all__` for environment-wide triggers.", Get, "/1/triggers/{dataset}", ""),
    tool("get_trigger", "Read a Honeycomb trigger", "Get a trigger.", Get, "/1/triggers/{dataset}/{trigger_id}", ""),
    tool("list_slos", "Listed Honeycomb SLOs", "List a dataset's SLOs. Use `__all__` for environment-wide SLOs.", Get, "/1/slos/{dataset}", ""),
    tool("get_slo", "Read a Honeycomb SLO", "Get an SLO; `detailed` adds its compliance and remaining budget (Enterprise).", Get, "/1/slos/{dataset}/{slo_id}", "detailed"),
    tool("run_query", "Ran a Honeycomb query", "Run a query and wait for its result. `query` is a Honeycomb query specification, e.g. {\"calculations\":[{\"op\":\"COUNT\"}],\"filters\":[{\"column\":\"error\",\"op\":\"=\",\"value\":true}],\"breakdowns\":[\"service.name\"],\"time_range\":3600}.", Post, "/1/queries/{dataset}", "!query"),
];

fn property(name: &str) -> Value {
    match name {
        "dataset" => json!({"type":"string","description":"Dataset slug."}),
        "query" => json!({"type":"object","description":"Query specification."}),
        "detailed" => json!({"type":"boolean"}),
        _ => json!({"type":"string"}),
    }
}

impl ToolProvider for Honeycomb {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            .map(|tool| tool.definition("honeycomb", property))
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
            if name == "run_query" {
                return run_query(access, &input).await;
            }
            rest::send(call(access, &tool.rest, input)?).await
        })
    }
}

fn call(access: &Access, spec: &rest::Rest, input: Value) -> ToolResult<reqwest::RequestBuilder> {
    let base = match model::optional(&access.values, "region") {
        Some("eu") => "https://api.eu1.honeycomb.io",
        _ => "https://api.honeycomb.io",
    };
    Ok(rest::request(access, "honeycomb_api", base, spec, input)?
        .header("X-Honeycomb-Team", access.secret("api_key")?))
}

async fn run_query(access: &Access, input: &Value) -> ToolResult<Value> {
    const fn step(verb: Verb, path: &'static str) -> rest::Rest {
        rest::Rest {
            name: "run_query",
            summary: "",
            description: "",
            verb,
            path,
            query: &[],
        }
    }
    let dataset = &input["dataset"];
    let Some(mut spec) = input["query"].as_object().cloned() else {
        return Err(ConnectError::invalid_argument("`query` must be an object"));
    };
    spec.insert("dataset".into(), dataset.clone());
    let query = rest::send(call(
        access,
        &step(Post, "/1/queries/{dataset}"),
        spec.into(),
    )?)
    .await?;
    let started = rest::send(call(
        access,
        &step(Post, "/1/query_results/{dataset}"),
        json!({"dataset": dataset, "query_id": query["data"]["id"], "disable_series": true}),
    )?)
    .await?;
    let id = started["data"]["id"].clone();
    // Honeycomb keeps a result pollable for a short while; queries finish in seconds.
    let mut wait = std::time::Duration::from_millis(500);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(std::time::Duration::from_secs(5));
        let result = rest::send(call(
            access,
            &step(Get, "/1/query_results/{dataset}/{result_id}"),
            json!({"dataset": dataset, "result_id": id}),
        )?)
        .await?;
        if result["data"]["complete"] == true {
            return Ok(result);
        }
    }
    Err(ConnectError::deadline_exceeded(
        "The Honeycomb query did not finish in time",
    ))
}
