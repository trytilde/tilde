//! GitHub egress: git fetch and push to github.com and a small REST allowlist on api.github.com.
//!
//! A grant's config narrows it to repositories (`owner/repo`; empty: every repository the
//! connection reaches), the branches pushes may update (`push.branches`, globs; empty: no push)
//! and whether pull requests and their comments may be written. Pushes never delete refs,
//! rewrite them or touch the default branch. App installations mint a token limited to those
//! repositories and permissions; personal access and OAuth tokens cannot be narrowed, so for
//! them the checks here are the only narrowing.
use crate::chat::providers::Access;
use crate::connections::egress::{
    Decision, Egress, EgressHook, EgressProtocol, EgressRequest, EnvTemplate, GitService, rules,
};
use crate::connections::mints::{self, MintScope};
use crate::error::Error;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::sync::LazyLock;

pub struct Github(Egress);
static HOOK: LazyLock<Github> = LazyLock::new(|| Github(definition()));
pub fn hook() -> &'static Github {
    &HOOK
}

fn definition() -> Egress {
    Egress {
        protocols: vec![
            EgressProtocol::Git {
                upstream: "https://github.com",
                services: vec![GitService::UploadPack, GitService::ReceivePack],
                deny_delete: true,
                deny_force: true,
                deny_default_branch: true,
            },
            EgressProtocol::Http {
                upstream: "https://api.github.com",
                allow: vec![
                    "GET /repos/{owner}/{repo}/**",
                    "POST /repos/{owner}/{repo}/pulls",
                    "PATCH /repos/{owner}/{repo}/pulls/*",
                    "POST /repos/{owner}/{repo}/pulls/*/comments",
                    "POST /repos/{owner}/{repo}/pulls/*/comments/*/replies",
                    "POST /repos/{owner}/{repo}/pulls/*/reviews",
                    "POST /repos/{owner}/{repo}/pulls/*/requested_reviewers",
                    "POST /repos/{owner}/{repo}/issues/*/comments",
                    "PATCH /repos/{owner}/{repo}/issues/comments/*",
                    "GET /rate_limit",
                ],
                deny: vec![
                    "* /orgs/**",
                    "DELETE /repos/*/*",
                    "* /repos/*/*/actions/secrets/**",
                    "* /repos/*/*/actions/variables/**",
                    "* /repos/*/*/environments/**",
                    "* /repos/*/*/hooks/**",
                    "* /repos/*/*/keys/**",
                    "* /repos/*/*/collaborators/**",
                    "* /repos/*/*/invitations/**",
                    "PUT /repos/*/*/branches/*/protection/**",
                    "POST /repos/*/*/branches/*/protection/**",
                    "PATCH /repos/*/*/branches/*/protection/**",
                    "DELETE /repos/*/*/branches/*/protection/**",
                ],
                wiring: vec![
                    EnvTemplate {
                        name: "GITHUB_API_URL",
                        value: "{url}",
                    },
                    EnvTemplate {
                        name: "GITHUB_TOKEN",
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
                    "title": "Repositories",
                    "items": {"type": "string", "pattern": "^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$"},
                    "uniqueItems": true,
                    "x-tilde-options": "github.list_repositories",
                    "description": "Empty: every repository the connection can reach"
                },
                "push": {
                    "type": "object",
                    "title": "Push",
                    "properties": {
                        "branches": {
                            "type": "array",
                            "title": "Branches",
                            "items": {"type": "string", "pattern": "^[A-Za-z0-9_./*-]+$"},
                            "default": ["factory/*"],
                            "description": "Branches pushes may create and update (`*` matches within a path segment). Empty: no pushes. Never the default branch."
                        }
                    },
                    "additionalProperties": false
                },
                "pull_requests": {
                    "type": "boolean",
                    "title": "Pull requests",
                    "default": true,
                    "description": "Open and update pull requests and comment on them"
                }
            },
            "additionalProperties": false
        }),
        config_version: 1,
        setup_queries: vec!["github.list_repositories", "github.list_branches"],
    }
}

struct Config<'a> {
    repositories: Vec<&'a str>,
    branches: Vec<&'a str>,
    pull_requests: bool,
}
fn strings(value: Option<&Value>) -> Vec<&str> {
    value
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}
fn config(value: &Value) -> Config<'_> {
    Config {
        repositories: strings(value.get("repositories")),
        branches: strings(value.pointer("/push/branches")),
        pull_requests: value
            .get("pull_requests")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}
impl Config<'_> {
    fn repository(&self, owner: &str, repo: &str) -> bool {
        let repo = repo.strip_suffix(".git").unwrap_or(repo);
        self.repositories.is_empty()
            || self.repositories.iter().any(|r| {
                r.split_once('/').is_some_and(|(o, n)| {
                    o.eq_ignore_ascii_case(owner) && n.eq_ignore_ascii_case(repo)
                })
            })
    }
    fn branch(&self, name: &str) -> bool {
        self.branches.iter().any(|pattern| glob(pattern, name))
    }
}
/// `*` matches any run of characters except `/`.
fn glob(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((prefix, rest)) => {
            let Some(tail) = name.strip_prefix(prefix) else {
                return false;
            };
            (0..=tail.find('/').unwrap_or(tail.len()))
                .filter(|&i| tail.is_char_boundary(i))
                .any(|i| glob(rest, &tail[i..]))
        }
    }
}

#[async_trait::async_trait]
impl EgressHook for Github {
    fn definition(&self) -> &Egress {
        &self.0
    }
    fn check(&self, request: &EgressRequest<'_>, value: &Value) -> Decision {
        let decision = rules::check(request);
        if decision != Decision::Allow {
            return decision;
        }
        let config = config(value);
        match request {
            EgressRequest::Http { method, path, .. } => {
                let mut segments = path.trim_start_matches('/').split('/');
                if let (Some("repos"), Some(owner), Some(repo)) =
                    (segments.next(), segments.next(), segments.next())
                    && !config.repository(owner, repo)
                {
                    return Decision::deny(format!("{owner}/{repo} is not granted"));
                }
                if !method.eq_ignore_ascii_case("GET") && !config.pull_requests {
                    return Decision::deny("Writing pull requests is not granted");
                }
                Decision::Allow
            }
            EgressRequest::Git {
                repository,
                service,
                updates,
                ..
            } => {
                let Some((owner, repo)) = repository
                    .split_once('/')
                    .filter(|(_, repo)| !repo.contains('/'))
                else {
                    return Decision::deny("Not a repository");
                };
                if !config.repository(owner, repo) {
                    return Decision::deny(format!("{repository} is not granted"));
                }
                if *service == GitService::ReceivePack && config.branches.is_empty() {
                    return Decision::deny("Pushing is not granted");
                }
                for update in *updates {
                    match update.name.strip_prefix("refs/heads/") {
                        Some(branch) if config.branch(branch) => {}
                        _ => {
                            return Decision::deny(format!(
                                "Pushing to {} is not granted",
                                update.name
                            ));
                        }
                    }
                }
                Decision::Allow
            }
        }
    }
    fn scope(&self, value: &Value) -> MintScope {
        let config = config(value);
        let mut permissions = vec![(
            "contents".to_owned(),
            if config.branches.is_empty() {
                "read"
            } else {
                "write"
            }
            .to_owned(),
        )];
        if config.pull_requests {
            permissions.push(("pull_requests".into(), "write".into()));
        }
        MintScope {
            resources: config.repositories.iter().map(|r| r.to_string()).collect(),
            permissions,
        }
    }
    async fn setup_query(&self, access: &Access, name: &str, input: Value) -> Result<Value, Error> {
        let app = access.values.contains_key("installation_id");
        let typ = if app { "github_app" } else { "pat" };
        let minted = mints::mint(access, "github", typ, &MintScope::default(), "setup")
            .await
            .map_err(|e| Error::Invalid(e.message.unwrap_or_default()))?;
        let api = access.endpoints.get("github_api", "https://api.github.com");
        let get = |path: String| {
            let mut request = access
                .http
                .client
                .get(format!("{api}{path}"))
                .header("accept", "application/vnd.github+json")
                .header("x-github-api-version", "2022-11-28")
                .header("user-agent", "tilde");
            for (header, value) in minted.headers() {
                request = request.header(header, value.expose_secret());
            }
            request
        };
        let mut options = vec![];
        let (path, list, field): (String, Option<&str>, &str) = match name {
            "github.list_repositories" if app => (
                "/installation/repositories".into(),
                Some("/repositories"),
                "full_name",
            ),
            "github.list_repositories" => (
                "/user/repos?affiliation=owner,collaborator,organization_member".into(),
                None,
                "full_name",
            ),
            "github.list_branches" => {
                let repository = input
                    .get("repository")
                    .and_then(Value::as_str)
                    .filter(|r| {
                        r.split_once('/').is_some_and(|(o, n)| {
                            [o, n].iter().all(|s| {
                                !s.is_empty()
                                    && s.bytes()
                                        .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                                    && !s.trim_matches('.').is_empty()
                            })
                        })
                    })
                    .ok_or_else(|| Error::Invalid("`repository` is owner/name".into()))?;
                (format!("/repos/{repository}/branches"), None, "name")
            }
            _ => return Err(Error::Invalid(format!("Unknown setup query {name}"))),
        };
        // At most ten pages of a hundred.
        for page in 1..=10 {
            let separator = if path.contains('?') { '&' } else { '?' };
            let response = access
                .http
                .json(get(format!("{path}{separator}per_page=100&page={page}")))
                .await?;
            let items = match list {
                Some(pointer) => response.0.pointer(pointer).cloned(),
                None => Some(response.0.clone()),
            };
            let items = items
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
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
