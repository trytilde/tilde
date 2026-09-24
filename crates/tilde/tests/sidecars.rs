mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use std::{sync::Arc, time::Duration};
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

/// The fixture agent process dials each sidecar over the same outbound run protocol as
/// the SDK. It hands every wake to the test and holds the invocation until released.
async fn connected_test_agent(
    runtime: String,
    token: &str,
    calls: tokio::sync::mpsc::Sender<host::InvokeRequest>,
    finish: Arc<tokio::sync::Notify>,
) -> tokio::task::JoinHandle<()> {
    use connectrpc::client::{ClientConfig, HttpClient};
    use tilde::proto::tilde::runtime::v1 as controls;
    use tilde::services::tilde::runtime::v1::InvocationControlServiceClient;
    let client = run_client(&runtime, token);
    let instance = Uuid::new_v4();
    let mut watch = client
        .watch(run::WatchRequest {
            instance_id: instance.to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    watch.message().await.unwrap().unwrap();
    client
        .heartbeat(run::HeartbeatRequest {
            instance_id: instance.to_string(),
            ready: true,
            ..Default::default()
        })
        .await
        .unwrap();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        let mut jobs = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _ = tick.tick() => { if client.heartbeat(run::HeartbeatRequest { instance_id: instance.to_string(), ready: true, ..Default::default() }).await.is_err() { break; } }
                frame = watch.message() => {
                    let Ok(Some(frame)) = frame else { break; };
                    let Some(run::watch_response::Frame::Wake(wake)) = frame.to_owned_message().frame else { continue; };
                    let runtime = runtime.clone(); let calls = calls.clone(); let finish = finish.clone();
                    jobs.spawn(async move {
                        let reports = run_client(&runtime, &wake.capability);
                        let mut headers = http::HeaderMap::new();
                        headers.insert(http::header::AUTHORIZATION, format!("Bearer {}", wake.capability).parse().unwrap());
                        let controls = InvocationControlServiceClient::new(HttpClient::plaintext_http2_only(), ClientConfig::new(runtime.parse().unwrap()).with_default_headers(headers));
                        let Ok(mut commands) = controls.watch_commands(controls::WatchCommandsRequest::default()).await else { return; };
                        let _ = commands.message().await;
                        let id = wake.invocation_id.clone();
                        let _ = reports.report(run::ReportRequest { invocation_id: id.clone(), event: Some(run::report_request::Event::Accepted(run::RunAccepted { command_id: wake.command_id.clone(), ..Default::default() }.into())), ..Default::default() }).await;
                        if calls.send(*wake).await.is_ok() {
                            let released = finish.notified();
                            tokio::pin!(released);
                            loop {
                                tokio::select! {
                                    _ = &mut released => break,
                                    command = commands.message() => match command {
                                        Ok(Some(command)) if matches!(command.to_owned_message().kind.as_known(), Some(controls::InvocationCommandKind::Stop | controls::InvocationCommandKind::Suspend)) => break,
                                        Ok(Some(_)) => {},
                                        _ => break,
                                    }
                                }
                            }
                        }
                        let _ = reports.report(run::ReportRequest { invocation_id: id, event: Some(run::report_request::Event::Stopped(run::RunStopped::default().into())), ..Default::default() }).await;
                    });
                }
            }
        }
        jobs.abort_all();
    })
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
            "{base}/agents/{agent}/tilde.provider.tilde.v1.ChatService/{method}"
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
    connections: Connections,
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
    /// `with_agents` false starts the sidecars without the fixture agent processes, so
    /// a test can dial in itself.
    async fn new_with(with_agents: bool) -> Self {
        Self::with_policy(with_agents, tilde::agent::ConcurrencyPolicy::Queue).await
    }
    async fn with_policy(with_agents: bool, policy: tilde::agent::ConcurrencyPolicy) -> Self {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_test_writer()
            .try_init();
        let (sent, calls) = tokio::sync::mpsc::channel(8);
        let finish = Arc::new(tokio::sync::Notify::new());
        let db = Database::new().await;
        let encryption = Arc::new(Encryption::initialize(&db.pool, seed(33)).await.unwrap());
        let agents = Agents::new(db.pool.clone(), encryption.clone());
        let agent = Uuid::new_v4();
        agents
            .create(CreateAgent {
                concurrency_policy: policy,
                id: agent,
                name: "Sidecar".into(),
                capabilities: tilde::iam::capabilities::Capabilities::from_wire(
                    types::Capabilities {
                        thread_read: types::BinaryPermission::Yes.into(),
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
                    target_reference: None,
                    repository: Some("acme/support".into()),
                    commit_sha: Some("abc123".into()),
                    external_id: Some("deploy-1".into()),
                    label: None,
                    commit_message: None,
                    branch: None,
                    commit_author: None,
                },
            )
            .await
            .unwrap();
        assert!(created);
        assert!(
            !deployment.routable,
            "Registration alone does not create a live connection chain"
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
                    target_reference: None,
                    repository: None,
                    commit_sha: None,
                    external_id: Some("deploy-1".into()),
                    label: None,
                    commit_message: None,
                    branch: None,
                    commit_author: None,
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
        .with_connections(connections.clone())
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
            runtime_listen: "127.0.0.1:0".parse().unwrap(),
            listen: "127.0.0.1:0".parse().unwrap(),
            public_url: None,
            idle_after: Duration::from_secs(600),
            cache_bytes: 256 * 1024 * 1024,
        };
        let first = sidecar::start(options(&gateway_url)).await.unwrap();
        let second = sidecar::start(options(&gateway_url)).await.unwrap();
        let mut agent_tasks = vec![];
        if with_agents {
            for node in [&first, &second] {
                agent_tasks.push(
                    connected_test_agent(
                        format!("http://{}/agents/{agent}", node.runtime_address),
                        token.expose_secret(),
                        sent.clone(),
                        finish.clone(),
                    )
                    .await,
                );
            }
        }

        assert_eq!(deployments.instances(agent).await.unwrap().len(), 2);
        if with_agents {
            eventually(async || {
                deployments
                    .instances(agent)
                    .await
                    .ok()
                    .filter(|rows| rows.iter().filter(|row| row.ready).count() == 2)
            })
            .await;
        }
        let ingress_token = deployments
            .issue_ingress_token(agent, None, Uuid::nil())
            .await
            .unwrap();
        Self {
            db,
            agent,
            deployments,
            connections,
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
            tasks: vec![gateway, relay, recovery]
                .into_iter()
                .chain(agent_tasks)
                .collect(),
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
        self.db
            .pool
            .get()
            .await
            .unwrap()
            .query_opt(
                "SELECT instance_id FROM thread_leases WHERE thread_id=$1 AND agent_id=$2",
                &[&thread, &self.agent],
            )
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
            self.db.pool.get().await.unwrap().execute("UPDATE agent_instances SET last_seen_at=NOW()-INTERVAL '1 minute' WHERE instance_id=$1", &[&instance]).await
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
    // A customer uses one gateway provider URL for reads, writes and live updates,
    // even when the agent executes on a sidecar. No management chat route exists.
    eventually(async || {
        fx.chat.thread(thread_id).await.ok().filter(|thread| {
            thread
                .participants
                .iter()
                .any(|p| p.agent_id.as_deref() == Some(&fx.agent.to_string()))
        })
    })
    .await;
    let mut headers = http::HeaderMap::new();
    headers.insert(
        http::header::AUTHORIZATION,
        format!("Bearer {}", fx.ingress_token.expose_secret())
            .parse()
            .unwrap(),
    );
    let provider = tilde::services::tilde::provider::tilde::v1::ChatServiceClient::new(
        connectrpc::client::HttpClient::plaintext_http2_only(),
        connectrpc::client::ClientConfig::new(
            format!("{}/agents/{}", fx.gateway_url, fx.agent)
                .parse()
                .unwrap(),
        )
        .with_default_headers(headers),
    );
    let mut updates = provider
        .watch_thread(
            tilde::proto::tilde::provider::tilde::v1::WatchThreadRequest {
                thread_id: thread_id.to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
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
    let receipt = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let update = updates.message().await.unwrap().unwrap().to_owned_message();
            if let Some(types::activity::Detail::Message(message)) = &update.activity.detail
                && message.text == "hello"
            {
                break update.cursor;
            }
        }
    })
    .await
    .expect("gateway provider stream receives projected sidecar messages");
    assert!(!receipt.is_empty());
    drop(updates);
    let mut updates = provider
        .watch_thread(
            tilde::proto::tilde::provider::tilde::v1::WatchThreadRequest {
                thread_id: thread_id.to_string(),
                after_cursor: receipt,
                ..Default::default()
            },
        )
        .await
        .unwrap();
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
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let update = updates.message().await.unwrap().unwrap().to_owned_message();
            if let Some(types::activity::Detail::Message(message)) = &update.activity.detail {
                assert_ne!(
                    message.text, "hello",
                    "resume must not replay the acknowledged message"
                );
                if message.text == "second" {
                    break;
                }
            }
        }
    })
    .await
    .expect("gateway stream resumes after its opaque cursor");
    drop(updates);
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
        fx.db.pool.get().await.unwrap().execute("UPDATE agent_instances SET last_seen_at=NOW()-INTERVAL '1 minute' WHERE instance_id=$1", &[&first_instance]).await
        .unwrap();
        let hydrated = fx
            .deployments
            .hydrate(
                fx.agent,
                Uuid::parse_str(&fx.deployment_id).unwrap(),
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
/// The agent process dials the sidecar with the deployment token, receives the wake as a frame and reports back; stopping the sidecar releases
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
    eventually(async || {
        fx.deployments
            .deployment(fx.agent, Uuid::parse_str(&fx.deployment_id).unwrap())
            .await
            .ok()
            .filter(|d| d.routable)
    })
    .await;
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
    drop(second_watch);
    eventually(async || {
        fx.deployments
            .deployment(fx.agent, Uuid::parse_str(&fx.deployment_id).unwrap())
            .await
            .ok()
            .filter(|d| !d.routable)
    })
    .await;
    fx.shutdown().await;
}

/// The projected status of a run, with or without an invocation.
async fn run_status(fx: &Fixture, run: Uuid) -> String {
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT status FROM chat_runs WHERE id=$1", &[&run])
        .await
        .unwrap()
        .get("status")
}
/// Publish frames as a replica would, straight into the gateway's projection.
fn event(
    agent: Uuid,
    instance: Uuid,
    thread: Uuid,
    sequence: i64,
    kind: &str,
    state: types::runtime_event::State,
) -> wire::Upstream {
    wire::Upstream {
        frame: Some(
            wire::Event {
                event: types::RuntimeEvent {
                    id: Uuid::new_v4().to_string(),
                    thread_id: thread.to_string(),
                    agent_id: agent.to_string(),
                    origin_instance_id: instance.to_string(),
                    origin_agent_id: agent.to_string(),
                    origin_sequence: sequence,
                    created_at: chrono::Utc::now().timestamp_millis(),
                    kind: kind.into(),
                    state: Some(state),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            }
            .into(),
        ),
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_projection_keeps_publish_order_and_checks_thread_membership() {
    let fx = Fixture::new().await;
    let deployment = Uuid::parse_str(&fx.deployment_id).unwrap();
    let (thread_id, _, _) = fx.room().await;
    // The room is created on a replica and projected behind it; wait for the roster.
    eventually(async || {
        fx.chat.thread(thread_id).await.ok().filter(|t| {
            t.participants
                .iter()
                .any(|p| p.agent_id.as_deref() == Some(&fx.agent.to_string()))
        })
    })
    .await;
    // A third replica, registered and live, takes the lease on the room.
    let instance = Uuid::new_v4();
    fx.deployments
        .register(
            fx.agent,
            deployment,
            &wire::WatchRequest {
                instance_id: instance.to_string(),
                public_url: "http://127.0.0.1:9".into(),
                runtime_url: "http://127.0.0.1:9".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // This test supplies Publish frames directly; model its open Watch generation.
    fx.db.pool.get().await.unwrap().execute(
        "UPDATE agent_instances SET connection_id=$1,agent_connected=true WHERE agent_id=$2 AND instance_id=$3",
        &[&Uuid::new_v4(),&fx.agent,&instance],
    ).await.unwrap();
    fx.deployments
        .heartbeat(
            fx.agent,
            instance,
            &wire::Heartbeat {
                ready: true,
                agent_ready: true,
                agent_connected: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // Take it back from whichever replica created the room: age that holder first.
    if let Some(holder) = fx.holder(thread_id).await {
        fx.db.pool.get().await.unwrap().execute("UPDATE agent_instances SET last_seen_at=NOW()-INTERVAL '1 minute' WHERE instance_id=$1", &[&holder]).await
            .unwrap();
    }
    let hydrated = fx
        .deployments
        .hydrate(
            fx.agent,
            deployment,
            Some(instance),
            wire::HydrateRequest {
                key: Some(wire::hydrate_request::Key::ThreadId(thread_id.to_string())),
                lease: true,
                instance_id: instance.to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let lease = hydrated.lease.into_option().unwrap();
    assert!(lease.held && lease.holder_instance_id == instance.to_string());
    assert!(lease.version > 0, "lease frames carry the row version");
    // The pair is now pinned to this deployment.
    let pinned = fx.deployments.pinned(fx.agent, thread_id).await.unwrap();
    assert_eq!(pinned, Some(deployment));
    // A run completes and the thread is released in the same publish: the completion
    // is recorded first, so the lease going does not discard it.
    let run_id = Uuid::new_v4();
    let run = types::Run {
        id: run_id.to_string(),
        thread_id: thread_id.to_string(),
        agent_id: fx.agent.to_string(),
        objective: "ordered".into(),
        status: "waiting".into(),
        ..Default::default()
    };
    let response = fx
        .deployments
        .publish(
            fx.agent,
            deployment,
            instance,
            wire::PublishRequest {
                instance_id: instance.to_string(),
                frames: vec![
                    event(fx.agent, instance, thread_id, 1, "run.updated", run.into()),
                    wire::Upstream {
                        frame: Some(
                            wire::Release {
                                thread_id: thread_id.to_string(),
                                ..Default::default()
                            }
                            .into(),
                        ),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(response.rejected_event_ids.is_empty(), "{response:?}");
    assert!(response.lost.is_empty(), "{response:?}");
    assert_eq!(run_status(&fx, run_id).await, "waiting");
    assert_eq!(
        fx.holder(thread_id).await,
        None,
        "the release still applied"
    );
    // Run state published after the release is refused with the (empty) lease.
    let late = fx
        .deployments
        .publish(
            fx.agent,
            deployment,
            instance,
            wire::PublishRequest {
                instance_id: instance.to_string(),
                frames: vec![event(
                    fx.agent,
                    instance,
                    thread_id,
                    2,
                    "run.updated",
                    types::Run {
                        id: run_id.to_string(),
                        thread_id: thread_id.to_string(),
                        agent_id: fx.agent.to_string(),
                        status: "failed".into(),
                        ..Default::default()
                    }
                    .into(),
                )],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(late.lost.len(), 1);
    assert_eq!(run_status(&fx, run_id).await, "waiting");
    // An agent that is not in a thread cannot append to it, even without run state.
    // The thread belongs to this agent on paper but has no participant row for it.
    let other_thread = Uuid::new_v4();
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO chat_threads(id,title,primary_agent_id) VALUES($1,'Private',$2)",
            &[&other_thread, &fx.agent],
        )
        .await
        .unwrap();
    let intruding = fx
        .deployments
        .publish(
            fx.agent,
            deployment,
            instance,
            wire::PublishRequest {
                instance_id: instance.to_string(),
                frames: vec![event(
                    fx.agent,
                    instance,
                    other_thread,
                    3,
                    "message.created",
                    types::Message {
                        id: Uuid::new_v4().to_string(),
                        thread_id: other_thread.to_string(),
                        participant_id: Uuid::new_v4().to_string(),
                        text: "intrusion".into(),
                        status: "complete".into(),
                        ..Default::default()
                    }
                    .into(),
                )],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(intruding.rejected_event_ids.len(), 1, "{intruding:?}");
    assert!(fx.chat.messages(other_thread, 10).await.unwrap().is_empty());
    fx.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sidecar_message_policies_dispatch_fresh_scoped_invocations() {
    use tilde::agent::ConcurrencyPolicy as Policy;
    for policy in [Policy::Queue, Policy::QueueAndBatch, Policy::Interrupt] {
        let mut fx = Fixture::with_policy(true, policy).await;
        let base = fx.base(fx.first());
        let (thread, alice, _) = fx.room().await;
        assert_eq!(fx.post(&base, thread, &alice, "first").await.0, 200);
        let first = fx.invocation("first").await;
        assert_eq!(fx.post(&base, thread, &alice, "second").await.0, 200);
        if policy == Policy::Interrupt {
            let next = fx.invocation("second").await;
            assert_eq!(next.objective, "second");
            assert_ne!(first.invocation_id, next.invocation_id);
            let response = fx
                .http
                .post(format!(
                    "{}/tilde.runtime.v1.ChatService/ListMessages",
                    first.callback_url
                ))
                .bearer_auth(&first.capability)
                .header("content-type", "application/json")
                .body("{}")
                .send()
                .await
                .unwrap();
            assert!(
                response.status().is_success(),
                "An issued token retains its scoped read authority until expiry"
            );
            let renewal = fx
                .http
                .post(format!(
                    "{}/tilde.runtime.v1.ChatService/RenewConnectToken",
                    first.callback_url
                ))
                .bearer_auth(&first.capability)
                .json(&serde_json::json!({}))
                .send()
                .await
                .unwrap();
            assert!(
                !renewal.status().is_success(),
                "Interrupted invocation cannot renew its token"
            );
        } else {
            assert_eq!(fx.post(&base, thread, &alice, "third").await.0, 200);
            assert!(
                tokio::time::timeout(Duration::from_millis(200), fx.calls.recv())
                    .await
                    .is_err()
            );
            let response = fx
                .http
                .post(format!(
                    "{}/tilde.runtime.v1.ChatService/ListMessages",
                    first.callback_url
                ))
                .bearer_auth(&first.capability)
                .header("content-type", "application/json")
                .body("{}")
                .send()
                .await
                .unwrap();
            let status = response.status();
            let history: serde_json::Value = response.json().await.unwrap();
            assert!(status.is_success(), "{history}");
            assert!(
                history["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|message| message["text"] != "second" && message["text"] != "third")
            );
            fx.finish.notify_one();
            let next = fx.invocation("queued response").await;
            assert_ne!(first.invocation_id, next.invocation_id);
            if policy == Policy::Queue {
                assert_eq!(next.objective, "second");
                assert!(next.messages.iter().all(|message| message.text != "third"));
                fx.finish.notify_one();
                assert_eq!(fx.invocation("third").await.objective, "third");
            } else {
                assert_eq!(next.objective, "second\nthird");
            }
        }
        fx.finish.notify_waiters();
        fx.shutdown().await;
    }
}

/// A stand-in OpenAI-compatible provider that records the credential it was given.
async fn inference_provider() -> (
    String,
    Arc<std::sync::Mutex<Vec<String>>>,
    tokio::task::JoinHandle<()>,
) {
    let seen: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
    let state = seen.clone();
    let router = axum::Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(
            move |headers: axum::http::HeaderMap, _body: axum::body::Bytes| {
                let state = state.clone();
                async move {
                    state.lock().unwrap().push(
                        headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or_default()
                            .to_owned(),
                    );
                    axum::Json(serde_json::json!({"id":"c1","choices":[{"message":{"role":"assistant","content":"hi"}}],"usage":{"prompt_tokens":12,"completion_tokens":3}}))
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (url, seen, task)
}
/// Inference credentials replicate to sidecars with the rest of the configuration; the
/// replica forwards from memory and ships usage back through its outbox.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sidecars_forward_inference_from_replicated_credentials_and_ship_usage() {
    use tilde::connections::model::{Assignment, Capability, Values};
    let mut fx = Fixture::new().await;
    let (upstream, seen, upstream_task) = inference_provider().await;
    let encryption = Arc::new(Encryption::initialize(&fx.db.pool, seed(33)).await.unwrap());
    let connections = Connections::new(
        fx.db.pool.clone(),
        encryption,
        "http://localhost:1".into(),
        "http://localhost:2".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    // Connect OpenAI through the broker, assigned to this agent for inference.
    let connection = Uuid::new_v4();
    let started = connections
        .start(
            connection,
            "OpenAI",
            "openai",
            "api",
            &[Assignment {
                capability: Capability::Inference,
                agent_id: fx.agent,
                alias: Some("primary".into()),
            }],
        )
        .await
        .unwrap();
    let url = url::Url::parse(&started.brokering_url).unwrap();
    let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let setup_token = url
        .query_pairs()
        .find(|(k, _)| k == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    let view = connections.view(setup, &setup_token).await.unwrap();
    let view = connections
        .set_connection_name(setup, &setup_token, view.action_id, "prod")
        .await
        .unwrap();
    let values: Values = [
        ("api_key".to_string(), SecretString::from("sk-sidecar")),
        (
            "base_url".to_string(),
            SecretString::from(upstream.as_str()),
        ),
    ]
    .into_iter()
    .collect();
    connections
        .advance(setup, &setup_token, view.action_id, values)
        .await
        .unwrap();
    // The configuration frame reaches both replicas without any sidecar-side lookup.
    for replica in [fx.first(), fx.second()] {
        let runtime = replica.nodes[0].runtime.clone();
        eventually(async || {
            let configuration = runtime.configuration().ok()?;
            configuration
                .connections
                .iter()
                .any(|c| {
                    c.capability == types::Capability::Inference
                        && c.status == "ready"
                        && c.name == "prod"
                        && c.credentials.iter().any(|f| f.name == "api_key")
                })
                .then_some(())
        })
        .await;
    }
    let base = fx.base(fx.first());
    let (thread, alice, _) = fx.room().await;
    assert_eq!(fx.post(&base, thread, &alice, "hello").await.0, 200);
    let invocation = fx.invocation("hello").await;
    // The agent process calls the provider through its own runtime listener, with the
    // invocation token it was woken with; the replica swaps in the real key.
    let response = fx
        .http
        .post(format!(
            "{}/inference/openai/prod/chat/completions",
            invocation.callback_url
        ))
        .bearer_auth(&invocation.capability)
        .json(&serde_json::json!({"model":"gpt-5","messages":[{"role":"user","content":"hi"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["usage"]["prompt_tokens"], 12);
    assert_eq!(seen.lock().unwrap().as_slice(), ["Bearer sk-sidecar"]);
    let aliased = fx
        .http
        .post(format!(
            "{}/inference/primary/chat/completions",
            invocation.callback_url
        ))
        .bearer_auth(&invocation.capability)
        .json(&serde_json::json!({"model":"gpt-5","messages":[{"role":"user","content":"hi"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        aliased.status(),
        200,
        "the alias replicated with the configuration"
    );
    assert_eq!(
        fx.http
            .post(format!(
                "{}/inference/openai/staging/chat/completions",
                invocation.callback_url
            ))
            .bearer_auth(&invocation.capability)
            .body("{}")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    // Usage travels through the outbox and lands in Postgres against this agent.
    let row = eventually(async || {
        tilde::inference::db::requests_for_agent_all(&fx.db.pool.get().await.ok()?, fx.agent)
            .await
            .ok()?
            .into_iter()
            .next()
    })
    .await;
    assert_eq!(
        (
            row.kind.as_str(),
            row.model.as_deref(),
            row.input_tokens,
            row.output_tokens,
            row.status
        ),
        ("chat", Some("gpt-5"), Some(12), Some(3), 200)
    );
    assert_eq!(row.connection_id, connection);
    assert_eq!(row.invocation_id.to_string(), invocation.invocation_id);
    fx.finish.notify_one();
    upstream_task.abort();
    fx.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sidecar_conversations_keep_their_owner_when_latest_becomes_lambda() {
    let mut fx = Fixture::new().await;
    let (thread, alice, _) = fx.room().await;
    let base = fx.base(fx.first());
    let (status, body) = fx.post(&base, thread, &alice, "before release").await;
    assert_eq!(status, 200, "{body}");
    let invocation = fx.invocation("before release").await;
    fx.projected(thread, "before release").await;
    let old = Uuid::parse_str(&fx.deployment_id).unwrap();
    assert_eq!(
        fx.deployments.pinned(fx.agent, thread).await.unwrap(),
        Some(old)
    );
    let (release, _, _) = fx
        .deployments
        .register_deployment(
            fx.agent,
            tilde::deployment::RegisterDeployment {
                source: types::DeploymentSource::Ci,
                target: types::DeploymentTarget::Lambda,
                target_reference: Some(
                    "arn:aws:lambda:eu-central-1:123456789012:function:next".into(),
                ),
                external_id: Some("mixed-lambda".into()),
                label: None,
                repository: None,
                commit_sha: None,
                commit_message: None,
                branch: None,
                commit_author: None,
            },
        )
        .await
        .unwrap();
    assert!(release.serving);
    let (status, body) = fx
        .post(&fx.gateway_url, thread, &alice, "after release")
        .await;
    assert_eq!(status, 200, "{body}");
    fx.projected(thread, "after release").await;
    assert_eq!(
        fx.deployments.pinned(fx.agent, thread).await.unwrap(),
        Some(old)
    );
    assert_eq!(fx.holder(thread).await, Some(fx.first().instance));
    let next = fx
        .chat
        .create_thread(tilde::chat::CreateThread {
            title: "Lambda conversation".into(),
            primary_agent_id: fx.agent.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(fx.agent.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let run = fx
        .chat
        .start_run(tilde::chat::StartRun {
            thread_id: next.id.clone(),
            agent_id: fx.agent.to_string(),
            objective: "Next type".into(),
            idempotency_key: "mixed-next".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let row = tilde::chat::db::invocation_endpoint_one(
        &fx.db.pool.get().await.unwrap(),
        Uuid::parse_str(&run.invocation_id).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(row.target.as_deref(), Some("lambda"));
    assert_eq!(row.deployment_id.map(|id| id.to_string()), Some(release.id));
    assert_eq!(
        invocation.owner_instance_id,
        fx.first().instance.to_string()
    );
    fx.finish.notify_one();
    fx.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_provider_identity_and_queue_survive_a_sidecar_handoff() {
    use base64::Engine as _;
    use serde_json::json;
    async fn call(
        fx: &Fixture,
        key: &SecretString,
        identity: &str,
        method: &str,
        body: serde_json::Value,
    ) -> (u16, serde_json::Value) {
        let response = fx
            .http
            .post(format!(
                "{}/agents/{}/tilde.provider.tilde.v1.ChatService/{method}",
                fx.gateway_url, fx.agent
            ))
            .bearer_auth(key.expose_secret())
            .header(
                "x-tilde-identity",
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(identity),
            )
            .header("connect-protocol-version", "1")
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap())
    }
    let mut fx = Fixture::new().await;
    eventually(async || {
        fx.deployments
            .instances(fx.agent)
            .await
            .ok()
            .filter(|instances| {
                instances.len() == 2 && instances.iter().all(|instance| instance.ready)
            })
    })
    .await;
    // The agent's Tilde connection was created with it, private. Open it for the handoff
    // part of this test; access modes are exercised at the end.
    let access = tilde::chat::access::AgentAccess::new(fx.connections.clone(), fx.chat.clone());
    let tilde =
        tilde::connections::db::tilde_connection_opt(&fx.db.pool.get().await.unwrap(), fx.agent)
            .await
            .unwrap()
            .expect("every agent owns a Tilde connection")
            .id;
    let key = fx.chat.tilde_chat_key(fx.agent, false).await.unwrap();
    assert_eq!(
        call(&fx, &key, "alice", "GetIdentity", json!({})).await.0,
        403,
        "private by default: an unseen identity is refused"
    );
    access
        .set_mode(tilde, fx.agent, types::ChannelAccessMode::Public)
        .await
        .unwrap();
    let (status, identity) = call(&fx, &key, "alice", "GetIdentity", json!({})).await;
    assert_eq!(status, 200, "{identity}");
    let user = identity["user"]["id"].as_str().unwrap().to_owned();
    let (status, created) = call(
        &fx,
        &key,
        "alice",
        "CreateThread",
        json!({"title":"Native","primaryAgentId":fx.agent}),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let thread = Uuid::parse_str(created["thread"]["id"].as_str().unwrap()).unwrap();
    let actor = created["thread"]["participants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["userId"] == user)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // Creation is immediately readable through the same gateway provider URL.
    assert_eq!(
        call(&fx, &key, "alice", "GetSession", json!({"threadId":thread}))
            .await
            .0,
        200
    );
    assert_eq!(
        call(&fx, &key, "bob", "GetSession", json!({"threadId":thread}))
            .await
            .0,
        403
    );
    let (status, listed) = call(&fx, &key, "bob", "ListSessions", json!({})).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["sessions"].as_array().map(Vec::len).unwrap_or(0), 0);

    let mut ids = vec![];
    for text in ["first", "second", "third", "fourth"] {
        let id = Uuid::new_v4();
        ids.push(id);
        let (status, response) = call(
            &fx,
            &key,
            "alice",
            "PostMessage",
            json!({"id":id,"threadId":thread,"participantId":actor,"text":text}),
        )
        .await;
        assert_eq!(status, 200, "{response}");
        if text == "first" {
            assert_eq!(fx.invocation("first").await.objective, "first");
        }
    }
    assert_eq!(
        call(
            &fx,
            &key,
            "alice",
            "RemoveQueuedMessage",
            json!({"threadId":thread,"id":ids[2]})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &fx,
            &key,
            "alice",
            "ReorderQueuedMessage",
            json!({"threadId":thread,"id":ids[3],"beforeId":ids[1]})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &fx,
            &key,
            "alice",
            "RenameSession",
            json!({"threadId":thread,"title":"Renamed native"})
        )
        .await
        .0,
        200
    );
    eventually(async || {
        let hydrated = fx
            .deployments
            .hydrate(
                fx.agent,
                Uuid::parse_str(&fx.deployment_id).unwrap(),
                None,
                wire::HydrateRequest {
                    key: Some(wire::hydrate_request::Key::ThreadId(thread.to_string())),
                    ..Default::default()
                },
            )
            .await
            .ok()?;
        (hydrated.thread.title == "Renamed native"
            && hydrated
                .queued_inputs
                .iter()
                .map(|i| i.id.clone())
                .collect::<Vec<_>>()
                == vec![ids[3].to_string(), ids[1].to_string()])
        .then_some(())
    })
    .await;
    let holder = fx.holder(thread).await.unwrap();
    let survivor = if fx.first().instance == holder {
        let survivor = fx.second().instance;
        fx.first.take().unwrap().stop().await;
        survivor
    } else {
        let survivor = fx.first().instance;
        fx.second.take().unwrap().stop().await;
        survivor
    };
    fx.fail_over(holder, thread, survivor).await;
    assert_eq!(fx.invocation("restored first").await.objective, "first");
    let (status, queue) = call(
        &fx,
        &key,
        "alice",
        "ListQueuedMessages",
        json!({"threadId":thread}),
    )
    .await;
    assert_eq!(status, 200, "{queue}");
    assert_eq!(
        queue["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![ids[3].to_string(), ids[1].to_string()]
    );
    let (status, result) = call(
        &fx,
        &key,
        "alice",
        "SteerQueuedMessage",
        json!({"threadId":thread,"id":ids[3]}),
    )
    .await;
    assert_eq!(status, 200, "{result}");
    assert_eq!(
        fx.invocation("fourth after steering").await.objective,
        "fourth"
    );
    fx.finish.notify_waiters();
    assert_eq!(
        fx.invocation("second after fourth").await.objective,
        "second"
    );
    fx.finish.notify_waiters();
    // Access modes gate the application-key exchange like any other channel: disabled admits
    // nobody, private only allowed identities while still recording unknown ones, public
    // everyone. Identities count as attested because the application asserts them.
    access
        .set_mode(tilde, fx.agent, types::ChannelAccessMode::Disabled)
        .await
        .unwrap();
    assert_eq!(
        call(&fx, &key, "alice", "GetIdentity", json!({})).await.0,
        403
    );
    access
        .set_mode(tilde, fx.agent, types::ChannelAccessMode::Private)
        .await
        .unwrap();
    assert_eq!(
        call(&fx, &key, "alice", "GetIdentity", json!({})).await.0,
        403
    );
    assert_eq!(
        call(&fx, &key, "carol", "GetIdentity", json!({})).await.0,
        403
    );
    let (recorded, _) = access
        .identities(fx.agent, Some(tilde), None, 10)
        .await
        .unwrap();
    let carol = recorded
        .iter()
        .find(|i| i.value == "carol")
        .expect("refused identities are still recorded");
    assert!(!carol.allowed && carol.attested_at.is_set());
    let alice = recorded.iter().find(|i| i.value == "alice").unwrap();
    access
        .set_allowed(tilde, fx.agent, Uuid::parse_str(&alice.id).unwrap(), true)
        .await
        .unwrap();
    assert_eq!(
        call(&fx, &key, "alice", "GetIdentity", json!({})).await.0,
        200
    );
    // Added ahead of first contact through the generic flow: nothing to deliver, allowed at once.
    let (_, status) = access
        .request_verification(tilde::chat::access::VerificationRequest {
            id: Uuid::new_v4(),
            connection: tilde,
            agent: fx.agent,
            identity_type: types::IdentityType::Username,
            value: "dave".into(),
            template_name: None,
            template_language: None,
        })
        .await
        .unwrap();
    assert_eq!(status, "approved");
    assert_eq!(
        call(&fx, &key, "dave", "GetIdentity", json!({})).await.0,
        200
    );
    access
        .set_mode(tilde, fx.agent, types::ChannelAccessMode::Public)
        .await
        .unwrap();
    assert_eq!(
        call(&fx, &key, "carol", "GetIdentity", json!({})).await.0,
        200
    );
    fx.shutdown().await;
}
