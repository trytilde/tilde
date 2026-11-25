//! GitLab egress: git fetch and push, and a small REST allowlist under `/api/v4`, on gitlab.com
//! or the connection's self-managed instance (`base_url`).
//!
//! A grant's config narrows it to projects (`group/project`, nested groups included; empty:
//! every project the token reaches), the branches pushes may update (`push.branches`, globs;
//! empty: no push) and whether merge requests and their comments may be written. Pushes never
//! delete refs, rewrite them or touch the default branch. GitLab tokens cannot be narrowed per
//! request, so the checks here are the only narrowing.
//!
//! The REST API names a project by ID or by its URL-encoded path (`group%2Fproject`). A grant
//! limited to projects only admits the path form, which is checked against the grant; other
//! encoded slashes in a path are refused (`rules::matches`), so files and branches with slashes
//! are read through git instead.
use crate::chat::providers::Access;
use crate::connections::egress::{
    Decision, Egress, EgressHook, EgressProtocol, EgressRequest, EnvTemplate, GitAuth, GitService,
    forge::{self, strings},
    rules,
};
use crate::connections::model;
use crate::error::Error;
use crate::tools::providers::gitlab::DEFAULT_URL;
use serde_json::{Value, json};
use std::sync::LazyLock;

pub struct Gitlab(Egress);
static HOOK: LazyLock<Gitlab> = LazyLock::new(|| Gitlab(definition()));
pub fn hook() -> &'static Gitlab {
    &HOOK
}

fn definition() -> Egress {
    Egress {
        protocols: vec![
            EgressProtocol::Git {
                upstream: DEFAULT_URL,
                auth: GitAuth::Basic("oauth2"),
                replicas: false,
                services: vec![GitService::UploadPack, GitService::ReceivePack],
                deny_delete: true,
                deny_force: true,
                deny_default_branch: true,
            },
            EgressProtocol::Http {
                upstream: DEFAULT_URL,
                allow: vec![
                    "GET /api/v4/projects/{project}/**",
                    "POST /api/v4/projects/{project}/merge_requests",
                    "PUT /api/v4/projects/{project}/merge_requests/*",
                    "POST /api/v4/projects/{project}/merge_requests/*/notes",
                    "POST /api/v4/projects/{project}/merge_requests/*/discussions",
                    "POST /api/v4/projects/{project}/merge_requests/*/discussions/*/notes",
                    "POST /api/v4/projects/{project}/issues/*/notes",
                    "GET /api/v4/user",
                    "GET /api/v4/version",
                ],
                deny: vec![
                    "* /api/v4/projects/*/variables/**",
                    "* /api/v4/projects/*/hooks/**",
                    "* /api/v4/projects/*/deploy_keys/**",
                    "* /api/v4/projects/*/deploy_tokens/**",
                    "* /api/v4/projects/*/access_tokens/**",
                    "* /api/v4/projects/*/triggers/**",
                    "* /api/v4/projects/*/secure_files/**",
                    "* /api/v4/projects/*/runners/**",
                    "* /api/v4/projects/*/integrations/**",
                    "* /api/v4/projects/*/services/**",
                    "* /api/v4/projects/*/remote_mirrors/**",
                    "* /api/v4/projects/*/export/**",
                ],
                // glab and python-gitlab read these; glab sends the token as `PRIVATE-TOKEN`.
                wiring: vec![
                    EnvTemplate {
                        name: "GITLAB_HOST",
                        value: "{url}",
                    },
                    EnvTemplate {
                        name: "GITLAB_TOKEN",
                        value: "{token}",
                    },
                ],
            },
        ],
        config_schema: json!({
            "type": "object",
            "properties": {
                "repositories": {
                    "type": "array",
                    "title": "Projects",
                    "items": {"type": "string", "pattern": "^[A-Za-z0-9_.-]+(/[A-Za-z0-9_.-]+)+$"},
                    "uniqueItems": true,
                    "x-tilde-options": "gitlab.list_projects",
                    "description": "Project paths (group/project). Empty: every project the connection can reach"
                },
                "push": forge::push_schema("Never the default branch."),
                "merge_requests": {
                    "type": "boolean",
                    "title": "Merge requests",
                    "default": true,
                    "description": "Open and update merge requests and comment on them"
                }
            },
            "additionalProperties": false
        }),
        config_version: 1,
        setup_queries: vec!["gitlab.list_projects", "gitlab.list_branches"],
    }
}

fn granted(repositories: &[&str], project: &str) -> bool {
    repositories.is_empty() || repositories.iter().any(|r| r.eq_ignore_ascii_case(project))
}
/// A project path as GitLab writes them: plain segments, at least a namespace and a name.
fn project_path(path: &str) -> bool {
    path.contains('/')
        && path.split('/').all(|segment| {
            !segment.trim_matches('.').is_empty()
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        })
}

#[async_trait::async_trait]
impl EgressHook for Gitlab {
    fn definition(&self) -> &Egress {
        &self.0
    }
    fn check(&self, request: &EgressRequest<'_>, config: &Value) -> Decision {
        let repositories = strings(config.get("repositories"));
        match request {
            EgressRequest::Http {
                protocol,
                method,
                path,
            } => {
                // The project segment is checked here, then stands in as a plain segment so
                // the manifest rules see no encoded slash in it.
                let mut segments: Vec<&str> = path.split('/').collect();
                if segments.get(1..4) == Some(&["api", "v4", "projects"])
                    && let Some(project) = segments.get(4).copied()
                {
                    let decoded = project.replace("%2F", "/").replace("%2f", "/");
                    let numeric =
                        !project.is_empty() && project.bytes().all(|b| b.is_ascii_digit());
                    if numeric && !repositories.is_empty() {
                        return Decision::deny(
                            "Name the project by its path: the grant is limited to projects",
                        );
                    }
                    if !numeric && !project_path(&decoded) {
                        return Decision::deny("Not a project");
                    }
                    if !numeric && !granted(&repositories, &decoded) {
                        return Decision::deny(format!("{decoded} is not granted"));
                    }
                    segments[4] = "project";
                }
                let path = segments.join("/");
                let decision = rules::check(&EgressRequest::Http {
                    protocol,
                    method,
                    path: &path,
                });
                let writes = config
                    .get("merge_requests")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if decision == Decision::Allow && !method.eq_ignore_ascii_case("GET") && !writes {
                    return Decision::deny("Writing merge requests is not granted");
                }
                decision
            }
            EgressRequest::Git {
                repository,
                service,
                updates,
                ..
            } => {
                let decision = rules::check(request);
                if decision != Decision::Allow {
                    return decision;
                }
                forge::check_git(
                    project_path(repository) && granted(&repositories, repository),
                    repository,
                    &strings(config.pointer("/push/branches")),
                    *service,
                    updates,
                )
            }
        }
    }
    fn upstream(&self, _protocol: &EgressProtocol, access: &Access) -> Result<String, Error> {
        crate::tools::providers::gitlab::base(access)
            .map(str::to_owned)
            .map_err(|e| Error::Invalid(e.message.unwrap_or_default()))
    }
    async fn setup_query(&self, access: &Access, name: &str, input: Value) -> Result<Value, Error> {
        let base = crate::tools::providers::gitlab::base(access)
            .map_err(|e| Error::Invalid(e.message.unwrap_or_default()))?;
        let api = access.endpoints.get("gitlab_api", base);
        let token = ["access_token", "token"]
            .iter()
            .find_map(|key| model::optional(&access.values, key))
            .ok_or_else(|| Error::Invalid("The connection holds no credential".into()))?;
        let (path, field) = match name {
            "gitlab.list_projects" => (
                "/api/v4/projects?membership=true&simple=true&order_by=path".to_owned(),
                "path_with_namespace",
            ),
            "gitlab.list_branches" => {
                let project = input
                    .get("repository")
                    .and_then(Value::as_str)
                    .filter(|p| project_path(p))
                    .ok_or_else(|| Error::Invalid("`repository` is a project path".into()))?;
                (
                    format!(
                        "/api/v4/projects/{}/repository/branches?",
                        project.replace('/', "%2F")
                    ),
                    "name",
                )
            }
            _ => return Err(Error::Invalid(format!("Unknown setup query {name}"))),
        };
        let mut options = vec![];
        // At most ten pages of a hundred.
        for page in 1..=10 {
            let separator = if path.ends_with('?') { "" } else { "&" };
            let response = access
                .http
                .json(
                    access
                        .http
                        .client
                        .get(format!("{api}{path}{separator}per_page=100&page={page}"))
                        .bearer_auth(token),
                )
                .await?;
            let items = response.0.as_array().cloned().unwrap_or_default();
            options.extend(
                items
                    .iter()
                    .filter_map(|item| item.get(field).and_then(Value::as_str))
                    .map(|v| json!({"value": v, "label": v})),
            );
            if items.len() < 100 {
                break;
            }
        }
        Ok(json!({ "options": options }))
    }
}
