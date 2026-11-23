//! `tilde deploy` for a project whose entry exports `@trytilde/iac` specs: one `Apply` call
//! carrying every agent, then the plan (or the actions a person must take) rendered for people
//! or, with `--json`, the response as protobuf JSON.
//!
//! Exit codes: 0 applied or planned, 1 an error, 2 blocked by preflight (nothing was applied).
use crate::{api::Client, deploy::Deploy, discover::Declarations};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fmt::Write as _;
use tilde_contracts::proto::tilde::management::v1 as wire;

/// The request for `declarations`: its agents as discovered, the code's declarations and the
/// external ID on the one agent that deploys this entry, and the CI's commit details.
pub fn request(args: &Deploy, declarations: &Declarations) -> Result<wire::ApplyRequest> {
    request_with(args, declarations, &Default::default())
}
/// The request with each Hosted agent's bundle (by key) in its external ID's digest, so changed
/// code is a new deployment even when its declarations are not.
fn request_with(
    args: &Deploy,
    declarations: &Declarations,
    bundles: &Bundles,
) -> Result<wire::ApplyRequest> {
    // The spec owns the agent and its deploy target.
    for (set, flag) in [
        (args.agent_id.is_some(), "--agent-id (TILDE_AGENT_ID)"),
        (args.target.is_some(), "--target"),
        (args.function_arn.is_some(), "--function-arn"),
    ] {
        if set {
            bail!(
                "{flag} does not apply to @trytilde/iac specs: the agent's spec names its agent and deploy target"
            );
        }
    }
    if let (Some(flag), Some(declared)) = (&args.team, &declarations.team)
        && flag != declared
    {
        bail!("--team {flag} differs from the team the code declares ({declared})");
    }
    let mut agents = declarations.agents.clone();
    // Each deploying agent releases its code's declarations (`discover::releases`); one without
    // a release has its code elsewhere (or `--check` read none), so its spec is applied without
    // a deployment rather than with an empty one.
    let mut deploying = Vec::new();
    for (i, agent) in agents.iter_mut().enumerate() {
        let released = agent["key"]
            .as_str()
            .is_some_and(|key| declarations.releases.contains_key(key));
        if released && agent.get("deployment").is_some() {
            deploying.push(i);
        } else if let Some(agent) = agent.as_object_mut() {
            agent.remove("deployment");
        }
    }
    for &i in &deploying {
        let key = agents[i]["key"].as_str().unwrap_or_default().to_owned();
        let released = declarations.releases[&key].clone();
        let deployment = agents[i]["deployment"]
            .as_object_mut()
            .context("deployment is an object")?;
        // Stable across reruns: the CI run, else the content of what is deployed (and its commit
        // when known), so an unchanged deploy registers nothing new. Per agent: each has its own.
        let external = args.external_id.clone().unwrap_or_else(|| {
            match (variable("GITHUB_REPOSITORY"), variable("GITHUB_RUN_ID")) {
                (Some(repository), Some(run)) => format!("{repository}:{run}"),
                _ => format!(
                    "sha256:{}",
                    digest(&json!([
                        released,
                        deployment.clone(),
                        variable("GITHUB_SHA"),
                        bundles.get(&key).map(|b| bundle_digest(b)),
                    ]))
                ),
            }
        });
        deployment.insert("externalId".into(), Value::String(external));
        deployment.insert("declarations".into(), released);
        if let Some(label) = &args.label {
            deployment.insert("label".into(), Value::String(label.clone()));
        }
    }
    let ci = crate::deploy::ci_metadata();
    let request = json!({
        "dryRun": args.dry_run,
        "agents": agents,
        "team": args.team.clone().or_else(|| declarations.team.clone()).unwrap_or_default(),
        "users": declarations.users,
        "groups": declarations.groups,
    });
    let mut request: wire::ApplyRequest =
        serde_json::from_value(request).context("The discovered specs are not a valid request")?;
    request.repository = ci.repository;
    request.commit_sha = ci.commit_sha;
    request.branch = ci.branch;
    Ok(request)
}

/// Hosted agents' bundles, by key.
type Bundles = std::collections::BTreeMap<String, Vec<u8>>;
const HOSTED: &str = "DEPLOYMENT_TARGET_HOSTED";
fn bundle_digest(bundle: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bundle))
}
/// Build the bundle of every Hosted agent whose code this deploy releases.
fn bundles(declarations: &Declarations) -> Result<Bundles> {
    let mut bundles = Bundles::new();
    for agent in &declarations.agents {
        let key = agent["key"].as_str().unwrap_or_default();
        if agent["deployment"]["target"] != HOSTED {
            continue;
        }
        let Some((dir, entry)) = declarations.sources.get(key) else {
            continue;
        };
        eprintln!("Agent {key}: bundling {entry} for Tilde to host");
        bundles.insert(key.to_owned(), crate::hosted::bundle(dir, entry)?);
    }
    Ok(bundles)
}

fn variable(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}
fn digest(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(&Sha256::digest(serde_json::to_vec(value).expect("JSON re-serialises"))[..16])
}

async fn apply(client: &Client, request: wire::ApplyRequest) -> Result<wire::ApplyResponse> {
    Ok(client
        .apply
        .apply_with_options(request, crate::api::options())
        .await
        .map_err(|error| match error.code {
            connectrpc::ErrorCode::Unimplemented => anyhow::anyhow!(
                "This Tilde server does not support declarative apply; upgrade it to deploy @trytilde/iac specs"
            ),
            _ => crate::api::failed("Apply", error),
        })?
        .into_owned())
}

/// The server lists an ontology connection's tools from the running ontology, so before this
/// project's ontologies roll out (a new environment's first above all) it reports the tools it
/// can't find there. With the project's compiled handlers at hand (`served`, from
/// `ontology::prepare`), each such finding on a `tilde_ontology/...` connection gives way to a
/// check of that source's tools against them. A connection whose tools the server did list (say,
/// another project's running ontology) keeps the server's word. Without handlers (`--only
/// agents`, `--ontology` choosing some, an ontology that does not compile) the server's findings
/// stand as they are.
pub(crate) fn with_ontology_tools(
    actions: Vec<wire::RequiredAction>,
    declarations: &Declarations,
    served: &std::collections::BTreeSet<String>,
) -> Vec<wire::RequiredAction> {
    if served.is_empty() {
        return actions;
    }
    let unlisted = |a: &wire::RequiredAction| {
        a.code.as_known() == Some(wire::RequiredActionCode::ToolNotOffered)
            && a.subject.contains("/tools/tilde_ontology/")
    };
    let subjects: std::collections::BTreeSet<String> = actions
        .iter()
        .filter(|a| unlisted(a))
        .map(|a| a.subject.clone())
        .collect();
    let mut kept: Vec<wire::RequiredAction> =
        actions.into_iter().filter(|a| !unlisted(a)).collect();
    for agent in &declarations.agents {
        let key = agent["key"].as_str().unwrap_or_default();
        for source in agent["tools"].as_array().into_iter().flatten() {
            let slug = source["connection"]["slug"].as_str().unwrap_or_default();
            let subject = format!("{key}/tools/{slug}");
            if !subjects.contains(&subject) {
                continue;
            }
            for name in source["tools"].as_array().into_iter().flatten() {
                let name = name.as_str().unwrap_or_default();
                if name != "*" && !served.contains(name) {
                    kept.push(wire::RequiredAction {
                        code: wire::RequiredActionCode::ToolNotOffered.into(),
                        subject: subject.clone(),
                        message: format!("The project's ontologies serve no tool {name}"),
                        ..Default::default()
                    });
                }
            }
        }
    }
    kept
}

/// The actions that would block applying the specs, from a dry run: `tilde deploy` asks before
/// it changes anything (its ontologies included), so a blocked deploy changed nothing. Tools on
/// the project's ontology connections are checked against `served`, the compiled handlers.
pub async fn preflight(
    args: &Deploy,
    declarations: &Declarations,
    served: &std::collections::BTreeSet<String>,
) -> Result<Vec<crate::ontology::Action>> {
    let Some(url) = &args.url else {
        bail!("--url (or TILDE_URL) is required");
    };
    let client = Client::new(url, args.api_key.clone())?;
    let mut request = request(args, declarations)?;
    request.dry_run = true;
    let response = apply(&client, request).await?;
    Ok(
        with_ontology_tools(response.required_actions, declarations, served)
            .into_iter()
            .map(|a| crate::ontology::Action {
                subject: a.subject,
                message: a.message,
                url: a.url,
            })
            .collect(),
    )
}

/// Apply the specs. A dry run's plan checks the tools on the project's ontology connections
/// against `served`, the compiled handlers, as `preflight` does.
pub async fn run(
    args: &Deploy,
    declarations: &Declarations,
    served: &std::collections::BTreeSet<String>,
) -> Result<()> {
    let Some(url) = &args.url else {
        bail!("--url (or TILDE_URL) is required");
    };
    let client = Client::new(url, args.api_key.clone())?;
    let mut bundles = if args.dry_run {
        Bundles::new()
    } else {
        bundles(declarations)?
    };
    let request = request_with(args, declarations, &bundles)?;
    let deploying = request
        .agents
        .iter()
        .filter(|a| a.deployment.is_set())
        .count();
    let mut response = apply(&client, request).await?;
    if args.dry_run && !served.is_empty() {
        let actions = with_ontology_tools(response.required_actions, declarations, served);
        response.outcome = if actions.is_empty() {
            wire::ApplyOutcome::Planned
        } else {
            wire::ApplyOutcome::Blocked
        }
        .into();
        response.required_actions = actions;
    }
    // Each Hosted deployment serves once its code is published: Tilde makes it a version of the
    // agent's function. Publishing one that already serves changes nothing.
    let hosted: std::collections::BTreeSet<String> = bundles.keys().cloned().collect();
    if response.outcome.as_known() == Some(wire::ApplyOutcome::Applied) {
        for agent in &response.agents {
            let Some(bundle) = bundles.remove(&agent.key) else {
                continue;
            };
            if agent.deployment_id.is_empty() {
                continue;
            }
            let published =
                crate::hosted::publish(&client, &agent.id, &agent.deployment_id, bundle).await?;
            eprintln!(
                "Agent {}: hosted by Tilde, function version {}",
                agent.key,
                crate::hosted::version(&published)
            );
        }
    }
    if args.json {
        println!("{}", serde_json::to_string(&response)?);
    } else {
        eprint!("{}", render(&response, client.gateway()));
        for change in response.changes.iter().filter(|c| {
            c.kind == "deployment" && c.action.as_known() == Some(wire::ChangeAction::Unchanged)
        }) {
            eprintln!("Deployment {} already registered (no token)", change.key);
        }
        // The dial-in deployments registered just now. A request with one deploying agent prints
        // its token alone (`TOKEN=$(tilde deploy)`); with several, a `<key> <token>` line each,
        // however many were issued this run (`--json` has them too).
        for agent in response
            .agents
            .iter()
            .filter(|a| !a.deployment_token.is_empty() && !hosted.contains(&a.key))
        {
            match deploying {
                1 => println!("{}", agent.deployment_token),
                _ => println!("{} {}", agent.key, agent.deployment_token),
            }
        }
    }
    if response.outcome.as_known() == Some(wire::ApplyOutcome::Blocked) {
        std::process::exit(2);
    }
    Ok(())
}

/// The plan for people: every create, update and removal with its fields, unchanged resources
/// counted, and when blocked every required action with its link (relative ones joined to the
/// server's URL, as servers without an app origin send them).
pub fn render(response: &wire::ApplyResponse, base: &str) -> String {
    use wire::{ApplyOutcome as O, ChangeAction as A};
    let mut out = String::new();
    let outcome = response.outcome.as_known();
    if outcome == Some(O::Blocked) {
        let _ = writeln!(
            out,
            "Blocked: {} action(s) needed before anything is applied",
            response.required_actions.len()
        );
        for action in &response.required_actions {
            let _ = writeln!(out, "  ✗ {}: {}", action.subject, action.message);
            if !action.url.is_empty() {
                let url = if action.url.starts_with('/') {
                    format!("{base}{}", action.url)
                } else {
                    action.url.clone()
                };
                let _ = writeln!(out, "      → {url}");
            }
        }
        return out;
    }
    let _ = writeln!(
        out,
        "{}",
        if outcome == Some(O::Planned) {
            "Plan (dry run, nothing applied):"
        } else {
            "Applied:"
        }
    );
    let mut unchanged = 0;
    for change in &response.changes {
        let mark = match change.action.as_known() {
            Some(A::Create) => "+",
            Some(A::Update) => "~",
            Some(A::Remove) => "-",
            _ => {
                unchanged += 1;
                continue;
            }
        };
        let _ = writeln!(out, "  {mark} {} {}", change.kind, change.key);
        for field in &change.fields {
            if field.sealed {
                let _ = writeln!(out, "      {}: (secret) changed", field.field);
            } else if field.before.is_empty() {
                let _ = writeln!(out, "      {}: {}", field.field, field.after);
            } else {
                let _ = writeln!(
                    out,
                    "      {}: {} → {}",
                    field.field, field.before, field.after
                );
            }
        }
        if !change.note.is_empty() {
            let _ = writeln!(out, "      warning: {}", change.note);
        }
    }
    if unchanged > 0 {
        let _ = writeln!(out, "  {unchanged} unchanged");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Bytes, http::header, response::IntoResponse, routing::post};
    use buffa::Message;
    use std::sync::{Arc, Mutex};

    fn args(url: String) -> Deploy {
        Deploy {
            entry: None,
            url: Some(url),
            agent_id: None,
            api_key: None,
            target: None,
            function_arn: None,
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
            dir: ".".into(),
        }
    }

    /// The deploying agent carries its release's declarations and a content-derived external ID, the project's team is sent, and the plan comes back rendered.
    #[tokio::test]
    async fn applies_discovered_specs_with_the_entrys_declarations() {
        let received: Arc<Mutex<Option<wire::ApplyRequest>>> = Arc::new(Mutex::new(None));
        let app = Router::new().route(
            "/tilde.management.v1.ApplyService/Apply",
            post({
                let received = received.clone();
                move |body: Bytes| async move {
                    *received.lock().unwrap() =
                        Some(wire::ApplyRequest::decode_from_slice(&body).unwrap());
                    let response = wire::ApplyResponse {
                        outcome: wire::ApplyOutcome::Applied.into(),
                        changes: vec![wire::Change {
                            kind: "tool_source".into(),
                            key: "triage/linear/acme".into(),
                            action: wire::ChangeAction::Update.into(),
                            fields: vec![wire::FieldChange {
                                field: "tools".into(),
                                before: "get_issue".into(),
                                after: "get_issue, search_issues".into(),
                                ..Default::default()
                            }],
                            ..Default::default()
                        }],
                        ..Default::default()
                    };
                    (
                        [(header::CONTENT_TYPE, "application/proto")],
                        response.encode_to_vec(),
                    )
                        .into_response()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let declarations = Declarations {
            value: json!({"prompts": [], "skills": [], "tools": []}),
            digest: "test".into(),
            agents: vec![
                json!({"key": "triage", "name": "Triage", "deployment": {"target": "DEPLOYMENT_TARGET_GATEWAY"},
                    "tools": [{"connection": {"slug": "linear/acme"}, "tools": ["get_issue", "search_issues"]}]}),
                json!({"key": "helper", "name": "Helper"}),
            ],
            team: Some("agents".into()),
            users: vec![json!({"email": "ada@acme.test", "name": "Ada", "login": true})],
            groups: vec![json!({"name": "eng", "members": [{"email": "ada@acme.test"}]})],
            ontologies: Default::default(),
            releases: [(
                "triage".to_string(),
                json!({"prompts": [], "skills": [], "tools": []}),
            )]
            .into(),
            ..Default::default()
        };
        // SAFETY: tests in this binary do not read these variables concurrently.
        unsafe { std::env::remove_var("GITHUB_RUN_ID") };
        run(&args(url), &declarations, &Default::default())
            .await
            .unwrap();
        let request = received.lock().unwrap().take().unwrap();
        assert_eq!(request.team, "agents");
        assert_eq!(request.users[0].email, "ada@acme.test");
        assert!(request.users[0].login);
        assert_eq!(request.groups[0].members[0].email, "ada@acme.test");
        assert_eq!(request.agents.len(), 2);
        let deployment = request.agents[0].deployment.as_option().unwrap();
        assert!(
            deployment.external_id.starts_with("sha256:"),
            "{}",
            deployment.external_id
        );
        assert!(deployment.declarations.is_set());
        assert!(request.agents[1].deployment.as_option().is_none());
        assert_eq!(
            request.agents[0].tools[0].tools,
            ["get_issue", "search_issues"]
        );
        // The same content gives the same external ID, so a rerun registers nothing new.
        let again = super::request(&args("http://x".into()), &declarations).unwrap();
        assert_eq!(
            again.agents[0].deployment.as_option().unwrap().external_id,
            deployment.external_id
        );
    }

    /// A Hosted agent's code is bundled (esbuild's module at `dist/index.mjs`, its prompts beside
    /// it and the SDK's version), its digest is in the external ID, and once Apply registers the
    /// deployment the bundle is published; Tilde runs it, so no token is printed for it.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_hosted_agent_is_bundled_and_published_after_apply() {
        let code = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(code.path().join("prompts")).unwrap();
        std::fs::write(code.path().join("prompts/system.md"), "Fix bugs.").unwrap();
        std::fs::write(code.path().join("index.js"), "export const handler = 1;").unwrap();
        std::fs::create_dir_all(code.path().join("node_modules/@trytilde/sdk")).unwrap();
        std::fs::write(
            code.path().join("node_modules/@trytilde/sdk/package.json"),
            r#"{"name": "@trytilde/sdk", "version": "9.9.9"}"#,
        )
        .unwrap();
        // A stand-in esbuild API: the "bundle" is the entry as it is, exporting `handler`.
        let esbuild = code.path().join("node_modules/esbuild/lib");
        std::fs::create_dir_all(&esbuild).unwrap();
        std::fs::write(
            esbuild.join("main.js"),
            "import { cpSync, mkdirSync } from 'node:fs'; import { dirname } from 'node:path';\n\
             export async function build(o) { mkdirSync(dirname(o.outfile), { recursive: true }); \
             cpSync(o.entryPoints[0], o.outfile); \
             return { metafile: { outputs: { [o.outfile]: { exports: ['handler'] } } } }; }\n",
        )
        .unwrap();
        let applied: Arc<Mutex<Option<wire::ApplyRequest>>> = Arc::default();
        let published: Arc<Mutex<Option<wire::PublishHostedDeploymentRequest>>> = Arc::default();
        let proto = |message: Vec<u8>| ([(header::CONTENT_TYPE, "application/proto")], message);
        let app = Router::new()
            .route(
                "/tilde.management.v1.ApplyService/Apply",
                post({
                    let applied = applied.clone();
                    move |body: Bytes| async move {
                        *applied.lock().unwrap() =
                            Some(wire::ApplyRequest::decode_from_slice(&body).unwrap());
                        proto(
                            wire::ApplyResponse {
                                outcome: wire::ApplyOutcome::Applied.into(),
                                agents: vec![wire::AppliedAgent {
                                    key: "fixer".into(),
                                    id: "a1".into(),
                                    deployment_id: "d1".into(),
                                    deployment_token: "unused".into(),
                                    ..Default::default()
                                }],
                                ..Default::default()
                            }
                            .encode_to_vec(),
                        )
                        .into_response()
                    }
                }),
            )
            .route(
                "/tilde.management.v1.DeploymentService/PublishHostedDeployment",
                post({
                    let published = published.clone();
                    move |body: Bytes| async move {
                        *published.lock().unwrap() = Some(
                            wire::PublishHostedDeploymentRequest::decode_from_slice(&body).unwrap(),
                        );
                        proto(
                            wire::PublishHostedDeploymentResponse {
                                deployment:
                                    tilde_contracts::proto::tilde::types::v1::AgentDeployment {
                                        id: "d1".into(),
                                        target_reference:
                                            "arn:aws:lambda:us-east-1:1:function:tilde-hosted-a1:7"
                                                .into(),
                                        ..Default::default()
                                    }
                                    .into(),
                                ..Default::default()
                            }
                            .encode_to_vec(),
                        )
                        .into_response()
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let declarations = Declarations {
            value: json!({"prompts": [], "skills": [], "tools": []}),
            agents: vec![
                json!({"key": "fixer", "name": "Fixer", "deployment": {"target": "DEPLOYMENT_TARGET_HOSTED"}}),
            ],
            releases: [(
                "fixer".to_string(),
                json!({"prompts": [], "skills": [], "tools": []}),
            )]
            .into(),
            sources: [(
                "fixer".to_string(),
                (code.path().to_path_buf(), "index.js".to_string()),
            )]
            .into(),
            ..Default::default()
        };
        // SAFETY: tests in this binary do not read these variables concurrently.
        unsafe { std::env::remove_var("GITHUB_RUN_ID") };
        run(&args(url), &declarations, &Default::default())
            .await
            .unwrap();
        let applied = applied.lock().unwrap().take().unwrap();
        let external = &applied.agents[0]
            .deployment
            .as_option()
            .unwrap()
            .external_id;
        // Changed code alone is a new deployment.
        assert_ne!(
            external,
            &super::request(&args("http://x".into()), &declarations)
                .unwrap()
                .agents[0]
                .deployment
                .as_option()
                .unwrap()
                .external_id
        );
        let published = published.lock().unwrap().take().unwrap();
        assert_eq!(
            (
                published.agent_id.as_str(),
                published.deployment_id.as_str()
            ),
            ("a1", "d1")
        );
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(published.bundle)).unwrap();
        let mut names: Vec<String> = zip.file_names().map(str::to_owned).collect();
        names.sort();
        assert_eq!(
            names,
            ["dist/index.mjs", "package.json", "prompts/system.md"]
        );
        let mut manifest = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("package.json").unwrap(), &mut manifest)
            .unwrap();
        assert!(manifest.contains("9.9.9"), "{manifest}");
    }

    /// A root `tilde deploy` releases every deploying agent's own code: each reads its folder
    /// (`agents/<key>/`, or its `code`), and each deployment carries those declarations and an
    /// external ID of its own. An agent with no code here keeps its spec and gets no deployment.
    /// From one agent's folder (a code entry) the entry is that agent's release, and can be only
    /// one agent's.
    #[tokio::test]
    async fn a_root_deploy_releases_each_agents_own_code() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("package.json"),
            r#"{"name": "project", "main": "index.js"}"#,
        )
        .unwrap();
        for dir in ["agents/triage", "code/helper"] {
            std::fs::create_dir_all(root.path().join(dir)).unwrap();
            std::fs::write(
                root.path().join(dir).join("package.json"),
                r#"{"name": "agent", "main": "index.js"}"#,
            )
            .unwrap();
        }
        // An agent folder may hold its own tilde.config.ts: its code is still the package's.
        std::fs::write(root.path().join("agents/triage/tilde.config.ts"), "").unwrap();
        std::fs::write(root.path().join("agents/triage/index.js"), "").unwrap();
        // A stand-in declaration reader: each agent's prompt is named after its folder and the
        // module it read.
        let reader = root.path().join("reader.sh");
        std::fs::write(
            &reader,
            "#!/bin/sh\necho '{\"prompts\": [{\"name\": \"'\"$(basename \"$PWD\"):$1\"'\"}], \"skills\": [], \"tools\": []}'\n",
        )
        .unwrap();
        std::fs::set_permissions(&reader, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        // SAFETY: tests in this binary do not read these variables concurrently.
        unsafe {
            std::env::set_var("TILDE_DECLARATIONS", &reader);
            std::env::remove_var("GITHUB_RUN_ID");
        }
        let gateway = json!({"target": "DEPLOYMENT_TARGET_GATEWAY"});
        let mut declarations = Declarations {
            value: json!({"prompts": [], "skills": [], "tools": []}),
            agents: vec![
                json!({"key": "triage", "name": "Triage", "deployment": gateway}),
                json!({"key": "helper", "name": "Helper", "deployment": gateway}),
                json!({"key": "quiet", "name": "Quiet"}),
                json!({"key": "remote", "name": "Remote", "deployment": gateway}),
            ],
            code: [("helper".to_string(), "code/helper".to_string())].into(),
            ..Default::default()
        };
        let project = crate::project::Project::find(root.path()).unwrap();
        crate::discover::releases(&project, "tilde.config.ts", &mut declarations)
            .await
            .unwrap();
        unsafe { std::env::remove_var("TILDE_DECLARATIONS") };
        let request = super::request(&args("http://x".into()), &declarations).unwrap();
        let released = |i: usize| request.agents[i].deployment.as_option().unwrap().clone();
        assert_eq!(released(0).declarations.prompts[0].name, "triage:index.js");
        assert_eq!(released(1).declarations.prompts[0].name, "helper:index.js");
        assert_ne!(released(0).external_id, released(1).external_id);
        assert!(request.agents[2].deployment.as_option().is_none());
        assert!(request.agents[3].deployment.as_option().is_none());
        assert_eq!(request.agents[3].name, "Remote");

        // From inside an agent's folder: its tilde.config.ts declares only it, and the folder's
        // package is its code.
        let triage_dir = crate::project::Project::find(&root.path().join("agents/triage")).unwrap();
        let mut inside = Declarations {
            value: json!({"prompts": [], "skills": [], "tools": []}),
            agents: vec![json!({"key": "triage", "name": "Triage", "deployment": gateway})],
            ..Default::default()
        };
        unsafe { std::env::set_var("TILDE_DECLARATIONS", &reader) };
        crate::discover::releases(&triage_dir, "tilde.config.ts", &mut inside)
            .await
            .unwrap();
        unsafe { std::env::remove_var("TILDE_DECLARATIONS") };
        assert_eq!(
            inside.releases["triage"]["prompts"][0]["name"],
            "triage:index.js"
        );
        // A Hosted deployment is bundled from that folder and its code entry.
        assert_eq!(
            inside.sources["triage"],
            (triage_dir.dir.clone(), "index.js".to_string())
        );

        // A config-only repo declaring one remote agent, with a package whose `main` is missing:
        // not the agent's folder, so its spec is applied without a release (the reader is never
        // run on the missing entry).
        let config_only = root.path().join("ops");
        std::fs::create_dir_all(&config_only).unwrap();
        std::fs::write(
            config_only.join("package.json"),
            r#"{"name": "ops", "main": "index.js"}"#,
        )
        .unwrap();
        std::fs::write(config_only.join("tilde.config.ts"), "").unwrap();
        let mut remote = Declarations {
            value: json!({"prompts": [], "skills": [], "tools": []}),
            agents: vec![json!({"key": "remote", "name": "Remote", "deployment": gateway})],
            ..Default::default()
        };
        unsafe { std::env::set_var("TILDE_DECLARATIONS", "/nonexistent/reader") };
        crate::discover::releases(
            &crate::project::Project::find(&config_only).unwrap(),
            "tilde.config.ts",
            &mut remote,
        )
        .await
        .unwrap();
        unsafe { std::env::remove_var("TILDE_DECLARATIONS") };
        assert!(remote.releases.is_empty());

        // From the agent's own folder its entry is the release of the one agent without code.
        let mut own = Declarations {
            value: json!({"prompts": [{"name": "entry"}], "skills": [], "tools": []}),
            agents: vec![json!({"key": "remote", "name": "Remote", "deployment": gateway})],
            ..Default::default()
        };
        crate::discover::releases(&project, "index.js", &mut own)
            .await
            .unwrap();
        assert_eq!(own.releases["remote"]["prompts"][0]["name"], "entry");
        own.agents
            .push(json!({"key": "other", "name": "Other", "deployment": gateway}));
        own.releases.clear();
        let error = crate::discover::releases(&project, "index.js", &mut own)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Agents remote, other declare deploy"),
            "{error}"
        );
        own.agents.clear();
        let error = crate::discover::releases(&project, "index.js", &mut own)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no exported agent declares deploy"),
            "{error}"
        );
    }

    #[test]
    fn flags_the_spec_owns_and_a_different_team_are_refused() {
        let declarations = |prompts: Value| Declarations {
            value: json!({"prompts": prompts, "skills": [], "tools": []}),
            digest: "test".into(),
            agents: vec![json!({"key": "triage", "name": "Triage"})],
            team: None,
            users: Vec::new(),
            groups: Vec::new(),
            ontologies: Default::default(),
            ..Default::default()
        };
        // Specs without code declarations configure agents only.
        super::request(&args("http://x".into()), &declarations(json!([]))).unwrap();
        let mut other_team = args("http://x".into());
        other_team.team = Some("other".into());
        let declared = Declarations {
            team: Some("acme".into()),
            ..declarations(json!([]))
        };
        let error = super::request(&other_team, &declared).unwrap_err();
        assert!(
            error.to_string().contains("differs from the team"),
            "{error}"
        );
        let mut flagged = args("http://x".into());
        flagged.agent_id = Some("11111111-2222-4333-8444-555555555555".into());
        let error = super::request(&flagged, &declarations(json!([]))).unwrap_err();
        assert!(error.to_string().contains("--agent-id"), "{error}");
    }

    #[test]
    fn a_blocked_plan_lists_every_action_with_an_absolute_link() {
        let response = wire::ApplyResponse {
            outcome: wire::ApplyOutcome::Blocked.into(),
            required_actions: vec![
                wire::RequiredAction {
                    code: wire::RequiredActionCode::ConnectionMissing.into(),
                    subject: "triage/tools/linear/acme".into(),
                    message: "Connection linear/acme does not exist".into(),
                    url: "/registry/tools/catalog/linear?name=acme".into(),
                    ..Default::default()
                },
                wire::RequiredAction {
                    code: wire::RequiredActionCode::ToolNotOffered.into(),
                    subject: "triage/tools/github/acme".into(),
                    message:
                        "Connection github/acme offers no tool get_isue; did you mean get_issue?"
                            .into(),
                    url: "https://app.example/registry/tools/catalog/github".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let text = render(&response, "http://tilde.local");
        assert!(text.starts_with("Blocked: 2 action(s)"), "{text}");
        assert!(
            text.contains("→ http://tilde.local/registry/tools/catalog/linear?name=acme"),
            "{text}"
        );
        assert!(
            text.contains("→ https://app.example/registry/tools/catalog/github"),
            "{text}"
        );
    }
}
