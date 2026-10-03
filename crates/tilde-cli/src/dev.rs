//! `tilde dev`: everything between a running quickstart gateway and talking to your agent.
//!
//! It finds or creates the agent, registers a deployment carrying the declarations the code
//! holds right now, starts the agent with the credentials it needs, and serves a local chat
//! page. Editing a prompt or skill and saving registers the next deployment and restarts the
//! agent, because a deployment's contents are immutable once registered.
use crate::{
    api::Client,
    chat,
    deploy::target_of,
    discover::{self, Declarations},
    project::Project,
    registry,
};
use anyhow::{Context, Result, bail};
use clap::Args;
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use tilde_contracts::proto::tilde::management::v1 as wire;
use tilde_contracts::proto::tilde::types::v1 as types;
use tokio::process::{Child, Command};

#[derive(Args)]
pub struct Dev {
    /// Management API of the gateway to develop against.
    #[arg(long, env = "TILDE_URL", default_value = "http://127.0.0.1:8080")]
    pub url: String,
    /// Develop against an existing agent instead of this project's own.
    #[arg(long, env = "TILDE_AGENT_ID")]
    pub agent_id: Option<String>,
    #[arg(long, env = "TILDE_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,
    /// Name of the registered agent to develop against. Defaults to this project's own name.
    #[arg(long)]
    pub name: Option<String>,
    /// Module declaring the agent's prompts, skills and bundled tools.
    #[arg(long)]
    pub entry: Option<String>,
    /// Address for the local chat page. Port 0 picks a free port.
    #[arg(long, default_value = "127.0.0.1:4242")]
    pub chat: SocketAddr,
    /// Start the agent and watch for changes, without serving the chat page.
    #[arg(long)]
    pub no_chat: bool,
    #[arg(long, default_value = ".", hide = true)]
    pub dir: PathBuf,
}

/// Who the local chat page talks to the agent as.
const IDENTITY: &str = "dev";

pub async fn run(args: Dev) -> Result<()> {
    let project = Project::find(&args.dir)?;
    let entry = match &args.entry {
        Some(entry) => entry.clone(),
        None => project.entry()?,
    };
    let run_command = project.run_command()?;
    let client = Client::new(&args.url, args.api_key.clone())?;
    let name = args.name.clone().unwrap_or_else(|| project.name.clone());

    // Resolve the agent: an explicit id, or the registered agent with this project's name.
    let agent_id = match &args.agent_id {
        Some(id) => {
            registry::get_agent(&client, id)
                .await
                .with_context(|| format!("No agent {id} on {}", client.gateway()))?;
            id.clone()
        }
        None => match registry::find_agent(&client, &name).await? {
            Some(agent) => {
                eprintln!("Using agent {} ({})", agent.name, agent.id);
                agent.id
            }
            None => bail!(
                "No agent named {name} on {gateway}.\n\
                 Register it at {gateway}/agent/new, then run this again: an agent needs its \
                 capabilities, inference and access set up before it can serve an invocation, so \
                 `tilde dev` will not create one for you.\n\
                 Pass --name to match a differently named agent, or --agent-id to pick one outright.",
                gateway = client.gateway()
            ),
        },
    };

    eprintln!("Reading declarations from {entry}");
    let mut declarations = discover::declarations(&project, &entry, true).await?;
    let mut token = dev_deployment(&client, &agent_id, &declarations).await?;

    let mut chat_server = None;
    if !args.no_chat {
        match registry::chat_key(&client, &agent_id).await {
            Ok(key) => {
                // The built-in channel is private, so the local identity has to be let in once.
                match registry::admit_identity(&client, &agent_id, &key, IDENTITY).await {
                    Ok(true) => eprintln!(
                        "Allowed the {IDENTITY} identity on this agent's Tilde chat channel"
                    ),
                    Ok(false) => {}
                    Err(error) => eprintln!("Could not allow the {IDENTITY} identity ({error})"),
                }
                let (bound, handle) =
                    chat::serve(client.gateway(), &agent_id, key, IDENTITY, args.chat).await?;
                eprintln!("Chat with your agent at http://{bound}");
                chat_server = Some(handle);
            }
            // Worth continuing without: the agent still runs and the UI still shows it.
            Err(error) => eprintln!("Chat page unavailable ({error})"),
        }
    }
    eprintln!("Agent page {}/agent/{agent_id}", client.gateway());

    let mut watched = fingerprint(&project.dir);
    let mut child = Some(spawn(&run_command, client.gateway(), &token, &project.dir)?);
    eprintln!("Running {} — press Ctrl-C to stop", run_command.join(" "));

    loop {
        // A dead agent leaves `child` empty; the loop keeps watching so the next save revives it.
        let exited = async {
            match child.as_mut() {
                Some(running) => running.wait().await.map(Some),
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            _ = shutdown() => break,
            status = exited => {
                child = None;
                match status.context("The agent process could not be waited on")? {
                    Some(status) if !status.success() => eprintln!(
                        "The agent exited ({status}); waiting for a change to restart it"
                    ),
                    _ => eprintln!("The agent exited; waiting for a change to restart it"),
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(600)) => {
                let current = fingerprint(&project.dir);
                if current == watched {
                    continue;
                }
                watched = current;
                let next = match discover::declarations(&project, &entry, true).await {
                    Ok(next) => next,
                    Err(error) => {
                        eprintln!("Change not applied: {error}");
                        continue;
                    }
                };
                // Restart on any edit, but only re-register when the declarations really moved.
                if next.digest != declarations.digest {
                    eprintln!("Declarations changed; registering the next deployment");
                    declarations = next;
                    token = dev_deployment(&client, &agent_id, &declarations).await?;
                }
                if let Some(mut running) = child.take() {
                    let _ = running.kill().await;
                }
                child = Some(spawn(&run_command, client.gateway(), &token, &project.dir)?);
                eprintln!("Restarted the agent");
            }
        }
    }

    eprintln!("Stopping the agent");
    if let Some(mut running) = child.take() {
        let _ = running.kill().await;
    }
    if let Some(handle) = chat_server {
        handle.abort();
    }
    Ok(())
}

/// Register (or reuse) the deployment for this exact set of declarations and return a token for
/// it. The external id carries the declaration digest, so an unchanged restart reuses one
/// deployment instead of filling the agent's history with near-identical rows.
async fn dev_deployment(
    client: &Client,
    agent_id: &str,
    declarations: &Declarations,
) -> Result<String> {
    let request = wire::RegisterDeploymentRequest {
        source: types::DeploymentSource::Manual.into(),
        target: target_of("gateway").into(),
        external_id: Some(format!("tilde-dev:{}", declarations.digest)),
        label: Some("tilde dev".into()),
        ..Default::default()
    };
    let deployment = registry::register(client, agent_id, declarations, request).await?;
    if !deployment.token.is_empty() {
        return Ok(deployment.token);
    }
    // Registration returns a token only on creation; mint one for the deployment we just reused.
    registry::issue_token(client, agent_id, &deployment.id).await
}

fn spawn(command: &[String], gateway: &str, token: &str, dir: &Path) -> Result<Child> {
    let (program, rest) = command
        .split_first()
        .context("the run command is not empty")?;
    Command::new(program)
        .args(rest)
        .current_dir(dir)
        .env("TILDE_GATEWAY_URL", gateway)
        .env("TILDE_DEPLOYMENT_TOKEN", token)
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("Could not start the agent with `{}`", command.join(" ")))
}

/// Ctrl-C, or the SIGTERM a supervisor sends. Both have to stop the agent too, or it outlives
/// the `tilde dev` that started it and keeps holding its deployment token.
async fn shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = match signal(SignalKind::terminate()) {
            Ok(terminate) => terminate,
            Err(_) => return tokio::signal::ctrl_c().await.unwrap_or(()),
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

/// Modification times of the project's own source files.
///
/// Polling beats watching here: it needs no platform file-watching and no inotify budget, and
/// the cost is one bounded directory walk per tick. It has to be a walk rather than the files
/// the declarations name, because a skill's origin points at the module that declared it, not
/// at the `SKILL.md` on disk that an edit actually touches.
fn fingerprint(dir: &Path) -> BTreeMap<PathBuf, SystemTime> {
    /// Dependency and build directories dwarf a project's own sources and never hold its
    /// declarations. `dist` and `build` are not skipped: a built entry is what gets read.
    /// Directories whose name starts with a dot are skipped too, which covers `.git`, `.venv`
    /// and the `.tilde` the CLI writes itself.
    const SKIP: [&str; 3] = ["node_modules", "target", "__pycache__"];
    /// What a prompt, skill or tool can be declared in or loaded from.
    const WATCHED: [&str; 11] = [
        "js", "mjs", "cjs", "ts", "mts", "cts", "py", "md", "json", "yaml", "yml",
    ];
    // A guard against walking something enormous by accident; a project of this size would make
    // the poll the slowest thing in the loop.
    const LIMIT: usize = 20_000;

    let mut seen = BTreeMap::new();
    let mut queue = vec![dir.to_path_buf()];
    while let Some(current) = queue.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            if seen.len() >= LIMIT {
                return seen;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !name.starts_with('.') && !SKIP.contains(&name.as_ref()) {
                    queue.push(entry.path());
                }
                continue;
            }
            let watched = entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| WATCHED.contains(&extension));
            if watched && let Ok(modified) = entry.metadata().and_then(|data| data.modified()) {
                seen.insert(entry.path(), modified);
            }
        }
    }
    seen
}

/// Shared by `dev` and `doctor`: a gateway that is not answering is the usual first problem.
pub async fn check_gateway(client: &Client) -> Result<()> {
    let probe = client
        .agents
        .list_agents_with_options(
            wire::ListAgentsRequest {
                page_size: 1,
                ..Default::default()
            },
            crate::api::options(),
        )
        .await;
    match probe {
        Ok(_) => Ok(()),
        Err(error) => bail!(
            "{}\nStart the quickstart gateway with `docker compose up -d --wait` in tilde/quickstart.",
            crate::api::failed("ListAgents", error)
        ),
    }
}
