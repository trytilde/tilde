//! `tilde deploy` for a project's ontologies (Tilde Cloud): `defineProject({ ontologies })`.
//!
//! Each ontology is the Tilde project of its name, built from `ontologies/<name>/` of the
//! repository at HEAD (`tilde deploy` runs at the repository's root). `prepare` changes nothing:
//! it checks every project, environment, connection and secret the specs name (each missing one
//! is a required action with a link, reported with the agents' own before anything changes) and
//! archives the repository once. `execute` then, per ontology: builds the archive into a version
//! (`VersionService.CreateVersion`), checks that the version declares exactly what the spec binds,
//! and binds and rolls out one environment at a time. Agents are applied after (`apply.rs`).
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
#[derive(Debug)]
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
    slots: Vec<SlotBinding>,
    secrets: Vec<(String, String)>,
}
/// A slot the spec binds: the connection's id, its slug, and the slug bound now (empty: unbound).
struct SlotBinding {
    slot: String,
    connection_id: String,
    slug: String,
    current: String,
}
struct Project {
    name: String,
    id: String,
    environments: Vec<Resolved>,
}

/// The largest source archive Tilde Cloud takes (its `versions::MAX_ARCHIVE`).
const MAX_ARCHIVE: usize = 32 * 1024 * 1024;

/// What `prepare` found, for `execute`: every project resolved, the repository archived once.
pub struct Plan {
    client: Client,
    projects: Vec<Project>,
    archive: Vec<u8>,
    commit: String,
    branch: String,
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
            let environment_id = found["id"].as_str().unwrap_or_default().to_string();
            let bound = match env.slots.is_empty() {
                true => Vec::new(),
                false => client
                    .ok(
                        &format!("{ENVIRONMENTS}/ListEnvironmentConnections"),
                        json!({"environmentId": environment_id}),
                    )
                    .await?["connections"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
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
                let current = bound
                    .iter()
                    .find(|b| b["slot"] == slot.as_str())
                    .and_then(|b| b["connectionName"].as_str())
                    .unwrap_or_default();
                slots.push(SlotBinding {
                    slot: slot.clone(),
                    connection_id: row["id"].as_str().unwrap_or_default().to_string(),
                    slug: connection.slug.clone(),
                    current: current.to_string(),
                });
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
                environment_id,
                slots,
                secrets: bound,
            });
        }
        out.push(Project {
            name: name.clone(),
            id,
            environments: resolved,
        });
    }
    Ok(out)
}

/// `git archive` of the repository at HEAD, with the commit and branch.
fn archive(dir: &Path, allow_dirty: bool) -> Result<(Vec<u8>, String, String)> {
    if !git(dir, &["status", "--porcelain"])?.is_empty() {
        if !allow_dirty {
            bail!(
                "the working tree has uncommitted changes; commit them, or pass --allow-dirty to deploy HEAD without them"
            );
        }
        eprintln!("warning: uncommitted changes are not included; deploying HEAD");
    }
    let commit = git(dir, &["rev-parse", "HEAD"])?;
    let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let out = std::process::Command::new("git")
        .args(["archive", "--format=tar", "HEAD"])
        .current_dir(dir)
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

/// Check and resolve the ontologies (all, or `--ontology`'s) and archive the repository, changing
/// nothing: the actions that block a deploy, else the plan (none when there is nothing to do or
/// it is a dry run).
pub async fn prepare(
    args: &Deploy,
    ontologies: &BTreeMap<String, OntologySpec>,
) -> Result<(Vec<Action>, Option<Plan>)> {
    let Some(url) = &args.url else {
        bail!("--url (or TILDE_URL) is required");
    };
    if let Some(unknown) = args.ontology.iter().find(|n| !ontologies.contains_key(*n)) {
        bail!("--ontology {unknown}: the project declares no such ontology");
    }
    let selected: BTreeMap<String, &OntologySpec> = ontologies
        .iter()
        .filter(|(name, _)| args.ontology.is_empty() || args.ontology.contains(name))
        .map(|(name, spec)| (name.clone(), spec))
        .collect();
    if selected.is_empty() {
        return Ok((Vec::new(), None));
    }
    // The control plane builds `ontologies/<name>/` of the archive, which is the repository root.
    let dir = args.dir.canonicalize().context("the project directory")?;
    let root = git(&dir, &["rev-parse", "--show-toplevel"])?;
    if Path::new(&root).canonicalize().ok().as_deref() != Some(dir.as_path()) {
        bail!("a project with ontologies deploys from its repository's root ({root})");
    }
    for name in selected.keys() {
        let folder = dir.join("ontologies").join(name);
        if !folder.join("ontology.ts").is_file() {
            bail!("ontology {name}: ontologies/{name}/ontology.ts does not exist");
        }
        if folder.join("tilde.toml").is_file() {
            bail!(
                "ontology {name}: ontologies/{name}/tilde.toml: a repository has at most one tilde.toml, at its root"
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
        return Ok((actions, None));
    }
    if args.dry_run {
        eprint!("{}", plan_text(&projects));
        return Ok((Vec::new(), None));
    }
    let (archive, commit, branch) = archive(&dir, args.allow_dirty)?;
    if archive.len() > MAX_ARCHIVE {
        bail!(
            "the repository archive is {} MiB, over Tilde's {} MiB: move large files out of the repository (or into .gitattributes export-ignore)",
            archive.len() / (1024 * 1024),
            MAX_ARCHIVE / (1024 * 1024)
        );
    }
    Ok((
        Vec::new(),
        Some(Plan {
            client,
            projects,
            archive,
            commit,
            branch,
        }),
    ))
}

/// The dry run's plan: each slot whose binding changes as `slot: current → new`, a count of the
/// unchanged ones.
fn plan_text(projects: &[Project]) -> String {
    let mut out = String::from("Plan (dry run, nothing deployed):\n");
    for p in projects {
        out.push_str(&format!(
            "  ontology {}: build HEAD into a version\n",
            p.name
        ));
        for env in &p.environments {
            let e = &env.environment;
            let mut unchanged = 0;
            for s in &env.slots {
                if s.current == s.slug {
                    unchanged += 1;
                    continue;
                }
                let current = if s.current.is_empty() {
                    "unbound"
                } else {
                    &s.current
                };
                out.push_str(&format!(
                    "    {e}: slot {}: {current} → {}\n",
                    s.slot, s.slug
                ));
            }
            if unchanged > 0 {
                let plural = if unchanged == 1 { "" } else { "s" };
                out.push_str(&format!("    {e}: {unchanged} slot{plural} unchanged\n"));
            }
            for (secret, _) in &env.secrets {
                out.push_str(&format!("    {e}: bind secret {secret}\n"));
            }
            out.push_str(&format!("    {e}: roll out\n"));
        }
    }
    out
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
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
}

/// That the version declares every slot and secret the environment binds, and that each one it
/// declares is bound (by the spec or already): checked for every environment before any binding.
async fn check_declared(client: &Client, p: &Project, version: &Value) -> Result<()> {
    let names = |field: &str, key: Option<&str>| -> Vec<String> {
        version[field]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| key.map_or(v, |k| &v[k]).as_str().map(str::to_string))
            .collect()
    };
    let (declared, secrets) = (names("slots", Some("name")), names("secrets", None));
    let mut problems = Vec::new();
    for env in &p.environments {
        let slots = client
            .ok(
                &format!("{ENVIRONMENTS}/ListEnvironmentConnections"),
                json!({"environmentId": env.environment_id}),
            )
            .await?["connections"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let set = client
            .ok(
                &format!("{ENVIRONMENTS}/ListEnvironmentSecrets"),
                json!({"environmentId": env.environment_id}),
            )
            .await?["secrets"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for SlotBinding { slot, .. } in &env.slots {
            if !declared.contains(slot) {
                problems.push(format!(
                    "{}: the version declares no slot {slot}",
                    env.environment
                ));
            }
        }
        for slot in &declared {
            let bound = slots.iter().any(|b| {
                b["slot"] == slot.as_str()
                    && b["connectionId"].as_str().is_some_and(|c| !c.is_empty())
            });
            if !bound && !env.slots.iter().any(|s| &s.slot == slot) {
                problems.push(format!(
                    "{}: slot {slot} is neither bound nor in the spec",
                    env.environment
                ));
            }
        }
        for (name, _) in &env.secrets {
            if !secrets.contains(name) {
                problems.push(format!(
                    "{}: the version declares no secret {name}",
                    env.environment
                ));
            }
        }
        for name in &secrets {
            let already = set
                .iter()
                .any(|s| s["name"] == name.as_str() && s["set"] == true);
            if !already && !env.secrets.iter().any(|(s, _)| s == name) {
                problems.push(format!(
                    "{}: secret {name} is neither set nor in the spec",
                    env.environment
                ));
            }
        }
    }
    if !problems.is_empty() {
        bail!(
            "ontology {}: the version and the spec disagree; nothing was bound:\n  {}",
            p.name,
            problems.join("\n  ")
        );
    }
    Ok(())
}

/// Build, check, then bind and roll out one environment at a time, ontology by ontology. On a
/// failure, says which environments of that ontology run the new version, which is bound to it
/// but not running it, and which are as they were; every rollout that completed before it, of any
/// ontology; and that the agents were not applied.
pub async fn execute(args: &Deploy, plan: Plan) -> Result<()> {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&plan.archive);
    let mut completed = Vec::new();
    for p in &plan.projects {
        if let Err(e) = deploy_one(args, &plan, p, &encoded, &mut completed).await {
            let before = if completed.is_empty() {
                "none".to_string()
            } else {
                completed.join(", ")
            };
            return Err(e.context(format!(
                "tilde deploy stopped at ontology {}; rollouts that completed: {before}; the agents were not applied",
                p.name
            )));
        }
    }
    Ok(())
}

async fn deploy_one(
    args: &Deploy,
    plan: &Plan,
    p: &Project,
    encoded: &str,
    completed: &mut Vec<String>,
) -> Result<()> {
    let client = &plan.client;
    {
        eprintln!(
            "Ontology {}: building {}",
            p.name,
            &plan.commit[..plan.commit.len().min(12)]
        );
        let created = client
            .ok(
                &format!("{VERSIONS}/CreateVersion"),
                json!({"projectId": p.id, "commitSha": plan.commit, "ref": plan.branch, "archive": encoded}),
            )
            .await?;
        let version_id = created["version"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let version = poll(
            client,
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
        check_declared(client, p, &version).await?;
        let mut done: Vec<&str> = Vec::new();
        for env in &p.environments {
            let state = |bound: bool| {
                let untouched: Vec<&str> = p
                    .environments
                    .iter()
                    .map(|e| e.environment.as_str())
                    .filter(|e| !done.contains(e) && *e != env.environment)
                    .collect();
                format!(
                    "running version {version_id}: {}; {}; as they were: {}",
                    if done.is_empty() {
                        "none".into()
                    } else {
                        done.join(", ")
                    },
                    if bound {
                        format!("bound to it but not running it: {}", env.environment)
                    } else {
                        format!("{} partly bound", env.environment)
                    },
                    if untouched.is_empty() {
                        "none".into()
                    } else {
                        untouched.join(", ")
                    }
                )
            };
            for s in &env.slots {
                client
                    .ok(&format!("{ENVIRONMENTS}/SetEnvironmentConnection"), json!({"environmentId": env.environment_id, "slot": s.slot, "connectionId": s.connection_id}))
                    .await
                    .map_err(|e| e.context(state(false)))?;
            }
            for (name, secret) in &env.secrets {
                client
                    .ok(&format!("{ENVIRONMENTS}/SetEnvironmentSecretBinding"), json!({"environmentId": env.environment_id, "name": name, "secretId": secret}))
                    .await
                    .map_err(|e| e.context(state(false)))?;
            }
            let started = client
                .ok(
                    &format!("{ENVIRONMENTS}/Deploy"),
                    json!({"environmentId": env.environment_id, "versionId": version_id, "acceptRebuild": args.accept_rebuild}),
                )
                .await
                .map_err(|e| e.context(state(true)))?;
            let deployment = started["deployment"]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let finished = poll(
                client,
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
            if finished["status"] != "DEPLOYMENT_STATUS_SUCCEEDED" {
                bail!(
                    "ontology {}: the rollout to {} failed ({}):\n{}",
                    p.name,
                    env.environment,
                    state(true),
                    finished["log"].as_str().unwrap_or("no log")
                );
            }
            eprintln!(
                "Ontology {}: {} runs version {version_id}",
                p.name, env.environment
            );
            done.push(env.environment.as_str());
            completed.push(format!("{}/{}", p.name, env.environment));
        }
    }
    Ok(())
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
    use axum::{
        Router, body::Bytes, extract::Path as Route, http::header, response::IntoResponse,
        routing::post,
    };
    use buffa::Message;
    use std::sync::{Arc, Mutex};
    use tilde_contracts::proto::tilde::management::v1 as wire;

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
            accept_rebuild: Vec::new(),
            dir: dir.to_path_buf(),
        }
    }

    fn spec() -> BTreeMap<String, OntologySpec> {
        serde_json::from_value(json!({
            "shop": {"environments": {
                "production": {
                    "slots": {"github": {"slug": "github/acme"}, "linear": {"slug": "linear/acme", "type": "api_key"}},
                    "secrets": {"PG_PASSWORD": "SHOP_PG_PASSWORD"}},
                "staging": {
                    "slots": {"github": {"slug": "github/acme"}, "linear": {"slug": "linear/acme", "type": "api_key"}},
                    "secrets": {"PG_PASSWORD": "SHOP_PG_PASSWORD"}}}},
            "warehouse": {"environments": {
                "production": {
                    "slots": {"github": {"slug": "github/acme"}, "linear": {"slug": "linear/acme", "type": "api_key"}},
                    "secrets": {"PG_PASSWORD": "SHOP_PG_PASSWORD"}}}}}))
        .unwrap()
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    /// A repository with `ontologies/shop/` and `ontologies/warehouse/` committed.
    fn repository() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for name in ["shop", "warehouse"] {
            std::fs::create_dir_all(dir.path().join("ontologies").join(name)).unwrap();
            std::fs::write(
                dir.path().join("ontologies").join(name).join("ontology.ts"),
                "export default {};\n",
            )
            .unwrap();
        }
        git(dir.path(), &["init", "-q", "-b", "main"]);
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "ontologies"]);
        dir
    }

    /// What the stand-in Tilde Cloud holds.
    struct World {
        connections: Value,
        secrets: Value,
        /// The environment whose rollout fails.
        failing: Option<&'static str>,
        /// Slots the built version declares.
        declared: Value,
        /// Whether a dry-run Apply of the agents comes back blocked.
        agents_blocked: bool,
        /// Per environment id, what ListEnvironmentConnections answers.
        bound: BTreeMap<&'static str, Value>,
    }

    impl Default for World {
        fn default() -> Self {
            World {
                connections: json!({"connections": [
                    {"id": "c1", "slug": "github/acme", "typeId": "github_app", "status": "ready"},
                    {"id": "c2", "slug": "linear/acme", "typeId": "api_key", "status": "ready"}]}),
                secrets: json!({"secrets": [{"id": "s-org", "name": "SHOP_PG_PASSWORD", "organization": true},
                                            {"id": "s-team", "name": "SHOP_PG_PASSWORD"}]}),
                failing: None,
                declared: json!(["github", "linear"]),
                agents_blocked: false,
                bound: BTreeMap::new(),
            }
        }
    }

    type Calls = Arc<Mutex<Vec<(String, Value)>>>;

    /// A stand-in Tilde Cloud: it records every call and answers as the control plane would.
    async fn cloud(world: World) -> (String, Calls) {
        let calls: Calls = Arc::default();
        let world = Arc::new(world);
        let app = Router::new().route(
            "/{service}/{method}",
            post({
                let calls = calls.clone();
                move |Route((service, method)): Route<(String, String)>, body: Bytes| async move {
                    if service.ends_with("ApplyService") {
                        let request = wire::ApplyRequest::decode_from_slice(&body).unwrap();
                        calls.lock().unwrap().push((format!("Apply dryRun={}", request.dry_run), Value::Null));
                        let response = wire::ApplyResponse {
                            outcome: if world.agents_blocked { wire::ApplyOutcome::Blocked } else { wire::ApplyOutcome::Planned }.into(),
                            required_actions: if world.agents_blocked {
                                vec![wire::RequiredAction {
                                    subject: "triage/tools/linear/acme".into(),
                                    message: "Connection linear/other does not exist".into(),
                                    url: "/registry/tools/catalog/linear".into(),
                                    ..Default::default()
                                }]
                            } else {
                                Vec::new()
                            },
                            ..Default::default()
                        };
                        return ([(header::CONTENT_TYPE, "application/proto")], response.encode_to_vec()).into_response();
                    }
                    let body: Value = serde_json::from_slice(&body).unwrap();
                    calls.lock().unwrap().push((method.clone(), body.clone()));
                    let env = body["environmentId"].as_str().unwrap_or_default().to_string();
                    let answer = match method.as_str() {
                        "ListProjects" => json!({"projects": [{"id": "p1", "name": "shop", "teamId": "t1"}, {"id": "p2", "name": "warehouse", "teamId": "t1"}]}),
                        "ListEnvironments" if body["projectId"] == "p1" => {
                            json!({"environments": [{"id": "e1", "name": "production"}, {"id": "e2", "name": "staging"}]})
                        }
                        "ListEnvironments" => json!({"environments": [{"id": "e3", "name": "production"}]}),
                        "ListConnections" => world.connections.clone(),
                        "ListSecrets" => world.secrets.clone(),
                        "CreateVersion" => json!({"version": {"id": "v1", "status": "VERSION_STATUS_BUILDING"}}),
                        "GetVersion" => json!({"version": {"id": "v1", "status": "VERSION_STATUS_READY", "secrets": ["PG_PASSWORD"],
                            "slots": world.declared.as_array().unwrap().iter().map(|slot| json!({"name": slot, "provider": slot})).collect::<Vec<_>>()}}),
                        // What the environment has bound (nothing yet); the slots it lists are the
                        // project's newest version's, which the CLI must not rely on.
                        "ListEnvironmentConnections" => world.bound.get(env.as_str()).cloned().unwrap_or_else(|| {
                            json!({"connections": [{"slot": "github", "declared": true}, {"slot": "linear", "declared": true}]})
                        }),
                        "ListEnvironmentSecrets" => json!({"secrets": [{"name": "PG_PASSWORD", "declared": true, "set": false}]}),
                        "Deploy" => json!({"deployment": {"id": format!("d-{env}"), "status": "DEPLOYMENT_STATUS_DEPLOYING"}}),
                        "ListDeployments" => {
                            let status = if world.failing == Some(env.as_str()) { "DEPLOYMENT_STATUS_FAILED" } else { "DEPLOYMENT_STATUS_SUCCEEDED" };
                            json!({"deployments": [{"id": format!("d-{env}"), "status": status, "log": "the rollout broke"}]})
                        }
                        _ => json!({}),
                    };
                    axum::Json(answer).into_response()
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

    fn declarations(agents: bool) -> crate::discover::Declarations {
        crate::discover::Declarations {
            value: json!({"prompts": [], "skills": [], "tools": []}),
            digest: "test".into(),
            agents: if agents {
                vec![json!({"key": "triage", "name": "Triage"})]
            } else {
                Vec::new()
            },
            team: None,
            users: Vec::new(),
            groups: Vec::new(),
            ontologies: spec(),
        }
    }

    /// The calls that change something, in order.
    fn changes(calls: &Calls) -> Vec<String> {
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, _)| {
                m.starts_with("Create")
                    || m.starts_with("Set")
                    || m == "Deploy"
                    || m == "Apply dryRun=false"
            })
            .map(|(m, body)| match body["environmentId"].as_str() {
                Some(env) => format!("{m} {env}"),
                None => m.clone(),
            })
            .collect()
    }

    /// Missing connections and secrets block before anything changes, each with its link.
    #[tokio::test]
    async fn preflight_blocks_with_links_and_changes_nothing() {
        let repo = repository();
        let (url, calls) = cloud(World {
            connections: json!({"connections": [{"id": "c1", "slug": "github/acme", "typeId": "github_app", "status": "ready"}]}),
            secrets: json!({"secrets": []}),
            ..Default::default()
        })
        .await;
        let actions = crate::deploy::ontologies_first(
            &args(url.clone(), repo.path()),
            &declarations(false),
            false,
        )
        .await
        .unwrap();
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
        assert!(changes(&calls).is_empty(), "{:?}", changes(&calls));
    }

    /// Agents blocked while the ontologies are fine: both are reported, and nothing changed (the
    /// ontologies are not built ahead of agents that cannot be applied).
    #[tokio::test]
    async fn blocked_agents_leave_the_ontologies_untouched() {
        let repo = repository();
        let (url, calls) = cloud(World {
            agents_blocked: true,
            ..Default::default()
        })
        .await;
        let actions = crate::deploy::ontologies_first(
            &args(url.clone(), repo.path()),
            &declarations(true),
            true,
        )
        .await
        .unwrap();
        assert_eq!(actions.len(), 1, "{}", render(&actions, &url));
        assert!(actions[0].message.contains("linear/other"));
        assert!(
            calls
                .lock()
                .unwrap()
                .iter()
                .any(|(m, _)| m == "Apply dryRun=true")
        );
        assert!(changes(&calls).is_empty(), "{:?}", changes(&calls));
    }

    /// A dry run reads only.
    #[tokio::test]
    async fn a_dry_run_changes_nothing() {
        let repo = repository();
        let (url, calls) = cloud(World::default()).await;
        let mut dry = args(url, repo.path());
        dry.dry_run = true;
        let actions = crate::deploy::ontologies_first(&dry, &declarations(true), true)
            .await
            .unwrap();
        assert!(actions.is_empty());
        assert!(changes(&calls).is_empty(), "{:?}", changes(&calls));
    }

    /// The plan shows each slot binding that changes, from what the environment has bound now,
    /// and counts the ones that stay.
    #[tokio::test]
    async fn the_plan_shows_slot_changes() {
        let (url, _) = cloud(World {
            bound: BTreeMap::from([(
                "e1",
                json!({"connections": [
                    {"slot": "github", "connectionId": "c9", "connectionName": "github/old"},
                    {"slot": "linear", "connectionId": "c2", "connectionName": "linear/acme"}]}),
            )]),
            ..Default::default()
        })
        .await;
        let client = Client {
            base: url,
            http: reqwest::Client::new(),
            key: None,
        };
        let specs = spec();
        let shop = BTreeMap::from([("shop".to_string(), &specs["shop"])]);
        let mut actions = Vec::new();
        let projects = preflight(&client, &shop, &mut actions).await.unwrap();
        assert!(actions.is_empty());
        let plan = plan_text(&projects);
        assert!(
            plan.contains("production: slot github: github/old → github/acme\n"),
            "{plan}"
        );
        assert!(plan.contains("production: 1 slot unchanged\n"), "{plan}");
        assert!(!plan.contains("production: slot linear"), "{plan}");
        assert!(
            plan.contains("staging: slot linear: unbound → linear/acme\n"),
            "{plan}"
        );
    }

    /// One version per ontology from the archive, then each environment bound and rolled out in
    /// turn (accepted rebuilds passed along), the team's secret before the organization's.
    #[tokio::test]
    async fn builds_then_binds_and_rolls_out_one_environment_at_a_time() {
        let repo = repository();
        let (url, calls) = cloud(World::default()).await;
        let mut a = args(url, repo.path());
        a.ontology = vec!["shop".into()];
        a.accept_rebuild = vec!["orders".into()];
        assert!(
            crate::deploy::ontologies_first(&a, &declarations(false), false)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            changes(&calls),
            [
                "CreateVersion",
                "SetEnvironmentConnection e1",
                "SetEnvironmentConnection e1",
                "SetEnvironmentSecretBinding e1",
                "Deploy e1",
                "SetEnvironmentConnection e2",
                "SetEnvironmentConnection e2",
                "SetEnvironmentSecretBinding e2",
                "Deploy e2",
            ]
        );
        let calls = calls.lock().unwrap().clone();
        let created = &calls.iter().find(|(m, _)| m == "CreateVersion").unwrap().1;
        assert_eq!(created["projectId"], "p1");
        assert!(!created["archive"].as_str().unwrap().is_empty());
        let secret = &calls
            .iter()
            .find(|(m, _)| m == "SetEnvironmentSecretBinding")
            .unwrap()
            .1;
        assert_eq!(
            (secret["name"].as_str(), secret["secretId"].as_str()),
            (Some("PG_PASSWORD"), Some("s-team"))
        );
        let deploy = &calls.iter().find(|(m, _)| m == "Deploy").unwrap().1;
        assert_eq!(deploy["acceptRebuild"], json!(["orders"]));
        // `--ontology shop`: warehouse was neither checked nor built.
        assert!(
            !calls
                .iter()
                .any(|(m, b)| m == "ListEnvironments" && b["projectId"] == "p2")
        );
    }

    /// A failed rollout stops there and says what runs the new version and what does not.
    #[tokio::test]
    async fn a_failed_rollout_stops_and_reports_each_environment() {
        let repo = repository();
        let (url, calls) = cloud(World {
            failing: Some("e2"),
            ..Default::default()
        })
        .await;
        let mut a = args(url, repo.path());
        a.ontology = vec!["shop".into()];
        let error = format!(
            "{:#}",
            crate::deploy::ontologies_first(&a, &declarations(false), false)
                .await
                .unwrap_err()
        );
        assert!(error.contains("the rollout to staging failed"), "{error}");
        assert!(error.contains("running version v1: production"), "{error}");
        assert!(
            error.contains("bound to it but not running it: staging"),
            "{error}"
        );
        assert!(error.contains("the rollout broke"), "{error}");
        assert_eq!(changes(&calls).last().unwrap(), "Deploy e2");
    }

    /// A later ontology failing names every rollout that completed before it, of every ontology,
    /// and says the agents were not applied.
    #[tokio::test]
    async fn a_later_ontology_failing_reports_every_completed_rollout() {
        let repo = repository();
        let (url, _) = cloud(World {
            failing: Some("e3"),
            ..Default::default()
        })
        .await;
        let a = args(url, repo.path());
        let error = format!(
            "{:#}",
            crate::deploy::ontologies_first(&a, &declarations(true), true)
                .await
                .unwrap_err()
        );
        // Ontologies deploy in name order: shop's two environments, then warehouse's, which fails.
        assert!(
            error.contains("stopped at ontology warehouse; rollouts that completed: shop/production, shop/staging; the agents were not applied"),
            "{error}"
        );
        assert!(
            error.contains("the rollout to production failed"),
            "{error}"
        );
    }

    /// A version that does not declare what the spec binds stops before any binding.
    #[tokio::test]
    async fn a_version_without_the_specs_slots_binds_nothing() {
        let repo = repository();
        let (url, calls) = cloud(World {
            declared: json!(["github"]),
            ..Default::default()
        })
        .await;
        let mut a = args(url, repo.path());
        a.ontology = vec!["shop".into()];
        let error = format!(
            "{:#}",
            crate::deploy::ontologies_first(&a, &declarations(false), false)
                .await
                .unwrap_err()
        );
        assert!(
            error.contains("production: the version declares no slot linear"),
            "{error}"
        );
        assert_eq!(changes(&calls), ["CreateVersion"]);
    }

    /// Uncommitted changes refuse the deploy (before any version), unless allowed; an unknown
    /// `--ontology` is refused.
    #[tokio::test]
    async fn a_dirty_tree_and_an_unknown_ontology_are_refused() {
        let repo = repository();
        std::fs::write(
            repo.path().join("ontologies/shop/ontology.ts"),
            "export default { changed: true };\n",
        )
        .unwrap();
        let (url, calls) = cloud(World::default()).await;
        let mut a = args(url, repo.path());
        let error = crate::deploy::ontologies_first(&a, &declarations(false), false)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("uncommitted changes"), "{error}");
        assert!(changes(&calls).is_empty());
        a.ontology = vec!["nope".into()];
        let error = crate::deploy::ontologies_first(&a, &declarations(false), false)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("--ontology nope"), "{error}");
        a.ontology = vec!["shop".into()];
        a.allow_dirty = true;
        crate::deploy::ontologies_first(&a, &declarations(false), false)
            .await
            .unwrap();
    }

    /// A server without the ontology control plane (open-source Tilde) needs Tilde Cloud.
    #[tokio::test]
    async fn a_server_without_ontologies_needs_tilde_cloud() {
        let repo = repository();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, Router::new()).await;
        });
        let (actions, plan) = prepare(&args(url, repo.path()), &spec()).await.unwrap();
        assert!(plan.is_none());
        assert_eq!(actions.len(), 1);
        assert!(
            actions[0].message.contains("need Tilde Cloud"),
            "{}",
            actions[0].message
        );
    }
}
