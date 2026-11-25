//! GitLab REST tools for projects, issues, merge requests and pipelines, on gitlab.com or a
//! self-managed instance (`base_url`). Every tool is one call to `/api/v4`; a project is its
//! numeric ID or its path (`group/project`), and fields beside the listed ones pass through.
//!
//! Personal, group and project access tokens and OAuth access tokens all go in `Authorization`
//! as bearer tokens. The OAuth type signs in on gitlab.com only: the standard OAuth flow has
//! fixed endpoints, so a self-managed instance connects with an access token. GitLab's hosted MCP
//! server joins this provider as its `mcp_oauth` type (`tools::mcp_catalog`). Both types also
//! offer sandbox egress (`connections::catalog::gitlab`).
use super::{
    ToolProvider,
    rest::{self, Tool, Verb, tool},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_DEVELOPER_TOOLS, model};
use crate::proto::tilde::types::v1 as types;
use futures::future::BoxFuture;
use serde_json::{Value, json};

pub const DEFAULT_URL: &str = "https://gitlab.com";

pub fn definition() -> model::Provider {
    let mut oauth = model::OAuth::standard("https://gitlab.com/oauth/token");
    oauth.authorization_url = Some("https://gitlab.com/oauth/authorize".into());
    oauth.scopes = vec!["api".into()];
    let capabilities = vec![model::Capability::Tool, model::Capability::Egress];
    model::Provider {
        account_name_label: Some("GitLab group or user".into()),
        icon_url: Some("/provider-icons/gitlab.svg".into()),
        instructions: Some(
            "Connect GitLab so agents can work with issues, merge requests and pipelines, and so sandboxes can clone and push through egress. Use a personal, group or project access token with the api scope (any GitLab instance), or a GitLab OAuth application on gitlab.com with the api scope."
                .into(),
        ),
        id: "gitlab".into(),
        name: "GitLab".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![
            model::ConnectionType {
                mcp: None,
                id: "token".into(),
                name: "GitLab access token".into(),
                credential_source: model::CredentialSource::Static {
                    schema: json!({"type":"object","properties":{
                        "token":{"type":"string","title":"Access token","description":"A personal, group or project access token with the api scope, e.g. glpat-...","minLength":1,"writeOnly":true},
                        "base_url":{"type":"string","title":"Self-managed GitLab URL","description":"Only for a self-managed instance: its URL, e.g. https://gitlab.example.com. Empty: gitlab.com.","format":"uri","pattern":"^https://[^/?#@]+/?$","maxLength":2048}
                    },"required":["token"],"additionalProperties":false}),
                },
                capabilities: capabilities.clone(),
            },
            model::ConnectionType {
                mcp: None,
                id: "oauth".into(),
                name: "GitLab OAuth app".into(),
                credential_source: model::CredentialSource::OAuth {
                    grant: model::OAuthGrant::AuthorizationCode,
                    configuration: Box::new(oauth),
                    additional_schema: None,
                },
                capabilities,
            },
        ],
    }
}

/// The instance the connection reaches: its `base_url`, else gitlab.com.
pub fn base(access: &Access) -> ToolResult<&str> {
    match model::optional(&access.values, "base_url") {
        Some(url) => rest::https(url),
        None => Ok(DEFAULT_URL),
    }
}

/// The connection value holding the bearer token differs by connection type.
pub struct Gitlab(&'static str);
pub static TOKEN: Gitlab = Gitlab("token");
pub static OAUTH: Gitlab = Gitlab("access_token");

const PAGE: &str = "per_page page";

use Verb::{Get, Post, Put};
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("list_projects", "Listed GitLab projects", "List projects the connection can see. Set `membership` to list only the ones it belongs to.", Get, "/api/v4/projects", "search membership owned order_by sort per_page page"),
    tool("get_project", "Read a GitLab project", "Get a project.", Get, "/api/v4/projects/{project}", ""),
    tool("get_file", "Read a GitLab file", "Get a file's metadata and base64 content at a ref.", Get, "/api/v4/projects/{project}/repository/files/{file_path}", "!ref"),
    tool("list_issues", "Listed GitLab issues", "List a project's issues.", Get, "/api/v4/projects/{project}/issues", "state labels search assignee_username author_username order_by sort per_page page"),
    tool("get_issue", "Read a GitLab issue", "Get an issue.", Get, "/api/v4/projects/{project}/issues/{issue_iid}", ""),
    tool("create_issue", "Created a GitLab issue", "Create an issue.", Post, "/api/v4/projects/{project}/issues", "!title description labels assignee_ids"),
    tool("update_issue", "Updated a GitLab issue", "Update an issue; `state_event` closes or reopens it.", Put, "/api/v4/projects/{project}/issues/{issue_iid}", "title description labels assignee_ids state_event"),
    tool("list_issue_notes", "Listed issue comments", "List the comments on an issue.", Get, "/api/v4/projects/{project}/issues/{issue_iid}/notes", "order_by sort per_page page"),
    tool("create_issue_note", "Commented on a GitLab issue", "Comment on an issue.", Post, "/api/v4/projects/{project}/issues/{issue_iid}/notes", "!body"),
    tool("list_merge_requests", "Listed merge requests", "List a project's merge requests.", Get, "/api/v4/projects/{project}/merge_requests", "state source_branch target_branch search labels author_username order_by sort per_page page"),
    tool("get_merge_request", "Read a merge request", "Get a merge request.", Get, "/api/v4/projects/{project}/merge_requests/{merge_request_iid}", ""),
    tool("list_merge_request_diffs", "Read a merge request's diffs", "List the file diffs of a merge request.", Get, "/api/v4/projects/{project}/merge_requests/{merge_request_iid}/diffs", PAGE),
    tool("create_merge_request", "Opened a merge request", "Create a merge request.", Post, "/api/v4/projects/{project}/merge_requests", "!source_branch !target_branch !title description labels assignee_ids reviewer_ids remove_source_branch squash"),
    tool("update_merge_request", "Updated a merge request", "Update a merge request; `state_event` closes or reopens it.", Put, "/api/v4/projects/{project}/merge_requests/{merge_request_iid}", "title description target_branch labels assignee_ids reviewer_ids state_event"),
    tool("list_merge_request_notes", "Listed merge request comments", "List the comments on a merge request.", Get, "/api/v4/projects/{project}/merge_requests/{merge_request_iid}/notes", "order_by sort per_page page"),
    tool("create_merge_request_note", "Commented on a merge request", "Comment on a merge request.", Post, "/api/v4/projects/{project}/merge_requests/{merge_request_iid}/notes", "!body"),
    tool("list_pipelines", "Listed pipelines", "List a project's pipelines.", Get, "/api/v4/projects/{project}/pipelines", "ref sha status source order_by sort per_page page"),
    tool("get_pipeline", "Read a pipeline", "Get a pipeline.", Get, "/api/v4/projects/{project}/pipelines/{pipeline_id}", ""),
    tool("list_pipeline_jobs", "Listed pipeline jobs", "List the jobs of a pipeline.", Get, "/api/v4/projects/{project}/pipelines/{pipeline_id}/jobs", "scope include_retried per_page page"),
    tool("get_job_log", "Read a job log", "Get the log (trace) of a job as text.", Get, "/api/v4/projects/{project}/jobs/{job_id}/trace", ""),
];

fn property(name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    let id = |description: &str| json!({"type":"integer","minimum":1,"description":description});
    match name {
        "project" => {
            json!({"type":["string","integer"],"description":"Project path (group/project) or numeric ID."})
        }
        "issue_iid" => id("Issue number within the project."),
        "merge_request_iid" => id("Merge request number within the project."),
        "pipeline_id" => id("Pipeline ID."),
        "job_id" => id("Job ID."),
        "page" => id("Result page, from 1."),
        "per_page" => {
            json!({"type":"integer","minimum":1,"maximum":100,"description":"Results per page."})
        }
        "file_path" => text("File path within the repository, e.g. src/main.rs."),
        "ref" => text("Branch, tag or commit SHA."),
        "labels" => text("Comma-separated label names."),
        "state" => text("opened, closed, merged (merge requests) or all."),
        "state_event" => json!({"enum":["close","reopen"]}),
        "sort" => json!({"enum":["asc","desc"]}),
        "membership" | "owned" | "include_retried" | "remove_source_branch" | "squash" => {
            json!({"type":"boolean"})
        }
        "assignee_ids" | "reviewer_ids" => {
            json!({"type":"array","items":{"type":"integer"},"description":"User IDs."})
        }
        "description" | "body" => text("Markdown."),
        _ => json!({"type":"string"}),
    }
}

impl ToolProvider for Gitlab {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            .map(|tool| tool.definition("gitlab", property))
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
            let request = rest::request(access, "gitlab_api", base(access)?, &tool.rest, input)?
                .bearer_auth(access.secret(self.0)?);
            rest::send(request).await
        })
    }
}
