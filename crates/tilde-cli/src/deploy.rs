//! `tilde deploy`: register a release carrying what the agent's code declares, or with
//! `@trytilde/iac` specs apply them: the project's ontologies, then its agents, each deploying
//! agent releasing its own code (`agents/<key>/`, or its `defineAgent({ code })`). This is the
//! command CI runs; it prints a dial-in deployment's token on stdout and its inventory on stderr,
//! so `TOKEN=$(tilde deploy)` works for one (several print a `<key> <token>` line each).
use crate::{api::Client, discover, project::Project, registry};
use anyhow::{Result, bail};
use clap::Args;
use serde_json::json;
use std::{collections::BTreeSet, env, path::PathBuf};
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
    /// Without specs; default gateway. A spec's `deploy` names its own target.
    /// `hosted` (Tilde Cloud): Tilde bundles and runs the code itself.
    #[arg(long, value_parser = ["gateway", "sidecar", "lambda", "hosted"])]
    pub target: Option<String>,
    /// Required for, and only for, a Lambda target.
    #[arg(long)]
    pub function_arn: Option<String>,
    /// A Lambda target's invoke role: the IAM role Tilde assumes to invoke the function. Tilde
    /// Cloud requires one (`arn:aws:iam::<account>:role/tilde-org-<name>`), assumed with the
    /// organization's ID as the external ID; open-source Tilde without one uses its own
    /// credentials.
    #[arg(long)]
    pub role: Option<String>,
    /// Stable release id. Registration is idempotent on it, so retries never double-register.
    #[arg(long)]
    pub external_id: Option<String>,
    #[arg(long)]
    pub label: Option<String>,
    /// Without `@trytilde/iac` specs: print the declarations as JSON and contact nothing. With
    /// them: ask the server for the plan and apply nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Build the project's ontologies as a deploy would (evaluate each, compile it on the server,
    /// bundle its handler module, build its spaces) and stop: nothing is uploaded or changed, and
    /// the working tree may be dirty. The spaces' contract is left in their `node_modules`. It
    /// runs the deploy's preflight, so a missing connection, secret or environment fails it.
    #[arg(long, conflicts_with = "dry_run")]
    pub check: bool,
    /// The team new agents and blueprints belong to (Tilde Cloud). It must match the team the
    /// code declares (`defineProject`, `defineTeam`), if any.
    #[arg(long, env = "TILDE_TEAM")]
    pub team: Option<String>,
    /// Print the result as JSON: the deployment's id, token and creation status, or with
    /// `@trytilde/iac` specs the Apply result with each agent's deployment id and token. Without
    /// it, a dial-in deployment registered now prints its token alone, several a `<key> <token>`
    /// line each.
    #[arg(long)]
    pub json: bool,
    /// Deploy only the project's ontologies, or only its agents (default: both, ontologies first).
    #[arg(long, value_parser = ["ontologies", "agents"])]
    pub only: Option<String>,
    /// Deploy only this ontology of the project (repeatable).
    #[arg(long)]
    pub ontology: Vec<String>,
    /// Deploy the ontologies though the working tree has uncommitted changes: they are built as
    /// they are, and the version records HEAD's commit.
    #[arg(long)]
    pub allow_dirty: bool,
    /// An ontology model whose rebuild the rollout accepts (repeatable): a rollout that would
    /// rebuild a deployed model fails, naming the models, unless each is accepted.
    #[arg(long)]
    pub accept_rebuild: Vec<String>,
    #[arg(long, default_value = ".", hide = true)]
    pub dir: PathBuf,
}

pub fn target_of(target: &str) -> types::DeploymentTarget {
    match target {
        "sidecar" => types::DeploymentTarget::Sidecar,
        "lambda" => types::DeploymentTarget::Lambda,
        "hosted" => types::DeploymentTarget::Hosted,
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

/// Check everything before changing anything (the ontologies' preflight and, when agents follow,
/// a dry-run Apply whose tools on the project's ontology connections are checked against the
/// compiled handlers, since a new environment serves none yet), then deploy the ontologies. The
/// actions that blocked it, if any, in which case nothing changed, and the compiled handlers.
pub async fn ontologies_first(
    args: &Deploy,
    declarations: &discover::Declarations,
    specs: bool,
) -> Result<(Vec<crate::ontology::Action>, BTreeSet<String>)> {
    let agents = specs && args.only.is_none();
    // A project the deploy creates belongs to the deploy's team, as its agents do.
    let team = args.team.as_deref().or(declarations.team.as_deref());
    let (mut actions, plan, mut served) =
        crate::ontology::prepare(args, &declarations.ontologies, team, agents).await?;
    // Only all of the project's ontologies say which tools their connections serve: a selection
    // that leaves some out has nothing to check against.
    if declarations
        .ontologies
        .keys()
        .any(|name| !args.ontology.is_empty() && !args.ontology.contains(name))
    {
        served.clear();
    }
    // A dry run's Apply comes after (`apply::run`), with these handlers.
    if agents && !args.dry_run {
        actions.extend(crate::apply::preflight(args, declarations, &served).await?);
    }
    if actions.is_empty()
        && let Some(plan) = plan
    {
        crate::ontology::execute(args, plan).await?;
    }
    Ok((actions, served))
}

pub async fn run(args: Deploy) -> Result<()> {
    let project = Project::find(&args.dir)?;
    let entry = match &args.entry {
        Some(entry) => entry.clone(),
        None => project.entry()?,
    };
    let mut declarations = discover::declarations(&project, &entry, false, true).await?;
    // The project's ontologies first (`ontology.rs`): build, bind, roll out; then its agents,
    // each releasing its own code (`discover::releases`). `--check` registers no release, so it
    // reads no agent's code: its dry-run Apply checks the specs.
    let only = args.only.as_deref();
    if only != Some("ontologies") && !args.check {
        discover::releases(&project, &entry, &mut declarations).await?;
    }
    let specs = !declarations.agents.is_empty()
        || !declarations.users.is_empty()
        || !declarations.groups.is_empty();
    if args.check && only == Some("agents") {
        bail!("--check builds a project's ontologies: it cannot be combined with --only agents");
    }
    if args.check && declarations.ontologies.is_empty() {
        bail!(
            "--check builds a project's ontologies, and this one declares none (defineProject({{ ontologies }}))"
        );
    }
    let mut served = BTreeSet::new();
    if only != Some("agents") && !declarations.ontologies.is_empty() {
        let actions;
        (actions, served) = ontologies_first(&args, &declarations, specs).await?;
        if !actions.is_empty() {
            let base = args.url.as_deref().unwrap_or_default();
            eprint!("{}", crate::ontology::render(&actions, base));
            std::process::exit(2);
        }
    }
    if args.check {
        return Ok(());
    }
    let nothing_else = declarations.agents.is_empty()
        && declarations.users.is_empty()
        && declarations.groups.is_empty()
        && ["prompts", "skills", "tools"].iter().all(|k| {
            declarations.value[k]
                .as_array()
                .is_none_or(|a| a.is_empty())
        });
    if !declarations.ontologies.is_empty() && nothing_else && only != Some("agents") {
        return Ok(());
    }
    if only == Some("ontologies") {
        if declarations.ontologies.is_empty() {
            bail!(
                "--only ontologies: the project declares no ontologies (defineProject({{ ontologies }}))"
            );
        }
        return Ok(());
    }
    // An entry exporting `@trytilde/iac` specs is applied whole; otherwise the deployment alone
    // is registered, as before specs existed.
    if specs {
        return crate::apply::run(&args, &declarations, &served).await;
    }
    let target = args.target.as_deref().unwrap_or("gateway");
    if (target == "lambda") != args.function_arn.is_some() {
        bail!("--function-arn is required for a lambda target, and only for lambda");
    }
    if target != "lambda" && args.role.is_some() {
        bail!("--role is only for a lambda target");
    }

    if args.dry_run {
        println!("{}", serde_json::to_string_pretty(&declarations.value)?);
        return Ok(());
    }

    let (Some(url), Some(agent_id)) = (&args.url, &args.agent_id) else {
        bail!("--url and --agent-id (or TILDE_URL and TILDE_AGENT_ID) are required");
    };
    let client = Client::new(url, args.api_key.clone())?;

    let mut request = ci_metadata();
    request.target = target_of(target).into();
    request.target_reference = args.function_arn.clone();
    request.target_role = args.role.clone();
    request.label = args.label.clone();
    // A GitHub run is the natural release id: stable across reruns of the same run.
    request.external_id = args.external_id.clone().or_else(|| {
        Some(format!(
            "{}:{}",
            variable("GITHUB_REPOSITORY")?,
            variable("GITHUB_RUN_ID")?
        ))
    });

    let bundle = match target {
        "hosted" => Some(crate::hosted::bundle(&project.dir, &entry)?),
        _ => None,
    };
    let mut deployment = registry::register(&client, agent_id, &declarations, request).await?;
    if let Some(bundle) = bundle {
        let published = crate::hosted::publish(&client, agent_id, &deployment.id, bundle).await?;
        eprintln!(
            "Deployment {}: hosted by Tilde, function version {}",
            deployment.id,
            crate::hosted::version(&published)
        );
        // Tilde runs it: there is nothing to dial in with.
        deployment.token.clear();
    }
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
