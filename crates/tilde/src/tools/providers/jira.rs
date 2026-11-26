//! Jira Cloud issues over its REST API (version 2, whose descriptions and comments are plain
//! text or wiki markup rather than Atlassian Document Format): search with JQL, read, create,
//! comment and transition.
//!
//! An API token connection calls the site itself with Basic `email:token`. An OAuth 2.0 (3LO)
//! connection calls `api.atlassian.com/ex/jira/{cloud id}` with a bearer token; the cloud ID is
//! read from Atlassian's accessible-resources list on each call (the site named at setup, or
//! the only site the grant reaches). Atlassian's hosted MCP server joins this provider as its
//! `mcp_oauth` type (`tools::mcp_catalog`).
use super::{
    ToolProvider,
    rest::{self, Tool, Verb, tool},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::model;
use crate::proto::tilde::types::v1 as types;
use base64::Engine;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Map, Value, json};

const ATLASSIAN_API: &str = "https://api.atlassian.com";
const SITE_PATTERN: &str = "^https://[A-Za-z0-9-]+\\.atlassian\\.net/?$";

pub fn definition() -> model::Provider {
    let mut oauth = model::OAuth::standard("https://auth.atlassian.com/oauth/token");
    oauth.authorization_url = Some("https://auth.atlassian.com/authorize".into());
    // `offline_access` asks for a refresh token; access tokens last an hour.
    oauth.scopes = [
        "read:jira-work",
        "write:jira-work",
        "read:jira-user",
        "offline_access",
    ]
    .map(String::from)
    .to_vec();
    oauth.authorization_parameters = [
        ("audience".to_owned(), "api.atlassian.com".to_owned()),
        ("prompt".to_owned(), "consent".to_owned()),
    ]
    .into();
    model::Provider {
        account_name_label: Some("Jira site name".into()),
        icon_url: Some("/provider-icons/jira.svg".into()),
        instructions: Some(
            "Connect Jira Cloud so agents can search, read, create, comment on and transition issues. Use an API token of an Atlassian account (id.atlassian.com > Security > API tokens) with the site URL, or an OAuth 2.0 (3LO) app from developer.atlassian.com with the Jira API scopes read:jira-work, write:jira-work and read:jira-user."
                .into(),
        ),
        id: "jira".into(),
        name: "Jira".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec!["productivity".into()],
        connection_types: vec![
            model::ConnectionType {
                mcp: None,
                id: "api_token".into(),
                name: "Jira API token".into(),
                credential_source: model::CredentialSource::Static {
                    schema: json!({"type":"object","properties":{
                        "site_url":{"type":"string","title":"Site URL","description":"e.g. https://acme.atlassian.net","format":"uri","pattern":SITE_PATTERN,"maxLength":2048},
                        "email":{"type":"string","title":"Account email","description":"The Atlassian account the token belongs to.","format":"email","minLength":3},
                        "api_token":{"type":"string","title":"API token","minLength":1,"writeOnly":true}
                    },"required":["site_url","email","api_token"],"additionalProperties":false}),
                },
                capabilities: vec![model::Capability::Tool, model::Capability::Egress],
            },
            model::ConnectionType {
                mcp: None,
                id: "oauth".into(),
                name: "Jira OAuth 2.0 (3LO) app".into(),
                credential_source: model::CredentialSource::OAuth {
                    grant: model::OAuthGrant::AuthorizationCode,
                    configuration: Box::new(oauth),
                    additional_schema: Some(json!({"type":"object","properties":{
                        "site_url":{"type":"string","title":"Site URL","description":"Optional. The site to use when the grant reaches several, e.g. https://acme.atlassian.net","format":"uri","pattern":SITE_PATTERN,"maxLength":2048}
                    },"required":[],"additionalProperties":false})),
                },
                capabilities: vec![model::Capability::Tool],
            },
        ],
    }
}

pub struct Jira {
    oauth: bool,
}
pub static API_TOKEN: Jira = Jira { oauth: false };
pub static OAUTH: Jira = Jira { oauth: true };

use Verb::{Get, Post};
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("search_issues", "Searched Jira issues", "Search issues with JQL, e.g. `project = ENG AND status = \"In Progress\" ORDER BY updated DESC`. `fields` names the fields to return; page with `nextPageToken` from the previous result.", Post, "/rest/api/2/search/jql", "!jql fields maxResults nextPageToken"),
    tool("get_issue", "Read a Jira issue", "Get an issue with its fields and comments.", Get, "/rest/api/2/issue/{issue}", "fields expand"),
    tool("create_issue", "Created a Jira issue", "Create an issue in a project.", Post, "/rest/api/2/issue", "!project !summary !issue_type description labels"),
    tool("add_comment", "Commented on a Jira issue", "Add a comment to an issue.", Post, "/rest/api/2/issue/{issue}/comment", "!body"),
    tool("list_transitions", "Listed issue transitions", "List the transitions an issue can make from its current status.", Get, "/rest/api/2/issue/{issue}/transitions", ""),
    tool("transition_issue", "Moved a Jira issue", "Move an issue through a transition from `list_transitions`.", Post, "/rest/api/2/issue/{issue}/transitions", "!transition_id"),
    tool("list_projects", "Listed Jira projects", "List the projects the connection can see.", Get, "/rest/api/2/project/search", "query maxResults startAt"),
];

fn property(name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    match name {
        "issue" => text("Issue key or ID, e.g. ENG-123."),
        "project" => text("Project key, e.g. ENG."),
        "issue_type" => text("Issue type name, e.g. Bug or Task."),
        "transition_id" => text("Transition ID."),
        "description" | "body" => text("Plain text or Jira wiki markup."),
        "fields" => {
            json!({"type":"array","items":{"type":"string"},"description":"Field names, e.g. summary, status, assignee."})
        }
        "labels" => json!({"type":"array","items":{"type":"string"}}),
        "maxResults" => json!({"type":"integer","minimum":1,"maximum":100}),
        "startAt" => json!({"type":"integer","minimum":0}),
        _ => json!({"type":"string"}),
    }
}

impl ToolProvider for Jira {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            .map(|tool| tool.definition("jira", property))
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
            match name {
                // GET /issue takes its field list comma-separated.
                "get_issue" => {
                    if let Some(Value::Array(fields)) = input.get("fields") {
                        let joined: Vec<&str> = fields.iter().filter_map(Value::as_str).collect();
                        input.insert("fields".into(), joined.join(",").into());
                    }
                }
                "create_issue" => {
                    let mut fields = Map::new();
                    for (key, field) in [("project", "key"), ("issue_type", "name")] {
                        let name = if key == "issue_type" {
                            "issuetype"
                        } else {
                            key
                        };
                        fields.insert(name.into(), json!({ field: input.remove(key) }));
                    }
                    // Everything else (summary, description, labels, custom fields) is a field.
                    fields.append(&mut input);
                    input.insert("fields".into(), fields.into());
                }
                "transition_issue" => {
                    let id = input.remove("transition_id");
                    input.insert("transition".into(), json!({ "id": id }));
                }
                _ => {}
            }
            let (base, authorization) = self.target(access).await?;
            // The OAuth base already stands on the `atlassian_api` endpoint.
            let endpoint = if self.oauth { "" } else { "jira_api" };
            let request = rest::request(access, endpoint, &base, &tool.rest, input.into())?
                .header("authorization", authorization.as_str())
                .header("accept", "application/json");
            rest::send(request).await
        })
    }
}

impl Jira {
    /// The REST base URL and the `Authorization` header for this connection.
    async fn target(&self, access: &Access) -> ToolResult<(String, zeroize::Zeroizing<String>)> {
        let site = model::optional(&access.values, "site_url").map(|s| s.trim_end_matches('/'));
        if !self.oauth {
            let site = rest::https(site.unwrap_or_default())?;
            let pair = zeroize::Zeroizing::new(format!(
                "{}:{}",
                access.secret("email")?,
                access.secret("api_token")?
            ));
            let encoded = base64::engine::general_purpose::STANDARD.encode(pair.as_bytes());
            return Ok((
                site.to_owned(),
                zeroize::Zeroizing::new(format!("Basic {encoded}")),
            ));
        }
        let token = access.secret("access_token")?;
        let url = access.url(
            "atlassian_api",
            ATLASSIAN_API,
            &["oauth", "token", "accessible-resources"],
        )?;
        let resources = access
            .json(access.http.client.get(url.clone()).bearer_auth(token))
            .await?;
        let resources = resources.as_array().cloned().unwrap_or_default();
        let chosen = match site {
            Some(site) => resources
                .iter()
                .find(|r| r["url"].as_str().map(|u| u.trim_end_matches('/')) == Some(site)),
            None if resources.len() == 1 => resources.first(),
            None => None,
        };
        let cloud = chosen.and_then(|r| r["id"].as_str()).ok_or_else(|| {
            ConnectError::failed_precondition(
                "The Jira grant reaches no site, or several: set the connection's site URL to one it reaches",
            )
        })?;
        let base = format!(
            "{}/ex/jira/{cloud}",
            url.origin().ascii_serialization().trim_end_matches('/')
        );
        Ok((base, zeroize::Zeroizing::new(format!("Bearer {token}"))))
    }
}
