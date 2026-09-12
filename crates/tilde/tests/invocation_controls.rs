#[allow(dead_code)]
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, CreateThread, StartRun, SteerInvocation},
    encryption::Encryption,
    proto::tilde::{runtime::v1 as wire, types::v1 as types},
};
use uuid::Uuid;
fn client(
    url: &str,
    token: &str,
) -> tilde::services::tilde::runtime::v1::InvocationControlServiceClient<
    connectrpc::client::HttpClient,
> {
    let mut headers = http::HeaderMap::new();
    headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
    tilde::services::tilde::runtime::v1::InvocationControlServiceClient::new(
        connectrpc::client::HttpClient::plaintext_http2_only(),
        connectrpc::client::ClientConfig::new(url.parse().unwrap()).with_default_headers(headers),
    )
}
#[tokio::test]
async fn controls_cross_gateway_instances_replay_and_remain_scoped_after_revocation() {
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(52)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let agent = Uuid::new_v4();
    agents
        .create(CreateAgent {
            id: agent,
            name: "Serverless".into(),
            endpoint_url: "http://127.0.0.1:1".into(),
            webhook_signing_key: SecretString::from("control-test-signing-key-0123456789"),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    let writer = Chat::new(db.pool.clone(), encryption.clone(), String::new());
    let reader = Chat::new(db.pool.clone(), encryption, String::new());
    let thread = writer
        .create_thread(CreateThread {
            title: "Controlled".into(),
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
            objective: "Wait".into(),
            idempotency_key: "first".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    sqlx::query_file!("../../queries/chat/invocation_claim.sql", invocation)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let token = writer
        .tokens
        .issue(
            agent,
            invocation,
            Uuid::parse_str(&thread.id).unwrap(),
            Uuid::parse_str(&run.id).unwrap(),
        )
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, tilde::chat::controls::router(reader))
            .await
            .unwrap()
    });
    let rpc = client(&url, token.expose_secret());
    let mut stream = rpc
        .watch_commands(wire::WatchCommandsRequest::default())
        .await
        .unwrap();
    assert_eq!(
        stream
            .message()
            .await
            .unwrap()
            .unwrap()
            .view()
            .kind
            .as_known(),
        Some(wire::InvocationCommandKind::Ready)
    );
    let input = Uuid::new_v4();
    writer
        .steer_invocation(SteerInvocation {
            invocation_id: run.invocation_id.clone(),
            input_id: input.to_string(),
            text: "new input".into(),
        })
        .await
        .unwrap();
    let command = tokio::time::timeout(std::time::Duration::from_secs(3), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(command.view().id, input.to_string());
    drop(stream);
    let mut stream = rpc
        .watch_commands(wire::WatchCommandsRequest::default())
        .await
        .unwrap();
    stream.message().await.unwrap();
    assert_eq!(
        stream.message().await.unwrap().unwrap().view().id,
        input.to_string()
    );
    rpc.acknowledge_command(wire::AcknowledgeCommandRequest {
        id: input.to_string(),
        ..Default::default()
    })
    .await
    .unwrap();
    rpc.acknowledge_command(wire::AcknowledgeCommandRequest {
        id: input.to_string(),
        ..Default::default()
    })
    .await
    .unwrap();
    assert!(
        rpc.acknowledge_command(wire::AcknowledgeCommandRequest {
            id: Uuid::new_v4().to_string(),
            ..Default::default()
        })
        .await
        .is_err()
    );
    writer.cancel_invocation(invocation).await.unwrap();
    let stop = tokio::time::timeout(std::time::Duration::from_secs(3), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        stop.view().kind.as_known(),
        Some(wire::InvocationCommandKind::Stop)
    );
    assert!(writer.scope(token.expose_secret()).await.is_err());
    rpc.acknowledge_command(wire::AcknowledgeCommandRequest {
        id: stop.view().id.to_owned(),
        ..Default::default()
    })
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM invocation_control_receipts WHERE invocation_id=$1",
    )
    .bind(invocation)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
    let mut late = rpc
        .watch_commands(wire::WatchCommandsRequest::default())
        .await
        .unwrap();
    assert_eq!(
        late.message()
            .await
            .unwrap()
            .unwrap()
            .view()
            .kind
            .as_known(),
        Some(wire::InvocationCommandKind::Stop)
    );
    // Archived sidecar controls preserve terminal authority across a newer
    // agent generation, while ordinary callbacks remain revoked.
    agents.pause(agent).await.unwrap();
    let connections = tilde::connections::service::Connections::new(
        db.pool.clone(),
        Arc::new(Encryption::initialize(&db.pool, seed(52)).await.unwrap()),
        "http://127.0.0.1:1".into(),
        "http://127.0.0.1:2".into(),
    )
    .unwrap();
    let deployments = tilde::deployment::Deployments::new(
        db.pool.clone(),
        Arc::new(Encryption::initialize(&db.pool, seed(52)).await.unwrap()),
        agents.clone(),
        connections,
    );
    deployments
        .set(
            agent,
            types::DeploymentMode::Sidecar,
            None,
            types::SidecarFailureMode::Stop,
            7,
        )
        .await
        .unwrap();
    let deployment_token = deployments.issue_token(agent).await.unwrap();
    let registration = deployments
        .register(
            agent,
            tilde::proto::tilde::agent_event_ingress::v1::RegisterSidecarRequest {
                instance_id: Uuid::new_v4().to_string(),
                public_ingress_url: "http://127.0.0.1:8082".into(),
                runtime_url: "http://127.0.0.1:8081".into(),
                agent_ingress_url: "http://127.0.0.1:8083".into(),
                gossip_address: "127.0.0.1:8787".into(),
                local_agent_endpoint: "http://127.0.0.1:3000".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let stale=secrecy::SecretString::from(jsonwebtoken::encode(&jsonwebtoken::Header::default(),&serde_json::json!({"iss":"tilde:invocation","aud":"tilde:agent-api","sub":agent,"invocation_id":invocation,"thread_id":thread.id,"run_id":run.id,"capabilities":{},"iat":chrono::Utc::now().timestamp(),"exp":chrono::Utc::now().timestamp()+900,"assignment_generation":1,"agent_generation":0}),&jsonwebtoken::EncodingKey::from_secret(registration.token_signing_key.as_bytes())).unwrap());
    use tower::ServiceExt as _;
    let proxy = tilde::deployment::proxy::router(deployments, writer.clone());
    for (method, body, status) in [
        (
            "InvocationControlService/AcknowledgeCommand",
            serde_json::json!({"id":stop.view().id}),
            200,
        ),
        ("ChatService/GetThread", serde_json::json!({}), 403),
    ] {
        let request = http::Request::builder()
            .method("POST")
            .uri(format!("/agents/{agent}/proxy/tilde.runtime.v1.{method}"))
            .header("content-type", "application/json")
            .header("connect-protocol-version", "1")
            .header("authorization", format!("Bearer {}", stale.expose_secret()))
            .header("x-tilde-sidecar-token", deployment_token.expose_secret())
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        assert_eq!(
            proxy.clone().oneshot(request).await.unwrap().status(),
            status
        );
    }
    drop(late);
    drop(stream);
    server.abort();
    db.close().await;
}
