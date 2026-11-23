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
    let deploying: Vec<usize> = agents
        .iter()
        .enumerate()
        .filter(|(_, agent)| agent.get("deployment").is_some())
        .map(|(i, _)| i)
        .collect();
    if deploying.len() > 1 {
        bail!(
            "Several agents in this entry declare deploy; an entry module deploys one agent's code"
        );
    }
    let declares = ["prompts", "skills", "tools"].iter().any(|kind| {
        declarations.value[kind]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    });
    if deploying.is_empty() && declares {
        bail!(
            "This entry declares prompts, skills or tools, but no exported agent declares deploy: \
             add `deploy: gateway()` (or `lambda(arn)`, `sidecar()`) to the agent whose code this is"
        );
    }
    if let Some(&i) = deploying.first() {
        let deployment = agents[i]["deployment"]
            .as_object_mut()
            .context("deployment is an object")?;
        // Stable across reruns: the CI run, else the content of what is deployed (and its commit
        // when known), so an unchanged deploy registers nothing new.
        let external = args.external_id.clone().unwrap_or_else(|| {
            match (variable("GITHUB_REPOSITORY"), variable("GITHUB_RUN_ID")) {
                (Some(repository), Some(run)) => format!("{repository}:{run}"),
                _ => format!(
                    "sha256:{}",
                    digest(&json!([
                        declarations.value,
                        deployment.clone(),
                        variable("GITHUB_SHA")
                    ]))
                ),
            }
        });
        deployment.insert("externalId".into(), Value::String(external));
        deployment.insert("declarations".into(), declarations.value.clone());
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

/// The actions that would block applying the specs, from a dry run: `tilde deploy` asks before
/// it changes anything (its ontologies included), so a blocked deploy changed nothing.
pub async fn preflight(
    args: &Deploy,
    declarations: &Declarations,
) -> Result<Vec<crate::ontology::Action>> {
    let Some(url) = &args.url else {
        bail!("--url (or TILDE_URL) is required");
    };
    let client = Client::new(url, args.api_key.clone())?;
    let mut request = request(args, declarations)?;
    request.dry_run = true;
    let response = apply(&client, request).await?;
    Ok(response
        .required_actions
        .into_iter()
        .map(|a| crate::ontology::Action {
            subject: a.subject,
            message: a.message,
            url: a.url,
        })
        .collect())
}

pub async fn run(args: &Deploy, declarations: &Declarations) -> Result<()> {
    let Some(url) = &args.url else {
        bail!("--url (or TILDE_URL) is required");
    };
    let client = Client::new(url, args.api_key.clone())?;
    let response = apply(&client, request(args, declarations)?).await?;
    if args.json {
        println!("{}", serde_json::to_string(&response)?);
    } else {
        eprint!("{}", render(&response, client.gateway()));
        for change in response.changes.iter().filter(|c| {
            c.kind == "deployment" && c.action.as_known() == Some(wire::ChangeAction::Unchanged)
        }) {
            eprintln!("Deployment {} already registered (no token)", change.key);
        }
        // `TOKEN=$(tilde deploy)` keeps working for a dial-in deployment registered just now.
        if let [token] = response
            .agents
            .iter()
            .filter(|a| !a.deployment_token.is_empty())
            .map(|a| a.deployment_token.as_str())
            .collect::<Vec<_>>()[..]
        {
            println!("{token}");
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

    /// The one agent that deploys the entry carries its declarations and a content-derived
    /// external ID, the project's team is sent, and the plan comes back rendered.
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
        };
        // SAFETY: tests in this binary do not read these variables concurrently.
        unsafe { std::env::remove_var("GITHUB_RUN_ID") };
        run(&args(url), &declarations).await.unwrap();
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

    #[test]
    fn code_without_a_deploying_agent_and_flags_the_spec_owns_are_refused() {
        let declarations = |prompts: Value| Declarations {
            value: json!({"prompts": prompts, "skills": [], "tools": []}),
            digest: "test".into(),
            agents: vec![json!({"key": "triage", "name": "Triage"})],
            team: None,
            users: Vec::new(),
            groups: Vec::new(),
            ontologies: Default::default(),
        };
        let error = super::request(
            &args("http://x".into()),
            &declarations(json!([{"name": "p"}])),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no exported agent declares deploy"),
            "{error}"
        );
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
