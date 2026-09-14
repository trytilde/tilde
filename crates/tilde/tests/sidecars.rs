#[allow(dead_code)]
mod common;
use buffa::Message;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tilde::proto::tilde::{
    agent_event_ingress::v1 as wire, agent_host::v1 as host, run::v1 as run, types::v1 as types,
};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::service::Connections,
    deployment::{Deployments, sidecar},
    encryption::Encryption,
};
use uuid::Uuid;

/// A fake agent process: acknowledges every invocation and holds it until released.
async fn agent_host() -> (
    String,
    tokio::sync::mpsc::Receiver<host::InvokeRequest>,
    Arc<tokio::sync::Notify>,
    tokio::task::JoinHandle<()>,
) {
    let (sent, received) = tokio::sync::mpsc::channel(8);
    let finish = Arc::new(tokio::sync::Notify::new());
    let done = finish.clone();
    let invoke = connectrpc::Router::new()
        .route_view_server_stream::<_, _, host::InvokeResponse>(
            "tilde.agent_host.v1.AgentService",
            "Invoke",
            connectrpc::view_streaming_handler_fn(move |_: connectrpc::RequestContext, request: tilde::services::tilde::agent_host::v1::OwnedInvokeRequestView| {
                let sent = sent.clone();
                let done = done.clone();
                async move {
                    let request = host::InvokeRequest::decode_from_slice(request.bytes()).unwrap();
                    let command = request.command_id.clone();
                    sent.send(request).await.unwrap();
                    connectrpc::Response::stream_ok(async_stream::try_stream! {
                        yield host::InvokeResponse { accepted_command_id: command, ..Default::default() };
                        done.notified().await;
                    })
                }
            }),
        )
        .into_axum_service();
    let router = axum::Router::new()
        .route_service("/tilde.agent_host.v1.AgentService/Invoke", invoke)
        .route(
            "/tilde.agent_host.v1.AgentService/Healthz",
            axum::routing::post(|| async {
                (
                    [("content-type", "application/proto")],
                    host::HealthzResponse {
                        ready: true,
                        ..Default::default()
                    }
                    .encode_to_vec(),
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (endpoint, received, finish, task)
}
async fn ingress(
    client: &reqwest::Client,
    base: &str,
    agent: Uuid,
    token: &SecretString,
    method: &str,
    body: serde_json::Value,
) -> (u16, serde_json::Value) {
    let response = client
        .post(format!(
            "{base}/agents/{agent}/tilde.ingress.v1.ChatService/{method}"
        ))
        .bearer_auth(token.expose_secret())
        .header("content-type", "application/json")
        .header("connect-protocol-version", "1")
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    (
        status,
        response.json().await.unwrap_or(serde_json::Value::Null),
    )
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
/// Two replicas of one sidecar agent behind a real gateway listener.
struct Fixture {
    db: Database,
    agent: Uuid,
    deployments: Deployments,
    chat: Chat,
    gateway_url: String,
    first: Option<sidecar::Sidecar>,
    second: Option<sidecar::Sidecar>,
    calls: tokio::sync::mpsc::Receiver<host::InvokeRequest>,
    finish: Arc<tokio::sync::Notify>,
    http: reqwest::Client,
    ingress_token: SecretString,
    deployment_token: SecretString,
    deployment_id: String,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    stop: tokio::sync::watch::Sender<bool>,
}
impl Fixture {
    async fn new() -> Self {
        Self::new_with(true).await
    }
    /// `with_endpoint` false leaves the sidecar without an HTTP agent endpoint: the
    /// agent process must dial in over the run protocol.
    async fn new_with(with_endpoint: bool) -> Self {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_test_writer()
            .try_init();
        let (endpoint, calls, finish, host_task) = agent_host().await;
        let db = Database::new().await;
        let encryption = Arc::new(Encryption::initialize(&db.pool, seed(33)).await.unwrap());
        let agents = Agents::new(db.pool.clone(), encryption.clone());
        let agent = Uuid::new_v4();
        agents
            .create(CreateAgent {
                id: agent,
                name: "Sidecar".into(),
                endpoint_url: "http://127.0.0.1:1".into(),
                webhook_signing_key: SecretString::from(
                    "sidecar-test-agent-signing-key-0123456789",
                ),
                capabilities: tilde::iam::capabilities::Capabilities::from_wire(
                    types::Capabilities {
                        agents_read: types::TargetPermission {
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
        let connections = Connections::new(
            db.pool.clone(),
            encryption.clone(),
            "http://localhost:1".into(),
            "http://localhost:2".into(),
        )
        .unwrap();
        let deployments = Deployments::new(
            db.pool.clone(),
            encryption.clone(),
            agents.clone(),
            connections.clone(),
        );
        deployments
            .set(
                agent,
                types::DeploymentMode::Sidecar,
                types::SidecarFailureMode::Reassign,
                types::DeploymentRouting::Latest,
            )
            .await
            .unwrap();
        agents.resume(agent).await.unwrap();
        // A manual sidecar deployment, as an operator would register from the UI. Its
        // token is what every replica of that deployment dials in with.
        let (deployment, token, created) = deployments
            .register_deployment(
                agent,
                tilde::deployment::RegisterDeployment {
                    source: types::DeploymentSource::Manual,
                    target: types::DeploymentTarget::Sidecar,
                    endpoint_url: None,
                    target_reference: None,
                    repository: Some("acme/support".into()),
                    commit_sha: Some("abc123".into()),
                    external_id: Some("deploy-1".into()),
                    label: None,
                },
            )
            .await
            .unwrap();
        assert!(created);
        assert!(
            deployment.serving,
            "latest routing serves the new deployment"
        );
        let token = token.expect("a new deployment returns its token once");
        let auth = deployments
            .authenticate(token.expose_secret())
            .await
            .unwrap();
        assert_eq!(
            (auth.agent, auth.deployment.to_string()),
            (agent, deployment.id.clone())
        );
        // Registering the same external id again is a no-op without a token.
        let (again, none, created) = deployments
            .register_deployment(
                agent,
                tilde::deployment::RegisterDeployment {
                    source: types::DeploymentSource::Ci,
                    target: types::DeploymentTarget::Sidecar,
                    endpoint_url: None,
                    target_reference: None,
                    repository: None,
                    commit_sha: None,
                    external_id: Some("deploy-1".into()),
                    label: None,
                },
            )
            .await
            .unwrap();
        assert!(!created && none.is_none() && again.id == deployment.id);
        assert!(
            deployments
                .authenticate("unrelated-deployment-token")
                .await
                .is_err()
        );
        // The gateway: sidecar protocol plus its public ingress for this agent, over real HTTP.
        let chat = Chat::new(
            db.pool.clone(),
            encryption.clone(),
            "http://127.0.0.1:1".into(),
        )
        .with_connections(connections)
        .with_deployments(deployments.clone());
        let router = tilde::deployment::rpc::sidecar_router(deployments.clone()).merge(
            tilde::deployment::public::router(deployments.clone(), chat.clone()),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gateway_url = format!("http://{}", listener.local_addr().unwrap());
        let gateway = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        let (stop, workers_rx) = tokio::sync::watch::channel(false);
        let relay = tokio::spawn(deployments.clone().relay_worker(workers_rx.clone()));
        let recovery = tokio::spawn(deployments.clone().recovery_worker(workers_rx));
        let options = |gateway_url: &str| sidecar::Options {
            gateway_url: gateway_url.to_owned(),
            tokens: vec![token.clone()],
            endpoints: if with_endpoint {
                BTreeMap::from([(agent, endpoint.clone())])
            } else {
                BTreeMap::new()
            },
            runtime_listen: "127.0.0.1:0".parse().unwrap(),
            listen: "127.0.0.1:0".parse().unwrap(),
            public_url: None,
            idle_after: Duration::from_secs(600),
            cache_bytes: 256 * 1024 * 1024,
        };
        let first = sidecar::start(options(&gateway_url)).await.unwrap();
        let second = sidecar::start(options(&gateway_url)).await.unwrap();
        assert_eq!(deployments.instances(agent).await.unwrap().len(), 2);
        let ingress_token = deployments
            .issue_ingress_token(agent, None, Uuid::nil())
            .await
            .unwrap();
        Self {
            db,
            agent,
            deployments,
            chat,
            gateway_url,
            first: Some(first),
            second: Some(second),
            calls,
            finish,
            http: reqwest::Client::new(),
            ingress_token,
            deployment_token: token.clone(),
            deployment_id: deployment.id.clone(),
            tasks: vec![host_task, gateway, relay, recovery],
            stop,
        }
    }
    fn first(&self) -> &sidecar::Sidecar {
        self.first.as_ref().expect("first replica is running")
    }
    fn second(&self) -> &sidecar::Sidecar {
        self.second.as_ref().expect("second replica is running")
    }
    fn base(&self, sidecar: &sidecar::Sidecar) -> String {
        format!("http://{}", sidecar.address)
    }
    async fn call(
        &self,
        base: &str,
        method: &str,
        body: serde_json::Value,
    ) -> (u16, serde_json::Value) {
        ingress(
            &self.http,
            base,
            self.agent,
            &self.ingress_token,
            method,
            body,
        )
        .await
    }
    async fn post(
        &self,
        base: &str,
        thread: Uuid,
        participant: &str,
        text: &str,
    ) -> (u16, serde_json::Value) {
        self.call(base, "PostMessage", serde_json::json!({"id":Uuid::new_v4(),"threadId":thread,"participantId":participant,"text":text})).await
    }
    /// A room created on `first`: (thread, Alice's participant, the agent's participant).
    async fn room(&self) -> (Uuid, String, String) {
        let base = self.base(self.first());
        let (status, user) = self
            .call(&base, "CreateUser", serde_json::json!({"name":"Alice"}))
            .await;
        assert_eq!(status, 200, "{user}");
        let user_id = user["user"]["id"].as_str().unwrap().to_owned();
        let (status, thread) = self
            .call(
                &base,
                "CreateThread",
                serde_json::json!({"title":"Room","primaryAgentId":self.agent,"participants":[{"userId":user_id},{"agentId":self.agent}]}),
            )
            .await;
        assert_eq!(status, 200, "{thread}");
        let thread_id = Uuid::parse_str(thread["thread"]["id"].as_str().unwrap()).unwrap();
        let participants = thread["thread"]["participants"].as_array().unwrap();
        let by = |key: &str| {
            participants.iter().find(|p| p[key].is_string()).unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        (thread_id, by("userId"), by("agentId"))
    }
    async fn invocation(&mut self, expected: &str) -> host::InvokeRequest {
        tokio::time::timeout(Duration::from_secs(10), self.calls.recv())
            .await
            .unwrap_or_else(|_| panic!("agent invocation for {expected}"))
            .unwrap()
    }
    /// The replica holding the thread's lease in the projection, if any.
    async fn holder(&self, thread: Uuid) -> Option<Uuid> {
        use sqlx::Row;
        sqlx::query("SELECT instance_id FROM thread_leases WHERE thread_id=$1 AND agent_id=$2")
            .bind(thread)
            .bind(self.agent)
            .fetch_optional(&self.db.pool)
            .await
            .unwrap()
            .map(|row| row.get("instance_id"))
    }
    async fn projected(&self, thread: Uuid, text: &str) {
        eventually(async || {
            self.chat
                .messages(thread, 20)
                .await
                .ok()
                .filter(|m| m.iter().any(|m| m.text == text))
        })
        .await;
    }
    /// Age a node until the gateway treats it as dead and moves its lease to `next`.
    async fn fail_over(&self, instance: Uuid, thread: Uuid, next: Uuid) {
        // A heartbeat already in flight may still land after a stop; keep aging the
        // node until recovery (the worker or this direct call) reassigns.
        eventually(async || {
            sqlx::query(
                "UPDATE agent_instances SET last_seen_at=NOW()-INTERVAL '1 minute' WHERE instance_id=$1",
            )
            .bind(instance)
            .execute(&self.db.pool)
            .await
            .unwrap();
            self.deployments.recover().await.unwrap();
            (self.holder(thread).await == Some(next)).then_some(())
        })
        .await;
    }
    async fn shutdown(self) {
        for replica in [self.first, self.second].into_iter().flatten() {
            replica.stop().await;
        }
        let _ = self.stop.send(true);
        for task in self.tasks {
            task.abort();
        }
        self.db.close().await;
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replicas_own_execute_project_forward_and_fail_over_through_the_gateway() {
    let mut fx = Fixture::new().await;
    let (first_base, second_base) = (fx.base(fx.first()), fx.base(fx.second()));
    // The replica that receives the first request takes the thread lease and runs the agent.
    let (thread_id, alice, agent_participant) = fx.room().await;
    let (status, posted) = fx.post(&first_base, thread_id, &alice, "hello").await;
    assert_eq!(status, 200, "{posted}");
    let invocation = fx.invocation("hello").await;
    assert_eq!(
        invocation.owner_instance_id,
        fx.first().instance.to_string()
    );
    assert_eq!(invocation.objective, "hello");
    assert_eq!(invocation.thread_id, thread_id.to_string());
    // Registry calls from the agent process are relayed to the gateway under the same token.
    let registry = |token: &str| {
        fx.http
            .post(format!(
                "{}/tilde.runtime.v1.AgentService/ListAgents",
                invocation.callback_url
            ))
            .bearer_auth(token)
            .header("content-type", "application/json")
            .header("connect-protocol-version", "1")
            .body("{}")
            .send()
    };
    let listed = registry(&invocation.capability).await.unwrap();
    let status = listed.status();
    let listed: serde_json::Value = listed.json().await.unwrap_or_default();
    assert_eq!(status, 200, "{listed}");
    assert!(
        listed["agents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == fx.agent.to_string())
    );
    assert_eq!(
        registry("not-an-invocation-token").await.unwrap().status(),
        401
    );
    fx.finish.notify_one();
    // The holder's events reach Postgres without the gateway ever calling the replica.
    let projected = eventually(async || {
        let messages = fx.chat.messages(thread_id, 10).await.ok()?;
        let run = fx
            .chat
            .run(Uuid::parse_str(&invocation.run_id).unwrap())
            .await
            .ok()?;
        (messages.iter().any(|m| m.text == "hello")
            && run.invocation_status == "stopped"
            && run.status == "waiting")
            .then_some(run)
    })
    .await;
    assert_eq!(projected.status, "waiting");
    assert_eq!(fx.holder(thread_id).await, Some(fx.first().instance));
    // A request landing on the other replica is handed to the holder: the gateway
    // refuses the second replica's lease and names the first, which executes it.
    let (status, forwarded) = fx.post(&second_base, thread_id, &alice, "second").await;
    assert_eq!(status, 200, "{forwarded}");
    let invocation = fx.invocation("second").await;
    assert_eq!(
        invocation.owner_instance_id,
        fx.first().instance.to_string()
    );
    assert_eq!(invocation.objective, "second");
    // Reads are answered from the projection wherever they land.
    let (status, read) = fx
        .call(
            &fx.gateway_url.clone(),
            "GetThread",
            serde_json::json!({"id":thread_id}),
        )
        .await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(
        read["thread"]["id"].as_str().unwrap(),
        thread_id.to_string()
    );
    let (status, listed) = fx
        .call(
            &second_base,
            "ListMessages",
            serde_json::json!({"threadId":thread_id,"limit":10}),
        )
        .await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["messages"].as_array().unwrap().len(), 2);
    let (status, listed) = fx
        .call(&second_base, "ListThreads", serde_json::json!({"limit":10}))
        .await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["threads"].as_array().unwrap().len(), 1);
    // The holder dies mid-invocation: recovery moves the lease and the survivor restarts the run.
    let pending_run = Uuid::parse_str(&invocation.run_id).unwrap();
    eventually(async || {
        fx.chat
            .run(pending_run)
            .await
            .ok()
            .filter(|r| matches!(r.invocation_status.as_str(), "pending" | "running"))
    })
    .await;
    eventually(async || {
        fx.deployments
            .instances(fx.agent)
            .await
            .ok()
            .filter(|nodes| nodes.len() == 2 && nodes.iter().all(|n| n.ready))
    })
    .await;
    let first_instance = fx.first().instance;
    fx.first.take().unwrap().stop().await;
    // Work queued for the dead holder while the gateway still believed it alive follows
    // the lease to the survivor instead of timing out.
    let queued = {
        let (http, base, agent, token) = (
            fx.http.clone(),
            fx.gateway_url.clone(),
            fx.agent,
            fx.ingress_token.clone(),
        );
        let participant = agent_participant.clone();
        tokio::spawn(async move {
            ingress(&http, &base, agent, &token, "PostMessage", serde_json::json!({"id":Uuid::new_v4(),"threadId":thread_id,"participantId":participant,"text":"queued"})).await
        })
    };
    fx.fail_over(first_instance, thread_id, fx.second().instance)
        .await;
    let recovered = fx.invocation("recovered second").await;
    assert_eq!(
        recovered.owner_instance_id,
        fx.second().instance.to_string()
    );
    assert_eq!(recovered.objective, "second");
    fx.finish.notify_one();
    fx.finish.notify_one();
    let (status, queued) = queued.await.unwrap();
    assert_eq!(status, 200, "{queued}");
    fx.projected(thread_id, "queued").await;
    assert_eq!(fx.holder(thread_id).await, Some(fx.second().instance));
    // Afterwards the survivor serves the conversation directly.
    let (status, after) = fx.post(&second_base, thread_id, &alice, "third").await;
    assert_eq!(status, 200, "{after}");
    let invocation = fx.invocation("third").await;
    assert_eq!(
        invocation.owner_instance_id,
        fx.second().instance.to_string()
    );
    fx.finish.notify_one();
    fx.projected(thread_id, "third").await;
    fx.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replica_that_lost_its_lease_is_fenced_and_hands_off_to_the_holder() {
    let mut fx = Fixture::new().await;
    let first_base = fx.base(fx.first());
    let (thread_id, alice, _agent_participant) = fx.room().await;
    let (status, posted) = fx.post(&first_base, thread_id, &alice, "hello").await;
    assert_eq!(status, 200, "{posted}");
    let invocation = fx.invocation("hello").await;
    assert_eq!(
        invocation.owner_instance_id,
        fx.first().instance.to_string()
    );
    fx.projected(thread_id, "hello").await;
    // The lease moves while the first replica is still mid-invocation and has not
    // heard about it: its heartbeats stopped arriving, so the second replica's
    // hydrate takes the lease from a holder the gateway considers dead. The first
    // replica is alive and will keep heartbeating, so the take-over must land
    // inside one heartbeat interval.
    let (first_instance, second_instance) = (fx.first().instance, fx.second().instance);
    eventually(async || {
        sqlx::query(
            "UPDATE agent_instances SET last_seen_at=NOW()-INTERVAL '1 minute' WHERE instance_id=$1",
        )
        .bind(first_instance)
        .execute(&fx.db.pool)
        .await
        .unwrap();
        let hydrated = fx
            .deployments
            .hydrate(
                fx.agent,
                Some(second_instance),
                wire::HydrateRequest {
                    key: Some(wire::hydrate_request::Key::ThreadId(thread_id.to_string())),
                    lease: true,
                    instance_id: second_instance.to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let lease = hydrated.lease.into_option().unwrap();
        (lease.held && lease.holder_instance_id == second_instance.to_string()).then_some(())
    })
    .await;
    fx.finish.notify_one();
    // Run state the stale replica publishes is refused as not the holder's; it learns
    // who holds the lease and gives the conversation up. The projection keeps the
    // second replica as the holder.
    eventually(async || (!fx.first().nodes[0].runtime.owns_thread(thread_id).await).then_some(()))
        .await;
    assert_eq!(fx.holder(thread_id).await, Some(fx.second().instance));
    // New work landing on the stale replica is handed to the holder. The second
    // replica may also have restarted the interrupted run when it hydrated; every
    // invocation from here on belongs to it, whichever order they arrive in.
    let (status, after) = fx.post(&first_base, thread_id, &alice, "after").await;
    assert_eq!(status, 200, "{after}");
    loop {
        let invocation = fx.invocation("after").await;
        assert_eq!(
            invocation.owner_instance_id,
            fx.second().instance.to_string()
        );
        fx.finish.notify_one();
        if invocation.objective == "after" {
            break;
        }
        assert_eq!(invocation.objective, "hello");
    }
    fx.projected(thread_id, "after").await;
    fx.shutdown().await;
}

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
/// Without an HTTP endpoint the agent process dials the sidecar with the deployment
/// token, receives the wake as a frame and reports back; stopping the sidecar releases
/// its leases at once instead of leaving them to the liveness window.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_local_agent_dials_the_sidecar_and_a_graceful_stop_releases_leases() {
    let mut fx = Fixture::new_with(false).await;
    let first_base = fx.base(fx.first());
    let runtime_base = format!("http://{}/agents/{}", fx.first().runtime_address, fx.agent);
    let agent_process = run_client(&runtime_base, fx.deployment_token.expose_secret());
    let instance = Uuid::new_v4();
    let mut watch = agent_process
        .watch(run::WatchRequest {
            instance_id: instance.to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let first: run::WatchResponse = watch.message().await.unwrap().unwrap().to_owned_message();
    match first.frame {
        Some(run::watch_response::Frame::Registered(registered)) => {
            assert_eq!(registered.deployment_id, fx.deployment_id);
            assert_eq!(registered.agent_id, fx.agent.to_string());
        }
        other => panic!("expected registration, got {other:?}"),
    }
    agent_process
        .heartbeat(run::HeartbeatRequest {
            instance_id: instance.to_string(),
            ready: true,
            ..Default::default()
        })
        .await
        .unwrap();
    // A wrong token never attaches.
    assert!(
        run_client(&runtime_base, "not-the-deployment-token")
            .heartbeat(run::HeartbeatRequest {
                instance_id: instance.to_string(),
                ready: true,
                ..Default::default()
            })
            .await
            .is_err()
    );
    let (thread_id, alice, _) = fx.room().await;
    let (status, posted) = fx.post(&first_base, thread_id, &alice, "hello").await;
    assert_eq!(status, 200, "{posted}");
    let wake = loop {
        let frame: run::WatchResponse =
            tokio::time::timeout(Duration::from_secs(10), watch.message())
                .await
                .expect("a wake frame")
                .unwrap()
                .unwrap()
                .to_owned_message();
        match frame.frame {
            Some(run::watch_response::Frame::Wake(wake)) => break *wake,
            Some(run::watch_response::Frame::Ping(_)) => continue,
            other => panic!("unexpected frame {other:?}"),
        }
    };
    assert_eq!(wake.objective, "hello");
    assert_eq!(wake.thread_id, thread_id.to_string());
    assert_eq!(wake.owner_instance_id, fx.first().instance.to_string());
    assert!(!wake.command_id.is_empty());
    // The agent never received an HTTP Invoke: the fixture's host saw nothing.
    assert!(fx.calls.try_recv().is_err());
    let execution = run_client(&runtime_base, &wake.capability);
    let report = |event| run::ReportRequest {
        invocation_id: wake.invocation_id.clone(),
        event: Some(event),
        ..Default::default()
    };
    execution
        .report(report(run::report_request::Event::Accepted(
            run::RunAccepted {
                command_id: wake.command_id.clone(),
                ..Default::default()
            }
            .into(),
        )))
        .await
        .unwrap();
    execution
        .report(report(run::report_request::Event::ReasoningDelta(
            "pondering".into(),
        )))
        .await
        .unwrap();
    execution
        .report(report(run::report_request::Event::Stopped(
            run::RunStopped::default().into(),
        )))
        .await
        .unwrap();
    let run_id = Uuid::parse_str(&wake.run_id).unwrap();
    eventually(async || {
        fx.chat
            .run(run_id)
            .await
            .ok()
            .filter(|r| r.invocation_status == "stopped" && r.status == "waiting")
    })
    .await;
    fx.projected(thread_id, "hello").await;
    assert_eq!(fx.holder(thread_id).await, Some(fx.first().instance));
    // Graceful stop: the idle thread's lease is released immediately, not after
    // fifteen seconds, and the instance reports itself gone.
    fx.first.take().unwrap().stop().await;
    eventually(async || (fx.holder(thread_id).await.is_none()).then_some(())).await;
    assert!(
        fx.deployments
            .instances(fx.agent)
            .await
            .unwrap()
            .iter()
            .all(|i| i.instance_id != instance.to_string() || !i.ready)
    );
    // The survivor takes the thread on its next touch and wakes its own agent process.
    let second_runtime = format!("http://{}/agents/{}", fx.second().runtime_address, fx.agent);
    let second_agent = run_client(&second_runtime, fx.deployment_token.expose_secret());
    let second_instance = Uuid::new_v4();
    let mut second_watch = second_agent
        .watch(run::WatchRequest {
            instance_id: second_instance.to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let _registered = second_watch.message().await.unwrap().unwrap();
    second_agent
        .heartbeat(run::HeartbeatRequest {
            instance_id: second_instance.to_string(),
            ready: true,
            ..Default::default()
        })
        .await
        .unwrap();
    let second_base = fx.base(fx.second());
    let (status, after) = fx.post(&second_base, thread_id, &alice, "after").await;
    assert_eq!(status, 200, "{after}");
    let wake = loop {
        let frame: run::WatchResponse =
            tokio::time::timeout(Duration::from_secs(10), second_watch.message())
                .await
                .expect("a wake on the survivor")
                .unwrap()
                .unwrap()
                .to_owned_message();
        match frame.frame {
            Some(run::watch_response::Frame::Wake(wake)) => break *wake,
            _ => continue,
        }
    };
    assert_eq!(wake.objective, "after");
    assert_eq!(wake.owner_instance_id, fx.second().instance.to_string());
    assert_eq!(fx.holder(thread_id).await, Some(fx.second().instance));
    run_client(&second_runtime, &wake.capability)
        .report(run::ReportRequest {
            invocation_id: wake.invocation_id.clone(),
            event: Some(run::report_request::Event::Stopped(
                run::RunStopped::default().into(),
            )),
            ..Default::default()
        })
        .await
        .unwrap();
    fx.projected(thread_id, "after").await;
    fx.shutdown().await;
}
