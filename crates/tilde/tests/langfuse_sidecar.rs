#[allow(dead_code)]
mod common;
use base64::Engine as _;
use common::{Database, seed};
use envconfig::Envconfig;
use opentelemetry_proto::tonic::{
    collector::logs::v1::ExportLogsServiceRequest,
    logs::v1::{LogRecord, ResourceLogs, ScopeLogs},
};
use opentelemetry_proto::tonic::{
    collector::trace::v1::{ExportTraceServiceRequest, ExportTraceServiceResponse},
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};
use prost::Message as _;
use secrecy::SecretString;
use serde::Deserialize;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tilde::proto::tilde::{agent_event_ingress::v1 as control, types::v1 as types};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, CreateThread, StartRun},
    connections::service::Connections,
    deployment::{
        Deployments,
        corrosion::process::Process,
        runtime::{Runtime, RuntimeOptions},
    },
    encryption::Encryption,
    iam::capabilities::{Capabilities, Capability, Reach},
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
async fn replicated_otlp_survives_gateway_outage_and_deduplicates_ha_replay() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .try_init();
    let (fallback, mut calls, finish, host_task) = host().await;
    let external = std::env::var("ENGINE_TEST_AGENT_ENDPOINT").ok();
    let endpoint = external.clone().unwrap_or(fallback);
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(43)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let agent = Uuid::new_v4();
    agents
        .create(CreateAgent {
            id: agent,
            name: "Langfuse sidecar smoke".into(),
            endpoint_url: endpoint.clone(),
            webhook_signing_key: SecretString::from("sidecar-test-agent-signing-key-0123456789"),
            capabilities: Capabilities(std::collections::BTreeMap::from([
                (Capability::ThreadRead, Reach::Yes),
                (Capability::RunUpdate, Reach::Yes),
                (Capability::ToolsInvoke, Reach::All),
            ])),
        })
        .await
        .unwrap();
    agents.pause(agent).await.unwrap();
    let captures = Arc::new(Mutex::new(Vec::<ExportTraceServiceRequest>::new()));
    let captured = captures.clone();
    let collector = axum::Router::new().route(
        "/api/public/otel/v1/traces",
        axum::routing::post(move |body: axum::body::Bytes| {
            let captured = captured.clone();
            async move {
                captured
                    .lock()
                    .unwrap()
                    .push(ExportTraceServiceRequest::decode(body).unwrap());
                (
                    [("content-type", "application/x-protobuf")],
                    ExportTraceServiceResponse::default().encode_to_vec(),
                )
            }
        }),
    );
    let log_captured = Arc::new(Mutex::new(Vec::<ExportLogsServiceRequest>::new()));
    let capture_logs = log_captured.clone();
    let collector = collector.route(
        "/v1/logs",
        axum::routing::post(move |body: axum::body::Bytes| {
            let capture = capture_logs.clone();
            async move {
                capture
                    .lock()
                    .unwrap()
                    .push(ExportLogsServiceRequest::decode(body).unwrap());
                http::StatusCode::OK
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let collector_url = format!("http://{}", listener.local_addr().unwrap());
    let collector_server =
        tokio::spawn(async move { axum::serve(listener, collector).await.unwrap() });
    let destination = if let Ok(base) = std::env::var("LANGFUSE_BASE_URL") {
        tilde::telemetry::Destination::langfuse(
            Some(base),
            None,
            Some(tilde::config::SecretEnv(
                std::env::var("LANGFUSE_PUBLIC_KEY").unwrap().into(),
            )),
            Some(tilde::config::SecretEnv(
                std::env::var("LANGFUSE_SECRET_KEY").unwrap().into(),
            )),
        )
        .unwrap()
    } else {
        tilde::telemetry::Destination::langfuse(
            Some(collector_url.clone()),
            None,
            Some(tilde::config::SecretEnv("pk-fixture".into())),
            Some(tilde::config::SecretEnv("sk-fixture".into())),
        )
        .unwrap()
    };
    let queue = tilde::telemetry::delivery::Queue::new(db.pool.clone(), true);
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://localhost:1".into(),
        "http://localhost:2".into(),
    )
    .unwrap();
    let logs_directory = std::env::temp_dir().join(format!("sidecar-log-spool-{}", Uuid::new_v4()));
    let logs_config = tilde::config::Config::init_from_hashmap(&std::collections::HashMap::from([
        ("DATABASE_URL".into(), "postgres://unused".into()),
        (
            "LOGS_QUEUE_DIR".into(),
            logs_directory.to_string_lossy().into_owned(),
        ),
        (
            "LOGS_OTLP_ENDPOINT".into(),
            format!("{collector_url}/v1/logs"),
        ),
    ]))
    .unwrap();
    let logs_runtime = tilde::logs::Runtime::start(db.pool.clone(), &logs_config).unwrap();
    let deployments = Deployments::new(db.pool.clone(), encryption, agents.clone(), connections)
        .with_telemetry(queue.clone())
        .with_logs(logs_runtime.delivery.clone());
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
    let configuration = deployments.configuration(agent).await.unwrap();
    assert!(configuration.tracing_enabled);
    assert!(configuration.logs_enabled);
    let directory = std::env::temp_dir().join(format!("tilde-langfuse-sidecar-{}", Uuid::new_v4()));
    let mut processes = Vec::new();
    let mut servers = Vec::new();
    let mut runtimes = Vec::new();
    let mut urls = Vec::new();
    for index in 0..2 {
        let instance = Uuid::new_v4();
        let gossip = udp();
        let api = port();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let callback = format!("http://{}", listener.local_addr().unwrap());
        let registration = deployments
            .register(
                agent,
                control::RegisterSidecarRequest {
                    instance_id: instance.to_string(),
                    public_ingress_url: callback.clone(),
                    agent_ingress_url: callback.clone(),
                    runtime_url: callback.clone(),
                    gossip_address: gossip.to_string(),
                    local_agent_endpoint: endpoint.clone(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
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
                callback_url: callback.clone(),
            },
        ));
        runtime.configure(&configuration).await.unwrap();
        let node = tilde::deployment::sidecar::Node::new(
            runtime.clone(),
            tilde::deployment::gateway::Client::new("http://127.0.0.1:1", token.clone()).unwrap(),
            api_token,
            format!("http://{api}"),
        );
        let router = node.runtime_router().merge(node.agent_router());
        servers.push(tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap()
        }));
        urls.push(callback);
        processes.push(process);
        runtimes.push(runtime);
    }
    let routes = tilde::deployment::telemetry::PlatformRoutes::default();
    routes.write().unwrap().insert(agent, runtimes[0].clone());
    let (provider, platform) = tilde::deployment::telemetry::platform(routes);
    opentelemetry::global::set_tracer_provider(provider.clone());
    let writer = Chat::from_sidecar(runtimes[0].clone());
    let thread = writer
        .create_thread(CreateThread {
            title: "Langfuse relay".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(agent.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let run = writer
        .start_run(StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective:
                "Write a short vegetarian lasagna recipe for four. Use your pantry tool once."
                    .into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let (stop, rx) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(runtimes[0].as_ref().clone().worker(rx.clone()));
    #[derive(Deserialize)]
    struct Trace {
        payload: String,
    }
    if external.is_none() {
        let call = tokio::time::timeout(Duration::from_secs(20), calls.recv())
            .await
            .unwrap()
            .unwrap();
        let invocation = runtimes[0]
            .invocation(Uuid::parse_str(&call.invocation_id).unwrap())
            .await
            .unwrap();
        let trace_id = hex::decode(invocation.traceparent.split('-').nth(1).unwrap()).unwrap();
        let request = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![ScopeSpans {
                    spans: vec![Span {
                        trace_id,
                        span_id: vec![7; 8],
                        name: "agent.model".into(),
                        start_time_unix_nano: 10,
                        end_time_unix_nano: 20,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };
        assert_eq!(
            reqwest::Client::new()
                .post(format!("{}/v1/traces", urls[0]))
                .bearer_auth(&call.capability)
                .header("content-type", "application/x-protobuf")
                .body(request.encode_to_vec())
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        let logs=ExportLogsServiceRequest{resource_logs:vec![ResourceLogs{scope_logs:vec![ScopeLogs{log_records:vec![LogRecord{time_unix_nano:chrono::Utc::now().timestamp_nanos_opt().unwrap() as u64,trace_id:request.resource_spans[0].scope_spans[0].spans[0].trace_id.clone(),severity_number:9,body:Some(opentelemetry_proto::tonic::common::v1::AnyValue{value:Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue("sidecar log during gateway outage".into()))}),..Default::default()}],..Default::default()}],..Default::default()}]};
        assert_eq!(
            reqwest::Client::new()
                .post(format!("{}/v1/logs", urls[0]))
                .bearer_auth(&call.capability)
                .header("content-type", "application/x-protobuf")
                .body(logs.encode_to_vec())
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if !runtimes[1]
                    .client
                    .query::<Trace>("SELECT payload FROM logs", vec![])
                    .await
                    .unwrap()
                    .is_empty()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap();
        assert!(log_captured.lock().unwrap().is_empty());
    }
    // Neither a gateway nor a collector is reachable from the sidecar at this point.
    tokio::time::timeout(Duration::from_secs(100), async {
        loop {
            let rows = runtimes[1]
                .client
                .query::<Trace>("SELECT id,payload FROM traces", vec![])
                .await
                .unwrap();
            if !rows.is_empty() {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&rows[0].payload)
                    .unwrap();
                assert!(ExportTraceServiceRequest::decode(bytes.as_slice()).is_ok());
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry_delivery")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(before, 0);
    let cancel = tokio_util::sync::CancellationToken::new();
    let export = tokio::spawn(queue.clone().export(destination.clone(), cancel.clone()));
    let mut replicas = Vec::new();
    for runtime in &runtimes {
        let service = deployments.clone();
        let source = runtime.clone();
        let shutdown = rx.clone();
        replicas.push(tokio::spawn(async move {
            let result = service.replicate_telemetry(&source, shutdown).await;
            if let Err(error) = &result {
                eprintln!("Trace projection stopped: {error}");
            }
            result
        }));
    }
    let completed = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM telemetry_delivery WHERE completed_at IS NOT NULL",
            )
            .fetch_one(&db.pool)
            .await
            .unwrap();
            if count > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    if completed.is_err() {
        let pending: Vec<(i32, bool)> =
            sqlx::query_as("SELECT attempts,completed_at IS NOT NULL FROM telemetry_delivery")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        eprintln!(
            "Delivery attempts/completed: {pending:?}; projection finished: {:?}",
            replicas.iter().map(|r| r.is_finished()).collect::<Vec<_>>()
        );
    }
    completed.unwrap();
    if external.is_none() {
        assert_eq!(
            captures
                .lock()
                .unwrap()
                .iter()
                .flat_map(|r| &r.resource_spans)
                .flat_map(|r| &r.scope_spans)
                .flat_map(|r| &r.spans)
                .filter(|s| s.name == "agent.model")
                .count(),
            1
        );
    }
    if external.is_some() {
        let reader = tilde::telemetry::viewer::Reader::new(db.pool.clone(), destination);
        tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                if let Ok(page) = reader
                    .list(&agent.to_string(), Default::default(), "", None, true)
                    .await
                    && page.observations.iter().any(|r| r.r#type == "GENERATION")
                {
                    println!("Sidecar Langfuse trace: {}", page.observations[0].trace_url);
                    break;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        })
        .await
        .unwrap();
    }
    if external.is_none() {
        tokio::time::timeout(Duration::from_secs(20), async {
            while log_captured.lock().unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Sidecar logs reached the external collector through the gateway");
    }
    let mut disabled = configuration.clone();
    disabled.tracing_enabled = false;
    disabled.logs_enabled = false;
    for runtime in &runtimes {
        runtime.configure(&disabled).await.unwrap();
    }
    finish.notify_waiters();
    stop.send(true).unwrap();
    worker.abort();
    for task in replicas {
        task.abort();
    }
    for task in servers {
        task.abort();
    }
    let _ = tokio::task::spawn_blocking(move || provider.shutdown()).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), platform).await;
    cancel.cancel();
    let _ = export.await;
    for process in &mut processes {
        process.stop().await;
    }
    host_task.abort();
    collector_server.abort();
    logs_runtime.shutdown().await;
    let _ = std::fs::remove_dir_all(logs_directory);
    let _ = std::fs::remove_dir_all(directory);
    db.close().await;
    let _ = run;
}
