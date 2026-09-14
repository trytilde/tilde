#[expect(dead_code, reason = "shared fixture helpers")]
mod common;
use axum::{Router, body::Body, extract::State, response::IntoResponse, routing::post};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{
        Chat,
        access::{AgentAccess, VerificationRequest, identity::Identity},
        providers::ingress::{IncomingKind, IncomingMessage},
    },
    connections::{
        catalog::Endpoints,
        model::{Assignment, Capability},
        service::Connections,
    },
    encryption::{Encryption, SecretBinding},
    proto::tilde::types::v1::{ChannelAccessMode, IdentityType},
};
use uuid::Uuid;
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<Value>>>);
async fn delivery(
    State(sink): State<Sink>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    sink.0.lock().unwrap().push(body);
    axum::Json(
        json!({"ok":true,"channel":{"id":"D1"},"message_id":"sent","id":"sent","messages":[{"id":"sent"}]}),
    )
}
async fn invoke() -> axum::response::Response {
    (
        [("content-type", "application/connect+proto")],
        Body::from_stream(futures::stream::pending::<
            Result<axum::body::Bytes, std::convert::Infallible>,
        >()),
    )
        .into_response()
}
async fn cancel() -> axum::response::Response {
    ([("content-type", "application/proto")], Vec::<u8>::new()).into_response()
}
async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    (
        origin,
        tokio::spawn(async move { axum::serve(socket, app).await.unwrap() }),
    )
}
async fn setup(
    db: &common::Database,
) -> (
    Connections,
    Chat,
    AgentAccess,
    Arc<Encryption>,
    Uuid,
    Sink,
    tokio::task::JoinHandle<()>,
) {
    let sink = Sink::default();
    let (origin, server) = serve(
        Router::new()
            .route("/tilde.agent_host.v1.AgentService/Invoke", post(invoke))
            .route("/tilde.agent_host.v1.AgentService/Cancel", post(cancel))
            .route("/{*path}", post(delivery))
            .with_state(sink.clone()),
    )
    .await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(99))
            .await
            .unwrap(),
    );
    let agent = Uuid::new_v4();
    Agents::new(db.pool.clone(), crypto.clone())
        .create(CreateAgent {
            id: agent,
            name: "Assistant".into(),
            endpoint_url: origin.clone(),
            webhook_signing_key: SecretString::from("shared-signing-key-0123456789abcdef"),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    let endpoints = Endpoints(BTreeMap::from_iter(
        [
            "meta_graph",
            "telnyx_api",
            "agentmail_api",
            "linq_api",
            "slack_api",
        ]
        .into_iter()
        .map(|key| (key.into(), origin.clone())),
    ));
    let connections = Connections::with_endpoints(
        db.pool.clone(),
        crypto.clone(),
        origin.clone(),
        "https://ingress.example".into(),
        endpoints,
    )
    .unwrap();
    connections.seed().await.unwrap();
    let chat =
        Chat::new(db.pool.clone(), crypto.clone(), origin).with_connections(connections.clone());
    let access = AgentAccess::new(connections.clone(), chat.clone());
    (connections, chat, access, crypto, agent, sink, server)
}
async fn connection(
    db: &common::Database,
    connections: &Connections,
    crypto: &Encryption,
    agent: Uuid,
    provider: &str,
    typ: &str,
) -> Uuid {
    let id = connections
        .start(
            Uuid::new_v4(),
            "Personal account",
            provider,
            typ,
            &[Assignment {
                capability: Capability::Channel,
                agent_id: agent,
            }],
        )
        .await
        .unwrap()
        .connection
        .id;
    for (key, value) in [
        ("access_token", "test-access-token"),
        ("api_key", "test-api-key"),
        ("api_token", "test-api-token"),
        ("phone_number", "+12025550111"),
        ("phone_number_id", "phone"),
        ("messaging_profile_id", "profile"),
        ("inbox_id", "inbox"),
    ] {
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: id,
                    name: key,
                },
                &SecretString::from(value),
            )
            .unwrap()
            .into_bytes();
        sqlx::query("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)").bind(id).bind(key).bind(sealed).execute(&db.pool).await.unwrap();
    }
    sqlx::query("UPDATE connections SET status='ready' WHERE id=$1")
        .bind(id)
        .execute(&db.pool)
        .await
        .unwrap();
    id
}
fn incoming(event: &str, value: &str) -> IncomingMessage {
    IncomingMessage {
        subject: None,
        kind: IncomingKind::Message,
        event_id: event.into(),
        message_id: event.into(),
        thread_id: value.into(),
        sender: Identity {
            identity_type: IdentityType::PhoneNumber,
            value: value.into(),
        },
        sender_name: value.into(),
        attachments: vec![],
        text: "Hello".into(),
        format: "text",
        reply_to: None,
    }
}
fn verification_link(sink: &Sink) -> url::Url {
    fn strings(value: &Value, output: &mut Vec<String>) {
        match value {
            Value::String(s) => output.push(s.clone()),
            Value::Array(v) => {
                for x in v {
                    strings(x, output)
                }
            }
            Value::Object(v) => {
                for x in v.values() {
                    strings(x, output)
                }
            }
            _ => {}
        }
    }
    let mut values = vec![];
    for value in sink.0.lock().unwrap().iter() {
        strings(value, &mut values)
    }
    url::Url::parse(
        values
            .iter()
            .flat_map(|v| v.split_whitespace())
            .find(|v| v.contains("/identity/verify/"))
            .expect("verification URL delivered to provider"),
    )
    .unwrap()
}
#[tokio::test]
async fn private_verification_public_identity_capture_and_revocation_are_enforced() {
    let db = common::Database::new().await;
    let (connections, chat, access, crypto, agent, sink, server) = setup(&db).await;
    let connection = connection(&db, &connections, &crypto, agent, "whatsapp", "meta").await;
    let sender = "+12025550101";
    chat.ingest(connection, incoming("blocked", sender))
        .await
        .unwrap();
    chat.ingest(connection, incoming("blocked", sender))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM chat_messages")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        0
    );
    let before = access
        .identities(agent, Some(connection), None, 10)
        .await
        .unwrap()
        .0;
    assert_eq!(before.len(), 1);
    assert!(!before[0].allowed);
    assert!(!before[0].verified_at.is_set());
    assert!(
        access
            .set_allowed(
                connection,
                agent,
                Uuid::parse_str(&before[0].id).unwrap(),
                true
            )
            .await
            .is_err()
    );
    let request = Uuid::new_v4();
    let (_, status) = access
        .request_verification(VerificationRequest {
            id: request,
            connection,
            agent,
            identity_type: IdentityType::PhoneNumber,
            value: sender.into(),
            template_name: None,
            template_language: None,
        })
        .await
        .unwrap();
    assert_eq!(status, "delivered");
    let link = verification_link(&sink);
    let token = link
        .query_pairs()
        .find(|(k, _)| k == "identity_verification_token")
        .unwrap()
        .1
        .to_string();
    assert!(access.verification(request, "wrong-token").await.is_err());
    let (public, public_server) =
        serve(tilde::chat::access::rpc::public_router(access.clone())).await;
    let http = reqwest::Client::new();
    let body = json!({"id":request.to_string(),"identityVerificationToken":token});
    let read = http
        .post(format!(
            "{public}/tilde.setup.v1.IdentityVerificationService/GetIdentityVerification"
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(read.status().is_success());
    assert!(
        !access
            .identities(agent, Some(connection), None, 10)
            .await
            .unwrap()
            .0[0]
            .allowed,
        "GET must not approve"
    );
    let approve = http
        .post(format!(
            "{public}/tilde.setup.v1.IdentityVerificationService/ApproveIdentityVerification"
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(approve.status().is_success());
    assert_eq!(
        access.approve(request, &token).await.unwrap().status,
        "approved"
    );
    chat.ingest(connection, incoming("allowed", sender))
        .await
        .unwrap();
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(chat.clone().worker(receiver));
    let row = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(row) = sqlx::query(
                "SELECT id,run_id,thread_id FROM chat_invocations WHERE status='running'",
            )
            .fetch_optional(&db.pool)
            .await
            .unwrap()
            {
                break row;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    use sqlx::Row;
    let invocation: Uuid = row.get("id");
    let run: Uuid = row.get("run_id");
    let thread: Uuid = row.get("thread_id");
    let runtime_token = chat
        .tokens
        .issue(agent, invocation, thread, run)
        .await
        .unwrap();
    assert!(
        chat.tokens
            .verify(runtime_token.expose_secret())
            .await
            .is_ok()
    );
    access
        .set_mode(connection, agent, ChannelAccessMode::Disabled)
        .await
        .unwrap();
    assert!(
        chat.tokens
            .verify(runtime_token.expose_secret())
            .await
            .is_err()
    );
    assert_eq!(chat.run(run).await.unwrap().invocation_status, "canceled");
    chat.ingest(connection, incoming("disabled", sender))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM chat_messages")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        1
    );
    access
        .set_mode(connection, agent, ChannelAccessMode::Public)
        .await
        .unwrap();
    chat.ingest(connection, incoming("public", "+12025550222"))
        .await
        .unwrap();
    assert_eq!(
        access
            .identities(agent, Some(connection), None, 10)
            .await
            .unwrap()
            .0
            .len(),
        2
    );
    access
        .set_mode(connection, agent, ChannelAccessMode::Private)
        .await
        .unwrap();
    let identity = Uuid::parse_str(&before[0].id).unwrap();
    access
        .set_allowed(connection, agent, identity, false)
        .await
        .unwrap();
    access.approve(request, &token).await.unwrap();
    assert!(
        !access
            .identities(agent, Some(connection), None, 10)
            .await
            .unwrap()
            .0
            .iter()
            .find(|i| i.id == identity.to_string())
            .unwrap()
            .allowed,
        "Replaying an approval cannot undo revocation"
    );
    shutdown.send(true).unwrap();
    worker.await.unwrap();
    public_server.abort();
    server.abort();
    db.close().await;
}
#[tokio::test]
async fn providers_deliver_supported_identity_types_and_github_rejects_private() {
    let db = common::Database::new().await;
    let (connections, _chat, access, crypto, agent, sink, server) = setup(&db).await;
    for (provider, typ, kind, value) in [
        (
            "agentmail",
            "inbox",
            IdentityType::Email,
            "alice@example.com",
        ),
        ("linq", "account", IdentityType::Email, "alice@example.com"),
        ("slack", "slack_app", IdentityType::Username, "U12345"),
        (
            "telnyx",
            "whatsapp",
            IdentityType::PhoneNumber,
            "+12025550101",
        ),
    ] {
        sink.0.lock().unwrap().clear();
        let connection = connection(&db, &connections, &crypto, agent, provider, typ).await;
        let id = Uuid::new_v4();
        let (_, status) = access
            .request_verification(VerificationRequest {
                id,
                connection,
                agent,
                identity_type: kind,
                value: value.into(),
                template_name: None,
                template_language: None,
            })
            .await
            .unwrap();
        assert_eq!(status, "delivered", "{provider}");
        let link = verification_link(&sink);
        let token = link
            .query_pairs()
            .find(|(key, _)| key == "identity_verification_token")
            .unwrap()
            .1
            .to_string();
        assert_eq!(access.verification(id, &token).await.unwrap().value, value);
        sqlx::query("UPDATE chat_identity_verifications SET expires_at=NOW()-INTERVAL '1 second' WHERE id=$1").bind(id).execute(&db.pool).await.unwrap();
        assert!(access.approve(id, &token).await.is_err());
    }
    let github = connection(&db, &connections, &crypto, agent, "github", "github_app").await;
    assert!(
        access
            .set_mode(github, agent, ChannelAccessMode::Private)
            .await
            .is_err()
    );
    access
        .set_mode(github, agent, ChannelAccessMode::Public)
        .await
        .unwrap();
    server.abort();
    db.close().await;
}
