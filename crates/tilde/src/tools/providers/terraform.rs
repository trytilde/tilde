//! HCP Terraform (and Terraform Enterprise, by hostname) over its JSON:API, read-only: the
//! organization's workspaces, their runs and plans, and state version metadata. The team or
//! user API token is a bearer token.
//!
//! A plan's JSON output is served by a redirect to a short-lived archive URL, which is read
//! without the token.
use super::{
    ToolProvider,
    rest::{self, Tool, Verb, tool},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_CLOUD_INFRASTRUCTURE, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("Terraform organization".into()),
        icon_url: Some("/provider-icons/terraform.svg".into()),
        instructions: Some(
            "Connect HCP Terraform or Terraform Enterprise so agents can read workspaces, runs, plans and state versions. Use a team API token (Organization settings > Teams) or a user API token, and the organization's name. Set the hostname only for Terraform Enterprise."
                .into(),
        ),
        id: "terraform".into(),
        name: "Terraform".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_CLOUD_INFRASTRUCTURE.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Terraform API token".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "organization":{"type":"string","title":"Organization","pattern":"^[A-Za-z0-9_-]+$","maxLength":100},
                    "token":{"type":"string","title":"API token","description":"A team or user API token.","minLength":1,"writeOnly":true},
                    "hostname":{"type":"string","title":"Terraform Enterprise hostname","description":"Only for Terraform Enterprise, e.g. tfe.example.com. Empty: app.terraform.io.","pattern":"^[A-Za-z0-9.-]+(:[0-9]+)?$","maxLength":253}
                },"required":["organization","token"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool, model::Capability::Egress],
        }],
    }
}

pub struct Terraform;

use Verb::Get;
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("list_workspaces", "Listed Terraform workspaces", "List the organization's workspaces, optionally by name.", Get, "/api/v2/organizations/{organization}/workspaces", "search page_number page_size"),
    tool("get_workspace", "Read a Terraform workspace", "Get a workspace by name, with its current run and resource count.", Get, "/api/v2/organizations/{organization}/workspaces/{workspace_name}", ""),
    tool("list_runs", "Listed Terraform runs", "List a workspace's runs, newest first.", Get, "/api/v2/workspaces/{workspace_id}/runs", "status operation page_number page_size"),
    tool("get_run", "Read a Terraform run", "Get a run with its status, message and timestamps; `include` can add plan and apply.", Get, "/api/v2/runs/{run_id}", "include"),
    tool("get_plan", "Read a Terraform plan", "Get a plan's status and its counts of resource additions, changes and destructions.", Get, "/api/v2/plans/{plan_id}", ""),
    tool("get_plan_json", "Read a Terraform plan's JSON", "Get a finished plan's JSON execution plan (the resource changes). Large plans exceed the output limit.", Get, "/api/v2/plans/{plan_id}/json-output", ""),
    tool("list_state_versions", "Listed state versions", "List a workspace's state versions (metadata only), newest first.", Get, "/api/v2/state-versions", "!workspace_name page_number page_size"),
    tool("get_current_state_version", "Read the current state version", "Get the metadata of a workspace's current state version: serial, Terraform version and resource summary.", Get, "/api/v2/workspaces/{workspace_id}/current-state-version", ""),
];

/// Friendly input names for JSON:API's bracketed query parameters.
const QUERY: &[(&str, &str)] = &[
    ("search", "search[name]"),
    ("status", "filter[status]"),
    ("operation", "filter[operation]"),
    ("workspace_name", "filter[workspace][name]"),
    ("page_number", "page[number]"),
    ("page_size", "page[size]"),
];

fn property(name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    match name {
        "workspace_id" => text("Workspace ID, e.g. ws-..., from list_workspaces."),
        "workspace_name" => text("Workspace name."),
        "run_id" => text("Run ID, e.g. run-..."),
        "plan_id" => text("Plan ID, e.g. plan-..., from a run's relationships."),
        "search" => text("Workspaces whose name contains this."),
        "status" => text("Comma-separated run statuses, e.g. planned,errored."),
        "operation" => text("Comma-separated: plan_only, plan_and_apply, refresh_only, destroy."),
        "include" => text("Comma-separated related resources, e.g. plan,apply."),
        "page_number" => json!({"type":"integer","minimum":1}),
        "page_size" => json!({"type":"integer","minimum":1,"maximum":100}),
        _ => json!({"type":"string"}),
    }
}

impl ToolProvider for Terraform {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            // The organization is the connection's, not an input.
            .map(|tool| rest::hide(tool.definition("terraform", property), "organization"))
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
            let organization = access.secret("organization")?;
            if name == "list_state_versions" {
                input.insert("filter[organization][name]".into(), organization.into());
            }
            for (from, to) in QUERY {
                // `workspace_name` is a filter on lists and a path segment on get_workspace.
                if tool.rest.path.contains(&format!("{{{from}}}")) {
                    continue;
                }
                if let Some(value) = input.remove(*from) {
                    input.insert((*to).into(), value);
                }
            }
            if tool.rest.path.contains("{organization}") {
                input.insert("organization".into(), organization.into());
            }
            let base = format!(
                "https://{}",
                model::optional(&access.values, "hostname").unwrap_or("app.terraform.io")
            );
            let request = rest::request(access, "terraform_api", &base, &tool.rest, input.into())?
                .bearer_auth(access.secret("token")?)
                .header("accept", "application/vnd.api+json");
            let response = request
                .send()
                .await
                .map_err(|_| ConnectError::unavailable("Terraform could not be reached"))?;
            if response.status().is_redirection() {
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|value| value.to_str().ok())
                    .filter(|location| {
                        location.starts_with("https://")
                            || access.endpoints.0.contains_key("terraform_api")
                    })
                    .ok_or_else(|| ConnectError::unknown("Terraform redirected nowhere"))?;
                return rest::send(access.http.client.get(location)).await;
            }
            rest::read(response).await
        })
    }
}
