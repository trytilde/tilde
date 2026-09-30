//! GitHub REST tools for issues, pull requests, code, Actions, checks, releases, reactions,
//! search, users and organizations. Every tool is one call to api.github.com; a tool's listed
//! fields document the common parameters and any other field passes through (as the query of a
//! read, otherwise into the JSON body), so agents can use the rest of GitHub's REST parameters.
use super::{
    ToolProvider,
    rest::{self, Rest, Verb},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Map, Value, json};

const API: &str = "https://api.github.com";

/// GitHub App installations and OAuth connections hold an `access_token` (the installation
/// token is refreshed by the connection); personal access tokens are a static `token`.
pub struct Github {
    key: &'static str,
}
pub static ACCESS_TOKEN: Github = Github {
    key: "access_token",
};
pub static PERSONAL_TOKEN: Github = Github { key: "token" };

struct Tool {
    rest: Rest,
    /// Space-separated input fields beside the path placeholders; `!` marks required ones.
    fields: &'static str,
}
const fn tool(
    name: &'static str,
    summary: &'static str,
    description: &'static str,
    verb: Verb,
    path: &'static str,
    fields: &'static str,
) -> Tool {
    Tool {
        rest: Rest {
            name,
            summary,
            description,
            verb,
            path,
            query: &[],
        },
        fields,
    }
}
const PAGE: &str = "max_results page";

use Verb::{Delete, Get, Patch, Post, Put};
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("github_create_issue", "Created a GitHub issue", "Create a GitHub issue.", Post, "/repos/{owner}/{repo}/issues", "!title body labels assignees milestone"),
    tool("github_update_issue", "Updated a GitHub issue", "Update GitHub issue fields; `state` is open or closed.", Patch, "/repos/{owner}/{repo}/issues/{issue_number}", "title body state state_reason labels assignees milestone"),
    tool("github_get_issue", "Read a GitHub issue", "Get a GitHub issue.", Get, "/repos/{owner}/{repo}/issues/{issue_number}", ""),
    tool("github_search_issues", "Searched GitHub issues", "Search GitHub issues and pull requests with GitHub search syntax, e.g. `repo:owner/name is:open label:bug`.", Get, "/search/issues", "!query sort order max_results page"),
    tool("github_list_issue_comments", "Listed issue comments", "List issue or PR conversation comments.", Get, "/repos/{owner}/{repo}/issues/{issue_number}/comments", "since max_results page"),
    tool("github_add_issue_comment", "Commented on a GitHub issue", "Add an issue or PR conversation comment.", Post, "/repos/{owner}/{repo}/issues/{issue_number}/comments", "!body"),
    tool("github_update_issue_comment", "Updated an issue comment", "Update an issue comment.", Patch, "/repos/{owner}/{repo}/issues/comments/{comment_id}", "!body"),
    tool("github_delete_issue_comment", "Deleted an issue comment", "Delete an issue comment.", Delete, "/repos/{owner}/{repo}/issues/comments/{comment_id}", ""),
    tool("github_list_labels", "Listed repository labels", "List repository labels.", Get, "/repos/{owner}/{repo}/labels", PAGE),
    tool("github_add_labels_to_issue", "Labelled a GitHub issue", "Add labels to an issue.", Post, "/repos/{owner}/{repo}/issues/{issue_number}/labels", "!labels"),
    tool("github_remove_label_from_issue", "Removed a label from an issue", "Remove a label from an issue.", Delete, "/repos/{owner}/{repo}/issues/{issue_number}/labels/{name}", ""),
    tool("github_list_milestones", "Listed milestones", "List repository milestones.", Get, "/repos/{owner}/{repo}/milestones", "state max_results page"),
    tool("github_create_pull_request", "Opened a pull request", "Create a pull request.", Post, "/repos/{owner}/{repo}/pulls", "!title !head !base body draft"),
    tool("github_update_pull_request", "Updated a pull request", "Update a pull request; `state` is open or closed.", Patch, "/repos/{owner}/{repo}/pulls/{pull_number}", "title body state base"),
    tool("github_list_pull_requests", "Listed pull requests", "List pull requests.", Get, "/repos/{owner}/{repo}/pulls", "state head base sort direction max_results page"),
    tool("github_get_pull_request", "Read a pull request", "Get pull request details.", Get, "/repos/{owner}/{repo}/pulls/{pull_number}", ""),
    tool("github_merge_pull_request", "Merged a pull request", "Merge a pull request; `sha` must match the head to merge.", Put, "/repos/{owner}/{repo}/pulls/{pull_number}/merge", "commit_title commit_message merge_method sha"),
    tool("github_list_pull_request_files", "Listed pull request files", "List files changed in a pull request.", Get, "/repos/{owner}/{repo}/pulls/{pull_number}/files", PAGE),
    tool("github_list_pull_request_commits", "Listed pull request commits", "List commits in a pull request.", Get, "/repos/{owner}/{repo}/pulls/{pull_number}/commits", PAGE),
    tool("github_request_pull_request_reviewers", "Requested pull request reviewers", "Request pull request reviewers.", Post, "/repos/{owner}/{repo}/pulls/{pull_number}/requested_reviewers", "reviewers team_reviewers"),
    tool("github_create_pull_request_review", "Reviewed a pull request", "Create a pull request review; omit `event` to leave it pending.", Post, "/repos/{owner}/{repo}/pulls/{pull_number}/reviews", "body event commit_id comments"),
    tool("github_submit_pull_request_review", "Submitted a pull request review", "Submit a pending pull request review.", Post, "/repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}/events", "!event body"),
    tool("github_list_pull_request_reviews", "Listed pull request reviews", "List pull request reviews.", Get, "/repos/{owner}/{repo}/pulls/{pull_number}/reviews", PAGE),
    tool("github_dismiss_pull_request_review", "Dismissed a pull request review", "Dismiss a pull request review.", Put, "/repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}/dismissals", "!message"),
    tool("github_list_pull_request_review_comments", "Listed review comments", "List pull request review comments.", Get, "/repos/{owner}/{repo}/pulls/{pull_number}/comments", PAGE),
    tool("github_create_pull_request_review_comment", "Commented on a pull request diff", "Create a pull request review comment on a line of the diff; `path` is the file.", Post, "/repos/{owner}/{repo}/pulls/{pull_number}/comments", "!body !commit_id !path line side start_line start_side"),
    tool("github_reply_to_pull_request_review_comment", "Replied to a review comment", "Reply to a pull request review comment.", Post, "/repos/{owner}/{repo}/pulls/{pull_number}/comments/{comment_id}/replies", "!body"),
    tool("github_update_pull_request_review_comment", "Updated a review comment", "Update a pull request review comment.", Patch, "/repos/{owner}/{repo}/pulls/comments/{comment_id}", "!body"),
    tool("github_delete_pull_request_review_comment", "Deleted a review comment", "Delete a pull request review comment.", Delete, "/repos/{owner}/{repo}/pulls/comments/{comment_id}", ""),
    tool("github_list_repos", "Listed repositories", "List repositories of the authenticated user (installation tokens: use github_list_org_repos).", Get, "/user/repos", "visibility affiliation type sort max_results page"),
    tool("github_get_repo", "Read a repository", "Get repository details.", Get, "/repos/{owner}/{repo}", ""),
    tool("github_list_branches", "Listed branches", "List repository branches.", Get, "/repos/{owner}/{repo}/branches", PAGE),
    tool("github_get_branch", "Read a branch", "Get a repository branch.", Get, "/repos/{owner}/{repo}/branches/{branch*}", ""),
    tool("github_create_branch", "Created a branch", "Create a Git ref for a branch; `ref` is the full name (refs/heads/<branch>) and `sha` the commit it points to.", Post, "/repos/{owner}/{repo}/git/refs", "!ref !sha"),
    tool("github_compare_refs", "Compared refs", "Compare two refs; `basehead` is BASE...HEAD.", Get, "/repos/{owner}/{repo}/compare/{basehead*}", PAGE),
    tool("github_list_tags", "Listed tags", "List repository tags.", Get, "/repos/{owner}/{repo}/tags", PAGE),
    tool("github_list_collaborators", "Listed collaborators", "List repository collaborators.", Get, "/repos/{owner}/{repo}/collaborators", "affiliation max_results page"),
    tool("github_get_file_content", "Read a repository file", "Get repository file or directory content (file content is base64).", Get, "/repos/{owner}/{repo}/contents/{path*}", "ref"),
    tool("github_create_or_update_file", "Wrote a repository file", "Create or update a repository file; `content` is base64 and `sha` is the blob being replaced.", Put, "/repos/{owner}/{repo}/contents/{path*}", "!message !content sha branch"),
    tool("github_delete_file", "Deleted a repository file", "Delete a repository file; `sha` is the blob being deleted.", Delete, "/repos/{owner}/{repo}/contents/{path*}", "!message !sha branch"),
    tool("github_get_tree", "Read a Git tree", "Get a Git tree.", Get, "/repos/{owner}/{repo}/git/trees/{sha}", "recursive"),
    tool("github_get_blob", "Read a Git blob", "Get a Git blob.", Get, "/repos/{owner}/{repo}/git/blobs/{sha}", ""),
    tool("github_create_blob", "Created a Git blob", "Create a Git blob.", Post, "/repos/{owner}/{repo}/git/blobs", "!content encoding"),
    tool("github_create_tree", "Created a Git tree", "Create a Git tree.", Post, "/repos/{owner}/{repo}/git/trees", "!tree base_tree"),
    tool("github_create_commit", "Created a Git commit", "Create a Git commit.", Post, "/repos/{owner}/{repo}/git/commits", "!message !tree parents"),
    tool("github_update_ref", "Updated a Git ref", "Update a Git ref; `ref` is e.g. heads/main.", Patch, "/repos/{owner}/{repo}/git/refs/{ref*}", "!sha force"),
    tool("github_list_workflows", "Listed workflows", "List GitHub Actions workflows.", Get, "/repos/{owner}/{repo}/actions/workflows", PAGE),
    tool("github_get_workflow", "Read a workflow", "Get a GitHub Actions workflow.", Get, "/repos/{owner}/{repo}/actions/workflows/{workflow_id}", ""),
    tool("github_dispatch_workflow", "Dispatched a workflow", "Dispatch a GitHub Actions workflow.", Post, "/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches", "!ref inputs"),
    tool("github_list_workflow_runs", "Listed workflow runs", "List workflow runs.", Get, "/repos/{owner}/{repo}/actions/runs", "branch status event actor head_sha max_results page"),
    tool("github_get_workflow_run", "Read a workflow run", "Get workflow run details.", Get, "/repos/{owner}/{repo}/actions/runs/{run_id}", ""),
    tool("github_cancel_workflow_run", "Cancelled a workflow run", "Cancel a workflow run.", Post, "/repos/{owner}/{repo}/actions/runs/{run_id}/cancel", ""),
    tool("github_rerun_workflow_run", "Re-ran a workflow run", "Rerun a workflow run.", Post, "/repos/{owner}/{repo}/actions/runs/{run_id}/rerun", ""),
    tool("github_list_workflow_jobs", "Listed workflow jobs", "List jobs for a workflow run.", Get, "/repos/{owner}/{repo}/actions/runs/{run_id}/jobs", "filter max_results page"),
    tool("github_get_workflow_run_logs", "Found workflow run logs", "Get a short-lived download URL for a workflow run's log archive.", Get, "/repos/{owner}/{repo}/actions/runs/{run_id}/logs", ""),
    tool("github_list_artifacts", "Listed workflow artifacts", "List workflow artifacts.", Get, "/repos/{owner}/{repo}/actions/artifacts", "name max_results page"),
    tool("github_list_check_runs_for_ref", "Listed check runs", "List check runs for a ref.", Get, "/repos/{owner}/{repo}/commits/{ref*}/check-runs", "check_name status filter max_results page"),
    tool("github_get_check_run", "Read a check run", "Get a check run.", Get, "/repos/{owner}/{repo}/check-runs/{check_run_id}", ""),
    tool("github_create_check_run", "Created a check run", "Create a check run (GitHub App tokens only).", Post, "/repos/{owner}/{repo}/check-runs", "!name !head_sha status conclusion details_url external_id output"),
    tool("github_update_check_run", "Updated a check run", "Update a check run (GitHub App tokens only).", Patch, "/repos/{owner}/{repo}/check-runs/{check_run_id}", "name status conclusion details_url output"),
    tool("github_list_commit_statuses", "Listed commit statuses", "List commit statuses.", Get, "/repos/{owner}/{repo}/commits/{ref*}/statuses", PAGE),
    tool("github_create_commit_status", "Set a commit status", "Create a commit status; `state` is error, failure, pending or success.", Post, "/repos/{owner}/{repo}/statuses/{sha}", "!state target_url description context"),
    tool("github_list_releases", "Listed releases", "List releases.", Get, "/repos/{owner}/{repo}/releases", PAGE),
    tool("github_get_release", "Read a release", "Get a release.", Get, "/repos/{owner}/{repo}/releases/{release_id}", ""),
    tool("github_create_release", "Created a release", "Create a release.", Post, "/repos/{owner}/{repo}/releases", "!tag_name target_commitish name body draft prerelease generate_release_notes"),
    tool("github_update_release", "Updated a release", "Update a release.", Patch, "/repos/{owner}/{repo}/releases/{release_id}", "tag_name target_commitish name body draft prerelease"),
    tool("github_delete_release", "Deleted a release", "Delete a release.", Delete, "/repos/{owner}/{repo}/releases/{release_id}", ""),
    tool("github_create_reaction", "Reacted to a comment", "Create a reaction on an issue comment.", Post, "/repos/{owner}/{repo}/issues/comments/{comment_id}/reactions", "!content"),
    tool("github_delete_reaction", "Removed a reaction", "Delete a reaction from an issue comment.", Delete, "/repos/{owner}/{repo}/issues/comments/{comment_id}/reactions/{reaction_id}", ""),
    tool("github_list_reactions", "Listed reactions", "List reactions for an issue comment.", Get, "/repos/{owner}/{repo}/issues/comments/{comment_id}/reactions", "content max_results page"),
    tool("github_search_code", "Searched GitHub code", "Search GitHub code with GitHub search syntax, e.g. `fn main repo:owner/name`.", Get, "/search/code", "!query max_results page"),
    tool("github_search_repositories", "Searched GitHub repositories", "Search GitHub repositories.", Get, "/search/repositories", "!query sort order max_results page"),
    tool("github_search_users", "Searched GitHub users", "Search GitHub users.", Get, "/search/users", "!query sort order max_results page"),
    tool("github_get_authenticated_user", "Read the GitHub account", "Get the authenticated user (not available to GitHub App installation tokens).", Get, "/user", ""),
    tool("github_get_user", "Read a GitHub user", "Get a GitHub user.", Get, "/users/{username}", ""),
    tool("github_list_orgs", "Listed organizations", "List organizations for the authenticated user.", Get, "/user/orgs", PAGE),
    tool("github_list_org_repos", "Listed organization repositories", "List organization repositories.", Get, "/orgs/{org}/repos", "type sort max_results page"),
    tool("github_list_org_members", "Listed organization members", "List organization members.", Get, "/orgs/{org}/members", "role max_results page"),
];

fn property(tool: &str, name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    let id = |description: &str| json!({"type":"integer","minimum":1,"description":description});
    let strings = |description: &str| json!({"type":"array","items":{"type":"string"},"description":description});
    match name {
        "owner" => text("Repository owner (user or organization login)."),
        "repo" => text("Repository name."),
        "issue_number" => id("Issue or pull request number."),
        "pull_number" => id("Pull request number."),
        "comment_id" => id("Comment ID."),
        "review_id" => id("Review ID."),
        "run_id" => id("Workflow run ID."),
        "check_run_id" => id("Check run ID."),
        "release_id" => id("Release ID."),
        "reaction_id" => id("Reaction ID."),
        "milestone" => id("Milestone number."),
        "line" | "start_line" => id("Line in the diff's file."),
        "page" => id("Result page, from 1."),
        "max_results" => {
            json!({"type":"integer","minimum":1,"maximum":100,"description":"Results per page."})
        }
        "query" => text("GitHub search query."),
        "path" => text("File path within the repository, e.g. src/main.rs."),
        "branch" => text("Branch name."),
        "ref" => text("Git ref: a branch, tag or commit SHA."),
        "sha" | "head_sha" | "commit_id" | "base_tree" => text("Commit, tree or blob SHA."),
        // A commit names its tree by SHA; only github_create_tree takes entries.
        "tree" if tool == "github_create_commit" => text("SHA of the commit's tree."),
        "basehead" => text("BASE...HEAD, e.g. main...feature."),
        "workflow_id" => text("Workflow file name (ci.yml) or ID."),
        "name" => text("Name."),
        "event" => json!({"enum":["APPROVE","REQUEST_CHANGES","COMMENT"]}),
        "side" | "start_side" => json!({"enum":["LEFT","RIGHT"]}),
        "merge_method" => json!({"enum":["merge","squash","rebase"]}),
        "direction" | "order" => json!({"enum":["asc","desc"]}),
        "content" => text(
            "File or blob content (base64 for files), or a reaction: +1, -1, laugh, confused, heart, hooray, rocket, eyes.",
        ),
        "draft" | "prerelease" | "generate_release_notes" | "force" | "recursive" => {
            json!({"type":"boolean"})
        }
        "labels" => strings("Label names."),
        "assignees" => strings("Assignee logins."),
        "reviewers" => strings("Reviewer logins."),
        "team_reviewers" => strings("Team slugs."),
        "parents" => strings("Parent commit SHAs."),
        "tree" => {
            json!({"type":"array","items":{"type":"object"},"description":"Entries with path, mode, type and sha or content."})
        }
        "comments" => {
            json!({"type":"array","items":{"type":"object"},"description":"Draft review comments with path, body and line."})
        }
        "inputs" => json!({"type":"object","description":"Workflow dispatch inputs."}),
        "output" => {
            json!({"type":"object","description":"Check run output: title, summary and text."})
        }
        _ => json!({"type":"string"}),
    }
}

fn definition(tool: &Tool) -> types::ToolDefinition {
    let mut properties = Map::new();
    let mut required = vec![];
    let placeholders = tool
        .rest
        .path
        .split('{')
        .skip(1)
        .filter_map(|part| part.split_once('}'))
        .map(|(name, _)| format!("!{}", name.trim_end_matches('*')));
    for field in placeholders.chain(tool.fields.split_whitespace().map(str::to_owned)) {
        let name = field.trim_start_matches('!');
        if field.starts_with('!') {
            required.push(name.to_owned());
        }
        properties.insert(name.to_owned(), property(tool.rest.name, name));
    }
    rest::definition(
        "github",
        tool.rest.name,
        tool.rest.summary,
        tool.rest.description,
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":true}),
        tool.rest.verb.hints(),
    )
}

impl ToolProvider for Github {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS.iter().map(definition).collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let tool = TOOLS
                .iter()
                .find(|tool| tool.rest.name == name)
                .ok_or_else(|| ConnectError::not_found("Unsupported GitHub tool"))?;
            let Value::Object(mut input) = input else {
                return Err(ConnectError::invalid_argument("Input must be an object"));
            };
            if tool.rest.verb == Get {
                if let Some(query) = input.remove("query") {
                    input.insert("q".into(), query);
                }
                if let Some(max) = input.remove("max_results") {
                    input.insert("per_page".into(), max);
                }
            }
            // Deleting a file is a DELETE with a JSON body; everything but the path moves there.
            let body = (tool.rest.name == "github_delete_file").then(|| {
                let path = ["owner", "repo", "path"].map(|key| (key, input.remove(key)));
                let body = std::mem::take(&mut input);
                for (key, value) in path {
                    if let Some(value) = value {
                        input.insert(key.into(), value);
                    }
                }
                Value::Object(body)
            });
            let mut request = rest::request(access, "github_api", API, &tool.rest, input.into())?
                .bearer_auth(access.secret(self.key)?)
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header("User-Agent", "tilde");
            if let Some(body) = body {
                request = request.json(&body);
            }
            let response = request
                .send()
                .await
                .map_err(|_| ConnectError::unavailable("GitHub could not be reached"))?;
            let header = |name: &str| {
                response
                    .headers()
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned)
            };
            let rate_limit = json!({
                "limit": header("x-ratelimit-limit"),
                "remaining": header("x-ratelimit-remaining"),
                "reset": header("x-ratelimit-reset"),
            });
            // Log archives answer with a redirect to a short-lived download URL.
            if response.status().is_redirection() {
                return Ok(json!({
                    "ok": true,
                    "status": response.status().as_u16(),
                    "data": {"url": header("location")},
                    "rate_limit": rate_limit,
                }));
            }
            let mut output = rest::read(response).await?;
            output["ok"] = true.into();
            output["rate_limit"] = rate_limit;
            Ok(output)
        })
    }
}
