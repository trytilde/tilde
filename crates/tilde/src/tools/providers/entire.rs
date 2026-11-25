//! Entire (entire.com) projects and repositories over its documented API: the Core (control
//! plane) lists projects and repositories and creates repositories; a repository's region API
//! serves its branches, commits and files. The connection is a service account, whose secret is
//! exchanged for a short-lived bearer token (`connections::catalog::entire`); it also offers
//! sandbox egress to the repositories' git remotes.
//!
//! Region calls follow Entire's routing rule: the Core repository directory names each
//! repository's placements, and the one matching `primaries.processing` gives the region API
//! host and the ID the region API knows the repository by. The token is only sent to hosts
//! under `api.entire.io` or `api.entire.com`.
use super::{
    ToolProvider,
    rest::{self, Tool, Verb, tool},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{catalog::entire, categories::CATEGORY_DEVELOPER_TOOLS, mints, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use secrecy::ExposeSecret;
use serde_json::{Value, json};

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("Entire project".into()),
        icon_url: Some("/provider-icons/entire.svg".into()),
        instructions: Some(
            "Connect an Entire project so agents can work with its repositories and sandboxes can clone and push through egress. Create a service account in the project (Settings > Service accounts, or `entire service-account create`), grant it writer on the repositories or the project, and create a secret with `entire service-account secret create`. The cluster host is the host of the repositories' clone URLs (entire://<cluster host>/et/<project>/<repo>)."
                .into(),
        ),
        id: "entire".into(),
        name: "Entire".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "service_account".into(),
            name: "Entire service account".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "secret":{"type":"string","title":"Service account secret","description":"Starts with ent_svc_. Secrets expire (90 days by default): replace it here before it does.","pattern":"^ent_svc_[a-z]+_[0-9A-Za-z]{26}_[0-9A-Za-z]{26}_\\S+$","writeOnly":true},
                    "cluster":{"type":"string","title":"Cluster host","description":"e.g. aws-us-east-2.entire.io","pattern":"^[a-z0-9-]+\\.entire\\.(io|com)$","maxLength":253}
                },"required":["secret","cluster"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool, model::Capability::Egress],
        }],
    }
}

pub struct Entire;

use Verb::{Get, Post};
#[rustfmt::skip]
const CORE: &[Tool] = &[
    tool("list_projects", "Listed Entire projects", "List the projects the service account can see, or one by name.", Get, "/api/v1/projects", "name pageSize pageToken"),
    tool("list_repositories", "Listed Entire repositories", "List the repositories the service account can read, with their regions (placements). `q` matches part of the full name (project/repo).", Get, "/api/v1/repos", "q projectId pageSize pageToken"),
    tool("get_repository", "Read an Entire repository", "Get a repository by ID, with its lifecycle state and placements.", Get, "/api/v1/repos/{repoId}", ""),
    tool("create_repository", "Created an Entire repository", "Create a repository in a project the service account may create in. It is ready once its state is active; clone it at entire://<cluster host>/et/<project>/<name>.", Post, "/api/v1/repos", "!name !projectId"),
];
#[rustfmt::skip]
const REGION: &[Tool] = &[
    tool("list_branches", "Listed repository branches", "List a repository's branches.", Get, "/api/v1/git/repos/{repoId}/branches", "!repository search pageSize pageToken"),
    tool("list_commits", "Listed repository commits", "List commits reachable from a ref, newest first, optionally touching a path or within a time range.", Get, "/api/v1/git/repos/{repoId}/commits", "!repository ref path since until author pageSize pageToken"),
    tool("get_file", "Read a repository file", "Read a file at a branch, tag or commit.", Get, "/api/v1/git/repos/{repoId}/raw/{path*}", "!repository !ref"),
];

fn property(name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    match name {
        "repository" => text("Full name: project/repo."),
        "repoId" | "projectId" => {
            json!({"type":"string","pattern":"^[0-9A-HJKMNP-TV-Z]{26}$","description":"ID (a ULID) from a list."})
        }
        "ref" => text("Branch, tag or commit SHA."),
        "path" => text("File path within the repository."),
        "since" | "until" => text("RFC 3339 timestamp."),
        "pageSize" => json!({"type":"integer","minimum":1,"maximum":500}),
        "pageToken" => text("`nextPageToken` of the previous result."),
        _ => json!({"type":"string"}),
    }
}

impl ToolProvider for Entire {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let core = CORE.iter().map(|tool| tool.definition("entire", property));
        // The repository is named, not its regional ID, which `invoke` looks up.
        let region = REGION
            .iter()
            .map(|tool| rest::hide(tool.definition("entire", property), "repoId"));
        core.chain(region).collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        mut input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let minted = mints::mint(
                access,
                "entire",
                "service_account",
                &Default::default(),
                "tool",
            )
            .await?;
            let authorized = |mut request: reqwest::RequestBuilder| {
                for (header, value) in minted.headers() {
                    request = request.header(header, value.expose_secret());
                }
                request
            };
            let core = entire::core(access)?;
            if let Ok(tool) = rest::find(CORE, name) {
                let request = rest::request(access, "entire_api", &core, &tool.rest, input)?;
                return rest::send(authorized(request)).await;
            }
            let tool = rest::find(REGION, name)?;
            let repository = input["repository"].as_str().unwrap_or_default().to_owned();
            let directory = access
                .json(authorized(
                    access
                        .http
                        .client
                        .get(format!("{core}/api/v1/repos"))
                        .query(&[("q", repository.as_str())]),
                ))
                .await?;
            let (base, id) = placement(access, &directory, &repository)?;
            if let Some(fields) = input.as_object_mut() {
                fields.remove("repository");
                fields.insert("repoId".into(), id.into());
            }
            let request = rest::request(access, "entire_region_api", &base, &tool.rest, input)?;
            rest::send(authorized(request)).await
        })
    }
}

/// The region API base and regional ID of `repository`'s processing primary in a directory
/// listing.
fn placement(access: &Access, directory: &Value, repository: &str) -> ToolResult<(String, String)> {
    let repo = directory["repos"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|repo| {
            repo["full_name"]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(repository))
        })
        .ok_or_else(|| ConnectError::not_found(format!("No Entire repository {repository}")))?;
    let primary = repo["primaries"]["processing"].as_str();
    let found = repo["placements"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|placement| placement["id"].as_str().is_some() && placement["id"].as_str() == primary)
        .and_then(|placement| Some((placement["apiBaseUrl"].as_str()?, placement["id"].as_str()?)));
    let Some((base, id)) = found else {
        return Err(ConnectError::unavailable(
            "The Entire repository has no processing region yet; try again shortly",
        ));
    };
    let trusted = url::Url::parse(base).ok().is_some_and(|url| {
        url.scheme() == "https"
            && url.port().is_none()
            && url.host_str().is_some_and(|host| {
                host.ends_with(".api.entire.io") || host.ends_with(".api.entire.com")
            })
    });
    if !trusted && !access.endpoints.0.contains_key("entire_region_api") {
        return Err(ConnectError::failed_precondition(
            "Entire named a region API outside api.entire.io",
        ));
    }
    Ok((base.trim_end_matches('/').to_owned(), id.to_owned()))
}
