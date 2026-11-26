//! Suspended runs across their boundaries: a dial-in host's tool call through the gateway to a
//! slow Lambda tool host answers pending, the host suspends its run with its state, the call's
//! outcome resumes the same run in a new invocation that reads both back; the deadline asks a
//! running invocation to suspend (and a call in flight then answers pending); a wait for a
//! reply is answered by the next message in the thread.
mod common;
use common::invocation::{drain, framed, tool_result};
use connectrpc::client::{ClientConfig, HttpClient};
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tilde::proto::tilde::agent_host::v1::InvokeRequest;
use tilde::proto::tilde::{run::v1 as run, runtime::v1 as wire, types::v1 as types};
use tilde::services::tilde::run::v1::RunServiceClient;
use tilde::services::tilde::runtime::v1::InvocationControlServiceClient;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, CreateThread, PostMessage, StartRun},
    connections::{catalog::Endpoints, service::Connections},
    deployment::{Deployments, RegisterDeployment},
    encryption::Encryption,
    tools::Target,
};
use uuid::Uuid;

fn headers(bearer: &str) -> http::HeaderMap {
    let mut headers = http::HeaderMap::new();
    headers.insert(
        http::header::AUTHORIZATION,
        format!("Bearer {bearer}").parse().unwrap(),
    );
    headers
}
fn runs(base: &str, bearer: &str) -> RunServiceClient<HttpClient> {
    RunServiceClient::new(
        HttpClient::plaintext_http2_only(),
        ClientConfig::new(base.parse().unwrap()).with_default_headers(headers(bearer)),
    )
}

/// One invocation as its host sees it: the wake and calls made with its capability.
struct Woken {
    base: String,
    wake: InvokeRequest,
}
impl Woken {
    /// A tool call from a framework adapter that can suspend: the result frame.
    async fn call(&self, name: &str, call: Uuid, input: Value) -> Value {
        let response = reqwest::Client::new()
            .post(format!(
                "{}/tilde.runtime.v1.ChatService/InvokeToolStream",
                self.base
            ))
            .bearer_auth(&self.wake.capability)
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(framed(
                json!({"name":name,"callId":call.to_string(),"sequence":0,
                "inputJson":input.to_string(),"finish":true,"suspendable":true}),
            ))
            .send()
            .await
            .unwrap();
        let mut bytes = response.bytes().await.unwrap().to_vec();
        tool_result(drain(&mut bytes))
    }
    async fn chat(&self, method: &str, body: Value) -> Value {
        let response = reqwest::Client::new()
            .post(format!(
                "{}/tilde.runtime.v1.ChatService/{method}",
                self.base
            ))
            .bearer_auth(&self.wake.capability)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body: Value = response.json().await.unwrap();
        assert!(status.is_success(), "{method}: {body}");
        body
    }
    async fn suspend(&self, state: Value, pending: &[Uuid]) {
        self.chat(
            "SuspendRun",
            json!({"stateJson": state.to_string(), "pendingCallIds": pending}),
        )
        .await;
    }
    async fn stopped(&self) {
        runs(&self.base, &self.wake.capability)
            .report(run::ReportRequest {
                invocation_id: self.wake.invocation_id.clone(),
                event: Some(run::report_request::Event::Stopped(
                    run::RunStopped::default().into(),
                )),
                ..Default::default()
            })
            .await
            .unwrap();
    }
    async fn checkpoint(&self) -> (Value, Vec<Value>) {
        let body = self.chat("GetCheckpoint", json!({})).await;
        (
            serde_json::from_str(body["stateJson"].as_str().unwrap()).unwrap(),
            body["results"].as_array().cloned().unwrap_or_default(),
        )
    }
}

async fn next_wake(
    wakes: &mut tokio::sync::mpsc::UnboundedReceiver<InvokeRequest>,
) -> InvokeRequest {
    tokio::time::timeout(Duration::from_secs(15), wakes.recv())
        .await
        .expect("a wake within 15 seconds")
        .expect("the host stays connected")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn suspended_runs_resume_with_their_state_and_pending_results() {
    // The gateway signs Lambda invokes with its own AWS credentials, and answers pending after
    // 300 ms; set before any thread of this test reads the environment.
    unsafe {
        std::env::set_var("AWS_ACCESS_KEY_ID", "AKIAFIXTURE");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "fixture-secret");
        std::env::set_var("TILDE_TOOL_PENDING_AFTER_MS", "300");
    }
    // A Lambda tool host whose `restart` of a "slow" service takes two seconds.
    let lambda = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let lambda_origin = format!("http://{}", lambda.local_addr().unwrap());
    let function = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/2015-03-31/functions/{arn}/invocations",
            axum::routing::post(|body: axum::Json<Value>| async move {
                if body.get("listTools").is_some() {
                    return axum::Json(json!({"tools":[{"name":"restart","description":"Restart a service.",
                        "inputSchemaJson":json!({"type":"object","properties":{"service":{"type":"string"}},"required":["service"]}).to_string()}]}));
                }
                let input: Value =
                    serde_json::from_str(body["call"]["inputJson"].as_str().unwrap()).unwrap();
                if input["service"] == "slow" {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                axum::Json(json!({"outputJson":json!({"restarted":input["service"]}).to_string()}))
            }),
        );
        axum::serve(lambda, app).await.unwrap()
    });

    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(97))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let connections = Connections::with_endpoints(
        db.pool.clone(),
        crypto.clone(),
        "http://localhost:1".into(),
        "http://localhost:2".into(),
        Endpoints(BTreeMap::from([("aws_lambda_api".into(), lambda_origin)])),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let deployments = Deployments::new(
        db.pool.clone(),
        crypto.clone(),
        agents.clone(),
        connections.clone(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let chat = Chat::new(db.pool.clone(), crypto, base.clone())
        .with_connections(connections)
        .with_deployments(deployments.clone());
    let agent = Uuid::new_v4();
    agents
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id: agent,
            name: "Suspending".into(),
            // Waiting for a reply reads the thread.
            capabilities: tilde::iam::capabilities::Capabilities::from_wire(types::Capabilities {
                thread_read: types::BinaryPermission::Yes.into(),
                ..common::invocation::all_tools()
            })
            .unwrap(),
        })
        .await
        .unwrap();
    let tools = chat.tools.clone().unwrap();
    let (host, _) = tools
        .hosts
        .register(
            "ops",
            Some("arn:aws:lambda:eu-central-1:123456789012:function:ops"),
        )
        .await
        .unwrap();
    tools
        .add_source(
            tilde::tools::Owner::Agent(agent),
            Target::ToolHost(host.id),
            &["restart".into()],
        )
        .await
        .unwrap();
    let router = tilde::iam::listeners::agent_rpc_router(agents.clone(), chat.clone());
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let (stop, stop_rx) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(chat.clone().worker(stop_rx));

    // The agent's host dials in and stays ready.
    let (_, token, _) = deployments
        .register_deployment(
            agent,
            RegisterDeployment {
                source: types::DeploymentSource::Ci,
                target: types::DeploymentTarget::Gateway,
                target_reference: None,
                repository: None,
                commit_sha: None,
                external_id: Some("suspending".into()),
                label: None,
                commit_message: None,
                branch: None,
                commit_author: None,
                declarations: Default::default(),
            },
        )
        .await
        .unwrap();
    let token = token.unwrap();
    let host_rpc = runs(&base, token.expose_secret());
    let instance = Uuid::new_v4();
    let mut watch = host_rpc
        .watch(run::WatchRequest {
            instance_id: instance.to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    watch.message().await.unwrap().unwrap();
    let (woken, mut wakes) = tokio::sync::mpsc::unbounded_channel();
    let frames = tokio::spawn(async move {
        while let Ok(Some(frame)) = watch.message().await {
            if let Some(run::watch_response::Frame::Wake(wake)) = frame.to_owned_message().frame {
                let _ = woken.send(*wake);
            }
        }
    });
    let heartbeats = tokio::spawn(async move {
        loop {
            let _ = host_rpc
                .heartbeat(run::HeartbeatRequest {
                    instance_id: instance.to_string(),
                    ready: true,
                    ..Default::default()
                })
                .await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    let user = chat.create_user("Operator").await.unwrap();
    let thread = chat
        .create_thread(CreateThread {
            title: "Restarts".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![
                types::ParticipantRef {
                    agent_id: Some(agent.to_string()),
                    ..Default::default()
                },
                types::ParticipantRef {
                    user_id: Some(user.id.clone()),
                    ..Default::default()
                },
            ],
        })
        .await
        .unwrap();
    let started = chat
        .start_run(StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "Restart search".into(),
            idempotency_key: "restart".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let pg = db.pool.get().await.unwrap();

    // A slow call answers pending once it has run for the threshold, and goes on.
    let first = Woken {
        base: base.clone(),
        wake: next_wake(&mut wakes).await,
    };
    assert_eq!(first.wake.run_id, started.id);
    assert!(!first.wake.resumes);
    let call = Uuid::new_v4();
    let began = std::time::Instant::now();
    let answer = first
        .call("ops.restart", call, json!({"service":"slow"}))
        .await;
    assert!(
        began.elapsed() < Duration::from_millis(1500),
        "it did not wait for the host"
    );
    assert_eq!(answer["pending"], true, "{answer}");
    let output: Value = serde_json::from_str(answer["outputJson"].as_str().unwrap()).unwrap();
    assert_eq!(output["status"], "pending");
    // A call that finishes in time answers as always.
    let quick = first
        .call("ops.restart", Uuid::new_v4(), json!({"service":"cache"}))
        .await;
    assert!(quick.get("pending").is_none(), "{quick}");
    // The adapter saves its conversation with the pending call and ends the invocation.
    let state = json!({"messages":[{"role":"user","content":"Restart search"},
        {"role":"assistant","toolCall":call}]});
    first.suspend(state.clone(), &[call]).await;
    first.stopped().await;
    let run_id = Uuid::parse_str(&started.id).unwrap();
    let suspended = chat.run(run_id).await.unwrap();
    assert_eq!(suspended.status, "waiting");
    // The call outlives the invocation; its outcome starts the same run's next invocation.
    let second = Woken {
        base: base.clone(),
        wake: next_wake(&mut wakes).await,
    };
    assert!(
        began.elapsed() >= Duration::from_secs(2),
        "it waited for the call"
    );
    assert_eq!(second.wake.run_id, started.id, "the same run");
    assert_ne!(second.wake.invocation_id, first.wake.invocation_id);
    assert!(second.wake.resumes);
    let (restored, results) = second.checkpoint().await;
    assert_eq!(restored, state);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"], call.to_string());
    assert_eq!(results[0]["status"], "completed");
    assert_eq!(
        serde_json::from_str::<Value>(results[0]["outputJson"].as_str().unwrap()).unwrap(),
        json!({"restarted":"slow"})
    );

    // The deadline: the invocation is asked to suspend through its control stream, and a call
    // in flight answers pending at the deadline rather than after the threshold.
    unsafe { std::env::set_var("TILDE_TOOL_PENDING_AFTER_MS", "60000") };
    let second_id = Uuid::parse_str(&second.wake.invocation_id).unwrap();
    pg.execute(
        "UPDATE chat_invocations SET suspend_at=NOW()+INTERVAL '500 milliseconds' WHERE id=$1",
        &[&second_id],
    )
    .await
    .unwrap();
    let controls = InvocationControlServiceClient::new(
        HttpClient::plaintext_http2_only(),
        ClientConfig::new(base.parse().unwrap())
            .with_default_headers(headers(&second.wake.capability)),
    );
    let mut commands = controls
        .watch_commands(wire::WatchCommandsRequest::default())
        .await
        .unwrap();
    let in_flight = Uuid::new_v4();
    let began = std::time::Instant::now();
    let answer = second
        .call("ops.restart", in_flight, json!({"service":"slow"}))
        .await;
    assert_eq!(answer["pending"], true, "{answer}");
    assert!(began.elapsed() < Duration::from_millis(1500));
    let suspend = loop {
        let command = tokio::time::timeout(Duration::from_secs(5), commands.message())
            .await
            .expect("a suspend command at the deadline")
            .unwrap()
            .unwrap()
            .to_owned_message();
        if command.kind.as_known() != Some(wire::InvocationCommandKind::Ready) {
            break command;
        }
    };
    assert_eq!(
        suspend.kind.as_known(),
        Some(wire::InvocationCommandKind::Suspend)
    );
    second.suspend(json!({"turn":2}), &[in_flight]).await;
    second.stopped().await;
    let third = Woken {
        base: base.clone(),
        wake: next_wake(&mut wakes).await,
    };
    assert_eq!(third.wake.run_id, started.id);
    let (restored, results) = third.checkpoint().await;
    assert_eq!(restored, json!({"turn":2}));
    assert_eq!(results[0]["status"], "completed");

    // A wait for a reply answers pending at once; the next message is its output and resumes
    // the run instead of starting one.
    let wait = Uuid::new_v4();
    let answer = third.call("thread.wait_for_reply", wait, json!({})).await;
    assert_eq!(answer["pending"], true, "{answer}");
    third.suspend(json!({"turn":3}), &[wait]).await;
    third.stopped().await;
    let roster = chat
        .thread(Uuid::parse_str(&thread.id).unwrap())
        .await
        .unwrap();
    let person = roster
        .participants
        .iter()
        .find(|p| p.user_id.is_some())
        .unwrap();
    let reply = Uuid::new_v4();
    chat.post(PostMessage {
        id: reply.to_string(),
        thread_id: thread.id.clone(),
        participant_id: person.id.clone(),
        text: "Yes, restart it".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    let fourth = Woken {
        base: base.clone(),
        wake: next_wake(&mut wakes).await,
    };
    assert_eq!(fourth.wake.run_id, started.id, "the reply resumed the run");
    let (restored, results) = fourth.checkpoint().await;
    assert_eq!(restored, json!({"turn":3}));
    assert_eq!(
        serde_json::from_str::<Value>(results[0]["outputJson"].as_str().unwrap()).unwrap(),
        json!({"message_id": reply, "text": "Yes, restart it"})
    );
    // A run that ends without suspending leaves no checkpoint behind.
    fourth.stopped().await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while pg
            .query_opt(
                "SELECT 1 FROM chat_run_checkpoints WHERE run_id=$1",
                &[&run_id],
            )
            .await
            .unwrap()
            .is_some()
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the checkpoint goes with the run's end");
    let runs: i64 = pg
        .query_one(
            "SELECT count(*) FROM chat_runs WHERE thread_id=$1",
            &[&Uuid::parse_str(&thread.id).unwrap()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(runs, 1, "every invocation continued the one run");

    drop(pg);
    let _ = stop.send(true);
    worker.abort();
    heartbeats.abort();
    frames.abort();
    server.abort();
    function.abort();
    db.close().await;
}
