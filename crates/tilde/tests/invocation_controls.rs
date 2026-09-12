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
    drop(late);
    drop(stream);
    server.abort();
    db.close().await;
}
