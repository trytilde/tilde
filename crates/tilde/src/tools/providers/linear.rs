//! Linear issues over its GraphQL API: read and search issues, comment, and move an issue to
//! another workflow state. Both connection types also emit signals from Linear webhooks
//! (`signals::linear`), verified with the webhook's signing secret.
//!
//! Personal API keys go in `Authorization` as they are; OAuth access tokens as bearer tokens.
//! Linear's hosted MCP server joins this provider as its `mcp_oauth` type (`tools::mcp_catalog`).
use super::ToolProvider;
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{catalog::runtime::Runtime, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

const API: &str = "https://api.linear.app/graphql";
const WEBHOOK_SECRET: &str = "Optional. The signing secret of the Linear webhook, so routines can start on issue and comment events.";

pub fn definition() -> model::Provider {
    let mut oauth = model::OAuth::standard("https://api.linear.app/oauth/token");
    oauth.authorization_url = Some("https://linear.app/oauth/authorize".into());
    oauth.scopes = vec!["read".into(), "write".into()];
    oauth.scope_separator = ",".into();
    oauth.client_auth = model::ClientAuth::Body;
    let webhook_secret = json!({"type":"string","title":"Webhook signing secret","description":WEBHOOK_SECRET,"writeOnly":true});
    model::Provider {
        account_name_label: Some("Linear workspace".into()),
        icon_url: Some("/provider-icons/linear.svg".into()),
        instructions: Some(
            "Connect Linear so agents can read, comment on and move issues, and so routines can start when issues or comments change."
                .into(),
        ),
        id: "linear".into(),
        name: "Linear".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec!["productivity".into()],
        connection_types: vec![
            model::ConnectionType {
                mcp: None,
                id: "api_key".into(),
                name: "Linear API key".into(),
                credential_source: model::CredentialSource::Static {
                    schema: json!({"type":"object","properties":{
                        "api_key":{"type":"string","title":"API key","description":"A personal API key, e.g. lin_api_...","minLength":1,"writeOnly":true},
                        "webhook_secret":webhook_secret
                    },"required":["api_key"],"additionalProperties":false}),
                },
                capabilities: vec![model::Capability::Tool, model::Capability::Signal, model::Capability::Egress],
            },
            model::ConnectionType {
                mcp: None,
                id: "oauth".into(),
                name: "Linear OAuth app".into(),
                credential_source: model::CredentialSource::OAuth {
                    grant: model::OAuthGrant::AuthorizationCode,
                    configuration: Box::new(oauth),
                    additional_schema: Some(json!({"type":"object","properties":{
                        "webhook_secret":webhook_secret
                    },"required":[],"additionalProperties":false})),
                },
                capabilities: vec![model::Capability::Tool, model::Capability::Signal, model::Capability::Egress],
            },
        ],
    }
}

/// Setup steps for the shared form, which shows the redirect and webhook URLs they refer to.
/// Signals read Issue and Comment payloads; tools need only the `read` and `write` scopes.
pub(crate) struct LinearSetup;
#[async_trait::async_trait]
impl Runtime for LinearSetup {
    fn instructions(&self, typ: &model::ConnectionType) -> &'static [&'static str] {
        match typ.id.as_str() {
            "oauth" => &[
                "In Linear, go to Settings > API > OAuth applications and create an application for this connection.",
                "Add the redirect URL below to its callback URLs.",
                "For routines, turn on webhooks, paste the webhook URL below and select Issues and Issue comments.",
                "Save, then copy the client ID, client secret and (with webhooks) signing secret into the fields below.",
                "Continue, and approve the read and write access Linear asks for.",
            ],
            "api_key" => &[
                "In Linear, go to Settings > Security & access > Personal API keys and create a key.",
                "For routines, a workspace admin creates a webhook under Settings > API > Webhooks with the webhook URL below.",
                "Select Issues and Issue comments, then save the webhook.",
                "Paste the API key, and the signing secret from the webhook's page, into the fields below.",
            ],
            _ => &[],
        }
    }
}

/// Which connection value holds the credential, and whether it is sent as a bearer token.
pub struct Linear {
    key: &'static str,
    bearer: bool,
}
pub static API_KEY: Linear = Linear {
    key: "api_key",
    bearer: false,
};
pub static OAUTH: Linear = Linear {
    key: "access_token",
    bearer: true,
};

fn tool(
    name: &str,
    summary: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
) -> types::ToolDefinition {
    types::ToolDefinition {
        name: name.into(),
        provider_id: "linear".into(),
        description: description.into(),
        summary: summary.into(),
        input_schema_json: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}).to_string(),
        output_schema_json: json!({"type":"object"}).to_string(),
        annotations: types::ToolAnnotations {
            read_only,
            idempotent: read_only,
            open_world: true,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}
fn issue_id() -> Value {
    json!({"type":"string","minLength":1,"maxLength":100,"description":"Issue ID or identifier, e.g. ENG-123."})
}

const ISSUE_FIELDS: &str = "id identifier title description url priority priorityLabel createdAt updatedAt \
    state { id name type } team { id key name } assignee { id name email } creator { id name email } \
    labels { nodes { name } } project { id name }";

impl ToolProvider for Linear {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        vec![
            tool(
                "get_issue",
                "Read a Linear issue",
                "Read one Linear issue with its state, team, assignee, labels and latest comments.",
                json!({"issue_id":issue_id()}),
                &["issue_id"],
                true,
            ),
            tool(
                "search_issues",
                "Searched Linear issues",
                "Search Linear issues by text; returns the best matches.",
                json!({"query":{"type":"string","minLength":1,"maxLength":400},"limit":{"type":"integer","minimum":1,"maximum":50}}),
                &["query"],
                true,
            ),
            tool(
                "create_comment",
                "Commented on a Linear issue",
                "Add a markdown comment to a Linear issue.",
                json!({"issue_id":issue_id(),"body":{"type":"string","minLength":1,"maxLength":65536}}),
                &["issue_id", "body"],
                false,
            ),
            tool(
                "update_issue_state",
                "Moved a Linear issue",
                "Move a Linear issue to another workflow state of its team, named as Linear shows it (e.g. In Progress, Done) or by state ID. An unknown state fails with the team's states.",
                json!({"issue_id":issue_id(),"state":{"type":"string","minLength":1,"maxLength":200}}),
                &["issue_id", "state"],
                false,
            ),
        ]
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let text = |key: &str| input[key].as_str().unwrap_or_default();
            match name {
                "get_issue" => {
                    let data = self
                        .query(
                            access,
                            &format!(
                                "query($id: String!) {{ issue(id: $id) {{ {ISSUE_FIELDS} \
                                 comments(last: 50) {{ nodes {{ id body url createdAt user {{ id name }} }} }} }} }}"
                            ),
                            json!({"id": text("issue_id")}),
                        )
                        .await?;
                    Ok(flatten(data["issue"].clone()))
                }
                "search_issues" => {
                    let data = self
                        .query(
                            access,
                            &format!(
                                "query($term: String!, $first: Int) {{ searchIssues(term: $term, first: $first) {{ nodes {{ {ISSUE_FIELDS} }} }} }}"
                            ),
                            json!({"term": text("query"), "first": input["limit"].as_i64().unwrap_or(20)}),
                        )
                        .await?;
                    let issues = data["searchIssues"]["nodes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .cloned()
                        .map(flatten)
                        .collect::<Vec<_>>();
                    Ok(json!({ "issues": issues }))
                }
                "create_comment" => {
                    let data = self
                        .query(
                            access,
                            "mutation($input: CommentCreateInput!) { commentCreate(input: $input) { success comment { id url body } } }",
                            json!({"input": {"issueId": text("issue_id"), "body": text("body")}}),
                        )
                        .await?;
                    succeeded(&data["commentCreate"])?;
                    Ok(data["commentCreate"]["comment"].clone())
                }
                "update_issue_state" => {
                    self.move_issue(access, text("issue_id"), text("state"))
                        .await
                }
                _ => Err(ConnectError::not_found("Unsupported Linear tool")),
            }
        })
    }
}
impl Linear {
    /// One GraphQL request; GraphQL errors fail the call with Linear's first message.
    async fn query(&self, access: &Access, query: &str, variables: Value) -> ToolResult<Value> {
        let url = access.url("linear_api", API, &[])?;
        let credential = access.secret(self.key)?;
        let authorization = zeroize::Zeroizing::new(if self.bearer {
            format!("Bearer {credential}")
        } else {
            credential.to_owned()
        });
        let response = access
            .json(
                access
                    .http
                    .client
                    .post(url)
                    .header("authorization", authorization.as_str())
                    .json(&json!({"query": query, "variables": variables})),
            )
            .await?;
        if let Some(error) = response["errors"].as_array().and_then(|e| e.first()) {
            return Err(ConnectError::failed_precondition(format!(
                "Linear: {}",
                error["message"].as_str().unwrap_or("request failed")
            )));
        }
        Ok(response["data"].clone())
    }
    async fn move_issue(&self, access: &Access, issue: &str, state: &str) -> ToolResult<Value> {
        let data = self
            .query(
                access,
                "query($id: String!) { issue(id: $id) { id team { states { nodes { id name type } } } } }",
                json!({"id": issue}),
            )
            .await?;
        let states = data["issue"]["team"]["states"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let wanted = state.trim();
        let Some(target) = states.iter().find(|s| {
            s["id"] == wanted
                || s["name"]
                    .as_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
        }) else {
            let names = states
                .iter()
                .filter_map(|s| s["name"].as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(ConnectError::invalid_argument(format!(
                "Linear has no state {wanted:?} for this issue's team; choose one of: {names}"
            )));
        };
        let data = self
            .query(
                access,
                &format!(
                    "mutation($id: String!, $input: IssueUpdateInput!) {{ issueUpdate(id: $id, input: $input) {{ success issue {{ {ISSUE_FIELDS} }} }} }}"
                ),
                json!({"id": data["issue"]["id"], "input": {"stateId": target["id"]}}),
            )
            .await?;
        succeeded(&data["issueUpdate"])?;
        Ok(flatten(data["issueUpdate"]["issue"].clone()))
    }
}
fn succeeded(payload: &Value) -> ToolResult<()> {
    if payload["success"] != true {
        return Err(ConnectError::failed_precondition(
            "Linear rejected the change",
        ));
    }
    Ok(())
}
/// GraphQL connections (`labels { nodes }`) read as plain lists.
fn flatten(mut issue: Value) -> Value {
    for key in ["labels", "comments"] {
        if let Some(nodes) = issue.get(key).and_then(|v| v.get("nodes")).cloned() {
            issue[key] = nodes;
        }
    }
    if let Some(labels) = issue["labels"].as_array() {
        issue["labels"] = labels.iter().map(|l| l["name"].clone()).collect();
    }
    issue
}
