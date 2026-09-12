#[allow(dead_code)]
mod common;
use common::{Database, seed};
use futures::StreamExt;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tilde::proto::tilde::{agent_event_ingress::v1 as control, types::v1 as types};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, CreateThread},
    connections::service::Connections,
    deployment::{
        Deployments,
        corrosion::process::Process,
        runtime::{Runtime, RuntimeOptions},
    },
    encryption::Encryption,
};
use uuid::Uuid;
fn port() -> SocketAddr {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}
fn udp() -> SocketAddr {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}
fn binary() -> std::path::PathBuf {
    std::env::var_os("ENGINE_TEST_CORROSION_BINARY")
        .map(Into::into)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tools/corrosion")
        })
}
#[derive(Deserialize)]
struct CommandObservation {
    id: String,
    acked_at: Option<i64>,
    finished_at: Option<i64>,
}
async fn object_store() -> (
    tilde::agent::avatar::AvatarStore,
    tokio::task::JoinHandle<()>,
) {
    type Objects = Arc<tokio::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>>;
    async fn serve(
        axum::extract::State(objects): axum::extract::State<Objects>,
        method: http::Method,
        uri: http::Uri,
        body: axum::body::Bytes,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        let mut objects = objects.lock().await;
        match method {
            http::Method::PUT => {
                objects.insert(uri.path().into(), body.to_vec());
                http::StatusCode::OK.into_response()
            }
            http::Method::GET => match objects.get(uri.path()) {
                Some(bytes) => bytes.clone().into_response(),
                None => http::StatusCode::NOT_FOUND.into_response(),
            },
            http::Method::DELETE => {
                objects.remove(uri.path());
                http::StatusCode::NO_CONTENT.into_response()
            }
            _ => http::StatusCode::METHOD_NOT_ALLOWED.into_response(),
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = axum::Router::new()
        .fallback(serve)
        .with_state(Arc::new(tokio::sync::Mutex::new(
            std::collections::BTreeMap::new(),
        )));
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let store = tilde::agent::avatar::AvatarStore::new(
        "sidecar-attachments".into(),
        "us-east-1".into(),
        url,
        None,
        Some(client_aws_sigv4::Credentials {
            access_key_id: "fixture".into(),
            secret_access_key: "fixture-secret".into(),
            session_token: None,
        }),
    )
    .unwrap();
    (store, task)
}
async fn host() -> (
    String,
    tokio::sync::mpsc::Receiver<tilde::proto::tilde::agent_host::v1::InvokeRequest>,
    Arc<tokio::sync::Notify>,
    tokio::task::JoinHandle<()>,
) {
    use buffa::Message;
    use tilde::proto::tilde::agent_host::v1 as host;
    let (sent, received) = tokio::sync::mpsc::channel(8);
    let finish = Arc::new(tokio::sync::Notify::new());
    let done = finish.clone();
    let service = connectrpc::Router::new().route_view_server_stream::<_,_,host::InvokeResponse>(
        "tilde.agent_host.v1.AgentService", "Invoke", connectrpc::view_streaming_handler_fn(move |ctx: connectrpc::RequestContext, request: tilde::services::tilde::agent_host::v1::OwnedInvokeRequestView| {
            let sent = sent.clone(); let done = done.clone();
            async move {
                assert!(ctx.headers().contains_key("x-tilde-signature"));
                let request = host::InvokeRequest::decode_from_slice(request.bytes()).unwrap();
                let command = request.command_id.clone();
                sent.send(request).await.unwrap();
                connectrpc::Response::stream_ok(async_stream::try_stream! {
                    yield host::InvokeResponse { accepted_command_id: command, ..Default::default() };
                    done.notified().await;
                })
            }
        })).into_axum_service();
    let router =
        axum::Router::new().route_service("/tilde.agent_host.v1.AgentService/Invoke", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (endpoint, received, finish, task)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ha_sidecars_execute_archive_failover_retire_and_proxy_without_rehydration() {
    let (endpoint, mut calls, finish, host_task) = host().await;
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
    let deployments = Deployments::new(db.pool.clone(), encryption, agents.clone(), connections);
    deployments
        .set(
            agent,
            types::DeploymentMode::Sidecar,
            None,
            types::SidecarFailureMode::Reassign,
            7,
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
    let configuration = deployments.configuration(agent).await.unwrap();
    let directory = std::env::temp_dir().join(format!("tilde-sidecar-test-{}", Uuid::new_v4()));
    let mut processes = vec![];
    let mut ingress_servers = vec![];
    let mut runtimes = vec![];
    for index in 0..2 {
        let instance = Uuid::new_v4();
        let gossip = udp();
        let api = port();
        let ingress_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ingress_url = format!("http://{}", ingress_listener.local_addr().unwrap());
        let registration = deployments
            .register(
                agent,
                control::RegisterSidecarRequest {
                    instance_id: instance.to_string(),
                    public_ingress_url: "http://127.0.0.1:8082".into(),
                    agent_ingress_url: ingress_url,
                    runtime_url: "http://127.0.0.1:8081".into(),
                    gossip_address: gossip.to_string(),
                    local_agent_endpoint: "http://127.0.0.1:1".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(registration.peers.len(), index);
        let api_token = SecretString::from(registration.corrosion_token.clone());
        let process = Process::start(
            binary(),
            &directory.join(index.to_string()),
            api,
            gossip,
            &api_token,
            &registration,
        )
        .await
        .unwrap();
        let runtime = Arc::new(Runtime::new(
            process.client.clone(),
            RuntimeOptions {
                agent_id: agent,
                instance_id: instance,
                encryption: Arc::new(
                    Encryption::from_agent_key(agent, registration.encryption_key.clone().into())
                        .unwrap(),
                ),
                signing_key: registration.token_signing_key.clone().into(),
                host_key: configuration.webhook_signing_key.clone().into(),
                local_endpoint: endpoint.clone(),
                callback_url: "http://127.0.0.1:8081".into(),
            },
        ));
        runtime.configure(&configuration).await.unwrap();
        let node = tilde::deployment::sidecar::Node::new(
            runtime.clone(),
            tilde::deployment::gateway::Client::new("http://127.0.0.1:1", token.clone()).unwrap(),
            api_token,
            format!("http://{api}"),
        );
        ingress_servers.push(tokio::spawn(async move {
            axum::serve(ingress_listener, node.agent_router())
                .await
                .unwrap()
        }));
        processes.push(process);
        runtimes.push(runtime);
    }
    #[derive(Deserialize)]
    struct ThreadRow {
        id: String,
    }
    let mut changes = runtimes[1]
        .client
        .subscribe::<ThreadRow>("SELECT id,payload FROM threads", vec![])
        .await
        .unwrap();
    // Closing the gateway pool demonstrates that the conversation path is local.
    db.pool.close().await;
    let writer = Chat::from_sidecar(runtimes[0].clone());
    let reader = Chat::from_sidecar(runtimes[1].clone());
    let human = writer.create_user("Alice").await.unwrap();
    let thread = writer
        .create_thread(CreateThread {
            title: "Replicated conversation".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![
                types::ParticipantRef {
                    agent_id: Some(agent.to_string()),
                    ..Default::default()
                },
                types::ParticipantRef {
                    user_id: Some(human.id.clone()),
                    ..Default::default()
                },
            ],
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(change) = changes.next().await {
            let change = change.unwrap();
            if !change.deleted && change.value.id == thread.id {
                break;
            }
        }
    })
    .await
    .unwrap();
    let replica = reader
        .thread(Uuid::parse_str(&thread.id).unwrap())
        .await
        .unwrap();
    assert_eq!(replica.title, thread.title);
    assert_eq!(replica.participants.len(), 2);
    let human_id = thread
        .participants
        .iter()
        .find(|p| p.user_id.as_deref() == Some(&human.id))
        .unwrap()
        .id
        .clone();
    let message = writer
        .post(tilde::chat::PostMessage {
            id: Uuid::new_v4().to_string(),
            thread_id: thread.id.clone(),
            participant_id: human_id,
            text: "Run locally".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(message.status, "complete");
    let attachment = writer
        .upload_attachment(tilde::chat::UploadAttachment {
            id: Uuid::new_v4().to_string(),
            thread_id: thread.id.clone(),
            filename: "note.txt".into(),
            media_type: "text/plain".into(),
            content: b"Sidecar attachment survives local eviction".to_vec(),
        })
        .await
        .unwrap();
    assert!(!attachment.persisted);
    let run = writer
        .start_run(tilde::chat::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "Run locally".into(),
            idempotency_key: message.id.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    let state = runtimes[0]
        .invocation(Uuid::parse_str(&run.invocation_id).unwrap())
        .await
        .unwrap();
    assert_eq!(state.owner_instance_id, runtimes[0].instance_id.to_string());
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let mut workers = vec![];
    for runtime in &runtimes {
        workers.push(tokio::spawn(
            runtime.as_ref().clone().worker(stopped.clone()),
        ));
    }
    let request = tokio::time::timeout(Duration::from_secs(10), calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        request.owner_instance_id,
        runtimes[0].instance_id.to_string()
    );
    assert_eq!(request.thread_id, thread.id);
    let mut commands = runtimes[0]
        .client
        .subscribe::<CommandObservation>("SELECT * FROM commands", vec![])
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(row) = commands.next().await {
            let row = row.unwrap();
            if !row.deleted && row.value.id == request.command_id && row.value.acked_at.is_some() {
                break;
            }
        }
    })
    .await
    .unwrap();
    let scope = runtimes[0].scope(&request.capability).await.unwrap();
    assert_eq!(scope.thread_id.to_string(), thread.id);
    assert!(
        calls.try_recv().is_err(),
        "Only the assigned replica executes"
    );
    // The execution's subscription and incoming controls can land on a peer
    // other than its owner. No inbound host control route is involved.
    runtimes[1]
        .client
        .catch_up(&runtimes[0].client)
        .await
        .unwrap();
    let control_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control_url = format!("http://{}", control_listener.local_addr().unwrap());
    let control_chat = reader.clone();
    let control_server = tokio::spawn(async move {
        axum::serve(
            control_listener,
            tilde::chat::controls::router(control_chat),
        )
        .await
        .unwrap()
    });
    let mut headers = http::HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", request.capability).parse().unwrap(),
    );
    let controls = tilde::services::tilde::runtime::v1::InvocationControlServiceClient::new(
        connectrpc::client::HttpClient::plaintext_http2_only(),
        connectrpc::client::ClientConfig::new(control_url.parse().unwrap())
            .with_default_headers(headers),
    );
    use tilde::proto::tilde::runtime::v1 as control_wire;
    let mut control_stream = controls
        .watch_commands(control_wire::WatchCommandsRequest::default())
        .await
        .unwrap();
    assert_eq!(
        control_stream
            .message()
            .await
            .unwrap()
            .unwrap()
            .view()
            .kind
            .as_known(),
        Some(control_wire::InvocationCommandKind::Ready)
    );
    let steering = Uuid::new_v4();
    writer
        .steer_invocation(tilde::chat::SteerInvocation {
            invocation_id: run.invocation_id.clone(),
            input_id: steering.to_string(),
            text: "Control from another peer".into(),
        })
        .await
        .unwrap();
    let command = tokio::time::timeout(Duration::from_secs(10), control_stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(command.view().input_id, steering.to_string());
    controls
        .acknowledge_command(control_wire::AcknowledgeCommandRequest {
            id: command.view().id.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    reader
        .cancel_invocation(Uuid::parse_str(&run.invocation_id).unwrap())
        .await
        .unwrap();
    let command = tokio::time::timeout(Duration::from_secs(10), control_stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        command.view().kind.as_known(),
        Some(control_wire::InvocationCommandKind::Stop)
    );
    controls
        .acknowledge_command(control_wire::AcknowledgeCommandRequest {
            id: command.view().id.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    runtimes[0]
        .client
        .catch_up(&runtimes[1].client)
        .await
        .unwrap();
    drop(control_stream);
    control_server.abort();
    finish.notify_one();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(row) = commands.next().await {
            let row = row.unwrap();
            if !row.deleted && row.value.id == request.command_id && row.value.finished_at.is_some()
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(runtimes[0].scope(&request.capability).await.is_err());
    stop.send(true).unwrap();
    for worker in workers {
        worker.await.unwrap();
    }
    drop(commands);
    // Reconnect the gateway and project every event into canonical Postgres
    // records. Replaying the subscription snapshot must not duplicate history.
    let pg = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(db.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let encryption = Arc::new(Encryption::initialize(&pg, seed(33)).await.unwrap());
    let (objects, object_server) = object_store().await;
    let agents = Agents::new(pg.clone(), encryption.clone()).with_avatar_store(objects.clone());
    let connections = Connections::new(
        pg.clone(),
        encryption.clone(),
        "http://localhost:1".into(),
        "http://localhost:2".into(),
    )
    .unwrap();
    let projections = Deployments::new(pg.clone(), encryption.clone(), agents, connections);
    let rows = runtimes[0]
        .client
        .query::<tilde::deployment::runtime::EventRow>(
            "SELECT * FROM events ORDER BY origin_instance_id,origin_sequence",
            vec![],
        )
        .await
        .unwrap();
    for _ in 0..2 {
        let mut pending = rows.iter().collect::<Vec<_>>();
        while !pending.is_empty() {
            let before = pending.len();
            let mut deferred = vec![];
            let mut last_error = None;
            for row in pending {
                let event = runtimes[0]
                    .open::<types::RuntimeEvent>(
                        Uuid::parse_str(&row.id).unwrap(),
                        "event",
                        &row.payload,
                    )
                    .unwrap();
                if let Err(error) = projections.project_event(&runtimes[0], event).await {
                    last_error = Some(error);
                    deferred.push(row);
                }
            }
            // Production retries dependency failures while continuing the
            // subscription. Replica ordering is not dependency ordering.
            assert!(
                deferred.len() < before,
                "Archive made no progress: {last_error:?}"
            );
            pending = deferred;
        }
    }
    let archived: i64 = sqlx::query_scalar("SELECT count(*) FROM sidecar_archive")
        .fetch_one(&pg)
        .await
        .unwrap();
    assert_eq!(archived, rows.len() as i64);
    let gateway_chat = Chat::new(pg.clone(), encryption.clone(), "http://localhost:1".into())
        .with_objects(Some(objects));
    assert_eq!(
        gateway_chat
            .thread(Uuid::parse_str(&thread.id).unwrap())
            .await
            .unwrap()
            .title,
        thread.title
    );
    assert_eq!(
        gateway_chat
            .message(Uuid::parse_str(&message.id).unwrap())
            .await
            .unwrap()
            .text,
        "Run locally"
    );
    for runtime in &runtimes {
        projections
            .heartbeat(
                agent,
                control::HeartbeatRequest {
                    instance_id: runtime.instance_id.to_string(),
                    ready: true,
                    agent_ready: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }
    runtimes[1]
        .client
        .catch_up(&runtimes[0].client)
        .await
        .unwrap();
    projections
        .transfer_attachment(
            agent,
            &types::AttachmentTransfer {
                attachment_id: attachment.id.clone(),
                thread_id: thread.id.clone(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // After successful S3 persistence, no in-memory holder is needed.
    runtimes[0]
        .release_attachment(Uuid::parse_str(&attachment.id).unwrap())
        .await;
    let download = gateway_chat
        .download_attachment(
            Uuid::parse_str(&thread.id).unwrap(),
            Uuid::parse_str(&attachment.id).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        download.content,
        b"Sidecar attachment survives local eviction"
    );
    assert!(download.attachment.persisted);
    // Two gateway replicas race the same expired acknowledgement. The shared
    // participant lock must produce exactly one new assignment generation.
    runtimes[1]
        .client
        .catch_up(&runtimes[0].client)
        .await
        .unwrap();
    let pending = reader
        .start_run(tilde::chat::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "Recover pending work".into(),
            idempotency_key: "recover-pending".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    runtimes[0]
        .client
        .catch_up(&runtimes[1].client)
        .await
        .unwrap();
    for row in runtimes[0]
        .client
        .query::<tilde::deployment::runtime::EventRow>(
            "SELECT * FROM events ORDER BY origin_sequence",
            vec![],
        )
        .await
        .unwrap()
    {
        let event = runtimes[0]
            .open(Uuid::parse_str(&row.id).unwrap(), "event", &row.payload)
            .unwrap();
        projections
            .project_event(&runtimes[0], event)
            .await
            .unwrap();
    }
    for row in runtimes[0]
        .client
        .query("SELECT * FROM commands", vec![])
        .await
        .unwrap()
    {
        projections
            .project_command(&runtimes[0], row)
            .await
            .unwrap();
    }
    projections
        .heartbeat(
            agent,
            control::HeartbeatRequest {
                instance_id: runtimes[1].instance_id.to_string(),
                ready: true,
                agent_ready: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    sqlx::query("UPDATE sidecar_commands SET dispatched_at=NOW()-INTERVAL '10 seconds' WHERE acked_at IS NULL").execute(&pg).await.unwrap();
    let (a, b) = tokio::join!(projections.recover(), projections.recover());
    assert_eq!(a.unwrap() + b.unwrap(), 1);
    let owner: (Uuid, i64) = sqlx::query_as(
        "SELECT owner_instance_id,generation FROM participant_assignments WHERE agent_id=$1",
    )
    .bind(agent)
    .fetch_one(&pg)
    .await
    .unwrap();
    assert_eq!(owner, (runtimes[1].instance_id, 2));
    let (outbox_id, payload): (Uuid, Vec<u8>) =
        sqlx::query_as("SELECT id,payload FROM sidecar_outbox WHERE kind='assignment'")
            .fetch_one(&pg)
            .await
            .unwrap();
    use base64::Engine as _;
    use buffa::Message as _;
    let secret = encryption
        .open(
            tilde::encryption::SecretBinding {
                resource_kind: "sidecar_control",
                resource_id: outbox_id,
                name: "assignment",
            },
            tilde::encryption::SealedSecret::from_bytes(&payload).unwrap(),
        )
        .unwrap();
    let payload = zeroize::Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(secret.expose_secret())
            .unwrap(),
    );
    let change = types::AssignmentChange::decode_from_slice(&payload).unwrap();
    runtimes[1]
        .client
        .catch_up(&runtimes[0].client)
        .await
        .unwrap();
    runtimes[1].apply_assignment(&change).await.unwrap();
    assert_eq!(
        runtimes[1]
            .invocation(Uuid::parse_str(&pending.invocation_id).unwrap())
            .await
            .unwrap()
            .status,
        "failed"
    );
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(runtimes[1].as_ref().clone().worker(stopped));
    let request = tokio::time::timeout(Duration::from_secs(10), calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request.assignment_generation, 2);
    assert_eq!(
        request.owner_instance_id,
        runtimes[1].instance_id.to_string()
    );
    finish.notify_one();
    let mut commands = runtimes[1]
        .client
        .subscribe::<CommandObservation>("SELECT * FROM commands", vec![])
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(row) = commands.next().await {
            let row = row.unwrap();
            if !row.deleted && row.value.id == request.command_id && row.value.finished_at.is_some()
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    drop(commands);
    stop.send(true).unwrap();
    worker.await.unwrap();
    host_task.abort();
    runtimes[0]
        .client
        .catch_up(&runtimes[1].client)
        .await
        .unwrap();
    for row in runtimes[1]
        .client
        .query::<tilde::deployment::runtime::EventRow>(
            "SELECT * FROM events ORDER BY created_at,origin_sequence",
            vec![],
        )
        .await
        .unwrap()
    {
        let event = runtimes[1]
            .open(Uuid::parse_str(&row.id).unwrap(), "event", &row.payload)
            .unwrap();
        projections
            .project_event(&runtimes[1], event)
            .await
            .unwrap();
    }
    for row in runtimes[1]
        .client
        .query("SELECT * FROM commands", vec![])
        .await
        .unwrap()
    {
        projections
            .project_command(&runtimes[1], row)
            .await
            .unwrap();
    }
    projections
        .set(
            agent,
            types::DeploymentMode::Sidecar,
            None,
            types::SidecarFailureMode::Stop,
            7,
        )
        .await
        .unwrap();
    let stopped_run = reader
        .start_run(tilde::chat::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "Stop when the owner cannot acknowledge".into(),
            idempotency_key: "stop-policy".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    for row in runtimes[1]
        .client
        .query::<tilde::deployment::runtime::EventRow>(
            "SELECT * FROM events ORDER BY origin_instance_id,origin_sequence",
            vec![],
        )
        .await
        .unwrap()
    {
        projections
            .project_event(
                &runtimes[1],
                runtimes[1]
                    .open(Uuid::parse_str(&row.id).unwrap(), "event", &row.payload)
                    .unwrap(),
            )
            .await
            .unwrap();
    }
    for row in runtimes[1]
        .client
        .query("SELECT * FROM commands", vec![])
        .await
        .unwrap()
    {
        projections
            .project_command(&runtimes[1], row)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE sidecar_commands SET dispatched_at=NOW()-INTERVAL '10 seconds' WHERE acked_at IS NULL").execute(&pg).await.unwrap();
    assert_eq!(projections.recover().await.unwrap(), 1);
    let (outbox_id,payload):(Uuid,Vec<u8>)=sqlx::query_as("SELECT id,payload FROM sidecar_outbox WHERE kind='assignment' ORDER BY created_at DESC LIMIT 1").fetch_one(&pg).await.unwrap();
    let secret = encryption
        .open(
            tilde::encryption::SecretBinding {
                resource_kind: "sidecar_control",
                resource_id: outbox_id,
                name: "assignment",
            },
            tilde::encryption::SealedSecret::from_bytes(&payload).unwrap(),
        )
        .unwrap();
    let payload = zeroize::Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(secret.expose_secret())
            .unwrap(),
    );
    let change = types::AssignmentChange::decode_from_slice(&payload).unwrap();
    assert!(change.assignment.stopped);
    assert!(!change.command.is_set());
    runtimes[1].apply_assignment(&change).await.unwrap();
    assert_eq!(
        reader
            .run(Uuid::parse_str(&stopped_run.id).unwrap())
            .await
            .unwrap()
            .status,
        "failed"
    );
    assert_eq!(
        gateway_chat
            .run(Uuid::parse_str(&stopped_run.id).unwrap())
            .await
            .unwrap()
            .status,
        "failed"
    );
    runtimes[0]
        .client
        .catch_up(&runtimes[1].client)
        .await
        .unwrap();
    // Pausing makes completed conversations eligible for safe mode migration.
    sqlx::query("UPDATE agents SET paused=TRUE WHERE id=$1")
        .bind(agent)
        .execute(&pg)
        .await
        .unwrap();
    for runtime in &runtimes {
        projections
            .heartbeat(
                agent,
                control::HeartbeatRequest {
                    instance_id: runtime.instance_id.to_string(),
                    ready: true,
                    agent_ready: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let retirement_workers = runtimes
        .iter()
        .map(|r| tokio::spawn(r.as_ref().clone().retirement_worker(stopped.clone())))
        .collect::<Vec<_>>();
    #[derive(Deserialize)]
    struct RetirementAck {
        instance_id: String,
        ready: i64,
    }
    let peer = projections.peer(agent).await.unwrap().unwrap();
    let mut acks = peer
        .client
        .subscribe::<RetirementAck>("SELECT * FROM retirement_acks", vec![])
        .await
        .unwrap();
    assert_eq!(projections.retire_conversations().await.unwrap(), 0);
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut seen = std::collections::BTreeSet::new();
        while let Some(row) = acks.next().await {
            let row = row.unwrap();
            if !row.deleted && row.value.ready == 1 {
                seen.insert(row.value.instance_id);
                if seen.len() == 2 {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(projections.retire_conversations().await.unwrap(), 1);
    runtimes[0].client.catch_up(&peer.client).await.unwrap();
    runtimes[1].client.catch_up(&peer.client).await.unwrap();
    assert!(
        !runtimes[0]
            .exists(Uuid::parse_str(&thread.id).unwrap())
            .await
            .unwrap()
    );
    assert!(
        !runtimes[1]
            .exists(Uuid::parse_str(&thread.id).unwrap())
            .await
            .unwrap()
    );
    assert_eq!(
        gateway_chat
            .message(Uuid::parse_str(&message.id).unwrap())
            .await
            .unwrap()
            .text,
        "Run locally"
    );
    let storage: String = sqlx::query_scalar(
        "SELECT storage FROM sidecar_conversations WHERE thread_id=$1 AND agent_id=$2",
    )
    .bind(Uuid::parse_str(&thread.id).unwrap())
    .bind(agent)
    .fetch_one(&pg)
    .await
    .unwrap();
    assert_eq!(storage, "postgres");
    // A subscription may replay an archived event after its source rows were
    // reaped. Duplicate archival must not require resurrecting those rows.
    for row in &rows {
        projections
            .project_event(
                &runtimes[0],
                runtimes[0]
                    .open(Uuid::parse_str(&row.id).unwrap(), "event", &row.payload)
                    .unwrap(),
            )
            .await
            .unwrap();
    }
    let gateway_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gateway_url = format!("http://{}", gateway_listener.local_addr().unwrap());
    let proxy = tilde::deployment::proxy::router(projections.clone(), gateway_chat.clone());
    let gateway_server =
        tokio::spawn(async move { axum::serve(gateway_listener, proxy).await.unwrap() });
    let node = tilde::deployment::sidecar::Node::new(
        runtimes[0].clone(),
        tilde::deployment::gateway::Client::new(&gateway_url, token.clone()).unwrap(),
        SecretString::from("unused"),
        String::new(),
    );
    let caller = projections
        .issue_ingress_token(agent, Some(thread.id.clone()), Uuid::new_v4())
        .await
        .unwrap();
    use tower::ServiceExt as _;
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/tilde.ingress.v1.ChatService/GetThread")
        .header("content-type", "application/json")
        .header("connect-protocol-version", "1")
        .header(
            "authorization",
            format!("Bearer {}", caller.expose_secret()),
        )
        .body(axum::body::Body::from(
            serde_json::json!({"id":thread.id}).to_string(),
        ))
        .unwrap();
    let response = node.public_router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let read: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(read["thread"]["id"], thread.id);
    assert!(
        !runtimes[0]
            .exists(Uuid::parse_str(&thread.id).unwrap())
            .await
            .unwrap()
    );
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/tilde.ingress.v1.ChatService/DownloadAttachment")
        .header("content-type", "application/json")
        .header("connect-protocol-version", "1")
        .header(
            "authorization",
            format!("Bearer {}", caller.expose_secret()),
        )
        .body(axum::body::Body::from(
            serde_json::json!({"threadId":thread.id,"attachmentId":attachment.id}).to_string(),
        ))
        .unwrap();
    let response = node.public_router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let download: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(download["content"].as_str().unwrap())
            .unwrap(),
        b"Sidecar attachment survives local eviction"
    );
    object_server.abort();
    gateway_server.abort();
    stop.send(true).unwrap();
    for worker in retirement_workers {
        worker.await.unwrap();
    }
    drop(acks);
    for server in ingress_servers {
        server.abort();
    }
    pg.close().await;
    for process in &mut processes {
        process.stop().await;
    }
    drop(changes);
    tokio::fs::remove_dir_all(directory).await.unwrap();
    db.close().await;
}

#[tokio::test]
async fn gateway_native_ingress_keeps_agent_scope_when_listing_or_reading_rooms() {
    use tower::ServiceExt as _;
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(34)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let chat = Chat::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
    );
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://localhost:1".into(),
        "http://localhost:2".into(),
    )
    .unwrap();
    let deployments = Deployments::new(db.pool.clone(), encryption, agents.clone(), connections);
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let mut rooms = vec![];
    for agent in [first, second] {
        agents
            .create(CreateAgent {
                id: agent,
                name: agent.to_string(),
                endpoint_url: "http://127.0.0.1:1".into(),
                webhook_signing_key: SecretString::from("test-agent-signing-key-01234567890123"),
                capabilities: Default::default(),
            })
            .await
            .unwrap();
        rooms.push(
            chat.create_thread(CreateThread {
                title: agent.to_string(),
                primary_agent_id: agent.to_string(),
                participants: vec![types::ParticipantRef {
                    agent_id: Some(agent.to_string()),
                    ..Default::default()
                }],
            })
            .await
            .unwrap(),
        );
    }
    // Gateway mode needs no sidecar registration token to issue scoped ingress.
    let token = deployments
        .issue_ingress_token(first, None, Uuid::new_v4())
        .await
        .unwrap();
    let router = tilde::deployment::public::router(deployments, chat);
    for (method, body, status) in [
        ("ListThreads", serde_json::json!({}), 200),
        ("GetThread", serde_json::json!({"id":rooms[1].id}), 403),
    ] {
        let request = axum::http::Request::builder()
            .method("POST")
            .uri(format!(
                "/agents/{first}/tilde.ingress.v1.ChatService/{method}"
            ))
            .header("content-type", "application/json")
            .header("connect-protocol-version", "1")
            .header("authorization", format!("Bearer {}", token.expose_secret()))
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), status);
        if status == 200 {
            let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap();
            let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(page["threads"].as_array().unwrap().len(), 1);
            assert_eq!(page["threads"][0]["id"], rooms[0].id);
        }
    }
    db.close().await;
}
