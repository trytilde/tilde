//! `tilde deploy`: register a release carrying what the agent's code declares. This is the
//! command CI runs; it prints the deployment token on stdout and its inventory on stderr, so
//! `TOKEN=$(tilde deploy)` works.
use crate::{api::Client, discover, project::Project, registry};
use anyhow::{Result, bail};
use clap::Args;
use serde_json::json;
use std::{env, path::PathBuf};
use tilde_contracts::proto::tilde::management::v1 as wire;
use tilde_contracts::proto::tilde::types::v1 as types;

#[derive(Args)]
pub struct Deploy {
    /// Module declaring the agent's prompts, skills and bundled tools. Defaults to the
    /// project's own entry point.
    pub entry: Option<String>,
    #[arg(long, env = "TILDE_URL")]
    pub url: Option<String>,
    #[arg(long, env = "TILDE_AGENT_ID")]
    pub agent_id: Option<String>,
    /// Required by Tilde Cloud; open-source Tilde needs none.
    #[arg(long, env = "TILDE_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,
    #[arg(long, default_value = "gateway", value_parser = ["gateway", "sidecar", "lambda"])]
    pub target: String,
    /// Required for, and only for, a Lambda target.
    #[arg(long)]
    pub function_arn: Option<String>,
    /// Stable release id. Registration is idempotent on it, so retries never double-register.
    #[arg(long)]
    pub external_id: Option<String>,
    #[arg(long)]
    pub label: Option<String>,
    /// Print the declarations as JSON and contact nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Print the deployment id, token and creation status as JSON.
    #[arg(long)]
    pub json: bool,
    #[arg(long, default_value = ".", hide = true)]
    pub dir: PathBuf,
}

pub fn target_of(target: &str) -> types::DeploymentTarget {
    match target {
        "sidecar" => types::DeploymentTarget::Sidecar,
        "lambda" => types::DeploymentTarget::Lambda,
        _ => types::DeploymentTarget::Gateway,
    }
}

fn variable(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

/// Whatever the CI provider tells us about the commit being released.
pub fn ci_metadata() -> wire::RegisterDeploymentRequest {
    wire::RegisterDeploymentRequest {
        source: if variable("CI").is_some() || variable("GITHUB_ACTIONS").is_some() {
            types::DeploymentSource::Ci.into()
        } else {
            types::DeploymentSource::Manual.into()
        },
        repository: variable("GITHUB_REPOSITORY"),
        commit_sha: variable("GITHUB_SHA"),
        branch: variable("GITHUB_REF_NAME"),
        ..Default::default()
    }
}

pub async fn run(args: Deploy) -> Result<()> {
    if (args.target == "lambda") != args.function_arn.is_some() {
        bail!("--function-arn is required for a lambda target, and only for lambda");
    }
    let project = Project::find(&args.dir)?;
    let entry = match &args.entry {
        Some(entry) => entry.clone(),
        None => project.entry()?,
    };
    let declarations = discover::declarations(&project, &entry, false).await?;

    if args.dry_run {
        println!("{}", serde_json::to_string_pretty(&declarations.value)?);
        return Ok(());
    }

    let (Some(url), Some(agent_id)) = (&args.url, &args.agent_id) else {
        bail!("--url and --agent-id (or TILDE_URL and TILDE_AGENT_ID) are required");
    };
    let client = Client::new(url, args.api_key.clone())?;

    let mut request = ci_metadata();
    request.target = target_of(&args.target).into();
    request.target_reference = args.function_arn.clone();
    request.label = args.label.clone();
    // A GitHub run is the natural release id: stable across reruns of the same run.
    request.external_id = args.external_id.clone().or_else(|| {
        Some(format!(
            "{}:{}",
            variable("GITHUB_REPOSITORY")?,
            variable("GITHUB_RUN_ID")?
        ))
    });

    let deployment = registry::register(&client, agent_id, &declarations, request).await?;
    eprintln!(
        "Deployment {} {}",
        deployment.id,
        if deployment.created {
            "registered"
        } else {
            "already registered (no token)"
        }
    );
    if args.json {
        println!(
            "{}",
            json!({
                "deploymentId": deployment.id,
                "token": (!deployment.token.is_empty()).then_some(deployment.token),
                "created": deployment.created,
            })
        );
    } else {
        println!("{}", deployment.token);
    }
    Ok(())
}
