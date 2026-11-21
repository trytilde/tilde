//! `tilde deploy` for a project's ontologies (Tilde Cloud): `defineProject({ ontologies })`.
//!
//! Each ontology is the Tilde project of its name, built from `ontologies/<name>/` of the
//! repository at HEAD. The order is fixed: preflight (every project, environment, connection and
//! secret the specs name exists, nothing changed yet; each missing one is a required action with
//! a link), then a version per ontology (the repository archive, `VersionService.CreateVersion`),
//! then each environment's slot and secret bindings (which name the slots and secrets of the
//! version just built), then the rollouts. Agents are applied after (`apply.rs`).
//!
//! The ontology control plane is Tilde Cloud's: its RPCs are called as Connect JSON (this CLI
//! does not depend on its contract), and a server without them answers 404, which becomes a
//! "needs Tilde Cloud" required action.
use crate::deploy::Deploy;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// An ontology as `@trytilde/iac`'s `defineProject` writes it.
#[derive(Debug, Deserialize)]
pub struct OntologySpec {
    pub dir: String,
    #[serde(default)]
    pub environments: BTreeMap<String, EnvironmentSpec>,
}
#[derive(Debug, Default, Deserialize)]
pub struct EnvironmentSpec {
    /// Slot -> the connection it binds.
    #[serde(default)]
    pub slots: BTreeMap<String, ConnectionSpec>,
    /// The ontology's secret name -> the Tilde secret's name.
    #[serde(default)]
    pub secrets: BTreeMap<String, String>,
}
#[derive(Debug, Deserialize)]
pub struct ConnectionSpec {
    pub slug: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// Something a person must do before anything is deployed, with where to do it.
pub struct Action {
    pub subject: String,
    pub message: String,
    pub url: String,
}

/// A failed Connect call: its status and message.
struct Failure {
    status: u16,
    message: String,
}

struct Client {
    base: String,
    http: reqwest::Client,
    key: Option<String>,
}
impl Client {
    async fn call(&self, method: &str, body: Value) -> std::result::Result<Value, Failure> {
        let mut request = self
            .http
            .post(format!("{}/{method}", self.base))
            .json(&body);
        if let Some(key) = &self.key {
            request = request.bearer_auth(key);
        }
        let response = request.send().await.map_err(|e| Failure {
            status: 0,
            message: e.to_string(),
        })?;
        let status = response.status().as_u16();
        let value: Value = response.json().await.unwrap_or(Value::Null);
        if status == 200 {
            return Ok(value);
        }
        let message = value["message"].as_str().unwrap_or("no detail").to_string();
        Err(Failure { status, message })
    }
    async fn ok(&self, method: &str, body: Value) -> Result<Value> {
        self.call(method, body)
            .await
            .map_err(|f| anyhow::anyhow!("{method} failed ({}): {}", f.status, f.message))
    }
}

const PROJECTS: &str = "tilde.ontology_control.v1.ProjectService";
const VERSIONS: &str = "tilde.ontology_control.v1.VersionService";
const ENVIRONMENTS: &str = "tilde.ontology_control.v1.EnvironmentService";

/// What preflight found for one environment: ids to bind and deploy with.
struct Resolved {
    environment: String,
    environment_id: String,
    slots: Vec<(String, String)>,
    secrets: Vec<(String, String)>,
}
struct Project {
    name: String,
    id: String,
    dir: String,
    environments: Vec<Resolved>,
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// Every project, environment, connection and secret the specs name; what is missing becomes an
/// action. Nothing is changed.
async fn preflight(
    client: &Client,
    ontologies: &BTreeMap<String, &OntologySpec>,
    actions: &mut Vec<Action>,
) -> Result<Vec<Project>> {
    let projects = match client
        .call(&format!("{PROJECTS}/ListProjects"), json!({}))
        .await
    {
        Ok(value) => value["projects"].as_array().cloned().unwrap_or_default(),
        Err(f) if f.status == 404 || f.status == 501 => {
            actions.push(Action {
                subject: "ontologies".into(),
                message: "Ontologies need Tilde Cloud: this server has no ontology control plane"
                    .into(),
                url: "https://trytilde.ai".into(),
            });
            return Ok(Vec::new());
        }
        Err(f) => bail!("ListProjects failed ({}): {}", f.status, f.message),
    };
    let connections = client
        .ok(
            "tilde.management.v1.ConnectionsService/ListConnections",
            json!({"pageSize": 1000}),
        )
        .await?["connections"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for (name, spec) in ontologies {
        let Some(project) = projects.iter().find(|p| p["name"] == name.as_str()) else {
            actions.push(Action {
                subject: format!("ontology {name}"),
                message: format!("The ontology project {name} does not exist; create it"),
                url: format!("/ontology/projects?new={}", encode(name)),
            });
            continue;
        };
        let id = project["id"].as_str().unwrap_or_default().to_string();
        let team = project["teamId"].as_str().unwrap_or_default().to_string();
        let environments = client
            .ok(
                &format!("{ENVIRONMENTS}/ListEnvironments"),
                json!({"projectId": id}),
            )
            .await?["environments"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let secrets = client
            .ok(
                "tilde.management.v1.SecretService/ListSecrets",
                json!({"teamId": team}),
            )
            .await?["secrets"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut resolved = Vec::new();
        for (env_name, env) in &spec.environments {
            let subject = format!("ontology {name}/{env_name}");
            let Some(found) = environments.iter().find(|e| e["name"] == env_name.as_str()) else {
                actions.push(Action {
                    subject,
                    message: format!(
                        "The environment {env_name} of {name} does not exist; create it"
                    ),
                    url: format!("/ontology/projects/{}/environments", encode(&id)),
                });
                continue;
            };
            let mut slots = Vec::new();
            for (slot, connection) in &env.slots {
                let (provider, connection_name) = connection
                    .slug
                    .split_once('/')
                    .unwrap_or((&connection.slug, ""));
                let create = format!(
                    "/registry/tools/catalog/{}?name={}{}",
                    encode(provider),
                    encode(connection_name),
                    connection
                        .kind
                        .as_deref()
                        .map(|t| format!("&type={}", encode(t)))
                        .unwrap_or_default()
                );
                let subject = format!("{subject} slot {slot}");
                let Some(row) = connections
                    .iter()
                    .find(|c| c["slug"] == connection.slug.as_str())
                else {
                    actions.push(Action {
                        subject,
                        message: format!(
                            "Connection {} does not exist; create it, named {connection_name}",
                            connection.slug
                        ),
                        url: create,
                    });
                    continue;
                };
                if let Some(kind) = &connection.kind
                    && row["typeId"] != kind.as_str()
                {
                    actions.push(Action {
                        subject,
                        message: format!(
                            "Connection {} is not a {kind} connection",
                            connection.slug
                        ),
                        url: create,
                    });
                    continue;
                }
                if row["status"] != "ready" {
                    actions.push(Action {
                        subject,
                        message: format!(
                            "Connection {} is not ready; reconnect it",
                            connection.slug
                        ),
                        url: format!(
                            "/registry/tools/connections?search={}",
                            encode(connection_name)
                        ),
                    });
                    continue;
                }
                slots.push((
                    slot.clone(),
                    row["id"].as_str().unwrap_or_default().to_string(),
                ));
            }
            let mut bound = Vec::new();
            for (secret, tilde_name) in &env.secrets {
                // The project's team's own secret before the organization's of the same name.
                let found = secrets
                    .iter()
                    .filter(|s| s["name"] == tilde_name.as_str())
                    .min_by_key(|s| s["organization"] == true);
                let Some(found) = found else {
                    let team = if team.is_empty() {
                        String::new()
                    } else {
                        format!("&team={}", encode(&team))
                    };
                    actions.push(Action {
                        subject: format!("{subject} secret {secret}"),
                        message: format!("Secret {tilde_name} does not exist; add it"),
                        url: format!("/connections/secrets?add={}{team}", encode(tilde_name)),
                    });
                    continue;
                };
                bound.push((
                    secret.clone(),
                    found["id"].as_str().unwrap_or_default().to_string(),
                ));
            }
            resolved.push(Resolved {
                environment: env_name.clone(),
                environment_id: found["id"].as_str().unwrap_or_default().to_string(),
                slots,
                secrets: bound,
            });
        }
        out.push(Project {
            name: name.clone(),
            id,
            dir: spec.dir.clone(),
            environments: resolved,
        });
    }
    Ok(out)
}

/// `git archive` of the repository at HEAD, with the commit and branch.
fn archive(dir: &Path, allow_dirty: bool) -> Result<(Vec<u8>, String, String)> {
    let git = |args: &[&str]| -> Result<String> {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .context("running git")?;
        if !out.status.success() {
            bail!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let root = git(&["rev-parse", "--show-toplevel"])?;
    if !git(&["status", "--porcelain"])?.is_empty() {
        if !allow_dirty {
            bail!(
                "the working tree has uncommitted changes; commit them, or pass --allow-dirty to deploy HEAD without them"
            );
        }
        eprintln!("warning: uncommitted changes are not included; deploying HEAD");
    }
    let commit = git(&["rev-parse", "HEAD"])?;
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let out = std::process::Command::new("git")
        .args(["archive", "--format=tar", "HEAD"])
        .current_dir(&root)
        .output()
        .context("running git archive")?;
    if !out.status.success() {
        bail!(
            "git archive: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok((
        out.stdout,
        commit,
        if branch == "HEAD" {
            String::new()
        } else {
            branch
        },
    ))
}

/// Until `read` says the value is no longer in progress (`waiting` is its in-progress status).
async fn poll(
    client: &Client,
    method: &str,
    body: Value,
    read: impl Fn(&Value) -> Value,
    waiting: &str,
) -> Result<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1800);
    loop {
        let value = read(&client.ok(method, body.clone()).await?);
        if value["status"] != waiting {
            return Ok(value);
        }
        if tokio::time::Instant::now() > deadline {
            bail!("{method}: still {waiting} after 30 minutes");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// Deploy the ontologies (all, or `--ontology`'s). Returns the actions that block it; when there
/// are any, nothing was changed.
pub async fn run(
    args: &Deploy,
    ontologies: &BTreeMap<String, OntologySpec>,
) -> Result<Vec<Action>> {
    let Some(url) = &args.url else {
        bail!("--url (or TILDE_URL) is required");
    };
    let selected: BTreeMap<String, &OntologySpec> = ontologies
        .iter()
        .filter(|(name, _)| args.ontology.is_empty() || args.ontology.contains(name))
        .map(|(name, spec)| (name.clone(), spec))
        .collect();
    if let Some(unknown) = args.ontology.iter().find(|n| !ontologies.contains_key(*n)) {
        bail!("--ontology {unknown}: the project declares no such ontology");
    }
    if selected.is_empty() {
        return Ok(Vec::new());
    }
    let dir = &args.dir;
    for (name, spec) in &selected {
        if !dir.join(&spec.dir).join("ontology.ts").is_file() {
            bail!("ontology {name}: {}/ontology.ts does not exist", spec.dir);
        }
        if dir.join(&spec.dir).join("tilde.toml").is_file() {
            bail!(
                "ontology {name}: {}/tilde.toml: a repository has at most one tilde.toml, at its root",
                spec.dir
            );
        }
    }
    let client = Client {
        base: crate::api::base_url(url)?,
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?,
        key: args.api_key.clone(),
    };
    let mut actions = Vec::new();
    let projects = preflight(&client, &selected, &mut actions).await?;
    if !actions.is_empty() {
        return Ok(actions);
    }
    if args.dry_run {
        eprintln!("Plan (dry run, nothing deployed):");
        for p in &projects {
            eprintln!(
                "  ontology {} ({}): build HEAD into a version",
                p.name, p.dir
            );
            for env in &p.environments {
                for (slot, _) in &env.slots {
                    eprintln!("    {}: bind slot {slot}", env.environment);
                }
                for (secret, _) in &env.secrets {
                    eprintln!("    {}: bind secret {secret}", env.environment);
                }
                eprintln!("    {}: roll out", env.environment);
            }
        }
        return Ok(Vec::new());
    }
    let (archive, commit, branch) = archive(dir, args.allow_dirty)?;
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&archive);
    for p in &projects {
        eprintln!(
            "Ontology {}: building {} at {}",
            p.name,
            p.dir,
            &commit[..commit.len().min(12)]
        );
        let created = client
            .ok(
                &format!("{VERSIONS}/CreateVersion"),
                json!({"projectId": p.id, "commitSha": commit, "ref": branch, "archive": encoded}),
            )
            .await?;
        let version_id = created["version"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let version = poll(
            &client,
            &format!("{VERSIONS}/GetVersion"),
            json!({"id": version_id}),
            |v| v["version"].clone(),
            "VERSION_STATUS_BUILDING",
        )
        .await?;
        if version["status"] != "VERSION_STATUS_READY" {
            bail!(
                "ontology {}: version {version_id} failed to build: {}",
                p.name,
                version["error"].as_str().unwrap_or("no detail")
            );
        }
        for env in &p.environments {
            for (slot, connection) in &env.slots {
                client
                    .ok(
                        &format!("{ENVIRONMENTS}/SetEnvironmentConnection"),
                        json!({"environmentId": env.environment_id, "slot": slot, "connectionId": connection}),
                    )
                    .await?;
            }
            for (name, secret) in &env.secrets {
                client
                    .ok(
                        &format!("{ENVIRONMENTS}/SetEnvironmentSecretBinding"),
                        json!({"environmentId": env.environment_id, "name": name, "secretId": secret}),
                    )
                    .await?;
            }
        }
        for env in &p.environments {
            let started = client
                .ok(
                    &format!("{ENVIRONMENTS}/Deploy"),
                    json!({"environmentId": env.environment_id, "versionId": version_id}),
                )
                .await?;
            let deployment = started["deployment"]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let done = poll(
                &client,
                &format!("{ENVIRONMENTS}/ListDeployments"),
                json!({"environmentId": env.environment_id}),
                |listed| {
                    listed["deployments"]
                        .as_array()
                        .and_then(|all| {
                            all.iter().find(|d| d["id"] == deployment.as_str()).cloned()
                        })
                        .unwrap_or(Value::Null)
                },
                "DEPLOYMENT_STATUS_DEPLOYING",
            )
            .await?;
            if done["status"] != "DEPLOYMENT_STATUS_SUCCEEDED" {
                bail!(
                    "ontology {}: the rollout to {} failed:\n{}",
                    p.name,
                    env.environment,
                    done["log"].as_str().unwrap_or("no log")
                );
            }
            eprintln!(
                "Ontology {}: {} runs version {version_id}",
                p.name, env.environment
            );
        }
    }
    Ok(Vec::new())
}

/// The actions for people, with links joined to the server's URL when relative.
pub fn render(actions: &[Action], base: &str) -> String {
    let mut out = format!(
        "Blocked: {} action(s) needed before anything is deployed\n",
        actions.len()
    );
    for action in actions {
        out.push_str(&format!("  ✗ {}: {}\n", action.subject, action.message));
        let url = if action.url.starts_with('/') {
            format!("{}{}", base.trim_end_matches('/'), action.url)
        } else {
            action.url.clone()
        };
        out.push_str(&format!("      → {url}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, extract::Path as Route, routing::post};
    use std::sync::{Arc, Mutex};

    fn args(url: String, dir: &Path) -> Deploy {
        Deploy {
            entry: None,
            url: Some(url),
            agent_id: None,
            api_key: Some("tilde_key_test".into()),
            target: None,
            function_arn: None,
            external_id: None,
            label: None,
            dry_run: false,
            team: None,
            json: false,
            only: None,
            ontology: Vec::new(),
            allow_dirty: false,
            dir: dir.to_path_buf(),
        }
    }

    fn spec() -> BTreeMap<String, OntologySpec> {
        serde_json::from_value(json!({"shop": {"dir": "ontologies/shop", "environments": {"production": {
            "slots": {"github": {"slug": "github/acme"}, "linear": {"slug": "linear/acme", "type": "api_key"}},
            "secrets": {"PG_PASSWORD": "SHOP_PG_PASSWORD"}}}}}))
        .unwrap()
    }

    /// A repository with `ontologies/shop/` committed.
    fn repository() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("ontologies/shop")).unwrap();
        std::fs::write(
            dir.path().join("ontologies/shop/ontology.ts"),
            "export default {};\n",
        )
        .unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["add", "-A"],
            vec![
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "-m",
                "shop",
            ],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(&args)
                    .current_dir(dir.path())
                    .status()
                    .unwrap()
                    .success()
            );
        }
        dir
    }

    /// A stand-in Tilde Cloud: it records every call and answers as the control plane would.
    async fn cloud(
        connections: Value,
        secrets: Value,
    ) -> (String, Arc<Mutex<Vec<(String, Value)>>>) {
        let calls: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let app = Router::new().route(
            "/{service}/{method}",
            post({
                let calls = calls.clone();
                move |Route((service, method)): Route<(String, String)>, Json(body): Json<Value>| async move {
                    calls.lock().unwrap().push((method.clone(), body.clone()));
                    let answer = match (service.as_str(), method.as_str()) {
                        (_, "ListProjects") => json!({"projects": [{"id": "p1", "name": "shop", "teamId": "t1"}]}),
                        (_, "ListEnvironments") => json!({"environments": [{"id": "e1", "name": "production"}]}),
                        (_, "ListConnections") => connections.clone(),
                        (_, "ListSecrets") => secrets.clone(),
                        (_, "CreateVersion") => json!({"version": {"id": "v1", "status": "VERSION_STATUS_BUILDING"}}),
                        (_, "GetVersion") => json!({"version": {"id": "v1", "status": "VERSION_STATUS_READY"}}),
                        (_, "Deploy") => json!({"deployment": {"id": "d1", "status": "DEPLOYMENT_STATUS_DEPLOYING"}}),
                        (_, "ListDeployments") => json!({"deployments": [{"id": "d1", "status": "DEPLOYMENT_STATUS_SUCCEEDED"}]}),
                        _ => json!({}),
                    };
                    Json(answer)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (url, calls)
    }

    /// Missing connections and secrets block before anything changes, each with its link; with
    /// them all there, the version is built, the environment bound, then rolled out.
    #[tokio::test]
    async fn preflight_blocks_with_links_then_builds_binds_and_rolls_out() {
        let repo = repository();
        let (url, calls) = cloud(
            json!({"connections": [{"id": "c1", "slug": "github/acme", "typeId": "github_app", "status": "ready"}]}),
            json!({"secrets": []}),
        )
        .await;
        let actions = run(&args(url.clone(), repo.path()), &spec()).await.unwrap();
        let rendered = render(&actions, &url);
        assert!(
            rendered.contains("Connection linear/acme does not exist"),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!(
                "{url}/registry/tools/catalog/linear?name=acme&type=api_key"
            )),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!(
                "{url}/connections/secrets?add=SHOP_PG_PASSWORD&team=t1"
            )),
            "{rendered}"
        );
        let methods: Vec<String> = calls
            .lock()
            .unwrap()
            .iter()
            .map(|(m, _)| m.clone())
            .collect();
        assert!(
            !methods
                .iter()
                .any(|m| m.starts_with("Create") || m.starts_with("Set") || m == "Deploy"),
            "{methods:?}"
        );

        let (url, calls) = cloud(
            json!({"connections": [
                {"id": "c1", "slug": "github/acme", "typeId": "github_app", "status": "ready"},
                {"id": "c2", "slug": "linear/acme", "typeId": "api_key", "status": "ready"}]}),
            json!({"secrets": [{"id": "s-org", "name": "SHOP_PG_PASSWORD", "organization": true},
                               {"id": "s-team", "name": "SHOP_PG_PASSWORD"}]}),
        )
        .await;
        let actions = run(&args(url, repo.path()), &spec()).await.unwrap();
        assert!(actions.is_empty());
        let calls = calls.lock().unwrap().clone();
        let order: Vec<&str> = calls
            .iter()
            .map(|(m, _)| m.as_str())
            .filter(|m| {
                [
                    "CreateVersion",
                    "SetEnvironmentConnection",
                    "SetEnvironmentSecretBinding",
                    "Deploy",
                ]
                .contains(m)
            })
            .collect();
        assert_eq!(
            order,
            [
                "CreateVersion",
                "SetEnvironmentConnection",
                "SetEnvironmentConnection",
                "SetEnvironmentSecretBinding",
                "Deploy"
            ]
        );
        let created = &calls.iter().find(|(m, _)| m == "CreateVersion").unwrap().1;
        assert_eq!(created["projectId"], "p1");
        assert!(!created["archive"].as_str().unwrap().is_empty());
        // The team's own secret before the organization's of the same name.
        let secret = &calls
            .iter()
            .find(|(m, _)| m == "SetEnvironmentSecretBinding")
            .unwrap()
            .1;
        assert_eq!(
            (secret["name"].as_str(), secret["secretId"].as_str()),
            (Some("PG_PASSWORD"), Some("s-team"))
        );
    }

    /// A server without the ontology control plane (open-source Tilde) needs Tilde Cloud.
    #[tokio::test]
    async fn a_server_without_ontologies_needs_tilde_cloud() {
        let repo = repository();
        let app = Router::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let actions = run(&args(url, repo.path()), &spec()).await.unwrap();
        assert_eq!(actions.len(), 1);
        assert!(
            actions[0].message.contains("need Tilde Cloud"),
            "{}",
            actions[0].message
        );
    }
}
