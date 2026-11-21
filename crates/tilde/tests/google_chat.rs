//! A Google Chat app across its boundaries: Google's signed request to the connection's webhook
//! is authenticated (JWT from Chat's certificates, audience the project number), an @mention in
//! a space thread or a DM becomes a message in the agent's external thread, the runtime worker
//! wakes the dialled-in agent host, and the host's reply is posted to a fake Chat API into the
//! same thread. Senders are attested by email, so private access needs no verification message.
mod common;
use common::host::Gateway;
use common::invocation::all_tools;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tilde::connections::{
    catalog::Endpoints,
    model::{Assignment, Capability},
};
use tilde::proto::tilde::types::v1 as types;
use uuid::Uuid;

const KEY: &str = include_str!("fixtures/connection-test-key.pem");
const CERTIFICATE: &str = include_str!("fixtures/signing-test-cert.pem");
const PROJECT: &str = "123456789012";

/// The bearer token Google Chat sends with each request.
fn token(audience: &str) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("chat-key-1".into());
    let now = chrono::Utc::now().timestamp();
    jsonwebtoken::encode(
        &header,
        &json!({"iss": "chat@system.gserviceaccount.com", "aud": audience, "iat": now, "exp": now + 3600}),
        &jsonwebtoken::EncodingKey::from_rsa_pem(KEY.as_bytes()).unwrap(),
    )
    .unwrap()
}
fn message(
    space: Value,
    name: &str,
    thread: &str,
    sender: &str,
    text: &str,
    mention: bool,
) -> Vec<u8> {
    let mut message = json!({
        "name": name,
        "sender": {"name": format!("users/{sender}"), "displayName": sender, "email": format!("{sender}@acme.com"), "type": "HUMAN"},
        "text": if mention { format!("@Fixer {text}") } else { text.to_owned() },
        "thread": {"name": thread},
        "space": space.clone(),
    });
    if mention {
        message["argumentText"] = json!(format!(" {text}"));
        message["annotations"] = json!([{"type": "USER_MENTION", "startIndex": 0, "length": 6,
            "userMention": {"user": {"name": "users/app", "displayName": "Fixer", "type": "BOT"}, "type": "MENTION"}}]);
    }
    serde_json::to_vec(
        &json!({"type": "MESSAGE", "eventTime": "2026-11-10T08:00:00Z",
        "message": message, "space": space, "user": message["sender"]}),
    )
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mentions_and_direct_messages_wake_the_agent_and_replies_reach_chat() {
    // Google's certificate endpoint and the Chat API.
    type Calls = Arc<Mutex<Vec<(String, String, String, Value)>>>;
    let calls: Calls = Arc::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let google = format!("http://{}", listener.local_addr().unwrap());
    let record = calls.clone();
    let upstream = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new()
                .route(
                    "/certificates",
                    axum::routing::get(|| async { axum::Json(json!({"chat-key-1": CERTIFICATE})) }),
                )
                .fallback(
                    move |method: http::Method,
                          uri: http::Uri,
                          headers: http::HeaderMap,
                          axum::Json(body): axum::Json<Value>| {
                        let record = record.clone();
                        async move {
                            record.lock().unwrap().push((
                                format!("{method} {}", uri.path()),
                                uri.query().unwrap_or_default().to_owned(),
                                headers["authorization"].to_str().unwrap().to_owned(),
                                body.clone(),
                            ));
                            let space = uri.path().split('/').take(3).collect::<Vec<_>>().join("/");
                            let thread = body["thread"]["name"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or(format!("{}/threads/new", &space[1..]));
                            axum::Json(json!({"name": format!("{}/messages/reply-1", &space[1..]), "thread": {"name": thread}}))
                        }
                    },
                ),
        )
        .into_future(),
    );
    let mut f = Gateway::start(
        81,
        Endpoints(
            [
                ("google_chat_api".to_owned(), google.clone()),
                (
                    "google_chat_certificates".to_owned(),
                    format!("{google}/certificates"),
                ),
            ]
            .into(),
        ),
        all_tools(),
    )
    .await;
    async move {
    let connection = f
        .connection(
            "google_chat",
            "app",
            &[
                ("access_token", "ya29.fixture"),
                ("project_number", PROJECT),
            ],
            &[Assignment {
                capability: Capability::Channel,
                agent_id: f.agent,
                alias: None,
            }],
        )
        .await;
    // Private access: Ada is added by email and allowed at once; Bob is not.
    let access = tilde::chat::access::AgentAccess::new(f.connections.clone(), f.chat.clone());
    access
        .set_mode(connection, f.agent, types::ChannelAccessMode::Private)
        .await
        .unwrap();
    let (_, status) = access
        .request_verification(tilde::chat::access::VerificationRequest {
            id: Uuid::new_v4(),
            connection,
            agent: f.agent,
            identity_type: types::IdentityType::Email,
            value: "Ada@acme.com".into(),
            template_name: None,
            template_language: None,
        })
        .await
        .unwrap();
    assert_eq!(status, "approved");

    let room = json!({"name": "spaces/ROOM", "type": "ROOM", "spaceType": "SPACE"});
    let signed = |audience: &str| vec![("authorization", format!("Bearer {}", token(audience)))];
    let mention = message(
        room.clone(),
        "spaces/ROOM/messages/m1",
        "spaces/ROOM/threads/t1",
        "ada",
        "why does checkout fail?",
        true,
    );
    assert_eq!(f.deliver(connection, &[], &mention).await.0, 400);
    assert_eq!(f.deliver(connection, &signed("999"), &mention).await.0, 400);
    // Unmentioned room chatter is not for the agent, and Bob is not allowed.
    let chatter = message(
        room.clone(),
        "spaces/ROOM/messages/m0",
        "spaces/ROOM/threads/t0",
        "ada",
        "lunch?",
        false,
    );
    assert_eq!(
        f.deliver(connection, &signed(PROJECT), &chatter).await,
        (200, "{}".into())
    );
    let bob = message(
        room.clone(),
        "spaces/ROOM/messages/m2",
        "spaces/ROOM/threads/t2",
        "bob",
        "deploy prod",
        true,
    );
    assert_eq!(f.deliver(connection, &signed(PROJECT), &bob).await.0, 200);
    f.idle(Duration::from_millis(1500)).await;
    assert_eq!(f.runs().await, 0);

    for _ in 0..2 {
        assert_eq!(
            f.deliver(connection, &signed(PROJECT), &mention).await,
            (200, "{}".into())
        );
    }
    let wake = f.wake().await;
    assert_eq!(wake.objective, "why does checkout fail?");
    let tool = format!("channel_{}.sendMessage", connection.simple());
    let sent = f
        .invoke_tool(
            &wake,
            &tool,
            json!({"conversation": "spaces/ROOM/threads/t1", "text": "Looking into it"}),
        )
        .await;
    assert!(sent.get("outputJson").is_some(), "{sent}");
    let thread = Uuid::parse_str(&wake.thread_id).unwrap();
    let messages = f.chat.messages(thread, 10).await.unwrap();
    assert_eq!(messages.len(), 2, "the reply joins the mention's thread");
    {
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        let (route, query, authorization, body) = &calls[0];
        assert_eq!(route, "POST /spaces/ROOM/messages");
        assert!(query.contains("messageReplyOption=REPLY_MESSAGE_FALLBACK_TO_NEW_THREAD"));
        assert!(query.contains("messageId=client-"));
        assert_eq!(authorization, "Bearer ya29.fixture");
        assert_eq!(body["thread"]["name"], "spaces/ROOM/threads/t1");
        assert_eq!(body["text"], "Looking into it");
    }
    let updated = f
        .invoke_tool(
            &wake,
            &format!("channel_{}.updateMessage", connection.simple()),
            json!({"message": "spaces/ROOM/messages/reply-1", "text": "Fixed in #12"}),
        )
        .await;
    assert!(updated.get("outputJson").is_some(), "{updated}");
    assert_eq!(
        calls.lock().unwrap()[1].0,
        "PATCH /spaces/ROOM/messages/reply-1"
    );
    assert_eq!(calls.lock().unwrap()[1].1, "updateMask=text");

    // A DM needs no mention and is one conversation.
    let dm = json!({"name": "spaces/DM1", "type": "DM", "spaceType": "DIRECT_MESSAGE", "singleUserBotDm": true});
    let direct = message(
        dm,
        "spaces/DM1/messages/d1",
        "spaces/DM1/threads/x",
        "ada",
        "status?",
        false,
    );
    assert_eq!(
        f.deliver(connection, &signed(PROJECT), &direct).await.0,
        200
    );
    let wake = f.wake().await;
    assert_eq!(wake.objective, "status?");
    let sent = f
        .invoke_tool(
            &wake,
            &tool,
            json!({"conversation": "spaces/DM1", "text": "All green"}),
        )
        .await;
    assert!(sent.get("outputJson").is_some(), "{sent}");
    let thread = Uuid::parse_str(&wake.thread_id).unwrap();
    assert_eq!(f.chat.messages(thread, 10).await.unwrap().len(), 2);
    assert!(calls.lock().unwrap()[2].3.get("thread").is_none());
    f.close().await;
    upstream.abort();
    }
    .await;
}
