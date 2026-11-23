//! A sidecar-hosted agent's tools: the sidecar offers the agent's tool sources from its
//! replicated configuration and calls them with the replicated credentials, and a credential
//! rotation at the gateway reaches the replica without a restart.
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tilde::proto::tilde::{agent_host::v1 as host, run::v1 as run, types::v1 as types};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::{catalog::Endpoints, service::Connections},
    deployment::{Deployments, sidecar},
    encryption::Encryption,
};
use uuid::Uuid;

fn run_client(
    base: &str,
    bearer: &str,
) -> tilde::services::tilde::run::v1::RunServiceClient<connectrpc::client::HttpClient> {
    let mut headers = http::HeaderMap::new();
    headers.insert(
        http::header::AUTHORIZATION,
        format!("Bearer {bearer}").parse().unwrap(),
    );
    tilde::services::tilde::run::v1::RunServiceClient::new(
        connectrpc::client::HttpClient::plaintext_http2_only(),
        connectrpc::client::ClientConfig::new(base.parse().unwrap()).with_default_headers(headers),
    )
}
/// The agent process beside the sidecar, dialing its runtime listener with the deployment
/// token as the SDK does. It hands each wake to the test and holds the invocation open until
/// the test is done with it.
async fn agent_process(
    runtime: String,
    token: String,
    wakes: tokio::sync::mpsc::Sender<host::InvokeRequest>,
    done: Arc<tokio::sync::Notify>,
) -> tokio::task::JoinHandle<()> {
    let client = run_client(&runtime, &token);
    let instance = Uuid::new_v4().to_string();
    let mut watch = client
        .watch(run::WatchRequest {
            instance_id: instance.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    watch.message().await.unwrap().unwrap();
    let heartbeat = run::HeartbeatRequest {
        instance_id: instance,
        ready: true,
        ..Default::default()
    };
    client.heartbeat(heartbeat.clone()).await.unwrap();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        let mut jobs = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _ = tick.tick() => if client.heartbeat(heartbeat.clone()).await.is_err() { break },
                frame = watch.message() => {
                    let Ok(Some(frame)) = frame else { break };
                    let Some(run::watch_response::Frame::Wake(wake)) = frame.to_owned_message().frame else { continue };
                    let reports = run_client(&runtime, &wake.capability);
                    let (wakes, done) = (wakes.clone(), done.clone());
                    jobs.spawn(async move {
                        let id = wake.invocation_id.clone();
                        let accepted = run::RunAccepted { command_id: wake.command_id.clone(), ..Default::default() };
                        let _ = reports.report(run::ReportRequest { invocation_id: id.clone(), event: Some(run::report_request::Event::Accepted(accepted.into())), ..Default::default() }).await;
                        let released = done.notified();
                        if wakes.send(*wake).await.is_ok() {
                            released.await;
                        }
                        let _ = reports.report(run::ReportRequest { invocation_id: id, event: Some(run::report_request::Event::Stopped(run::RunStopped::default().into())), ..Default::default() }).await;
                    });
                }
            }
        }
        jobs.abort_all();
    })
}
async fn eventually<T>(mut probe: impl AsyncFnMut() -> Option<T>) -> T {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = probe().await {
            return value;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for condition"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
/// The fake Tavily API: answers `/usage` with the API key it was called with.
async fn upstream() -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
    let keys: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = keys.clone();
    let app = axum::Router::new().route(
        "/usage",
        axum::routing::get(move |headers: http::HeaderMap| {
            let seen = seen.clone();
            async move {
                let key = headers
                    .get(http::header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.strip_prefix("Bearer "))
                    .unwrap_or_default()
                    .to_owned();
                seen.lock().unwrap().push(key.clone());
                axum::Json(json!({"key": {"usage": 1, "limit": 1000}, "seen": key}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (origin, keys, server)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_sidecar_agent_lists_and_calls_its_tool_sources_with_replicated_credentials() {
    let (origin, keys, upstream) = upstream().await;
    let endpoints = || Endpoints([("tavily_api".to_owned(), origin.clone())].into());
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(61)).await.unwrap());
    let connections = Connections::with_endpoints(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
        "http://127.0.0.1:2".into(),
        endpoints(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let deployments = Deployments::new(
        db.pool.clone(),
        encryption.clone(),
        agents.clone(),
        connections.clone(),
    );
    let chat = Chat::new(db.pool.clone(), encryption.clone(), String::new())
        .with_connections(connections.clone())
        .with_deployments(deployments.clone());
    let tools = chat.tools.clone().unwrap();
    let agent = Uuid::new_v4();
    let connection = Uuid::new_v4();
    let setup = async {
        agents
            .create(CreateAgent {
                description: String::new(),
                concurrency_policy: tilde::agent::ConcurrencyPolicy::Queue,
                id: agent,
                name: "Researcher".into(),
                capabilities: tilde::iam::capabilities::Capabilities::from_wire(
                    types::Capabilities {
                        tools_invoke: types::TargetPermission {
                            mode: types::TargetSelection::All.into(),
                            ..Default::default()
                        }
                        .into(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            })
            .await
            .unwrap();
        agents.pause(agent).await.unwrap();
        deployments
            .set(
                agent,
                types::SidecarFailureMode::Reassign,
                types::DeploymentRouting::Latest,
            )
            .await
            .unwrap();
        agents.resume(agent).await.unwrap();
        let (_, token, _) = deployments
            .register_deployment(
                agent,
                tilde::deployment::RegisterDeployment {
                    source: types::DeploymentSource::Manual,
                    target: types::DeploymentTarget::Sidecar,
                    target_reference: None,
                    repository: None,
                    commit_sha: None,
                    external_id: Some("sidecar-tools".into()),
                    label: None,
                    commit_message: None,
                    branch: None,
                    commit_author: None,
                    declarations: Default::default(),
                },
            )
            .await
            .unwrap();
        // An installation Tavily connection, ready with its API key, as setup leaves it.
        connections
            .start(connection, "Search", "tavily", "api", &[])
            .await
            .unwrap();
        db.pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE connections SET status='ready' WHERE id=$1",
                &[&connection],
            )
            .await
            .unwrap();
        connections
            .replace_value(connection, "api_key", &SecretString::from("key-one"))
            .await
            .unwrap();
        let slug = tools
            .add_source(
                tilde::tools::Owner::Agent(agent),
                tilde::tools::Target::Connection(connection),
                &["usage".to_owned()],
            )
            .await
            .unwrap()
            .slug;
        let ingress = deployments.issue_ingress_token(agent, None).await.unwrap();
        (token.unwrap(), slug, ingress)
    };
    let (token, slug, ingress) = setup.await;
    let name = format!("{slug}.usage");

    // The gateway's sidecar route group over real HTTP.
    let gateway = tilde::deployment::rpc::sidecar_router(deployments.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gateway_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, gateway).await.unwrap() });
    let replica = sidecar::start(sidecar::Options {
        gateway_url,
        tokens: vec![token.clone()],
        runtime_listen: "127.0.0.1:0".parse().unwrap(),
        listen: "127.0.0.1:0".parse().unwrap(),
        public_url: None,
        idle_after: Duration::from_secs(600),
        cache_bytes: 1024 * 1024,
        endpoints: endpoints(),
    })
    .await
    .unwrap();
    let (wakes, mut woken) = tokio::sync::mpsc::channel(4);
    let done = Arc::new(tokio::sync::Notify::new());
    let process = agent_process(
        format!("http://{}/agents/{agent}", replica.runtime_address),
        token.expose_secret().to_owned(),
        wakes,
        done.clone(),
    )
    .await;

    // A customer message through the replica's ingress wakes the agent process there.
    let http = reqwest::Client::new();
    let ingress_call = async |method: &str, body: Value| -> Value {
        let response = http
            .post(format!(
                "http://{}/agents/{agent}/tilde.provider.tilde.v1.ChatService/{method}",
                replica.address
            ))
            .bearer_auth(ingress.expose_secret())
            .header("connect-protocol-version", "1")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success(), "{method}");
        response.json().await.unwrap()
    };
    let user = ingress_call("CreateUser", json!({"name": "Alice"})).await;
    let user = user["user"]["id"].as_str().unwrap().to_owned();
    let thread = ingress_call(
        "CreateThread",
        json!({"title":"Research","primaryAgentId":agent,"participants":[{"userId":user},{"agentId":agent}]}),
    )
    .await;
    let thread_id = thread["thread"]["id"].as_str().unwrap().to_owned();
    let alice = thread["thread"]["participants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["userId"].is_string())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut post = async |text: &str| -> host::InvokeRequest {
        ingress_call(
            "PostMessage",
            json!({"id":Uuid::new_v4(),"threadId":thread_id,"participantId":alice,"text":text}),
        )
        .await;
        tokio::time::timeout(Duration::from_secs(10), woken.recv())
            .await
            .expect("the agent process is woken")
            .unwrap()
    };
    let invocation = post("what is our usage?").await;
    assert_eq!(invocation.owner_instance_id, replica.instance.to_string());

    // The agent calls its tools on the replica as everywhere else.
    let chat_call = async |wake: &host::InvokeRequest, method: &str, body: Vec<u8>, json: bool| {
        http.post(format!(
            "{}/tilde.runtime.v1.ChatService/{method}",
            wake.callback_url
        ))
        .bearer_auth(&wake.capability)
        .header(
            "content-type",
            if json {
                "application/json"
            } else {
                "application/connect+json"
            },
        )
        .header("connect-protocol-version", "1")
        .body(body)
        .send()
        .await
        .unwrap()
    };
    let listed: Value = chat_call(&invocation, "ListTools", b"{}".to_vec(), true)
        .await
        .json()
        .await
        .unwrap();
    let tool = listed["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == name)
        .unwrap_or_else(|| panic!("{name} is offered on the sidecar: {listed}"));
    assert_eq!(tool["providerId"], "tavily");
    let invoke = async |wake: &host::InvokeRequest| -> Value {
        let body = common::invocation::framed(
            json!({"name":name,"callId":Uuid::new_v4().to_string(),"sequence":0,"inputJson":"{}","finish":true}),
        );
        let mut bytes = chat_call(wake, "InvokeToolStream", body, false)
            .await
            .bytes()
            .await
            .unwrap()
            .to_vec();
        let result = common::invocation::tool_result(common::invocation::drain(&mut bytes));
        serde_json::from_str(
            result["outputJson"]
                .as_str()
                .unwrap_or_else(|| panic!("{result}")),
        )
        .unwrap()
    };
    // The replica calls the upstream itself, with the credential it holds in memory.
    assert_eq!(invoke(&invocation).await["seen"], "key-one");

    // Rotating the key at the gateway pushes a new configuration; the replica's next call uses it.
    let rotated = SecretString::from("key-two");
    let rotate = connections.replace_value(connection, "api_key", &rotated);
    rotate.await.unwrap();
    eventually(async || {
        let config = replica.nodes[0].runtime.configuration().ok()?;
        config
            .connections
            .iter()
            .any(|c| {
                c.id == connection.to_string() && c.credentials.iter().any(|f| f.value == "key-two")
            })
            .then_some(())
    })
    .await;
    assert_eq!(invoke(&invocation).await["seen"], "key-two");
    assert_eq!(*keys.lock().unwrap(), ["key-one", "key-two"]);

    // Removing the tool from the agent removes it from the replica's catalog.
    let source = tools.sources(tilde::tools::Filter {
        agent: Some(agent),
        ..Default::default()
    });
    let source = source.await.unwrap().pop().unwrap().id;
    let removed = tools.remove_tool(source, "usage");
    removed.await.unwrap();
    eventually(async || {
        let config = replica.nodes[0].runtime.configuration().ok()?;
        config.tools.is_empty().then_some(())
    })
    .await;
    let listed: Value = chat_call(&invocation, "ListTools", b"{}".to_vec(), true)
        .await
        .json()
        .await
        .unwrap();
    assert!(
        !listed["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == name),
        "{listed}"
    );

    done.notify_waiters();
    replica.stop().await;
    process.abort();
    server.abort();
    upstream.abort();
    db.close().await;
}
