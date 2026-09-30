mod common;
use connectrpc::client::{ClientConfig, HttpClient};
use std::{sync::Arc, time::Duration};
use tilde::proto::tilde::{run::v1 as run, types::v1 as types};
use tilde::services::tilde::run::v1::RunServiceClient;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, CreateThread, StartRun},
    connections::service::Connections,
    deployment::{Deployments, RegisterDeployment},
    encryption::Encryption,
};
use uuid::Uuid;

fn client(base: &str, bearer: &str) -> RunServiceClient<HttpClient> {
    let mut headers = http::HeaderMap::new();
    headers.insert(
        http::header::AUTHORIZATION,
        format!("Bearer {bearer}").parse().unwrap(),
    );
    RunServiceClient::new(
        HttpClient::plaintext_http2_only(),
        ClientConfig::new(base.parse().unwrap()).with_default_headers(headers),
    )
}

/// A long-running host dials in with its deployment token, receives the wake for a new
/// run as a frame instead of an inbound Invoke, and reports its progress with the
/// invocation capability.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_connected_host_is_woken_over_its_stream_and_reports_back() {
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(93))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let connections = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        "http://localhost:1".into(),
        "http://localhost:2".into(),
    )
    .unwrap();
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
            name: "Connected".into(),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    let router = tilde::iam::listeners::agent_rpc_router(agents.clone(), chat.clone());
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let (stop, stop_rx) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(chat.clone().worker(stop_rx));
    // CI registers the deployment the process runs; the token is the join.
    let (deployment, token, _) = deployments
        .register_deployment(
            agent,
            RegisterDeployment {
                source: types::DeploymentSource::Ci,
                target: types::DeploymentTarget::Gateway,
                target_reference: None,
                repository: Some("acme/connected".into()),
                commit_sha: Some("feedbeef".into()),
                external_id: Some("run-7".into()),
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
    let host = client(&base, secrecy::ExposeSecret::expose_secret(&token));
    let instance = Uuid::new_v4();
    let mut watch = host
        .watch(run::WatchRequest {
            instance_id: instance.to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let first: run::WatchResponse = watch.message().await.unwrap().unwrap().to_owned_message();
    match first.frame {
        Some(run::watch_response::Frame::Registered(registered)) => {
            assert_eq!(registered.deployment_id, deployment.id);
            assert_eq!(registered.agent_id, agent.to_string());
        }
        other => panic!("expected a registration frame, got {other:?}"),
    }
    host.heartbeat(run::HeartbeatRequest {
        instance_id: instance.to_string(),
        ready: true,
        ..Default::default()
    })
    .await
    .unwrap();
    let instances = deployments.instances(agent).await.unwrap();
    assert_eq!(instances.len(), 1);
    assert!(instances[0].ready && instances[0].deployment_id == deployment.id);
    // A registry skill the agent has, pushed with the wake so the host need not list it.
    let skills = tilde::skills::Skills::new(db.pool.clone());
    let source = skills.create_editor("Playbooks").await.unwrap();
    let (_, written) = skills
        .write(
            source,
            "refunds",
            vec![tilde::skills::Upload::Data {
                path: "SKILL.md".into(),
                data: b"---\nname: refunds\ndescription: Refund within policy\n---\n".to_vec(),
                executable: false,
            }],
            "",
            true,
        )
        .await
        .unwrap();
    skills.assign_source(agent, source).await.unwrap();
    skills.enable_source(agent, source, true).await.unwrap();
    // Work for the agent wakes the connected host over its open stream.
    let thread = chat
        .create_thread(CreateThread {
            title: "Connected".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(agent.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let started = chat
        .start_run(StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "Say hello".into(),
            idempotency_key: "hello".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let wake = loop {
        let frame: run::WatchResponse =
            tokio::time::timeout(Duration::from_secs(15), watch.message())
                .await
                .expect("a wake frame within 15 seconds")
                .unwrap()
                .expect("stream stays open")
                .to_owned_message();
        match frame.frame {
            Some(run::watch_response::Frame::Wake(wake)) => break *wake,
            Some(run::watch_response::Frame::Ping(_)) => continue,
            other => panic!("unexpected frame {other:?}"),
        }
    };
    assert_eq!(wake.invocation_id, started.invocation_id);
    assert_eq!(wake.objective, "Say hello");
    assert_eq!(wake.deployment_id, deployment.id);
    assert!(!wake.command_id.is_empty() && !wake.capability.is_empty());
    assert_eq!(wake.callback_url, base);
    let pushed = &wake.state.skills;
    assert_eq!(pushed.len(), 1);
    assert_eq!(
        (pushed[0].name.as_str(), pushed[0].version_id.as_str()),
        ("refunds", written.version.to_string().as_str())
    );
    assert!(!pushed[0].deployed && pushed[0].files[0].path == "SKILL.md");
    // The host reports with the invocation capability; the gateway never needed the
    // wake call's response.
    let execution = client(&base, &wake.capability);
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
            "thinking".into(),
        )))
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&wake.invocation_id).unwrap();
    let status: (String,) = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT status FROM chat_invocations WHERE id=$1",
            &[&invocation],
        )
        .await
        .map(|row| (row.get::<_, String>(0),))
        .unwrap();
    assert_eq!(status.0, "running");
    execution
        .report(report(run::report_request::Event::Stopped(
            run::RunStopped::default().into(),
        )))
        .await
        .unwrap();
    let run_id = Uuid::parse_str(&started.id).unwrap();
    let settled = loop {
        let run = chat.run(run_id).await.unwrap();
        if run.invocation_status == "stopped" {
            break run;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(settled.status, "waiting");
    tilde::chat::audit::flush(&db.pool).await.unwrap();
    let deltas: (i64,) = db.pool.get().await.unwrap().query_one("SELECT COUNT(*) FROM chat_activity WHERE thread_id=$1 AND kind='reasoning.delta' AND text_delta='thinking'", &[&(Uuid::parse_str(&thread.id).unwrap())]).await.map(|row| (row.get::<_, i64>(0),))
    .unwrap();
    assert_eq!(deltas.0, 1);
    // The wake directive was acknowledged, so a reconnect does not replay it.
    let pending: (i64,) = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM sidecar_directives WHERE agent_id=$1 AND acked_at IS NULL",
            &[&agent],
        )
        .await
        .map(|row| (row.get::<_, i64>(0),))
        .unwrap();
    assert_eq!(pending.0, 0);
    // A retired deployment's token no longer opens a stream.
    deployments
        .set(
            agent,
            types::SidecarFailureMode::Reassign,
            types::DeploymentRouting::Weighted,
        )
        .await
        .unwrap();
    let (other, other_token, _) = deployments
        .register_deployment(
            agent,
            RegisterDeployment {
                source: types::DeploymentSource::Ci,
                target: types::DeploymentTarget::Gateway,
                target_reference: None,
                repository: None,
                commit_sha: None,
                external_id: Some("run-8".into()),
                label: None,
                commit_message: None,
                branch: None,
                commit_author: None,
                declarations: Default::default(),
            },
        )
        .await
        .unwrap();
    assert!(
        deployments
            .promote(agent, Uuid::parse_str(&other.id).unwrap())
            .await
            .is_err()
    );
    let other_host = client(
        &base,
        secrecy::ExposeSecret::expose_secret(&other_token.unwrap()),
    );
    let other_instance = Uuid::new_v4();
    let mut other_watch = other_host
        .watch(run::WatchRequest {
            instance_id: other_instance.to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    other_watch.message().await.unwrap().unwrap();
    other_host
        .heartbeat(run::HeartbeatRequest {
            instance_id: other_instance.to_string(),
            ready: true,
            ..Default::default()
        })
        .await
        .unwrap();
    deployments
        .promote(agent, Uuid::parse_str(&other.id).unwrap())
        .await
        .unwrap();
    deployments
        .retire(agent, Uuid::parse_str(&deployment.id).unwrap())
        .await
        .unwrap();
    assert!(
        host.heartbeat(run::HeartbeatRequest {
            instance_id: instance.to_string(),
            ready: true,
            ..Default::default()
        })
        .await
        .is_err()
    );
    assert!(
        deployments
            .deployment(agent, Uuid::parse_str(&other.id).unwrap())
            .await
            .unwrap()
            .routable
    );
    drop(other_watch);
    tokio::time::timeout(Duration::from_secs(3), async {
        while deployments
            .deployment(agent, Uuid::parse_str(&other.id).unwrap())
            .await
            .unwrap()
            .routable
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("dropping Watch immediately removes routing eligibility");
    other_host
        .heartbeat(run::HeartbeatRequest {
            instance_id: other_instance.to_string(),
            ready: true,
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(
        !deployments
            .deployment(agent, Uuid::parse_str(&other.id).unwrap())
            .await
            .unwrap()
            .routable
    );
    let _ = stop.send(true);
    worker.abort();
    server.abort();
    db.close().await;
}
