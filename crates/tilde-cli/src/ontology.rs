//! `tilde deploy` for a project's ontologies (Tilde Cloud): `defineProject({ ontologies })`.
//!
//! Each ontology is the Tilde project of its name, `ontologies/<name>/` of the repository
//! (`tilde deploy` runs at the repository's root), built here, on the client: Tilde never builds
//! a project. `prepare` changes nothing: it checks every connection and secret the specs name
//! (each missing one is a required action with a link, reported with the agents' own before
//! anything changes), then builds each ontology into an artifact with the project's own
//! `@trytilde/ontology` and `@trytilde/spaces` ([`build`]): the IR, the errors or the contract
//! Tilde compiles for it (`VersionService.CompileOntology`), the handler module, the spaces.
//! `execute` then, per ontology: uploads the artifact as a version
//! (`VersionService.CreateVersion`), checks that the version declares exactly what the spec binds,
//! and binds and rolls out one environment at a time. Agents are applied after (`apply.rs`).
//!
//! A first deploy needs nothing set up by hand: a project or an environment the server does not
//! have is part of the plan, and `execute` creates it (`ProjectService.CreateProject` in the
//! deploy's team, `EnvironmentService.CreateEnvironment` with the storage the IR declares and the
//! spec's `live`). Compiling needs the project, so a new project's ontology is built in `execute`,
//! once the project exists, rather than in `prepare`.
//!
//! The ontology control plane is Tilde Cloud's: its RPCs are called as Connect JSON (this CLI
//! does not depend on its contract), and a server without them answers 404, which becomes a
//! "needs Tilde Cloud" required action.
use crate::deploy::Deploy;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// An ontology as `@trytilde/iac`'s `defineProject` writes it.
#[derive(Debug, Deserialize)]
pub struct OntologySpec {
    #[serde(default)]
    pub environments: BTreeMap<String, EnvironmentSpec>,
}
#[derive(Debug, Default, Deserialize)]
pub struct EnvironmentSpec {
    /// The ontology's connection -> the organization's connection it binds.
    #[serde(default)]
    pub connections: BTreeMap<String, ConnectionSpec>,
    /// The ontology's secret name -> the Tilde secret's name.
    #[serde(default)]
    pub secrets: BTreeMap<String, String>,
    /// Whether the environment is live (`LiveService` serves its spaces): set when the deploy
    /// creates it. No RPC changes it afterwards, so a difference from an existing environment is
    /// reported and left.
    pub live: Option<bool>,
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
    /// None: the deploy creates it.
    id: Option<String>,
    /// The spec's `live`, for an environment the deploy creates.
    live: bool,
    /// The spec's `live` when the existing environment's differs.
    live_differs: Option<bool>,
    connections: Vec<(String, String)>,
    secrets: Vec<(String, String)>,
}
struct Project {
    name: String,
    /// None: the deploy creates it, in `team`.
    id: Option<String>,
    team: String,
    environments: Vec<Resolved>,
}

/// An ontology built on the client: its artifact, the tools it serves and the storage its IR
/// declares (what a new environment is created with).
struct Built {
    artifact: Vec<u8>,
    handlers: BTreeSet<String>,
    storage: &'static str,
}

/// The largest artifact Tilde Cloud takes (its `versions::MAX_ARTIFACT`).
const MAX_ARTIFACT: usize = 32 * 1024 * 1024;

/// What `prepare` found, for `execute`: every project resolved, the repository archived once.
pub struct Plan {
    client: Client,
    projects: Vec<Project>,
    /// Each existing project's build, by project name; a new project's is built once it exists.
    built: BTreeMap<String, Built>,
    dir: PathBuf,
    commit: String,
    branch: String,
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// Every project, environment, connection and secret the specs name. A missing project or
/// environment is created by the deploy (in `team`, a slug, or the organization); a missing
/// connection or secret becomes an action. Nothing is changed.
async fn preflight(
    client: &Client,
    ontologies: &BTreeMap<String, &OntologySpec>,
    team: Option<&str>,
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
    let missing = |name: &String| !projects.iter().any(|p| p["name"] == name.as_str());
    // The team new projects are created in: its id, asked for only when one is created.
    let mut new_team = String::new();
    if let Some(slug) = team.filter(|_| ontologies.keys().any(missing)) {
        let teams = client
            .ok("tilde.management.v1.IamService/ListTeams", json!({}))
            .await?;
        let found = teams["teams"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|t| t["slug"] == slug);
        match found {
            Some(found) => new_team = found["id"].as_str().unwrap_or_default().to_string(),
            None => actions.push(Action {
                subject: format!("team {slug}"),
                message: format!("The team {slug} does not exist; create it"),
                url: "/settings/teams".into(),
            }),
        }
    }
    let mut out = Vec::new();
    for (name, spec) in ontologies {
        let project = projects.iter().find(|p| p["name"] == name.as_str());
        let id = project.map(|p| p["id"].as_str().unwrap_or_default().to_string());
        let team = match project {
            Some(p) => p["teamId"].as_str().unwrap_or_default().to_string(),
            None => new_team.clone(),
        };
        let environments = match &id {
            Some(id) => client
                .ok(
                    &format!("{ENVIRONMENTS}/ListEnvironments"),
                    json!({"projectId": id}),
                )
                .await?["environments"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            None => Vec::new(),
        };
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
            let found = environments.iter().find(|e| e["name"] == env_name.as_str());
            let mut bindings = Vec::new();
            for (name, connection) in &env.connections {
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
                let subject = format!("{subject} connection {name}");
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
                bindings.push((
                    name.clone(),
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
            // Protobuf JSON leaves a false `live` out.
            let current = found.map(|e| e["live"] == true);
            resolved.push(Resolved {
                environment: env_name.clone(),
                id: found.map(|e| e["id"].as_str().unwrap_or_default().to_string()),
                live: env.live.unwrap_or(false),
                live_differs: env.live.filter(|live| current.is_some_and(|c| c != *live)),
                connections: bindings,
                secrets: bound,
            });
        }
        out.push(Project {
            name: name.clone(),
            id,
            team,
            environments: resolved,
        });
    }
    Ok(out)
}

/// The commit and branch a deploy is of: HEAD. Uncommitted changes are refused, unless allowed
/// (they are built, under HEAD's commit).
fn head(dir: &Path, allow_dirty: bool) -> Result<(String, String)> {
    if !git(dir, &["status", "--porcelain"])?.is_empty() {
        if !allow_dirty {
            bail!(
                "the working tree has uncommitted changes; commit them, or pass --allow-dirty to build it as it is"
            );
        }
        eprintln!("warning: building uncommitted changes as HEAD's commit");
    }
    let commit = git(dir, &["rev-parse", "HEAD"])?;
    let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    Ok((
        commit,
        if branch == "HEAD" {
            String::new()
        } else {
            branch
        },
    ))
}

/// `<package>` as `dir` resolves it (its own `node_modules` or an ancestor's): its directory.
fn installed(dir: &Path, package: &str) -> Option<PathBuf> {
    dir.ancestors()
        .map(|d| d.join("node_modules").join(package))
        .find(|p| p.join("package.json").is_file())
}

/// `node <cli> <args>` from a project package, as the user runs it: its stdout.
fn node(cli: &Path, args: &[&std::ffi::OsStr]) -> Result<Vec<u8>> {
    let out = std::process::Command::new("node")
        .arg(cli)
        .args(args)
        .output()
        .context("running node (Node 22 or newer builds ontologies)")?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(out.stdout)
}

/// Build the ontology in `folder` (`ontologies/<name>/`) into an artifact, as Tilde takes it (a
/// plain tar): `artifact.json` (the runtime the project's `@trytilde/ontology` builds for),
/// `ontology.ir.json`, `worker.js` (the handler module) and `spaces/<id>/` (built against the
/// contract Tilde compiles for the IR, which is written into the spaces project as
/// `@tilde/contract`). An IR Tilde refuses fails the deploy with its errors.
/// Evaluate an ontology here with the project's `@trytilde/ontology` and have Tilde compile it:
/// the package's client build, its runtime, the IR and Tilde's answer (its contract).
async fn compile(
    client: &Client,
    name: &str,
    project_id: &str,
    folder: &Path,
) -> Result<(PathBuf, u64, Vec<u8>, Value)> {
    let package = installed(folder, "@trytilde/ontology").with_context(|| {
        format!(
            "ontologies/{name} needs @trytilde/ontology installed (pnpm add @trytilde/ontology)"
        )
    })?;
    let manifest: Value = serde_json::from_slice(&std::fs::read(package.join("package.json"))?)?;
    let runtime = manifest["tilde"]["runtime"].as_u64().with_context(|| {
        format!(
            "{} declares no runtime: upgrade @trytilde/ontology",
            package.display()
        )
    })?;
    let cli = package.join("dist/cli.js");
    let ir = node(&cli, &["ir".as_ref(), folder.as_os_str()])
        .with_context(|| format!("ontology {name}: evaluating ontology.ts"))?;
    let compiled = client
        .ok(
            &format!("{VERSIONS}/CompileOntology"),
            json!({"projectId": project_id, "ir": String::from_utf8(ir.clone())?}),
        )
        .await?;
    if let Some(errors) = compiled["errors"].as_array().filter(|e| !e.is_empty()) {
        let errors: Vec<&str> = errors.iter().filter_map(Value::as_str).collect();
        bail!(
            "ontology {name} does not compile:\n  {}",
            errors.join("\n  ")
        );
    }
    Ok((cli, runtime, ir, compiled))
}

/// The handlers of a compiled ontology (its contract's `contract.json`) that are tools: the ones
/// its `tilde_ontology` connection serves once it runs (reads and actions; realtime views are
/// not tools).
fn handlers(compiled: &Value) -> Result<BTreeSet<String>> {
    let contract: Value = serde_json::from_str(
        compiled["contract"]["contract.json"]
            .as_str()
            .context("Tilde's contract has no contract.json")?,
    )?;
    Ok(contract["handlers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|h| h["kind"] != "realtime")
        .filter_map(|h| h["handler"].as_str().map(str::to_owned))
        .collect())
}

/// Build an ontology into its artifact (and its handlers, `handlers`).
async fn build(client: &Client, name: &str, project_id: &str, folder: &Path) -> Result<Built> {
    let (cli, runtime, ir, compiled) = compile(client, name, project_id, folder).await?;
    // The IR's `storage`: `postgres`, or cells (the default).
    let storage = match serde_json::from_slice::<Value>(&ir)?["storage"].as_str() {
        Some("postgres") => "STORAGE_POSTGRES",
        _ => "STORAGE_CELL",
    };
    let out = folder.join(".tilde/build");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out)?;
    // The build is never part of the tree: the next deploy needs it clean.
    std::fs::write(folder.join(".tilde/.gitignore"), "*\n")?;
    let worker = out.join("worker.js");
    node(
        &cli,
        &["worker".as_ref(), folder.as_os_str(), worker.as_os_str()],
    )
    .with_context(|| format!("ontology {name}: bundling the handler module"))?;
    let mut files: Vec<(String, Vec<u8>)> = vec![
        (
            "artifact.json".into(),
            json!({ "runtime": runtime }).to_string().into_bytes(),
        ),
        ("ontology.ir.json".into(), ir),
        ("worker.js".into(), std::fs::read(&worker)?),
    ];
    let spaces = folder.join("spaces");
    if spaces.join("package.json").is_file() {
        let contract = spaces.join("node_modules/@tilde/contract");
        let _ = std::fs::remove_dir_all(&contract);
        for (path, text) in compiled["contract"].as_object().into_iter().flatten() {
            let file = contract.join(path);
            if file.strip_prefix(&contract).is_err() || path.contains("..") {
                bail!("the contract names {path}, outside its package");
            }
            std::fs::create_dir_all(file.parent().unwrap_or(&contract))?;
            std::fs::write(file, text.as_str().unwrap_or_default())?;
        }
        let kit = installed(&spaces, "@trytilde/spaces").with_context(|| {
            format!("ontologies/{name}/spaces needs @trytilde/spaces installed (pnpm add @trytilde/spaces)")
        })?;
        let built = out.join("spaces");
        node(
            &kit.join("dist/cli.js"),
            &["build".as_ref(), spaces.as_os_str(), built.as_os_str()],
        )
        .with_context(|| format!("ontology {name}: building the spaces"))?;
        let mut stack = vec![built.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let relative = path.strip_prefix(&built)?.to_string_lossy().into_owned();
                    files.push((format!("spaces/{relative}"), std::fs::read(&path)?));
                }
            }
        }
    }
    files.sort();
    let mut tar = tar::Builder::new(Vec::new());
    for (path, bytes) in &files {
        let mut header = tar::Header::new_ustar();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, path, bytes.as_slice())?;
    }
    let artifact = tar.into_inner()?;
    if artifact.len() > MAX_ARTIFACT {
        bail!(
            "ontology {name}: the artifact is {} MiB, over Tilde's {} MiB (its spaces' assets, most likely)",
            artifact.len() / (1024 * 1024),
            MAX_ARTIFACT / (1024 * 1024)
        );
    }
    Ok(Built {
        artifact,
        handlers: handlers(&compiled)?,
        storage,
    })
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

/// Check and resolve the ontologies (all, or `--ontology`'s) and build each into an artifact,
/// changing nothing: the actions that block a deploy, else the plan (none when there is nothing to
/// do or it is a dry run), and the tools the ontologies' connections will serve (their compiled
/// handlers: a dry run compiles for them only when `tools` asks).
pub async fn prepare(
    args: &Deploy,
    ontologies: &BTreeMap<String, OntologySpec>,
    team: Option<&str>,
    tools: bool,
) -> Result<(Vec<Action>, Option<Plan>, BTreeSet<String>)> {
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
        return Ok((Vec::new(), None, BTreeSet::new()));
    }
    // Each ontology is `ontologies/<name>/` of the repository.
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
    }
    let client = Client {
        base: crate::api::base_url(url)?,
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?,
        key: args.api_key.clone(),
    };
    let mut actions = Vec::new();
    let projects = preflight(&client, &selected, team, &mut actions).await?;
    // A check deploys nothing, and Tilde compiles an ontology for its project.
    if args.check {
        for p in projects.iter().filter(|p| p.id.is_none()) {
            actions.push(Action {
                subject: format!("ontology {}", p.name),
                message: format!(
                    "The ontology project {} does not exist yet, so Tilde cannot compile it: its first `tilde deploy` creates it",
                    p.name
                ),
                url: "/ontology/projects".into(),
            });
        }
    }
    if !actions.is_empty() {
        return Ok((actions, None, BTreeSet::new()));
    }
    if args.dry_run {
        eprint!("{}", plan_text(&client, &projects, &selected).await?);
        let mut served = BTreeSet::new();
        if tools {
            for p in &projects {
                let Some(id) = &p.id else { continue };
                let folder = dir.join("ontologies").join(&p.name);
                served.extend(handlers(&compile(&client, &p.name, id, &folder).await?.3)?);
            }
        }
        return Ok((Vec::new(), None, served));
    }
    // A check uploads nothing, so it builds the working tree as it is.
    let (commit, branch) = match args.check {
        true => Default::default(),
        false => head(&dir, args.allow_dirty)?,
    };
    let mut built = BTreeMap::new();
    let mut served = BTreeSet::new();
    for p in &projects {
        let Some(id) = &p.id else { continue };
        eprintln!("Ontology {}: building", p.name);
        let folder = dir.join("ontologies").join(&p.name);
        let b = build(&client, &p.name, id, &folder).await?;
        served.extend(b.handlers.iter().cloned());
        built.insert(p.name.clone(), b);
    }
    if args.check {
        eprintln!("Checked (nothing deployed): each ontology compiles and builds");
        return Ok((Vec::new(), None, served));
    }
    Ok((
        Vec::new(),
        Some(Plan {
            client,
            projects,
            built,
            dir,
            commit,
            branch,
        }),
        served,
    ))
}

/// A binding's connection name.
fn binding_name(binding: &Value) -> &str {
    binding["name"].as_str().unwrap_or_default()
}

/// The dry run's plan: a project or environment the deploy creates, each connection whose binding
/// changes as `connection name: current → new` (from what the environment has bound now), a count
/// of the unchanged ones, and the optional connections left unbound.
async fn plan_text(
    client: &Client,
    projects: &[Project],
    specs: &BTreeMap<String, &OntologySpec>,
) -> Result<String> {
    let mut out = String::from("Plan (dry run, nothing deployed):\n");
    for p in projects {
        if p.id.is_none() {
            out.push_str(&format!("  ontology {}: create the project\n", p.name));
        }
        out.push_str(&format!(
            "  ontology {}: build HEAD into a version\n",
            p.name
        ));
        for env in &p.environments {
            let e = &env.environment;
            let bound = match &env.id {
                None => {
                    let live = if env.live { " (live)" } else { "" };
                    out.push_str(&format!("    {e}: create the environment{live}\n"));
                    Vec::new()
                }
                Some(id) => client
                    .ok(
                        &format!("{ENVIRONMENTS}/ListEnvironmentConnections"),
                        json!({"environmentId": id}),
                    )
                    .await?["connections"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            };
            if let Some(live) = env.live_differs {
                out.push_str(&format!("    {e}: {}\n", live_differs(live)));
            }
            let mut unchanged = 0;
            for (name, connection) in &env.connections {
                let current = bound.iter().find(|b| binding_name(b) == name.as_str());
                if current.is_some_and(|b| b["connectionId"] == connection.as_str()) {
                    unchanged += 1;
                    continue;
                }
                let current = current
                    .and_then(|b| b["connectionName"].as_str())
                    .filter(|c| !c.is_empty())
                    .unwrap_or("unbound");
                let new = &specs[&p.name].environments[e].connections[name].slug;
                out.push_str(&format!("    {e}: connection {name}: {current} → {new}\n"));
            }
            if unchanged > 0 {
                let plural = if unchanged == 1 { "" } else { "s" };
                out.push_str(&format!(
                    "    {e}: {unchanged} connection{plural} unchanged\n"
                ));
            }
            // What the project's newest version declares: the version this deploy builds decides.
            for b in bound.iter().filter(|b| {
                b["optional"] == true
                    && b["connectionId"].as_str().is_none_or(str::is_empty)
                    && !env.connections.iter().any(|(n, _)| n == binding_name(b))
            }) {
                out.push_str(&format!("    {e}: {}\n", skipped(binding_name(b))));
            }
            for (secret, _) in &env.secrets {
                out.push_str(&format!("    {e}: bind secret {secret}\n"));
            }
            out.push_str(&format!("    {e}: roll out\n"));
        }
    }
    Ok(out)
}

/// An environment's `live` is set when it is created; no RPC changes it.
fn live_differs(live: bool) -> String {
    format!(
        "live stays {}: the spec says {live}, and an environment's live is set when it is created",
        !live
    )
}

/// An optional connection nothing binds: the rollout leaves out what needs it.
fn skipped(name: &str) -> String {
    format!(
        "connection {name}: skipped (optional and unbound: its pipelines do not run until it is bound)"
    )
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

/// That the version declares every connection and secret the environment binds, and that each one it
/// declares is bound (by the spec or already): checked for every environment before any binding.
/// An optional connection (`{ provider, optional: true }`) may stay unbound: it is reported as
/// skipped.
async fn check_declared(
    client: &Client,
    p: &Project,
    environments: &[String],
    version: &Value,
) -> Result<()> {
    // Each declared connection's name, and whether it is optional.
    let declared: Vec<(&str, bool)> = version["connections"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| Some((c["name"].as_str()?, c["optional"] == true)))
        .collect();
    let secrets: Vec<&str> = version["secrets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut problems = Vec::new();
    for (env, environment_id) in p.environments.iter().zip(environments) {
        let bindings = client
            .ok(
                &format!("{ENVIRONMENTS}/ListEnvironmentConnections"),
                json!({"environmentId": environment_id}),
            )
            .await?["connections"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let set = client
            .ok(
                &format!("{ENVIRONMENTS}/ListEnvironmentSecrets"),
                json!({"environmentId": environment_id}),
            )
            .await?["secrets"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for (name, _) in &env.connections {
            if !declared.iter().any(|(d, _)| d == name) {
                problems.push(format!(
                    "{}: the version declares no connection {name}",
                    env.environment
                ));
            }
        }
        for (name, optional) in &declared {
            let bound = bindings.iter().any(|b| {
                binding_name(b) == *name
                    && b["connectionId"].as_str().is_some_and(|c| !c.is_empty())
            });
            if bound || env.connections.iter().any(|(n, _)| n == name) {
                continue;
            }
            if *optional {
                eprintln!(
                    "Ontology {}: {}: {}",
                    p.name,
                    env.environment,
                    skipped(name)
                );
            } else {
                problems.push(format!(
                    "{}: connection {name} is neither bound nor in the spec",
                    env.environment
                ));
            }
        }
        for (name, _) in &env.secrets {
            if !secrets.contains(&name.as_str()) {
                problems.push(format!(
                    "{}: the version declares no secret {name}",
                    env.environment
                ));
            }
        }
        for name in &secrets {
            let already = set.iter().any(|s| s["name"] == *name && s["set"] == true);
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

/// Create what is new, build, check, then bind and roll out one environment at a time, ontology
/// by ontology. On a failure, says which environments of that ontology run the new version, which is bound to it
/// but not running it, and which are as they were; every rollout that completed before it, of any
/// ontology; and that the agents were not applied.
pub async fn execute(args: &Deploy, plan: Plan) -> Result<()> {
    let mut completed = Vec::new();
    for p in &plan.projects {
        if let Err(e) = deploy_one(args, &plan, p, &mut completed).await {
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
    completed: &mut Vec<String>,
) -> Result<()> {
    use base64::Engine;
    let client = &plan.client;
    {
        let project_id = match &p.id {
            Some(id) => id.clone(),
            None => {
                let mut body = json!({"name": p.name});
                if !p.team.is_empty() {
                    body["teamId"] = json!(p.team);
                }
                let created = client
                    .ok(&format!("{PROJECTS}/CreateProject"), body)
                    .await?;
                eprintln!("Ontology {}: created the project", p.name);
                created["project"]["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()
            }
        };
        // A new project's ontology compiles only now that the project exists.
        let new;
        let built = match plan.built.get(&p.name) {
            Some(built) => built,
            None => {
                eprintln!("Ontology {}: building", p.name);
                let folder = plan.dir.join("ontologies").join(&p.name);
                new = build(client, &p.name, &project_id, &folder).await?;
                &new
            }
        };
        let encoded = base64::engine::general_purpose::STANDARD.encode(&built.artifact);
        eprintln!(
            "Ontology {}: uploading {}",
            p.name,
            &plan.commit[..plan.commit.len().min(12)]
        );
        let created = client
            .ok(
                &format!("{VERSIONS}/CreateVersion"),
                json!({"projectId": project_id, "commitSha": plan.commit, "ref": plan.branch, "artifact": encoded}),
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
        let mut environments = Vec::new();
        for env in &p.environments {
            if let Some(live) = env.live_differs {
                eprintln!(
                    "Ontology {}: {}: {}",
                    p.name,
                    env.environment,
                    live_differs(live)
                );
            }
            environments.push(match &env.id {
                Some(id) => id.clone(),
                None => {
                    let created = client
                        .ok(
                            &format!("{ENVIRONMENTS}/CreateEnvironment"),
                            json!({"projectId": project_id, "name": env.environment, "target": "TARGET_CLOUDFLARE", "storage": built.storage, "live": env.live}),
                        )
                        .await?;
                    eprintln!(
                        "Ontology {}: created the environment {}",
                        p.name, env.environment
                    );
                    created["environment"]["id"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string()
                }
            });
        }
        check_declared(client, p, &environments, &version).await?;
        let mut done: Vec<&str> = Vec::new();
        for (env, environment_id) in p.environments.iter().zip(&environments) {
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
            for (name, connection) in &env.connections {
                client
                    .ok(&format!("{ENVIRONMENTS}/SetEnvironmentConnection"), json!({"environmentId": environment_id, "name": name, "connectionId": connection}))
                    .await
                    .map_err(|e| e.context(state(false)))?;
            }
            for (name, secret) in &env.secrets {
                client
                    .ok(
                        &format!("{ENVIRONMENTS}/SetEnvironmentSecretBinding"),
                        json!({"environmentId": environment_id, "name": name, "secretId": secret}),
                    )
                    .await
                    .map_err(|e| e.context(state(false)))?;
            }
            let started = client
                .ok(
                    &format!("{ENVIRONMENTS}/Deploy"),
                    json!({"environmentId": environment_id, "versionId": version_id, "acceptRebuild": args.accept_rebuild}),
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
                json!({"environmentId": environment_id}),
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
            role: None,
            external_id: None,
            label: None,
            dry_run: false,
            check: false,
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
                    "connections": {"github": {"slug": "github/acme"}, "linear": {"slug": "linear/acme", "type": "api_key"}},
                    "secrets": {"PG_PASSWORD": "SHOP_PG_PASSWORD"}},
                "staging": {
                    "connections": {"github": {"slug": "github/acme"}, "linear": {"slug": "linear/acme", "type": "api_key"}},
                    "secrets": {"PG_PASSWORD": "SHOP_PG_PASSWORD"}}}},
            "warehouse": {"environments": {
                "production": {
                    "connections": {"github": {"slug": "github/acme"}, "linear": {"slug": "linear/acme", "type": "api_key"}},
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

    /// A stand-in `@trytilde/ontology` (runtime 1): `ir` prints an IR naming the folder, `worker`
    /// writes a module.
    const CLIENT: &str = r#"import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
const [command, root, out] = process.argv.slice(2);
if (command === "ir") process.stdout.write(JSON.stringify({ root, storage: "postgres" }));
else { mkdirSync(dirname(out), { recursive: true }); writeFileSync(out, "export default {};\n"); }
"#;

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
        // The project's packages: installed, not committed.
        // `.tilde/` is not listed: the build ignores its own output.
        std::fs::write(dir.path().join(".gitignore"), "node_modules/\n").unwrap();
        let package = dir.path().join("node_modules/@trytilde/ontology");
        std::fs::create_dir_all(package.join("dist")).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name": "@trytilde/ontology", "type": "module", "tilde": {"runtime": 1}}"#,
        )
        .unwrap();
        std::fs::write(package.join("dist/cli.js"), CLIENT).unwrap();
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
        /// Connections the built version declares.
        declared: Value,
        /// Whether a dry-run Apply of the agents comes back blocked.
        agents_blocked: bool,
        /// Per environment id, what ListEnvironmentConnections answers.
        bound: BTreeMap<&'static str, Value>,
        /// What CompileOntology answers when the IR does not compile.
        compile_errors: Option<Value>,
        /// Whether a dry-run Apply finds the ontology's tools unserved, as on a new environment.
        ontology_unserved: bool,
        /// An organization with no project or environment yet.
        fresh: bool,
        /// The declared connections the version marks optional.
        optional: &'static [&'static str],
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
                compile_errors: None,
                ontology_unserved: false,
                fresh: false,
                optional: &[],
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
                        let unserved = world.ontology_unserved && request.dry_run;
                        let response = wire::ApplyResponse {
                            outcome: if world.agents_blocked || unserved { wire::ApplyOutcome::Blocked } else { wire::ApplyOutcome::Planned }.into(),
                            required_actions: if unserved {
                                vec![wire::RequiredAction {
                                    code: wire::RequiredActionCode::ToolNotOffered.into(),
                                    subject: "triage/tools/tilde_ontology/shop".into(),
                                    message: "Connection tilde_ontology/shop offers no tool listOrders".into(),
                                    ..Default::default()
                                }]
                            } else if world.agents_blocked {
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
                        "ListProjects" | "ListEnvironments" if world.fresh => json!({}),
                        "ListTeams" => json!({"teams": [{"id": "t1", "slug": "factory"}]}),
                        "CreateProject" => json!({"project": {"id": "p1", "name": body["name"], "teamId": body["teamId"]}}),
                        "CreateEnvironment" => json!({"environment": {"id": format!("e-{}", body["name"].as_str().unwrap())}}),
                        "ListProjects" => json!({"projects": [{"id": "p1", "name": "shop", "teamId": "t1"}, {"id": "p2", "name": "warehouse", "teamId": "t1"}]}),
                        "ListEnvironments" if body["projectId"] == "p1" => {
                            json!({"environments": [{"id": "e1", "name": "production"}, {"id": "e2", "name": "staging"}]})
                        }
                        "ListEnvironments" => json!({"environments": [{"id": "e3", "name": "production"}]}),
                        "ListConnections" => world.connections.clone(),
                        "ListSecrets" => world.secrets.clone(),
                        "CompileOntology" => match &world.compile_errors {
                            Some(errors) => json!({"errors": errors}),
                            None => json!({"errors": [], "contract": {"contract.json": r#"{"handlers": [{"handler": "listOrders", "kind": "read"}, {"handler": "watchOrders", "kind": "realtime"}]}"#}}),
                        },
                        "CreateVersion" => json!({"version": {"id": "v1", "status": "VERSION_STATUS_BUILDING"}}),
                        "GetVersion" => {
                            let declared: Vec<Value> = world.declared.as_array().unwrap().iter().map(|name| json!({"name": name, "provider": name, "optional": world.optional.contains(&name.as_str().unwrap())})).collect();
                            json!({"version": {"id": "v1", "status": "VERSION_STATUS_READY", "secrets": ["PG_PASSWORD"], "connections": declared}})
                        }
                        // What the environment has bound (nothing yet); the connections it lists are the
                        // project's newest version's, which the CLI must not rely on.
                        "ListEnvironmentConnections" => world
                            .bound
                            .get(env.as_str())
                            .cloned()
                            .unwrap_or_else(|| json!({"connections": [{"name": "github", "declared": true}, {"name": "linear", "declared": true}]})),
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

    /// A project whose agent `triage` uses `tools` of the shop ontology's connection.
    fn agent_using(tools: &[&str]) -> crate::discover::Declarations {
        let mut d = declarations(true);
        d.agents = vec![json!({"key": "triage", "name": "Triage",
            "tools": [{"connection": {"slug": "tilde_ontology/shop"}, "tools": tools}]})];
        d
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
            ..Default::default()
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
        let (actions, _) = crate::deploy::ontologies_first(
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
        let (actions, _) = crate::deploy::ontologies_first(
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
        let (actions, _) = crate::deploy::ontologies_first(&dry, &declarations(true), true)
            .await
            .unwrap();
        assert!(actions.is_empty());
        assert!(changes(&calls).is_empty(), "{:?}", changes(&calls));
    }

    /// A new environment's first deploy is one command: its ontology isn't running yet, so the
    /// server lists none of its tools, and the agents' tools on its connection are checked against
    /// the compiled handlers instead; a tool the ontology does not serve still blocks the deploy
    /// before anything changes.
    #[tokio::test]
    async fn a_new_environment_deploys_its_ontology_and_agents_in_one_command() {
        let repo = repository();
        let (url, calls) = cloud(World {
            ontology_unserved: true,
            ..World::default()
        })
        .await;
        let a = args(url, repo.path());
        let (actions, served) =
            crate::deploy::ontologies_first(&a, &agent_using(&["listOrders"]), true)
                .await
                .unwrap();
        assert!(actions.is_empty(), "{actions:?}");
        assert!(served.contains("listOrders"));
        assert!(
            changes(&calls).iter().any(|c| c.starts_with("Deploy")),
            "{:?}",
            changes(&calls)
        );

        // `--ontology` naming every ontology of the project is all of them: the same deploy.
        let repo = repository();
        let (url, _) = cloud(World {
            ontology_unserved: true,
            ..World::default()
        })
        .await;
        let mut every = args(url, repo.path());
        every.ontology = vec!["shop".into(), "warehouse".into()];
        let (actions, served) =
            crate::deploy::ontologies_first(&every, &agent_using(&["listOrders"]), true)
                .await
                .unwrap();
        assert!(
            actions.is_empty() && served.contains("listOrders"),
            "{actions:?}"
        );

        let repo = repository();
        let (url, calls) = cloud(World {
            ontology_unserved: true,
            ..World::default()
        })
        .await;
        let (actions, _) = crate::deploy::ontologies_first(
            &args(url, repo.path()),
            &agent_using(&["listOrders", "listOrderz"]),
            true,
        )
        .await
        .unwrap();
        assert_eq!(actions.len(), 1, "{actions:?}");
        assert!(
            actions[0].message.contains("serve no tool listOrderz"),
            "{actions:?}"
        );
        assert!(changes(&calls).is_empty(), "{:?}", changes(&calls));
    }

    /// The server's findings on ontology connections give way only where there are compiled
    /// handlers to check against, and only for the connections it could not list: `--only agents
    /// --dry-run` (no handlers) keeps them, a misspelling included; a connection whose tools the
    /// server listed (another project's running ontology) is not checked here; a realtime view is
    /// not a tool.
    #[test]
    fn ontology_tools_are_checked_only_against_what_was_compiled() {
        let finding = |slug: &str| wire::RequiredAction {
            code: wire::RequiredActionCode::ToolNotOffered.into(),
            subject: format!("triage/tools/{slug}"),
            message: format!("Connection {slug} offers no tool listOrderz"),
            ..Default::default()
        };
        let mut declarations = agent_using(&["listOrderz"]);
        let kept = crate::apply::with_ontology_tools(
            vec![finding("tilde_ontology/shop")],
            &declarations,
            &BTreeSet::new(),
        );
        assert_eq!(kept.len(), 1);
        assert!(
            kept[0].message.contains("offers no tool listOrderz"),
            "{kept:?}"
        );

        let served: BTreeSet<String> = ["listOrders".to_string()].into();
        declarations.agents[0]["tools"] = json!([
            {"connection": {"slug": "tilde_ontology/shop"}, "tools": ["listOrders", "watchOrders"]},
            {"connection": {"slug": "tilde_ontology/other"}, "tools": ["theirTool"]},
        ]);
        let found = crate::apply::with_ontology_tools(
            vec![finding("tilde_ontology/shop")],
            &declarations,
            &served,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].subject, "triage/tools/tilde_ontology/shop");
        assert!(
            found[0].message.contains("serve no tool watchOrders"),
            "{found:?}"
        );
    }

    /// `--check` runs the agents' dry-run Apply too (with the compiled handlers), and changes
    /// nothing.
    #[tokio::test]
    async fn a_check_runs_the_agents_dry_run_apply() {
        let repo = repository();
        let (url, calls) = cloud(World {
            ontology_unserved: true,
            ..World::default()
        })
        .await;
        let mut check = args(url, repo.path());
        check.check = true;
        let (actions, served) =
            crate::deploy::ontologies_first(&check, &agent_using(&["listOrders"]), true)
                .await
                .unwrap();
        assert!(actions.is_empty(), "{actions:?}");
        assert!(served.contains("listOrders") && !served.contains("watchOrders"));
        let calls = calls.lock().unwrap().clone();
        assert!(
            calls.iter().any(|(m, _)| m == "Apply dryRun=true"),
            "{calls:?}"
        );
        assert!(
            !calls
                .iter()
                .any(|(m, _)| m == "Apply dryRun=false" || m == "Deploy" || m == "CreateVersion")
        );
    }

    /// A check builds each ontology as a deploy would, compiling it on the server, from a dirty
    /// tree, and changes nothing.
    #[tokio::test]
    async fn a_check_compiles_and_changes_nothing() {
        let repo = repository();
        std::fs::write(
            repo.path().join("ontologies/shop/notes.ts"),
            "// not committed\n",
        )
        .unwrap();
        let (url, calls) = cloud(World::default()).await;
        let mut check = args(url, repo.path());
        check.check = true;
        let (actions, _) = crate::deploy::ontologies_first(&check, &declarations(false), false)
            .await
            .unwrap();
        assert!(actions.is_empty());
        assert!(changes(&calls).is_empty(), "{:?}", changes(&calls));
        let compiled = calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, _)| m == "CompileOntology")
            .count();
        assert_eq!(compiled, 2, "both ontologies compile");
        assert!(
            repo.path()
                .join("ontologies/shop/.tilde/build/worker.js")
                .is_file()
        );
    }

    /// The plan shows each connection binding that changes, from what the environment has bound
    /// now, and counts the ones that stay.
    #[tokio::test]
    async fn the_plan_shows_connection_changes() {
        let (url, _) = cloud(World {
            bound: BTreeMap::from([(
                "e1",
                json!({"connections": [
                    {"name": "github", "connectionId": "c9", "connectionName": "github/old"},
                    {"name": "linear", "connectionId": "c2", "connectionName": "linear/acme"}]}),
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
        let projects = preflight(&client, &shop, None, &mut actions).await.unwrap();
        assert!(actions.is_empty());
        let plan = plan_text(&client, &projects, &shop).await.unwrap();
        assert!(
            plan.contains("production: connection github: github/old → github/acme\n"),
            "{plan}"
        );
        assert!(
            plan.contains("production: 1 connection unchanged\n"),
            "{plan}"
        );
        assert!(!plan.contains("production: connection linear"), "{plan}");
        assert!(
            plan.contains("staging: connection linear: unbound → linear/acme\n"),
            "{plan}"
        );
    }

    /// A first deploy into an organization with nothing yet: the project is created in the
    /// deploy's team, compiled and built once it exists, then each environment is created with the
    /// IR's storage and the spec's `live`, bound and rolled out. A team that does not exist blocks
    /// it before anything changes.
    #[tokio::test]
    async fn a_first_deploy_creates_the_project_and_its_environments() {
        let repo = repository();
        let (url, calls) = cloud(World {
            fresh: true,
            ..World::default()
        })
        .await;
        let mut a = args(url.clone(), repo.path());
        a.ontology = vec!["shop".into()];
        a.team = Some("factory".into());
        let mut d = declarations(false);
        d.ontologies
            .get_mut("shop")
            .unwrap()
            .environments
            .get_mut("production")
            .unwrap()
            .live = Some(true);

        let mut nowhere = args(url, repo.path());
        nowhere.ontology = vec!["shop".into()];
        nowhere.team = Some("nope".into());
        let (actions, _) = crate::deploy::ontologies_first(&nowhere, &d, false)
            .await
            .unwrap();
        assert_eq!(actions.len(), 1, "{actions:?}");
        assert!(actions[0].message.contains("team nope does not exist"));
        assert!(changes(&calls).is_empty(), "{:?}", changes(&calls));

        let (actions, _) = crate::deploy::ontologies_first(&a, &d, false)
            .await
            .unwrap();
        assert!(actions.is_empty(), "{actions:?}");
        assert_eq!(
            changes(&calls),
            [
                "CreateProject",
                "CreateVersion",
                "CreateEnvironment",
                "CreateEnvironment",
                "SetEnvironmentConnection e-production",
                "SetEnvironmentConnection e-production",
                "SetEnvironmentSecretBinding e-production",
                "Deploy e-production",
                "SetEnvironmentConnection e-staging",
                "SetEnvironmentConnection e-staging",
                "SetEnvironmentSecretBinding e-staging",
                "Deploy e-staging",
            ]
        );
        let calls = calls.lock().unwrap().clone();
        let body = |method: &str, nth: usize| {
            let mut found = calls.iter().filter(|(m, _)| m == method);
            found.nth(nth).unwrap().1.clone()
        };
        assert_eq!(
            body("CreateProject", 0),
            json!({"name": "shop", "teamId": "t1"})
        );
        // Tilde compiles for a project: only once it was created.
        let order: Vec<&str> = calls.iter().map(|(m, _)| m.as_str()).collect();
        let at = |method: &str| order.iter().position(|m| *m == method).unwrap();
        assert!(at("CreateProject") < at("CompileOntology"), "{order:?}");
        assert_eq!(body("CompileOntology", 0)["projectId"], "p1");
        assert_eq!(
            body("CreateEnvironment", 0),
            json!({"projectId": "p1", "name": "production", "target": "TARGET_CLOUDFLARE",
                   "storage": "STORAGE_POSTGRES", "live": true})
        );
        assert_eq!(body("CreateEnvironment", 1)["live"], false);
    }

    /// The plan names what a first deploy creates. An existing environment whose `live` differs
    /// from the spec keeps its own (no RPC changes it), and the plan says so.
    #[tokio::test]
    async fn the_plan_shows_what_is_created_and_a_live_that_stays() {
        let specs: BTreeMap<String, OntologySpec> =
            serde_json::from_value(json!({"shop": {"environments": {
                "production": {"live": true},
                "staging": {}}}}))
            .unwrap();
        let shop = BTreeMap::from([("shop".to_string(), &specs["shop"])]);
        let plan = |fresh: bool| {
            let shop = &shop;
            async move {
                let (url, calls) = cloud(World {
                    fresh,
                    ..World::default()
                })
                .await;
                let client = Client {
                    base: url,
                    http: reqwest::Client::new(),
                    key: None,
                };
                let mut actions = Vec::new();
                let projects = preflight(&client, shop, None, &mut actions).await.unwrap();
                assert!(actions.is_empty());
                let plan = plan_text(&client, &projects, shop).await.unwrap();
                assert!(changes(&calls).is_empty());
                plan
            }
        };
        let first = plan(true).await;
        assert!(
            first.contains("ontology shop: create the project\n"),
            "{first}"
        );
        assert!(
            first.contains("production: create the environment (live)\n")
                && first.contains("staging: create the environment\n"),
            "{first}"
        );
        let existing = plan(false).await;
        assert!(!existing.contains("create the"), "{existing}");
        assert!(
            existing.contains("production: live stays false: the spec says true"),
            "{existing}"
        );
        assert!(!existing.contains("staging: live"), "{existing}");
    }

    /// A connection the version declares that is neither bound nor in the spec stops the deploy
    /// before any binding, unless the version marks it optional: then it rolls out without it, and
    /// the plan lists it as skipped.
    #[tokio::test]
    async fn an_unbound_connection_is_refused_unless_optional() {
        let repo = repository();
        let (url, calls) = cloud(World {
            declared: json!(["github", "linear", "datadog"]),
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
            error.contains("production: connection datadog is neither bound nor in the spec"),
            "{error}"
        );
        assert_eq!(changes(&calls), ["CreateVersion"]);

        let repo = repository();
        let unbound = json!({"connections": [
            {"name": "github", "declared": true}, {"name": "linear", "declared": true},
            {"name": "datadog", "declared": true, "optional": true}]});
        let (url, calls) = cloud(World {
            declared: json!(["github", "linear", "datadog"]),
            optional: &["datadog"],
            bound: BTreeMap::from([("e1", unbound.clone()), ("e2", unbound)]),
            ..Default::default()
        })
        .await;
        let mut a = args(url.clone(), repo.path());
        a.ontology = vec!["shop".into()];
        crate::deploy::ontologies_first(&a, &declarations(false), false)
            .await
            .unwrap();
        let changed = changes(&calls);
        assert!(changed.contains(&"Deploy e2".to_string()), "{changed:?}");
        assert!(
            !calls
                .lock()
                .unwrap()
                .iter()
                .any(|(m, b)| { m == "SetEnvironmentConnection" && b["name"] == "datadog" })
        );

        let client = Client {
            base: url,
            http: reqwest::Client::new(),
            key: None,
        };
        let specs = spec();
        let shop = BTreeMap::from([("shop".to_string(), &specs["shop"])]);
        let projects = preflight(&client, &shop, None, &mut Vec::new())
            .await
            .unwrap();
        let plan = plan_text(&client, &projects, &shop).await.unwrap();
        assert!(
            plan.contains("production: connection datadog: skipped (optional and unbound"),
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
                .0
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
        // The artifact the client built: the runtime, the IR the project's package evaluated, the
        // handler module.
        use base64::Engine;
        let artifact = base64::engine::general_purpose::STANDARD
            .decode(created["artifact"].as_str().unwrap())
            .unwrap();
        let mut entries = BTreeMap::new();
        for entry in tar::Archive::new(artifact.as_slice()).entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().to_string_lossy().into_owned();
            let mut text = String::new();
            std::io::Read::read_to_string(&mut entry, &mut text).unwrap();
            entries.insert(path, text);
        }
        assert_eq!(entries["artifact.json"], r#"{"runtime":1}"#);
        assert!(
            entries["ontology.ir.json"].contains("ontologies/shop"),
            "{entries:?}"
        );
        assert_eq!(entries["worker.js"], "export default {};\n");
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
        // The build left the tree clean: the next deploy builds HEAD again.
        assert!(head(repo.path(), false).is_ok());
    }

    /// An IR Tilde does not compile fails the deploy with its errors, before anything changes.
    #[tokio::test]
    async fn an_ontology_that_does_not_compile_changes_nothing() {
        let repo = repository();
        let (url, calls) = cloud(World {
            compile_errors: Some(json!(["ontology.ts:3:1 (objects.Customer): no key"])),
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
            error.contains("ontology shop does not compile") && error.contains("no key"),
            "{error}"
        );
        assert!(changes(&calls).is_empty(), "{:?}", changes(&calls));
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
    async fn a_version_without_the_specs_connections_binds_nothing() {
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
            error.contains("production: the version declares no connection linear"),
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
        let (actions, plan, _) = prepare(&args(url, repo.path()), &spec(), None, false)
            .await
            .unwrap();
        assert!(plan.is_none());
        assert_eq!(actions.len(), 1);
        assert!(
            actions[0].message.contains("need Tilde Cloud"),
            "{}",
            actions[0].message
        );
    }
}
