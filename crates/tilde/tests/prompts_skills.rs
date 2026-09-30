//! Prompts and skills across their boundaries: a deployment registers what the agent's code
//! declares, the gateway links inference calls to prompt versions by their text or the SDK's
//! stamp, invocations read their deployment's skills, and the management service reads
//! history and usage; skill sources are enabled from the catalog,
//! synced from a (stubbed) GitHub repository or authored, versioned, assigned to agents
//! directly or through a connection, and read by agents through the runtime listener.
mod common;
use axum::{Router, body::Bytes, extract::State, response::IntoResponse, routing::post};
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;
use std::sync::Arc;
use tilde::chat as application;
use tilde::proto::tilde::{management::v1 as management, types::v1 as types};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::{model::*, service::Connections},
    deployment::{Deployments, RegisterDeployment},
    encryption::Encryption,
    inference::{self, Gateway, audit},
    prompts::{self, Prompts},
    skills::{Skills, Upload},
};
use uuid::Uuid;

async fn provider() -> String {
    async fn chat(State(()): State<()>, _body: Bytes) -> axum::response::Response {
        axum::Json(
            json!({"id":"c1","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5}}),
        )
        .into_response()
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/v1/chat/completions", post(chat))
        .with_state(());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}
async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    url
}
async fn create_agent(agents: &Agents) -> Uuid {
    create_agent_with(agents, Default::default()).await
}
async fn create_agent_with(
    agents: &Agents,
    capabilities: tilde::iam::capabilities::Capabilities,
) -> Uuid {
    let id = Uuid::new_v4();
    agents
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id,
            name: "Prompt fixture".into(),
            capabilities,
        })
        .await
        .unwrap();
    id
}
async fn invocation(
    chat: &Chat,
    pool: &tilde::database::Pool,
    agent: Uuid,
) -> (SecretString, Uuid) {
    let thread = chat
        .create_thread(application::CreateThread {
            title: "Prompts".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(agent.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let run = chat
        .start_run(application::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "Answer".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    pool.get().await.unwrap().execute("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1", &[&invocation]).await.unwrap();
    let token = chat
        .tokens
        .issue(
            agent,
            invocation,
            Uuid::parse_str(&thread.id).unwrap(),
            Uuid::parse_str(&run.id).unwrap(),
        )
        .await
        .unwrap();
    (token, invocation)
}
async fn connect(connections: &Connections, agent: Uuid, upstream: &str) -> Uuid {
    let id = Uuid::new_v4();
    let started = connections
        .start(
            id,
            "OpenAI",
            "openai",
            "api",
            &[Assignment {
                capability: Capability::Inference,
                agent_id: agent,
                alias: None,
            }],
        )
        .await
        .unwrap();
    let url = url::Url::parse(&started.brokering_url).unwrap();
    let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let token = url
        .query_pairs()
        .find(|(key, _)| key == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    let view = connections.view(setup, &token).await.unwrap();
    let view = connections
        .set_connection_name(setup, &token, view.action_id, "prod")
        .await
        .unwrap();
    connections
        .advance(
            setup,
            &token,
            view.action_id,
            [
                ("api_key".to_string(), SecretString::from("sk-test")),
                ("base_url".to_string(), SecretString::from(upstream)),
            ]
            .into_iter()
            .collect(),
        )
        .await
        .unwrap();
    id
}
fn prompt(name: &str, format: types::PromptFormat, template: &str) -> management::DeclaredPrompt {
    management::DeclaredPrompt {
        name: name.into(),
        format: format.into(),
        template: template.into(),
        config: "{}".into(),
        hash: hex::encode(prompts::content_hash(template, &Default::default(), "{}")),
        origin: format!("src/agent.ts#{name}"),
        ..Default::default()
    }
}
fn bundled_skill(name: &str, body: &str) -> management::DeclaredSkill {
    management::DeclaredSkill {
        name: name.into(),
        files: vec![management::DeclaredSkillFile {
            path: "SKILL.md".into(),
            body: Some(management::declared_skill_file::Body::Content(skill_md(
                name,
                "Handle refunds",
                body,
            ))),
            ..Default::default()
        }],
        origin: format!("skills/{name}"),
        ..Default::default()
    }
}
fn deployments(
    pool: &tilde::database::Pool,
    encryption: Arc<Encryption>,
    connections: Connections,
) -> Deployments {
    Deployments::new(
        pool.clone(),
        encryption.clone(),
        Agents::new(pool.clone(), encryption),
        connections,
    )
    .with_skills(Skills::new(pool.clone()))
}
async fn deploy(
    deployments: &Deployments,
    agent: Uuid,
    external: &str,
    declarations: management::DeploymentDeclarations,
) -> (Uuid, bool) {
    let (deployment, _, created) = deployments
        .register_deployment(
            agent,
            RegisterDeployment {
                source: types::DeploymentSource::Ci,
                target: types::DeploymentTarget::Gateway,
                target_reference: None,
                repository: None,
                commit_sha: None,
                external_id: Some(external.into()),
                label: None,
                commit_message: None,
                branch: None,
                commit_author: None,
                declarations,
            },
        )
        .await
        .unwrap();
    (Uuid::parse_str(&deployment.id).unwrap(), created)
}
async fn pin(pool: &tilde::database::Pool, invocation: Uuid, deployment: Uuid) {
    pool.get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET deployment_id=$2 WHERE id=$1",
            &[&invocation, &deployment],
        )
        .await
        .unwrap();
}

/// A deployment ships what its code declares; the gateway links calls to the versions they
/// used by text (plain, braces) and by stamp (dynamic); a changed prompt is a new version on
/// the next deployment, and each invocation sees the skills its own deployment shipped.
#[tokio::test]
async fn deployments_ship_prompts_and_skills_that_calls_and_invocations_resolve() {
    use types::PromptFormat as F;
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(21)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let agent = create_agent(&agents).await;
    connect(&connections, agent, &provider().await).await;
    let loader = inference::gateway::Loader::new(db.pool.clone(), connections.clone());
    loader.load().await.unwrap();
    let gateway = Gateway {
        upstreams: loader.upstreams.clone(),
        audit: audit::Audit::start(audit::Sink::Postgres(db.pool.clone())),
    };
    let chat = Chat::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
    )
    .with_inference(gateway.clone());
    let deployments = deployments(&db.pool, encryption.clone(), connections.clone());
    let management = tilde::deployment::rpc::management_router(deployments.clone());
    let management = serve(management).await;
    let runtime = serve(tilde::iam::listeners::agent_rpc_router(
        agents.clone(),
        chat.clone(),
    ))
    .await;
    let http = reqwest::Client::new();
    let post = |url: String, token: Option<String>, body: serde_json::Value| {
        let http = http.clone();
        async move {
            let mut request = http.post(url).json(&body);
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            let response = request.send().await.unwrap();
            assert_eq!(response.status(), 200);
            response.json::<serde_json::Value>().await.unwrap()
        }
    };

    let instructions = "You are the support agent for Acme. Answer politely.";
    let goal = "Find the latest news about {topic} for {audience} readers.";
    let dynamic = prompt(
        "support/today",
        F::Dynamic,
        "() => `Today is ${new Date()}`",
    );
    let first = management::DeploymentDeclarations {
        prompts: vec![
            prompt("support/instructions", F::Plain, instructions),
            prompt("agents/researcher/goal", F::Braces, goal),
            dynamic.clone(),
            // The same content found twice is declared once.
            dynamic.clone(),
        ],
        skills: vec![bundled_skill("refunds", "Refund within 30 days.")],
        ..Default::default()
    };
    let (d1, created) = deploy(&deployments, agent, "ci:1", first).await;
    assert!(created);
    let contents = post(
        format!("{management}/tilde.management.v1.DeploymentService/GetDeploymentContents"),
        None,
        json!({"agentId":agent,"deploymentId":d1}),
    )
    .await;
    let shipped: Vec<(String, String, String)> = contents["prompts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["name"].as_str().unwrap().to_owned(),
                p["version"]["format"].as_str().unwrap().to_owned(),
                p["version"]["variables"].to_string(),
            )
        })
        .collect();
    assert_eq!(
        shipped,
        [
            (
                "agents/researcher/goal".into(),
                "PROMPT_FORMAT_BRACES".into(),
                r#"["topic","audience"]"#.into()
            ),
            (
                "support/instructions".into(),
                "PROMPT_FORMAT_PLAIN".into(),
                "null".into()
            ),
            (
                "support/today".into(),
                "PROMPT_FORMAT_DYNAMIC".into(),
                "null".into()
            ),
        ]
    );
    assert_eq!(
        contents["prompts"][1]["version"]["origin"],
        "src/agent.ts#support/instructions"
    );
    assert_eq!(contents["skills"][0]["name"], "refunds");
    assert_eq!(contents["skills"][0]["origin"], "skills/refunds");
    let d1_skill = contents["skills"][0]["versionId"].clone();

    // One call renders the plain and braces prompts and stamps the dynamic one.
    let (token, invocation_id) = invocation(&chat, &db.pool, agent).await;
    pin(&db.pool, invocation_id, d1).await;
    let response = http
        .post(format!("{runtime}/inference/openai/prod/chat/completions"))
        .bearer_auth(token.expose_secret())
        .header(
            "x-tilde-prompt",
            format!("support/today@{}, garbage", dynamic.hash),
        )
        .json(&json!({"model":"gpt-5","messages":[
            {"role":"system","content":instructions},
            {"role":"user","content":[{"type":"text","text":"Find the latest news about tides for sailing readers."}]}
        ]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    response.bytes().await.unwrap();
    gateway.audit.flush().await.unwrap();
    let prompts = Prompts::new(db.pool.clone());
    let listed = prompts.list(agent).await.unwrap();
    assert_eq!(listed.len(), 3);
    for p in &listed {
        let (_, versions, usage) = prompts.get(p.id).await.unwrap();
        assert_eq!(versions[0].deployment_id, Some(d1));
        assert_eq!(usage.len(), 1, "{} was not linked", p.name);
        assert_eq!(
            (
                usage[0].requests,
                usage[0].input_tokens,
                usage[0].output_tokens
            ),
            (1, 10, 5)
        );
    }

    // A repeat of the external id changes nothing; a changed prompt is a second version.
    let changed = management::DeploymentDeclarations {
        prompts: vec![
            prompt(
                "support/instructions",
                F::Plain,
                "You are the support agent for Acme. Answer briefly.",
            ),
            prompt("agents/researcher/goal", F::Braces, goal),
        ],
        skills: vec![bundled_skill("refunds", "Refund within 14 days.")],
        ..Default::default()
    };
    assert_eq!(
        deploy(&deployments, agent, "ci:1", changed.clone()).await,
        (d1, false)
    );
    let (d2, _) = deploy(&deployments, agent, "ci:2", changed).await;
    let instructions_prompt = listed
        .iter()
        .find(|p| p.name == "support/instructions")
        .unwrap();
    let (_, versions, _) = prompts.get(instructions_prompt.id).await.unwrap();
    assert_eq!(
        versions
            .iter()
            .map(|v| (v.number, v.deployment_id))
            .collect::<Vec<_>>(),
        [(2, Some(d2)), (1, Some(d1))]
    );
    let first_contents = deployments.contents(agent, d1).await.unwrap();
    assert_eq!(first_contents.prompts.len(), 3);
    assert_eq!(
        first_contents
            .prompts
            .iter()
            .find(|p| p.name == "support/instructions")
            .unwrap()
            .version
            .number,
        1
    );
    let second_contents = deployments.contents(agent, d2).await.unwrap();
    // The unchanged goal is the same version in both deployments.
    let goal_version = |c: &management::GetDeploymentContentsResponse| {
        c.prompts
            .iter()
            .find(|p| p.name == "agents/researcher/goal")
            .unwrap()
            .version
            .id
            .clone()
    };
    assert_eq!(
        goal_version(&first_contents),
        goal_version(&second_contents)
    );

    // Each invocation reads the skill version its deployment shipped, over an assigned skill
    // of the same name; another agent's listing never sees bundled skills.
    let skills = Skills::new(db.pool.clone());
    let drafts = skills.create_editor("Drafts").await.unwrap();
    let (assigned, written) = skills
        .write(
            drafts,
            "refunds",
            vec![text("SKILL.md", &skill_md("refunds", "Assigned", ""))],
            "",
            true,
        )
        .await
        .unwrap();
    skills.assign_skill(agent, assigned).await.unwrap();
    let (second_token, second_invocation) = invocation(&chat, &db.pool, agent).await;
    pin(&db.pool, second_invocation, d2).await;
    let d2_skill = second_contents.skills[0].version_id.clone();
    assert_ne!(json!(d2_skill), d1_skill);
    for (token, expected) in [(&token, d1_skill), (&second_token, json!(d2_skill))] {
        let listed = post(
            format!("{runtime}/tilde.runtime.v1.SkillService/ListSkills"),
            Some(token.expose_secret().to_owned()),
            json!({}),
        )
        .await;
        let refunds: Vec<_> = listed["skills"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["name"] == "refunds")
            .collect();
        assert_eq!(refunds.len(), 1);
        assert_eq!(refunds[0]["versionId"], expected);
        let read = post(
            format!("{runtime}/tilde.runtime.v1.SkillService/ReadSkillFile"),
            Some(token.expose_secret().to_owned()),
            json!({"name":"refunds","path":"SKILL.md"}),
        )
        .await;
        assert!(read["content"].as_str().unwrap().contains("Refund within"));
    }
    let outside = skills.for_agent(agent, None).await.unwrap();
    assert_eq!(outside.len(), 1);
    assert_eq!(outside[0].version_id, written.version);
    assert!(!outside[0].deployed);
    // Bundled skills are never listed or given out.
    assert_eq!(
        skills.sources().await.unwrap().len(),
        1,
        "only the editor source"
    );
    let bundled_skill_id = Uuid::parse_str(&second_contents.skills[0].skill_id).unwrap();
    assert!(skills.assign_skill(agent, bundled_skill_id).await.is_err());
    db.close().await;
}

/// A GitHub REST API stand-in serving one repository whose files the test changes between
/// syncs. Blob ids are content digests, as git's are.
#[derive(Default)]
struct Repo {
    commit: u32,
    files: std::collections::BTreeMap<String, (Vec<u8>, &'static str)>,
    broken: bool,
    /// Requests served, to show which calls reach GitHub.
    requests: usize,
    /// Pauses the next file download: signals the first, waits on the second.
    hold: Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
}
type SharedRepo = Arc<std::sync::Mutex<Repo>>;
async fn github(repo: SharedRepo) -> String {
    use axum::extract::Path;
    use axum::routing::get;
    fn blob_id(data: &[u8]) -> String {
        hex::encode(sha2::Sha256::digest(data))
    }
    use sha2::Digest;
    async fn commit(State(repo): State<SharedRepo>) -> axum::response::Response {
        let mut repo = repo.lock().unwrap();
        repo.requests += 1;
        if repo.broken {
            return (http::StatusCode::NOT_FOUND, "{}").into_response();
        }
        axum::Json(json!({"sha": format!("{:040x}", repo.commit)})).into_response()
    }
    async fn tree(
        State(repo): State<SharedRepo>,
        axum::extract::Query(query): axum::extract::Query<
            std::collections::HashMap<String, String>,
        >,
    ) -> axum::response::Response {
        // GitHub lists nested paths only when asked to.
        if query.get("recursive").map(String::as_str) != Some("1") {
            return http::StatusCode::BAD_REQUEST.into_response();
        }
        let mut repo = repo.lock().unwrap();
        repo.requests += 1;
        let tree: Vec<_> = repo
            .files
            .iter()
            .map(|(path, (data, mode))| {
                json!({"path":path,"mode":mode,"type":"blob","sha":blob_id(data),"size":data.len()})
            })
            .chain([json!({"path":"skills","mode":"040000","type":"tree","sha":"t"})])
            .collect();
        axum::Json(json!({"tree":tree,"truncated":false})).into_response()
    }
    async fn raw(
        State(repo): State<SharedRepo>,
        Path((_, _, commit, path)): Path<(String, String, String, String)>,
    ) -> axum::response::Response {
        let hold = repo.lock().unwrap().hold.take();
        if let Some((reached, release)) = hold {
            reached.notify_one();
            release.notified().await;
        }
        let mut repo = repo.lock().unwrap();
        repo.requests += 1;
        // Raw content is pinned to the commit the sync resolved.
        if commit != format!("{:040x}", repo.commit) {
            return http::StatusCode::NOT_FOUND.into_response();
        }
        match repo.files.get(&path) {
            Some((data, _)) => data.clone().into_response(),
            None => http::StatusCode::NOT_FOUND.into_response(),
        }
    }
    let app = Router::new()
        .route("/repos/{owner}/{repo}/commits/{git_ref}", get(commit))
        .route("/repos/{owner}/{repo}/git/trees/{sha}", get(tree))
        .route("/raw/{owner}/{repo}/{commit}/{*path}", get(raw))
        .with_state(repo);
    serve(app).await
}
fn text(path: &str, content: &str) -> Upload {
    Upload::Data {
        path: path.into(),
        data: content.as_bytes().to_vec(),
        executable: false,
    }
}
fn skill_md(name: &str, description: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n{body}")
}

#[tokio::test]
async fn skill_sources_sync_version_and_reach_agents_directly_and_through_connections() {
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(22)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let agent = create_agent(&agents).await;
    let repo: SharedRepo = Arc::default();
    let stub = github(repo.clone()).await;
    let skills = Skills::new(db.pool.clone()).with_github(
        tilde::skills::github::GitHub::new(&stub, &format!("{stub}/raw"), None).unwrap(),
    );

    // Catalog: enabling a group creates one source, synced from the binary.
    assert!(
        skills
            .catalog()
            .await
            .unwrap()
            .iter()
            .any(|e| e.group.id == "tilde-runtime" && e.source.is_none() && e.skills.len() == 4)
    );
    let (runtime, created) = skills.enable_catalog("tilde-runtime").await.unwrap();
    assert!(created);
    let (again, created) = skills.enable_catalog("tilde-runtime").await.unwrap();
    assert!(!created && again == runtime);
    // Concurrent enables of one group agree on a single source.
    let (a, b) = tokio::join!(
        skills.enable_catalog("tilde-working-style",),
        skills.enable_catalog("tilde-working-style",)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(a.0 == b.0 && a.1 != b.1, "{a:?} {b:?}");
    skills.delete_source(a.0).await.unwrap();
    assert_eq!(skills.source(runtime).await.unwrap().skill_count, 4);
    assert_eq!(skills.skills(None,).await.unwrap().len(), 4);
    // Catalog skills follow the catalog: even the source's editor cannot change or delete them.
    let first = skills.skills(Some(runtime)).await.unwrap()[0].row.id;
    assert!(
        skills
            .update(first, vec![text("SKILL.md", "changed")])
            .await
            .is_err()
    );
    assert!(skills.delete_skill(first).await.is_err());
    // Startup reconciliation re-syncs a catalog skill whose stored copy drifted.
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE skill_versions SET hash='\\x00'::bytea WHERE skill_id=(SELECT id FROM skills WHERE source_id=$1 AND name='tilde-channels')",
            &[&runtime],
        )
        .await
        .unwrap();
    skills.reconcile().await.unwrap();
    skills.reconcile().await.unwrap();
    let channels = skills
        .skills(Some(runtime))
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.row.name == "tilde-channels")
        .unwrap();
    assert_eq!(skills.versions(channels.row.id).await.unwrap().len(), 2);
    // A description stored by an older SKILL.md reader is corrected in place, with no version.
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE skill_versions SET description='>-' WHERE skill_id=$1",
            &[&channels.row.id],
        )
        .await
        .unwrap();
    skills.reconcile().await.unwrap();
    let channels = skills.skill(channels.row.id).await.unwrap();
    assert_eq!(skills.versions(channels.row.id).await.unwrap().len(), 2);
    assert_ne!(channels.latest.unwrap().description, ">-");

    // Git: a binary asset without object storage fails the sync, which is recorded, not raised.
    {
        let mut repo = repo.lock().unwrap();
        repo.commit = 1;
        for (path, data, mode) in [
            ("README.md", b"# repo".to_vec(), "100644"),
            (
                "skills/pdf/SKILL.md",
                skill_md("pdf", "Fill PDF forms", "Use forms.md").into_bytes(),
                "100644",
            ),
            ("skills/pdf/forms.md", b"fields".to_vec(), "100644"),
            ("skills/pdf/scripts/fill.py", b"print(1)".to_vec(), "100755"),
            (
                "skills/pdf/logo.png",
                vec![0x89, 0x50, 0xff, 0xfe],
                "100644",
            ),
            (
                "skills/docx/SKILL.md",
                skill_md("docx", "Edit Word files", "").into_bytes(),
                "100644",
            ),
            (
                "other/SKILL.md",
                skill_md("other", "Outside the path", "").into_bytes(),
                "100644",
            ),
        ] {
            repo.files.insert(path.into(), (data, mode));
        }
    }
    assert!(
        skills
            .add_git("Docs", "https://example.com/acme/skills", "main", "",)
            .await
            .is_err(),
        "only GitHub URLs"
    );
    let git = skills
        .add_git("Doc tools", "https://github.com/acme/skills", "", "skills")
        .await
        .unwrap();
    let source = skills.source(git).await.unwrap();
    assert_eq!(
        (source.slug.as_str(), source.git_ref.as_deref()),
        ("doc-tools", Some("main"))
    );
    assert!(
        source
            .sync_error
            .unwrap()
            .contains("ENGINE_SKILLS_S3_BUCKET")
    );
    assert_eq!(source.skill_count, 0);
    repo.lock().unwrap().files.remove("skills/pdf/logo.png");
    repo.lock().unwrap().commit = 2;
    skills.sync(git).await.unwrap();
    let source = skills.source(git).await.unwrap();
    assert_eq!(source.sync_error, None);
    assert_eq!(
        source.commit_sha.as_deref(),
        Some(format!("{:040x}", 2).as_str())
    );
    let names =
        |list: Vec<tilde::skills::Skill>| list.into_iter().map(|s| s.row.name).collect::<Vec<_>>();
    assert_eq!(
        names(skills.skills(Some(git),).await.unwrap()),
        ["docx", "pdf"]
    );
    let pdf = skills
        .skills(Some(git))
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.row.name == "pdf")
        .unwrap();
    let version = skills.version(pdf.latest.unwrap().id).await.unwrap();
    assert_eq!(
        version
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.executable))
            .collect::<Vec<_>>(),
        [
            ("SKILL.md", false),
            ("forms.md", false),
            ("scripts/fill.py", true)
        ]
    );
    assert_eq!(version.row.description, "Fill PDF forms");
    // A push changes pdf and drops docx; an unchanged skill keeps its version.
    {
        let mut repo = repo.lock().unwrap();
        repo.commit = 3;
        repo.files.insert(
            "skills/pdf/forms.md".into(),
            (b"fields v2".to_vec(), "100644"),
        );
        repo.files.remove("skills/docx/SKILL.md");
    }
    skills.sync(git).await.unwrap();
    let pdf = skills.skill(pdf.row.id).await.unwrap();
    assert_eq!(skills.versions(pdf.row.id).await.unwrap().len(), 2);
    assert_eq!(names(skills.skills(Some(git),).await.unwrap()), ["pdf"]);
    // A failing sync keeps the last good skills and says why.
    repo.lock().unwrap().broken = true;
    skills.sync(git).await.unwrap();
    let source = skills.source(git).await.unwrap();
    assert!(source.sync_error.is_some() && source.skill_count == 1);
    repo.lock().unwrap().broken = false;
    // A sync that finishes after a newer one started changes nothing.
    let (reached, release) = (
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
    );
    {
        let mut repo = repo.lock().unwrap();
        repo.commit = 4;
        repo.files.insert(
            "skills/pdf/forms.md".into(),
            (b"fields v3".to_vec(), "100644"),
        );
        repo.hold = Some((reached.clone(), release.clone()));
    }
    let newer = async {
        reached.notified().await;
        db.pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE skill_sources SET sync_generation=sync_generation+1 WHERE id=$1",
                &[&git],
            )
            .await
            .unwrap();
        release.notify_one();
    };
    let (older, ()) = tokio::join!(skills.sync(git), newer);
    older.unwrap();
    let source = skills.source(git).await.unwrap();
    assert_eq!(
        source.commit_sha.as_deref(),
        Some(format!("{:040x}", 3).as_str())
    );
    assert!(
        source.sync_error.is_some(),
        "the stale outcome is not recorded"
    );
    assert_eq!(skills.versions(pdf.row.id).await.unwrap().len(), 2);
    repo.lock().unwrap().files.insert(
        "skills/pdf/forms.md".into(),
        (b"fields v2".to_vec(), "100644"),
    );
    skills.sync(git).await.unwrap();
    let source = skills.source(git).await.unwrap();
    assert_eq!(source.sync_error, None);
    assert_eq!(skills.versions(pdf.row.id).await.unwrap().len(), 2);

    // Editor: skills are authored, revised by path, and only here.
    let drafts = skills.create_editor("Drafts").await.unwrap();
    let policy = skill_md("refunds", "Handle refunds", "Read reference/policy.md");
    let (refunds, first) = skills
        .write(
            drafts,
            "refunds",
            vec![
                text("SKILL.md", &policy),
                text("reference/policy.md", "30 days"),
            ],
            "",
            true,
        )
        .await
        .unwrap();
    assert!(first.created && first.number == 1);
    assert!(
        skills
            .write(drafts, "refunds", vec![text("SKILL.md", &policy)], "", true)
            .await
            .is_err(),
        "create refuses an existing name"
    );
    let (_, same) = skills
        .write(
            drafts,
            "refunds",
            vec![
                text("SKILL.md", &policy),
                Upload::Keep {
                    path: "reference/policy.md".into(),
                    from: None,
                },
            ],
            "noop",
            false,
        )
        .await
        .unwrap();
    assert!(!same.created && same.number == 1);
    skills
        .update(
            refunds,
            vec![
                text("SKILL.md", &format!("{policy}\nBe kind.")),
                Upload::Keep {
                    path: "reference/policy.md".into(),
                    from: None,
                },
            ],
        )
        .await
        .unwrap();
    let revised = skills
        .version(skills.skill(refunds).await.unwrap().latest.unwrap().id)
        .await
        .unwrap();
    assert_eq!(revised.row.number, 2);
    assert_eq!(revised.row.message, "Updated");
    assert_eq!(revised.files[1].content.as_deref(), Some("30 days"));
    // Editing the front matter name renames the skill in place; its id and history stay.
    let keep = || Upload::Keep {
        path: "reference/policy.md".into(),
        from: None,
    };
    let renamed = skill_md(
        "refund-policy",
        "Handle refunds",
        "Read reference/policy.md",
    );
    skills
        .update(refunds, vec![text("SKILL.md", &renamed), keep()])
        .await
        .unwrap();
    let skill = skills.skill(refunds).await.unwrap();
    assert_eq!(
        (skill.row.name.as_str(), skill.latest.unwrap().number),
        ("refund-policy", 3)
    );
    let bad = skill_md("Not Valid", "Handle refunds", "");
    assert!(
        skills
            .update(refunds, vec![text("SKILL.md", &bad), keep()])
            .await
            .is_err()
    );
    skills
        .update(
            refunds,
            vec![text("SKILL.md", &format!("{policy}\nBe kind.")), keep()],
        )
        .await
        .unwrap();
    assert_eq!(skills.skill(refunds).await.unwrap().row.name, "refunds");
    // A kept file moves to a new path without its bytes being sent again.
    let body = || text("SKILL.md", &format!("{policy}\nBe kind."));
    let moved = |to: &str, from: &str| Upload::Keep {
        path: to.into(),
        from: Some(from.into()),
    };
    skills
        .update(
            refunds,
            vec![body(), moved("docs/policy.md", "reference/policy.md")],
        )
        .await
        .unwrap();
    let version = skills
        .version(skills.skill(refunds).await.unwrap().latest.unwrap().id)
        .await
        .unwrap();
    assert_eq!(version.files[1].path, "docs/policy.md");
    assert_eq!(version.files[1].content.as_deref(), Some("30 days"));
    skills
        .update(
            refunds,
            vec![body(), moved("reference/policy.md", "docs/policy.md")],
        )
        .await
        .unwrap();
    assert!(
        skills
            .write(
                drafts,
                "logo",
                vec![
                    text("SKILL.md", &policy),
                    Upload::Data {
                        path: "logo.png".into(),
                        data: vec![0xff, 0xfe],
                        executable: false
                    }
                ],
                "",
                true
            )
            .await
            .is_err(),
        "binary files need object storage"
    );
    assert!(
        skills
            .write(git, "x", vec![text("SKILL.md", &policy)], "", true)
            .await
            .is_err()
    );
    assert!(
        skills.delete_skill(pdf.row.id).await.is_err(),
        "git skills follow their repository"
    );

    // Agents: whole sources and single skills; a shared name needs its source.
    // A group arrives switched off: it is listed but gives the agent none of its skills.
    skills.assign_source(agent, git).await.unwrap();
    skills.assign_skill(agent, refunds).await.unwrap();
    assert!(skills.assign_skill(agent, Uuid::new_v4()).await.is_err());
    let names = |given: Vec<tilde::skills::RuntimeSkill>| {
        given.into_iter().map(|s| s.name).collect::<Vec<_>>()
    };
    assert_eq!(
        names(skills.for_agent(agent, None).await.unwrap()),
        ["refunds"]
    );
    assert_eq!(skills.disabled_sources(agent).await.unwrap(), [git]);
    assert!(skills.enable_source(agent, drafts, true).await.is_err());
    skills.enable_source(agent, git, true).await.unwrap();
    let given = skills.for_agent(agent, None).await.unwrap();
    assert_eq!(
        given
            .iter()
            .map(|s| (s.name.as_str(), s.source.as_str()))
            .collect::<Vec<_>>(),
        [("pdf", "doc-tools"), ("refunds", "drafts")]
    );
    // In an enabled group each skill switches off on its own; switching the group off and on
    // again turns every skill back on.
    let names = |given: Vec<tilde::skills::RuntimeSkill>| {
        given.into_iter().map(|s| s.name).collect::<Vec<_>>()
    };
    skills.enable_skill(agent, pdf.row.id, false).await.unwrap();
    assert_eq!(
        names(skills.for_agent(agent, None).await.unwrap()),
        ["refunds"]
    );
    assert_eq!(skills.excluded_skills(agent).await.unwrap(), [pdf.row.id]);
    skills.enable_source(agent, git, false).await.unwrap();
    skills.enable_source(agent, git, true).await.unwrap();
    assert_eq!(
        names(skills.for_agent(agent, None).await.unwrap()),
        ["pdf", "refunds"]
    );
    assert!(skills.excluded_skills(agent).await.unwrap().is_empty());
    assert!(matches!(
        skills.read(agent, None, "pdf", "forms.md").await.unwrap(),
        tilde::skills::FileBody::Text(ref t, _) if t == "fields v2"
    ));
    let (shadow, _) = skills
        .write(
            drafts,
            "pdf",
            vec![text("SKILL.md", &skill_md("pdf", "Another pdf", ""))],
            "",
            true,
        )
        .await
        .unwrap();
    skills.assign_skill(agent, shadow).await.unwrap();
    assert!(
        skills.read(agent, None, "pdf", "SKILL.md").await.is_err(),
        "ambiguous name"
    );
    assert!(
        skills
            .read(agent, None, "doc-tools/pdf", "SKILL.md")
            .await
            .is_ok()
    );
    assert!(
        skills
            .read(agent, None, "refunds", "../secret")
            .await
            .is_err()
    );
    let (sources, singles) = skills.agent_skills(agent).await.unwrap();
    assert_eq!((sources.len(), singles.len()), (1, 2));
    // Removing a group drops the skills from it that were switched on one by one.
    skills.unassign_source(agent, drafts).await.unwrap();
    let (sources, singles) = skills.agent_skills(agent).await.unwrap();
    assert_eq!((sources.len(), singles.len()), (1, 0));
    skills.assign_skill(agent, refunds).await.unwrap();

    // Connections: a linked source reaches agents holding the connection's skills capability.
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let helper = create_agent(&agents).await;
    let connection = connect(&connections, helper, &provider().await).await;
    skills.link_source(connection, runtime, true).await.unwrap();
    assert_eq!(
        skills.connection_sources(connection,).await.unwrap().len(),
        1
    );
    let learner = create_agent(&agents).await;
    assert!(skills.for_agent(learner, None).await.unwrap().is_empty());
    let grant = Assignment {
        capability: Capability::Skills,
        agent_id: learner,
        alias: None,
    };
    connections.assign(connection, &grant).await.unwrap();
    assert_eq!(skills.for_agent(learner, None).await.unwrap().len(), 4);
    assert_eq!(
        skills.agent_connections(learner).await.unwrap()[0].id,
        connection
    );
    assert!(
        connections
            .assign(
                connection,
                &Assignment {
                    alias: Some("x".into()),
                    ..grant.clone()
                }
            )
            .await
            .is_err(),
        "skills assignments take no alias"
    );
    connections.unassign(connection, &grant).await.unwrap();
    assert!(skills.for_agent(learner, None).await.unwrap().is_empty());

    // Deleting a source takes its skills and assignments.
    skills.delete_source(git).await.unwrap();
    assert!(
        skills
            .for_agent(agent, None)
            .await
            .unwrap()
            .iter()
            .all(|s| s.source != "doc-tools")
    );
    db.close().await;
}

/// Moving an editor skill keeps its id, history and single-skill assignments; the groups it
/// leaves and joins decide which whole-source assignments reach it.
/// Managed providers sync their trusted repository through the GitHub client and keep only the
/// skills within their scopes: Zoom includes `skills/` (and the file `.mcp.json`), Cursor
/// excludes `third_party/`. Before enabling, a provider's panel previews the same skills live,
/// storing nothing and reading GitHub once an hour. Startup reconciliation leaves them alone.
#[tokio::test]
async fn managed_providers_sync_only_skills_within_their_scopes() {
    let db = Database::new().await;
    let repo: SharedRepo = Arc::default();
    {
        let mut repo = repo.lock().unwrap();
        repo.commit = 1;
        for (path, content) in [
            (".mcp.json", "{}".to_owned()),
            (
                "skills/meetings/SKILL.md",
                skill_md("meetings", "Run meetings", ""),
            ),
            ("skills/meetings/notes.md", "notes".to_owned()),
            ("third_party/crm/SKILL.md", skill_md("crm", "Vendored", "")),
            ("other/SKILL.md", skill_md("other", "Outside skills/", "")),
        ] {
            repo.files
                .insert(path.into(), (content.into_bytes(), "100644"));
        }
    }
    let stub = github(repo.clone()).await;
    let skills = Skills::new(db.pool.clone()).with_github(
        tilde::skills::github::GitHub::new(&stub, &format!("{stub}/raw"), None).unwrap(),
    );
    let management = tilde::skills::rpc::management_router(skills.clone());
    let management = serve(management).await;
    let http = reqwest::Client::new();
    let catalog = || async {
        let groups: serde_json::Value = http
            .post(format!(
                "{management}/tilde.management.v1.SkillService/ListCatalog"
            ))
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        groups["groups"].as_array().unwrap().clone()
    };
    let entry = |groups: &[serde_json::Value], id: &str| {
        groups.iter().find(|g| g["id"] == id).unwrap().clone()
    };
    let groups = catalog().await;
    assert_eq!(
        groups.len(),
        20,
        "3 built-in groups and 17 managed providers"
    );
    let zoom = entry(&groups, "zoom");
    assert_eq!(
        (&zoom["category"], &zoom["iconUrl"], &zoom["repositoryUrl"]),
        (
            &json!("Productivity"),
            &json!("/skill-provider-icons/zoom.svg"),
            &json!("https://github.com/zoom/skills")
        )
    );
    assert!(zoom.get("skills").is_none() && zoom.get("sourceId").is_none());
    let runtime = entry(&groups, "tilde-runtime");
    assert_eq!(
        (&runtime["category"], &runtime["iconUrl"]),
        (&json!("Tilde"), &json!("/tilde-mark.svg"))
    );
    assert_eq!(runtime["skills"].as_array().unwrap().len(), 4);

    let get_group = |id: &'static str| {
        let http = http.clone();
        let management = management.clone();
        async move {
            let response = http
                .post(format!(
                    "{management}/tilde.management.v1.SkillService/GetCatalogGroup"
                ))
                .json(&json!({ "id": id }))
                .send()
                .await
                .unwrap();
            (
                response.status(),
                response.json::<serde_json::Value>().await.unwrap(),
            )
        }
    };
    // A preview reads the commit, the tree and only each in-scope SKILL.md: 3 + 4 requests.
    let (status, preview) = get_group("zoom").await;
    assert!(status.is_success(), "{preview}");
    let preview = &preview["group"];
    assert_eq!(
        (&preview["branch"], preview.get("sourceId")),
        (&json!("main"), None)
    );
    assert_eq!(
        preview["skills"],
        json!([{"name": "meetings", "description": "Run meetings", "path": "skills/meetings"}])
    );
    let (_, cursor) = get_group("cursor").await;
    assert_eq!(
        cursor["group"]["skills"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["meetings", "other"]
    );
    let served = repo.lock().unwrap().requests;
    assert_eq!(served, 7);
    assert!(skills.sources().await.unwrap().is_empty());
    // Reopening a panel within the hour is served from memory.
    assert_eq!(
        get_group("zoom").await.1["group"]["skills"],
        preview["skills"]
    );
    assert_eq!(repo.lock().unwrap().requests, served);

    let names =
        |list: Vec<tilde::skills::Skill>| list.into_iter().map(|s| s.row.name).collect::<Vec<_>>();
    let (zoom_source, _) = skills.enable_catalog("zoom").await.unwrap();
    let (cursor_source, _) = skills.enable_catalog("cursor").await.unwrap();
    let source = skills.source(zoom_source).await.unwrap();
    assert_eq!(source.sync_error, None);
    assert_eq!(source.commit_sha, Some(format!("{:040x}", 1)));
    assert_eq!(
        names(skills.skills(Some(zoom_source),).await.unwrap()),
        ["meetings"]
    );
    assert_eq!(
        names(skills.skills(Some(cursor_source),).await.unwrap()),
        ["meetings", "other"]
    );
    let zoom = entry(&catalog().await, "zoom");
    assert_eq!(zoom["sourceId"], json!(zoom_source.to_string()));
    assert_eq!(
        zoom["skills"],
        json!([{"name": "meetings", "description": "Run meetings", "path": "skills/meetings"}])
    );
    // Once enabled, the panel shows the synced skills.
    assert_eq!(
        get_group("zoom").await.1["group"]["sourceId"],
        json!(zoom_source.to_string())
    );

    // Startup never fetches a provider; the hourly worker and manual syncs do.
    repo.lock().unwrap().broken = true;
    skills.reconcile().await.unwrap();
    assert_eq!(skills.source(zoom_source).await.unwrap().sync_error, None);
    skills.sync(zoom_source).await.unwrap();
    assert!(
        skills
            .source(zoom_source)
            .await
            .unwrap()
            .sync_error
            .is_some()
    );
    // A preview GitHub refuses is an error the panel shows.
    let (status, error) = get_group("stripe").await;
    assert!(!status.is_success());
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("GitHub has no such repository"),
        "{error}"
    );
    db.close().await;
}

#[tokio::test]
async fn runtime_capabilities_gate_prompt_and_skill_management() {
    use std::collections::BTreeMap;
    use tilde::iam::capabilities::{Capabilities, Capability, Reach};
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(23)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let skills = Skills::new(db.pool.clone());
    let (catalog, _) = skills.enable_catalog("tilde-runtime").await.unwrap();
    let notes = skills.create_editor("Agent notes").await.unwrap();
    let plain = create_agent(&agents).await;
    let other = create_agent(&agents).await;
    let manager = create_agent_with(
        &agents,
        Capabilities(BTreeMap::from([
            (Capability::AgentsRead, Reach::All),
            (
                Capability::AgentsEditSkills,
                Reach::Selected {
                    ids: vec![other.to_string()],
                },
            ),
            (
                Capability::SkillsRead,
                Reach::Selected {
                    ids: vec![catalog.to_string()],
                },
            ),
            (
                Capability::SkillsEdit,
                Reach::Selected {
                    ids: vec![notes.to_string()],
                },
            ),
        ])),
    )
    .await;
    // Edits other's skills but reads no source.
    let blind = create_agent_with(
        &agents,
        Capabilities(BTreeMap::from([(
            Capability::AgentsEditSkills,
            Reach::Selected {
                ids: vec![other.to_string()],
            },
        )])),
    )
    .await;
    let chat = Chat::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
    );
    let prompts = Prompts::new(db.pool.clone());
    let url = serve(tilde::iam::listeners::agent_rpc_router(
        agents.clone(),
        chat.clone(),
    ))
    .await;
    let http = reqwest::Client::new();
    let call = |token: SecretString, path: &'static str, body: serde_json::Value| {
        let http = http.clone();
        let url = url.clone();
        async move {
            let response = http
                .post(format!("{url}/tilde.runtime.v1.{path}"))
                .bearer_auth(token.expose_secret())
                .json(&body)
                .send()
                .await
                .unwrap();
            (
                response.status().as_u16(),
                response
                    .json::<serde_json::Value>()
                    .await
                    .unwrap_or_default(),
            )
        }
    };
    let (plain_token, _) = invocation(&chat, &db.pool, plain).await;
    let (manager_token, _) = invocation(&chat, &db.pool, manager).await;
    let (blind_token, _) = invocation(&chat, &db.pool, blind).await;
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    deploy(
        &deployments(&db.pool, encryption.clone(), connections),
        plain,
        "ci:1",
        management::DeploymentDeclarations {
            prompts: vec![prompt(
                "triage",
                types::PromptFormat::Plain,
                "Triage the request.",
            )],
            ..Default::default()
        },
    )
    .await;
    let prompt = prompts.list(plain).await.unwrap()[0].id.to_string();
    let status = |r: (u16, serde_json::Value)| r.0;
    // A plain agent reads its own prompts and skills and reaches nothing else.
    assert_eq!(
        status(call(plain_token.clone(), "PromptService/ListPrompts", json!({})).await),
        200
    );
    assert_eq!(
        status(
            call(
                plain_token.clone(),
                "PromptService/ListPrompts",
                json!({"agentId":other})
            )
            .await
        ),
        403
    );
    assert_eq!(
        status(call(plain_token.clone(), "SkillService/ListSkills", json!({})).await),
        200
    );
    assert_eq!(
        status(
            call(
                plain_token.clone(),
                "SkillService/AssignSkill",
                json!({"source":"tilde-runtime"})
            )
            .await
        ),
        403
    );
    let slugs = |r: (u16, serde_json::Value)| -> Vec<String> {
        assert_eq!(r.0, 200, "{}", r.1);
        r.1["sources"]
            .as_array()
            .map(|sources| {
                sources
                    .iter()
                    .map(|s| s["slug"].as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    assert!(
        slugs(
            call(
                plain_token.clone(),
                "SkillService/ListSkillSources",
                json!({})
            )
            .await
        )
        .is_empty()
    );
    // Without skills.read on a source an agent neither sees nor hands it out, even to an agent
    // whose skills it edits.
    assert!(
        slugs(
            call(
                blind_token.clone(),
                "SkillService/ListSkillSources",
                json!({})
            )
            .await
        )
        .is_empty()
    );
    for body in [
        json!({"agentId":other,"source":"tilde-runtime"}),
        json!({"agentId":other,"skill":"tilde-runtime/tilde-channels"}),
    ] {
        assert_eq!(
            status(call(blind_token.clone(), "SkillService/AssignSkill", body).await),
            403
        );
    }
    assert_eq!(
        status(call(plain_token.clone(), "SkillService/WriteSkill", json!({"source":"agent-notes","name":"x","files":[{"path":"SKILL.md","content":"x"}]})).await),
        403
    );
    // The manager assigns only on its selected target, by source or by skill.
    assert_eq!(
        status(
            call(
                manager_token.clone(),
                "SkillService/AssignSkill",
                json!({"source":"tilde-runtime"})
            )
            .await
        ),
        403,
        "itself is not among the selected targets"
    );
    assert_eq!(
        status(
            call(
                manager_token.clone(),
                "SkillService/AssignSkill",
                json!({"agentId":other,"source":"tilde-runtime"})
            )
            .await
        ),
        200
    );
    assert_eq!(
        status(
            call(
                manager_token.clone(),
                "SkillService/AssignSkill",
                json!({"agentId":other,"source":"nope"})
            )
            .await
        ),
        404
    );
    assert_eq!(skills.agent_skills(other).await.unwrap().0[0].id, catalog);
    let listed = call(
        manager_token.clone(),
        "SkillService/ListSkillSources",
        json!({}),
    )
    .await
    .1;
    // skills.read and skills.edit targets, and nothing else.
    let mut listed_slugs: Vec<&str> = listed["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["slug"].as_str().unwrap())
        .collect();
    listed_slugs.sort();
    assert_eq!(listed_slugs, ["agent-notes", "tilde-runtime"]);
    assert!(
        listed["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["slug"] == "tilde-runtime" && s["skills"].as_array().unwrap().len() == 4)
    );
    assert_eq!(
        call(
            manager_token.clone(),
            "SkillService/ListSkills",
            json!({"agentId":other})
        )
        .await
        .1["skills"]
            .as_array()
            .map(Vec::len),
        Some(4)
    );
    let read = call(
        manager_token.clone(),
        "SkillService/ReadSkillFile",
        json!({"agentId":other,"name":"tilde-channels","path":"SKILL.md"}),
    )
    .await;
    assert_eq!(read.0, 200);
    assert!(read.1["content"].as_str().unwrap().contains("channel"));
    // skills.edit lets it author into the source it targets, which it may then assign.
    let written = call(
        manager_token.clone(),
        "SkillService/WriteSkill",
        json!({"source":"agent-notes","name":"customer-faq","files":[{"path":"SKILL.md","content":"---\ndescription: Answers\n---\n# FAQ"}]}),
    )
    .await;
    assert_eq!(written.0, 200, "{}", written.1);
    assert_eq!(written.1["created"], true);
    assert_eq!(
        status(call(manager_token.clone(), "SkillService/WriteSkill", json!({"source":"tilde-runtime","name":"x","files":[{"path":"SKILL.md","content":"x"}]})).await),
        403,
        "skills.edit reaches only its targets"
    );
    assert_eq!(
        status(
            call(
                manager_token.clone(),
                "SkillService/AssignSkill",
                json!({"agentId":other,"skill":"agent-notes/customer-faq"})
            )
            .await
        ),
        200
    );
    assert!(
        skills
            .for_agent(other, None)
            .await
            .unwrap()
            .iter()
            .any(|s| s.name == "customer-faq")
    );
    assert_eq!(
        status(
            call(
                manager_token.clone(),
                "SkillService/UnassignSkill",
                json!({"agentId":other,"source":"tilde-runtime"})
            )
            .await
        ),
        200
    );
    assert_eq!(skills.for_agent(other, None).await.unwrap().len(), 1);
    // agents.read reaches another agent's prompts.
    let listed = call(
        manager_token.clone(),
        "PromptService/ListPrompts",
        json!({"agentId":plain}),
    )
    .await;
    assert_eq!(listed.0, 200, "{}", listed.1);
    assert_eq!(listed.1["prompts"][0]["id"], json!(prompt));
    db.close().await;
}

fn declared_tool(name: &str, summary: &str) -> management::DeclaredTool {
    management::DeclaredTool {
        name: name.into(),
        description: format!("The {name} tool"),
        summary: summary.into(),
        input_schema_json: r#"{"type":"object","properties":{"count":{"type":"integer"}}}"#.into(),
        annotations: types::ToolAnnotations {
            read_only: true,
            ..Default::default()
        }
        .into(),
        display: types::ToolDisplay::Summary.into(),
        origin: format!("src/index.ts#bundledTools.tools.{name}"),
        ..Default::default()
    }
}

/// A deployment's declared bundled tools are validated as an invocation's are, stored with it
/// and fixed: a refused declaration registers nothing, a repeat of its external id keeps the
/// first set, and each deployment returns its own.
#[tokio::test]
async fn deployments_declare_bundled_tools_their_contents_return() {
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(22)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    let agent = create_agent(&agents).await;
    let deployments = deployments(&db.pool, encryption, connections);
    let tools = |tools: Vec<management::DeclaredTool>| management::DeploymentDeclarations {
        tools,
        ..Default::default()
    };

    for invalid in [
        vec![
            declared_tool("roll_dice", ""),
            declared_tool("roll_dice", ""),
        ],
        vec![declared_tool("tools.execute", "")],
        vec![management::DeclaredTool {
            output_schema_json: "[]".into(),
            ..declared_tool("roll_dice", "")
        }],
    ] {
        let refused = deployments
            .register_deployment(
                agent,
                RegisterDeployment {
                    source: types::DeploymentSource::Ci,
                    target: types::DeploymentTarget::Gateway,
                    target_reference: None,
                    repository: None,
                    commit_sha: None,
                    external_id: Some("ci:refused".into()),
                    label: None,
                    commit_message: None,
                    branch: None,
                    commit_author: None,
                    declarations: tools(invalid),
                },
            )
            .await;
        assert!(matches!(refused, Err(tilde::error::Error::Invalid(_))));
    }

    let first = tools(vec![
        declared_tool("roll_dice", "Rolled dice"),
        management::DeclaredTool {
            output_schema_json: r#"{"type":"object"}"#.into(),
            display: types::ToolDisplay::Unspecified.into(),
            ..declared_tool("local_time", "Checked the time")
        },
    ]);
    let (d1, created) = deploy(&deployments, agent, "ci:1", first.clone()).await;
    assert!(created);
    // The refused registrations left nothing behind under their external id.
    let (_, created) = deploy(&deployments, agent, "ci:refused", tools(vec![])).await;
    assert!(created);
    assert_eq!(
        deploy(
            &deployments,
            agent,
            "ci:1",
            tools(vec![declared_tool("other", "")])
        )
        .await,
        (d1, false)
    );
    let (d2, _) = deploy(
        &deployments,
        agent,
        "ci:2",
        tools(vec![declared_tool("roll_dice", "Rolled some dice")]),
    )
    .await;

    let first_tools = deployments.contents(agent, d1).await.unwrap().tools;
    assert_eq!(
        first_tools
            .iter()
            .map(|t| (t.name.as_str(), t.summary.as_str()))
            .collect::<Vec<_>>(),
        [
            ("local_time", "Checked the time"),
            ("roll_dice", "Rolled dice")
        ]
    );
    let dice = &first_tools[1];
    assert_eq!(dice.origin, "src/index.ts#bundledTools.tools.roll_dice");
    assert_eq!(dice.input_schema_json, first.tools[0].input_schema_json);
    assert!(dice.output_schema_json.is_empty());
    assert!(dice.annotations.read_only && !dice.annotations.destructive);
    assert_eq!(dice.display.as_known(), Some(types::ToolDisplay::Summary));
    // Unspecified display is stored and returned as full, as invocations register it.
    assert_eq!(
        first_tools[0].display.as_known(),
        Some(types::ToolDisplay::Full)
    );
    assert_eq!(first_tools[0].output_schema_json, r#"{"type":"object"}"#);
    let second_tools = deployments.contents(agent, d2).await.unwrap().tools;
    assert_eq!(second_tools.len(), 1);
    assert_eq!(second_tools[0].summary, "Rolled some dice");
}
