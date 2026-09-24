//! Local SDK example bootstrap: registers one agent and one Gateway deployment per SDK
//! adapter example and starts each process, which then dials in with its deployment token.
//! This command is absent from release builds.
use crate::database::Pool;
use envconfig::Envconfig;
use secrecy::ExposeSecret;
use std::{collections::BTreeMap, sync::Arc};
use tilde::{
    agent::{Agents, CreateAgent},
    config::SecretEnv,
    encryption::Encryption,
    iam::capabilities::{Capabilities, Capability, Reach},
};
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
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::init_from_env()?;
    let selected: Vec<&str> = config
        .selected
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .collect();
    if let Some(unknown) = selected
        .iter()
        .find(|key| !EXAMPLES.iter().any(|example| example.key == **key))
    {
        return Err(format!("Unknown DEV_AGENTS entry: {unknown}").into());
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut children = tokio::task::JoinSet::new();
    for example in EXAMPLES
        .iter()
        .filter(|example| selected.is_empty() || selected.contains(&example.key))
    {
        let mut child = start(example, &root, &config, &pool, &encryption).await?;
        let name = example.name;
        children.spawn(async move { (name, child.wait().await) });
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
    children.shutdown().await;
    pool.close().await;
    result.map_err(Into::into)
}

/// Register one example and its own Gateway deployment, then spawn the process that dials in.
async fn start(
    example: &Example,
    root: &std::path::Path,
    config: &Config,
    pool: &Pool,
    encryption: &Arc<Encryption>,
) -> Result<tokio::process::Child, Box<dyn std::error::Error>> {
    let id = Uuid::parse_str(example.id)?;
    let agents = Agents::new(pool.clone(), encryption.clone());
    if tilde::agent::db::get_for_create_opt(&pool.get().await?, id)
        .await?
        .is_none()
    {
        agents
            .create(CreateAgent {
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
    let connections = tilde::connections::service::Connections::new(
        pool.clone(),
        encryption.clone(),
        config.gateway_url.clone(),
        config.gateway_url.clone(),
    )?;
    let deployments =
        tilde::deployment::Deployments::new(pool.clone(), encryption.clone(), agents, connections);
    // This local process owns a Gateway deployment, regardless of which other
    // deployment currently receives traffic for the agent.
    let (deployment, token, _) = deployments
        .register_deployment(
            id,
            tilde::deployment::RegisterDeployment {
                source: tilde::proto::tilde::types::v1::DeploymentSource::Manual,
                target: tilde::proto::tilde::types::v1::DeploymentTarget::Gateway,
                target_reference: None,
                external_id: Some(format!("local-example:{}", example.key)),
                label: Some("Local example".into()),
                repository: None,
                commit_sha: None,
                commit_message: None,
                branch: None,
                commit_author: None,
            },
        )
        .await?;
    // The token is returned once, on first registration; restarts rotate it.
    let deployment_token = match token {
        Some(token) => token,
        None => {
            deployments
                .issue_token(id, Uuid::parse_str(&deployment.id)?)
                .await?
        }
    };
    let mut command = match example.launch {
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
        .env("TILDE_DEPLOYMENT_TOKEN", deployment_token.expose_secret())
        .env("OPENAI_API_KEY", config.openai_api_key.0.expose_secret())
        .env("OPENAI_MODEL", &config.model)
        .kill_on_drop(true);
    let child = command.spawn()?;
    drop(command);
    drop(deployment_token);
    println!(
        "Registered {} ({id}); it dials in to {}",
        example.name, config.gateway_url
    );
    Ok(child)
}
