//! Local SDK example bootstrap: registers one agent and one Gateway deployment per SDK
//! adapter example, and one remote tool server per SDK, and starts each process, which then
//! dials in with its deployment or tool host token. Each deployment carries the prompts, skills
//! and bundled tools `tilde deploy --dry-run` discovers in the example, so a changed prompt
//! registers a new deployment that Latest routing prefers. The OpenAI key becomes one inference
//! connection assigned to every example under the alias `default`; examples reach it through
//! `ctx.inference("default")` and never see the key. Once the tool servers are up, every example
//! agent is given their tools. This command is absent from release builds.
use crate::database::Pool;
use envconfig::Envconfig;
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use tilde::proto::tilde::management::v1::DeploymentDeclarations;
use tilde::{
    agent::{Agents, CreateAgent, avatar::ObjectStore},
    config::SecretEnv,
    connections::{
        model::{Assignment, Capability as ConnectionCapability, Values},
        service::Connections,
    },
    encryption::Encryption,
    iam::capabilities::{Capabilities, Capability, Reach},
};

/// The examples' shared OpenAI connection, `openai/dev-examples`, kept across restarts.
const INFERENCE_CONNECTION: &str = "7d3c9f1e-2a4b-4c6d-8e0f-1a2b3c4d5e6f";
const INFERENCE_ALIAS: &str = "default";
use uuid::Uuid;

/// How an example process is started; every example reads the same environment contract.
enum Launch {
    /// `node <repo>/<script>` for the TypeScript examples built by `pnpm --dir sdk/ts build`.
    Node(&'static str),
    /// `sdk/py/<venv>/bin/python <script>` from the uv workspace that `scripts/run-dev-agent.py`
    /// syncs. The interpreter runs directly (not through `uv run`) so killing the child stops
    /// the agent instead of orphaning it. CrewAI has its own environment because its OpenAI
    /// client pin conflicts with the OpenAI Agents SDK.
    Python(&'static str, &'static str),
}

/// Stable IDs keep registrations, channel assignments and history across restarts.
struct Example {
    key: &'static str,
    id: &'static str,
    name: &'static str,
    launch: Launch,
}

const EXAMPLES: &[Example] = &[
    Example {
        key: "vercel-ai",
        id: "9a97ef65-4f0c-4ce8-9d95-7b8aa39bca01",
        name: "Example Agent 1",
        launch: Launch::Node("sdk/ts/examples/vercel-ai-example-agent/dist/index.js"),
    },
    Example {
        key: "langchain",
        id: "5f0c1f0e-7a57-4d0b-9f6e-3d1c8b1f2a02",
        name: "Example Agent LangChain",
        launch: Launch::Node("sdk/ts/examples/langchain-example-agent/dist/index.js"),
    },
    Example {
        key: "openai-agents",
        id: "0b6f3c52-3f1d-4b9a-8c47-5e2d9a7c4b03",
        name: "Example Agent OpenAI Agents",
        launch: Launch::Node("sdk/ts/examples/openai-agents-example-agent/dist/index.js"),
    },
    Example {
        key: "mastra",
        id: "c1d2a9e4-6b38-4f75-a1c0-8f4e6d2b9c04",
        name: "Example Agent Mastra",
        launch: Launch::Node("sdk/ts/examples/mastra-example-agent/dist/index.js"),
    },
    Example {
        key: "py-langchain",
        id: "7e4a2b91-0c5d-4e86-b3f7-1a9d5c6e8f05",
        name: "Example Agent LangChain (Python)",
        launch: Launch::Python(".venv", "examples/example-agent-langchain/main.py"),
    },
    Example {
        key: "py-pydantic-ai",
        id: "2d8c7f63-9e14-4a0b-8d52-6b3f1e7a9c06",
        name: "Example Agent Pydantic AI (Python)",
        launch: Launch::Python(".venv", "examples/example-agent-pydantic-ai/main.py"),
    },
    Example {
        key: "py-openai-agents",
        id: "a9b1e5d7-4c26-4f3a-9e80-7d5c2b4f6a07",
        name: "Example Agent OpenAI Agents (Python)",
        launch: Launch::Python(".venv", "examples/example-agent-openai-agents/main.py"),
    },
    Example {
        key: "py-agno",
        id: "e3f6c8a2-1b47-4d95-a6c3-9f0e2d8b5a08",
        name: "Example Agent Agno (Python)",
        launch: Launch::Python(".venv", "examples/example-agent-agno/main.py"),
    },
    Example {
        key: "py-crewai",
        id: "4a1d7c90-8e53-4b2f-9c16-d0e7f3a5b609",
        name: "Example Agent CrewAI (Python)",
        launch: Launch::Python(".venv-crewai", "examples/example-agent-crewai/main.py"),
    },
];

/// The example CRM workspace every example agent uses; stable so restarts reuse it.
const DEMO_CRM: &str = "3f5e8a2c-6d14-4b7e-9a01-c2d4e6f8a010";
/// This example finds its remote tools through `tools.search`; the others list them.
const DYNAMIC_EXAMPLE: &str = "vercel-ai";

/// Example remote tool servers, one per SDK. Registered by name, so a restart rotates the token
/// of the same server instead of adding another.
struct ToolServer {
    key: &'static str,
    name: &'static str,
    launch: Launch,
}

const TOOL_SERVERS: &[ToolServer] = &[
    ToolServer {
        key: "tool-server",
        name: "Example CRM (TypeScript)",
        launch: Launch::Node("sdk/ts/examples/example-tool-server/dist/index.js"),
    },
    ToolServer {
        key: "py-tool-server",
        name: "Example tools (Python)",
        launch: Launch::Python(".venv", "examples/example-tool-server/main.py"),
    },
];

#[derive(Envconfig)]
struct Config {
    #[envconfig(from = "OPENAI_API_KEY")]
    openai_api_key: SecretEnv,
    #[envconfig(from = "OPENAI_MODEL", default = "gpt-4o-mini")]
    model: String,
    #[envconfig(from = "ENGINE_RUNTIME_PUBLIC_URL", default = "http://127.0.0.1:8080")]
    gateway_url: String,
    /// Comma-separated example keys; empty starts every example.
    #[envconfig(from = "DEV_AGENTS", default = "")]
    selected: String,
}

pub async fn run(
    pool: Pool,
    encryption: Arc<Encryption>,
    skills_store: Option<ObjectStore>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::init_from_env()?;
    let selected: Vec<&str> = config
        .selected
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .collect();
    if let Some(unknown) = selected.iter().find(|key| {
        !EXAMPLES.iter().any(|example| example.key == **key)
            && !TOOL_SERVERS.iter().any(|server| server.key == **key)
    }) {
        return Err(format!("Unknown DEV_AGENTS entry: {unknown}").into());
    }
    // The examples live in open-source Tilde's sdk/; Tilde Cloud points this at a checkout of it.
    let root = std::env::var_os("TILDE_SDK_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let connections = Connections::new(
        pool.clone(),
        encryption.clone(),
        config.gateway_url.clone(),
        config.gateway_url.clone(),
    )?;
    // The registrar can run before any API process has seeded the built-in providers.
    connections.seed().await?;
    let inference = inference_connection(&connections, &config).await?;
    let mut children = tokio::task::JoinSet::new();
    for example in EXAMPLES
        .iter()
        .filter(|example| selected.is_empty() || selected.contains(&example.key))
    {
        let mut child = start(
            example,
            &root,
            &config,
            &pool,
            &encryption,
            &connections,
            inference,
            &skills_store,
        )
        .await?;
        let name = example.name;
        children.spawn(async move { (name, child.wait().await) });
    }
    // Seeding syncs from GitHub; it runs beside the examples so a shutdown never waits on it.
    let seeding = tokio::spawn(super::dev_skills::seed(pool.clone(), skills_store.clone()));
    for server in TOOL_SERVERS
        .iter()
        .filter(|server| selected.is_empty() || selected.contains(&server.key))
    {
        let mut child = start_tool_server(server, &root, &config, &pool, &encryption).await?;
        let name = server.name;
        children.spawn(async move { (name, child.wait().await) });
    }
    let examples: Vec<&Example> = EXAMPLES
        .iter()
        .filter(|example| selected.is_empty() || selected.contains(&example.key))
        .collect();
    if selected.is_empty() {
        let connections = connections.clone();
        tokio::spawn(async move {
            if let Err(error) = give_example_tools(connections, &examples).await {
                eprintln!("Example agents were not given the example tool servers' tools: {error}");
            }
        });
    }
    drop(config.openai_api_key);
    // Children are killed on drop, so one failed example or a shutdown stops them all.
    let result = tokio::select! {
        exited = children.join_next() => match exited {
            Some(Ok((name, Ok(status)))) if status.success() => Err(format!("{name} exited")),
            Some(Ok((name, _))) => Err(format!("{name} exited unsuccessfully")),
            _ => Err("No example agent was started".to_owned()),
        },
        _ = super::shutdown() => Ok(()),
    };
    seeding.abort();
    children.shutdown().await;
    pool.close().await;
    result.map_err(Into::into)
}

/// The OpenAI key as a ready inference connection, created through the broker once.
async fn inference_connection(
    connections: &Connections,
    config: &Config,
) -> Result<Uuid, Box<dyn std::error::Error>> {
    let id = Uuid::parse_str(INFERENCE_CONNECTION)?;
    if let Ok(existing) = connections.get(id).await
        && existing.status == "ready"
    {
        return Ok(id);
    }
    let started = connections
        .start(id, "OpenAI", "openai", "api", &[])
        .await?;
    let url = url::Url::parse(&started.brokering_url)?;
    let setup = Uuid::parse_str(
        url.path_segments()
            .and_then(|mut s| s.next_back())
            .ok_or("Invalid brokering URL")?,
    )?;
    let token = url
        .query_pairs()
        .find(|(key, _)| key == "connection_setup_token")
        .ok_or("Brokering URL has no setup token")?
        .1
        .into_owned();
    let view = connections.view(setup, &token).await?;
    let view = connections
        .set_connection_name(setup, &token, view.action_id, "dev-examples")
        .await?;
    let values: Values = [(
        "api_key".to_string(),
        secrecy::SecretString::from(config.openai_api_key.0.expose_secret().to_owned()),
    )]
    .into_iter()
    .collect();
    connections
        .advance(setup, &token, view.action_id, values)
        .await?;
    Ok(id)
}

/// What the SDK's `tilde deploy --dry-run` discovers in the example, with the digest of its
/// output. None, after a warning, when the CLI is missing or fails: the example still starts.
async fn declarations(
    example: &Example,
    root: &std::path::Path,
    config: &Config,
) -> Option<(DeploymentDeclarations, String)> {
    let mut command = match example.launch {
        Launch::Node(script) => {
            let mut command = tokio::process::Command::new("node");
            command
                .arg(root.join("sdk/ts/packages/sdk/dist/cli.js"))
                .args(["deploy", &root.join(script).to_string_lossy(), "--dry-run"])
                .current_dir(root);
            command
        }
        Launch::Python(venv, script) => {
            let workspace = root.join("sdk/py");
            let script = std::path::Path::new(script);
            let mut command = tokio::process::Command::new(workspace.join(venv).join("bin/python"));
            command
                .args(["-m", "tilde", "deploy"])
                .arg(script.file_name().unwrap_or_default())
                .arg("--dry-run")
                .current_dir(workspace.join(script.parent().unwrap_or(std::path::Path::new(""))));
            command
        }
    };
    // Discovery imports the example's module; it gets the example's environment, no secrets.
    command
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("TILDE_INFERENCE", INFERENCE_ALIAS)
        .env("OPENAI_MODEL", &config.model)
        .kill_on_drop(true);
    let failed = |reason: String| {
        eprintln!(
            "warning: tilde deploy --dry-run failed for {}; registering it without declarations: {reason}",
            example.name
        );
        None
    };
    let output = match command.output().await {
        Ok(output) => output,
        Err(error) => return failed(error.to_string()),
    };
    if !output.status.success() {
        return failed(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    match serde_json::from_slice::<DeploymentDeclarations>(&output.stdout) {
        Ok(declared) => Some((
            declared,
            hex::encode(Sha256::digest(&output.stdout))[..12].to_owned(),
        )),
        Err(error) => failed(format!("unreadable output: {error}")),
    }
}

/// Register one example and its own Gateway deployment, then spawn the process that dials in.
#[allow(clippy::too_many_arguments)]
async fn start(
    example: &Example,
    root: &std::path::Path,
    config: &Config,
    pool: &Pool,
    encryption: &Arc<Encryption>,
    connections: &Connections,
    inference: Uuid,
    skills_store: &Option<ObjectStore>,
) -> Result<tokio::process::Child, Box<dyn std::error::Error>> {
    let id = Uuid::parse_str(example.id)?;
    let agents = Agents::new(pool.clone(), encryption.clone());
    if tilde::agent::db::get_for_create_opt(&pool.get().await?, id)
        .await?
        .is_none()
    {
        agents
            .create(CreateAgent {
                description: String::new(),
                concurrency_policy: Default::default(),
                id,
                name: example.name.into(),
                capabilities: Capabilities(BTreeMap::from([
                    (Capability::ThreadRead, Reach::Yes),
                    (Capability::RunUpdate, Reach::Yes),
                    (Capability::ToolsInvoke, Reach::All),
                ])),
            })
            .await?;
    }
    connections
        .assign(
            inference,
            &Assignment {
                capability: ConnectionCapability::Inference,
                agent_id: id,
                alias: Some(INFERENCE_ALIAS.into()),
            },
        )
        .await?;
    let deployments = tilde::deployment::Deployments::new(
        pool.clone(),
        encryption.clone(),
        agents,
        connections.clone(),
    )
    .with_skills(tilde::skills::Skills::new(pool.clone()).with_store(skills_store.clone()));
    let registration = |external_id: String, declarations: DeploymentDeclarations| {
        tilde::deployment::RegisterDeployment {
            source: tilde::proto::tilde::types::v1::DeploymentSource::Manual,
            target: tilde::proto::tilde::types::v1::DeploymentTarget::Gateway,
            target_reference: None,
            external_id: Some(external_id),
            label: Some("Local example".into()),
            repository: None,
            commit_sha: None,
            commit_message: None,
            branch: None,
            commit_author: None,
            declarations,
        }
    };
    // This local process owns a Gateway deployment, regardless of which other deployment
    // currently receives traffic for the agent. Its external id changes with the declared
    // contents, so an edited prompt or skill is a new deployment.
    let plain = format!("local-example:{}", example.key);
    let registered = match declarations(example, root, config).await {
        Some((declared, digest)) => {
            match deployments
                .register_deployment(id, registration(format!("{plain}:{digest}"), declared))
                .await
            {
                Ok(registered) => Some(registered),
                Err(error) => {
                    eprintln!(
                        "warning: {} declarations were refused; registering it without them: {error}",
                        example.name
                    );
                    None
                }
            }
        }
        None => None,
    };
    let (deployment, token, _) = match registered {
        Some(registered) => registered,
        None => {
            deployments
                .register_deployment(id, registration(plain, Default::default()))
                .await?
        }
    };
    // The token is returned once, on first registration; restarts rotate it.
    let deployment_token = match token {
        Some(token) => token,
        None => {
            deployments
                .issue_token(id, Uuid::parse_str(&deployment.id)?)
                .await?
        }
    };
    let mut command = command(&example.launch, root, config);
    command
        .env("TILDE_DEPLOYMENT_TOKEN", deployment_token.expose_secret())
        .env("TILDE_INFERENCE", INFERENCE_ALIAS)
        .env("OPENAI_MODEL", &config.model);
    let child = command.spawn()?;
    drop(command);
    drop(deployment_token);
    println!(
        "Registered {} ({id}); it dials in to {}",
        example.name, config.gateway_url
    );
    Ok(child)
}

/// The example process with a clean environment: only PATH and the gateway URL.
fn command(launch: &Launch, root: &std::path::Path, config: &Config) -> tokio::process::Command {
    let mut command = match *launch {
        Launch::Node(script) => {
            let mut command = tokio::process::Command::new("node");
            command.arg(root.join(script));
            command
        }
        Launch::Python(venv, script) => {
            let workspace = root.join("sdk/py");
            let mut command = tokio::process::Command::new(workspace.join(venv).join("bin/python"));
            command.arg(script).current_dir(workspace);
            command
        }
    };
    command
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("TILDE_GATEWAY_URL", &config.gateway_url)
        .kill_on_drop(true);
    command
}

/// Register the tool server once (by name) or rotate its token, then spawn it to dial in.
async fn start_tool_server(
    server: &ToolServer,
    root: &std::path::Path,
    config: &Config,
    pool: &Pool,
    encryption: &Arc<Encryption>,
) -> Result<tokio::process::Child, Box<dyn std::error::Error>> {
    let hosts = tilde::tools::hosts::ToolHosts::new(tilde::connections::service::Connections::new(
        pool.clone(),
        encryption.clone(),
        config.gateway_url.clone(),
        config.gateway_url.clone(),
    )?);
    let token = match hosts
        .list(None, None)
        .await?
        .into_iter()
        .find(|h| h.name == server.name)
    {
        Some(host) => hosts.rotate_token(host.id).await?,
        None => hosts
            .register(server.name, None)
            .await?
            .1
            .ok_or("Connected tool hosts receive a token")?,
    };
    let mut command = command(&server.launch, root, config);
    command.env("TILDE_TOOL_HOST_TOKEN", token.expose_secret());
    let child = command.spawn()?;
    drop(command);
    drop(token);
    println!(
        "Registered tool server {}; it dials in to {}",
        server.name, config.gateway_url
    );
    Ok(child)
}

/// Once both example tool servers have published their tools, give each example agent the
/// Python server's tools and a demo workspace of the TypeScript CRM, set up the way a person
/// would through its setup page. Sources an agent already has are left as they are, so changes
/// made in the web app survive restarts.
async fn give_example_tools(
    connections: tilde::connections::service::Connections,
    examples: &[&Example],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tools = tilde::tools::Tools::new(connections.clone());
    let hosts = tilde::tools::hosts::ToolHosts::new(connections.clone());
    let mut published = None;
    for _ in 0..90 {
        let (mut crm, mut py) = (None, None);
        for host in hosts.list(None, None).await? {
            if !host.available || host.tools.is_empty() {
                continue;
            }
            if host.name == TOOL_SERVERS[0].name {
                crm = Some(host);
            } else if host.name == TOOL_SERVERS[1].name {
                py = Some(host);
            }
        }
        if let (Some(crm), Some(py)) = (crm, py) {
            published = Some((crm, py));
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    let (crm_host, py_host) = published.ok_or("the example tool servers did not dial in")?;
    let crm = Uuid::parse_str(DEMO_CRM)?;
    match connections.get(crm).await {
        Ok(existing) if existing.status == "ready" => {}
        Ok(existing) => {
            return Err(format!("the demo CRM workspace is {}", existing.status).into());
        }
        Err(_) => {
            let provider = crm_host
                .provider_id
                .as_deref()
                .ok_or("the CRM publishes no provider")?;
            let started = connections
                .start(crm, "Acme", provider, "api_key", &[])
                .await?;
            let url = url::Url::parse(&started.brokering_url)?;
            let setup = Uuid::parse_str(
                url.path_segments()
                    .and_then(|mut s| s.next_back())
                    .ok_or("setup URL")?,
            )?;
            let token = url
                .query_pairs()
                .find(|(key, _)| key == "connection_setup_token")
                .ok_or("setup token")?
                .1
                .into_owned();
            let view = connections.view(setup, &token).await?;
            let key: tilde::connections::model::Values = [(
                "api_key".to_owned(),
                secrecy::SecretString::from("demo-acme"),
            )]
            .into_iter()
            .collect();
            connections
                .advance(setup, &token, view.action_id, key)
                .await?;
        }
    }
    let crm_tools: Vec<String> = tools
        .provider_tools(crm)
        .await?
        .into_iter()
        .map(|t| t.name)
        .collect();
    let py_tools: Vec<String> = py_host.tools.iter().map(|t| t.name.clone()).collect();
    for example in examples {
        let agent = Uuid::parse_str(example.id)?;
        let existing = tools
            .sources(tilde::tools::Filter {
                agent: Some(agent),
                ..Default::default()
            })
            .await?;
        for (target, names) in [
            (tilde::tools::Target::Connection(crm), &crm_tools),
            (tilde::tools::Target::ToolHost(py_host.id), &py_tools),
        ] {
            if !existing.iter().any(|source| source.target == target) {
                tools.add_source(agent, target, names).await?;
            }
        }
        if existing.is_empty() && example.key == DYNAMIC_EXAMPLE {
            tools.set_mode(agent, true).await?;
        }
    }
    println!("Gave the example agents the example tool servers' tools");
    Ok(())
}
