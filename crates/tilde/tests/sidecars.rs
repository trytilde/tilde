#[allow(dead_code)]
mod common;
use buffa::Message;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tilde::proto::tilde::{agent_host::v1 as host, types::v1 as types};
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
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replicas_own_execute_project_forward_and_fail_over_through_the_gateway() {
    let (endpoint, mut calls, finish, host_task) = agent_host().await;
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(33)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let agent = Uuid::new_v4();
    agents
        .create(CreateAgent {
            id: agent,
            name: "Sidecar".into(),
            endpoint_url: "http://127.0.0.1:1".into(),
            webhook_signing_key: SecretString::from("sidecar-test-agent-signing-key-0123456789"),
            capabilities: Default::default(),
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
            None,
            types::SidecarFailureMode::Reassign,
        )
        .await
        .unwrap();
    agents.resume(agent).await.unwrap();
    let token = deployments.issue_token(agent).await.unwrap();
    assert_eq!(
        deployments
            .authenticate(token.expose_secret())
            .await
            .unwrap(),
        agent
    );
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
    let gateway = tokio::spawn(axum::serve(listener, router).into_future());
    let (stop, workers_rx) = tokio::sync::watch::channel(false);
    let relay = tokio::spawn(deployments.clone().relay_worker(workers_rx.clone()));
    let recovery = tokio::spawn(deployments.clone().recovery_worker(workers_rx));
    let options = |gateway_url: &str| sidecar::Options {
        gateway_url: gateway_url.to_owned(),
        tokens: vec![token.clone()],
        endpoints: BTreeMap::from([(agent, endpoint.clone())]),
        runtime_listen: "127.0.0.1:0".parse().unwrap(),
        listen: "127.0.0.1:0".parse().unwrap(),
        public_url: None,
    };
    let first = sidecar::start(options(&gateway_url)).await.unwrap();
    let second = sidecar::start(options(&gateway_url)).await.unwrap();
    assert_eq!(deployments.nodes(agent).await.unwrap().len(), 2);
    let first_base = format!("http://{}", first.address);
    let second_base = format!("http://{}", second.address);
    let http = reqwest::Client::new();
    let ingress_token = deployments
        .issue_ingress_token(agent, None, Uuid::nil())
        .await
        .unwrap();
    // The replica that receives the first request owns the conversation and runs the agent.
    let (status, user) = ingress(
        &http,
        &first_base,
        agent,
        &ingress_token,
        "CreateUser",
        serde_json::json!({"name":"Alice"}),
    )
    .await;
    assert_eq!(status, 200, "{user}");
    let user_id = user["user"]["id"].as_str().unwrap().to_owned();
    let (status, thread) = ingress(
        &http,
        &first_base,
        agent,
        &ingress_token,
        "CreateThread",
        serde_json::json!({"title":"Room","primaryAgentId":agent,"participants":[{"userId":user_id},{"agentId":agent}]}),
    )
    .await;
    assert_eq!(status, 200, "{thread}");
    let thread_id = Uuid::parse_str(thread["thread"]["id"].as_str().unwrap()).unwrap();
    let participants = thread["thread"]["participants"].as_array().unwrap();
    let alice = participants
        .iter()
        .find(|p| p["userId"].is_string())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, posted) = ingress(&http, &first_base, agent, &ingress_token, "PostMessage", serde_json::json!({"id":Uuid::new_v4(),"threadId":thread_id,"participantId":alice,"text":"hello"})).await;
    assert_eq!(status, 200, "{posted}");
    let invocation = tokio::time::timeout(Duration::from_secs(10), calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(invocation.owner_instance_id, first.instance.to_string());
    assert_eq!(invocation.assignment_generation, 1);
    assert_eq!(invocation.objective, "hello");
    assert_eq!(invocation.thread_id, thread_id.to_string());
    finish.notify_one();
    // The owner's events reach Postgres without the gateway ever calling the replica.
    let projected = eventually(async || {
        let messages = chat.messages(thread_id, 10).await.ok()?;
        let run = chat
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
    use sqlx::Row;
    let owner = sqlx::query("SELECT owner_instance_id,generation FROM participant_assignments WHERE thread_id=$1 AND agent_id=$2").bind(thread_id).bind(agent).fetch_one(&db.pool).await.unwrap();
    assert_eq!(owner.get::<Uuid, _>("owner_instance_id"), first.instance);
    assert_eq!(owner.get::<i64, _>("generation"), 1);
    // A request landing on the other replica is executed by the owner through the gateway.
    let (status, forwarded) = ingress(&http, &second_base, agent, &ingress_token, "PostMessage", serde_json::json!({"id":Uuid::new_v4(),"threadId":thread_id,"participantId":alice,"text":"second"})).await;
    assert_eq!(status, 200, "{forwarded}");
    let invocation = tokio::time::timeout(Duration::from_secs(10), calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(invocation.owner_instance_id, first.instance.to_string());
    assert_eq!(invocation.objective, "second");
    // Gateway public ingress reads and writes through the owner as well.
    let (status, read) = ingress(
        &http,
        &gateway_url,
        agent,
        &ingress_token,
        "GetThread",
        serde_json::json!({"id":thread_id}),
    )
    .await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(
        read["thread"]["id"].as_str().unwrap(),
        thread_id.to_string()
    );
    let (status, listed) = ingress(
        &http,
        &second_base,
        agent,
        &ingress_token,
        "ListMessages",
        serde_json::json!({"threadId":thread_id,"limit":10}),
    )
    .await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["messages"].as_array().unwrap().len(), 2);
    // The owner dies mid-invocation: the gateway reassigns and the survivor restarts the run.
    let pending_run = Uuid::parse_str(&invocation.run_id).unwrap();
    eventually(async || {
        chat.run(pending_run)
            .await
            .ok()
            .filter(|r| matches!(r.invocation_status.as_str(), "pending" | "running"))
    })
    .await;
    eventually(async || {
        deployments
            .nodes(agent)
            .await
            .ok()
            .filter(|nodes| nodes.len() == 2 && nodes.iter().all(|n| n.ready))
    })
    .await;
    let first_instance = first.instance;
    first.stop().await;
    // A heartbeat already in flight may still land after the stop; age the node until
    // the gateway (the worker or this direct call) sees it as dead and reassigns.
    eventually(async || {
        sqlx::query(
            "UPDATE sidecar_nodes SET last_seen_at=NOW()-INTERVAL '1 minute' WHERE instance_id=$1",
        )
        .bind(first_instance)
        .execute(&db.pool)
        .await
        .unwrap();
        deployments.recover().await.unwrap();
        let owner = sqlx::query("SELECT owner_instance_id,generation FROM participant_assignments WHERE thread_id=$1 AND agent_id=$2")
            .bind(thread_id)
            .bind(agent)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        (owner.get::<i64, _>("generation") == 2 && owner.get::<Uuid, _>("owner_instance_id") == second.instance).then_some(())
    })
    .await;
    let recovered = tokio::time::timeout(Duration::from_secs(10), calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.owner_instance_id, second.instance.to_string());
    assert_eq!(recovered.assignment_generation, 2);
    assert_eq!(recovered.objective, "second");
    finish.notify_one();
    finish.notify_one();
    let owner = sqlx::query("SELECT owner_instance_id,generation FROM participant_assignments WHERE thread_id=$1 AND agent_id=$2").bind(thread_id).bind(agent).fetch_one(&db.pool).await.unwrap();
    assert_eq!(owner.get::<Uuid, _>("owner_instance_id"), second.instance);
    assert_eq!(owner.get::<i64, _>("generation"), 2);
    // Afterwards the survivor serves the conversation directly.
    let (status, after) = ingress(&http, &second_base, agent, &ingress_token, "PostMessage", serde_json::json!({"id":Uuid::new_v4(),"threadId":thread_id,"participantId":alice,"text":"third"})).await;
    assert_eq!(status, 200, "{after}");
    let invocation = tokio::time::timeout(Duration::from_secs(10), calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(invocation.owner_instance_id, second.instance.to_string());
    assert_eq!(invocation.assignment_generation, 2);
    finish.notify_one();
    eventually(async || {
        chat.messages(thread_id, 10)
            .await
            .ok()
            .filter(|m| m.iter().any(|m| m.text == "third"))
    })
    .await;
    second.stop().await;
    let _ = stop.send(true);
    relay.abort();
    recovery.abort();
    gateway.abort();
    host_task.abort();
    db.close().await;
}
