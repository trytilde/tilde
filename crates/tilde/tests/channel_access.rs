mod common;
use axum::{Router, extract::State, routing::post};
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
    deployment::{Deployments, RegisterDeployment},
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
async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    (
        origin,
        tokio::spawn(async move { axum::serve(socket, app).await.unwrap() }),
    )
}
/// The agent's host: dials in to the gateway with its deployment token, accepts every
/// wake and then holds the invocation open, so runs stay `running` until canceled.
async fn host(
    tasks: &mut tokio::task::JoinSet<()>,
    pool: &tilde::database::Pool,
    crypto: Arc<Encryption>,
    deployments: &Deployments,
    chat: &Chat,
    agent: Uuid,
) {
    use connectrpc::client::{ClientConfig, HttpClient};
    use tilde::proto::tilde::{run::v1 as run, types::v1 as types};
    use tilde::services::tilde::run::v1::RunServiceClient;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router =
        tilde::iam::listeners::agent_rpc_router(Agents::new(pool.clone(), crypto), chat.clone());
    tasks.spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let (_, token, _) = deployments
        .register_deployment(
            agent,
            RegisterDeployment {
                source: types::DeploymentSource::Manual,
                target: types::DeploymentTarget::Gateway,
                target_reference: None,
                repository: None,
                commit_sha: None,
                external_id: None,
                label: None,
                commit_message: None,
                branch: None,
                commit_author: None,
            },
        )
        .await
        .unwrap();
    let client = move |bearer: &str| {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::header::AUTHORIZATION,
            format!("Bearer {bearer}").parse().unwrap(),
        );
        RunServiceClient::new(
            HttpClient::plaintext_http2_only(),
            ClientConfig::new(base.parse().unwrap()).with_default_headers(headers),
        )
    };
    let gateway = client(token.unwrap().expose_secret());
    let heartbeat = run::HeartbeatRequest {
        instance_id: Uuid::new_v4().to_string(),
        ready: true,
        ..Default::default()
    };
    let mut watch = gateway
        .watch(run::WatchRequest {
            instance_id: heartbeat.instance_id.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    watch.message().await.unwrap().unwrap();
    gateway.heartbeat(heartbeat.clone()).await.unwrap();
    tasks.spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        loop {
            tokio::select! {
                _ = tick.tick() => { let _ = gateway.heartbeat(heartbeat.clone()).await; }
                frame = watch.message() => {
                    let Ok(Some(frame)) = frame else { break };
                    let Some(run::watch_response::Frame::Wake(wake)) = frame.to_owned_message().frame else { continue };
                    let accepted = run::RunAccepted { command_id: wake.command_id.clone(), ..Default::default() };
                    let _ = client(&wake.capability)
                        .report(run::ReportRequest {
                            invocation_id: wake.invocation_id.clone(),
                            event: Some(run::report_request::Event::Accepted(accepted.into())),
                            ..Default::default()
                        })
                        .await;
                }
            }
        }
    });
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
    tokio::task::JoinSet<()>,
) {
    let sink = Sink::default();
    let (origin, server) = serve(
        Router::new()
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
            concurrency_policy: Default::default(),
            id: agent,
            name: "Assistant <&>".into(),
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
    let deployments = Deployments::new(
        db.pool.clone(),
        crypto.clone(),
        Agents::new(db.pool.clone(), crypto.clone()),
        connections.clone(),
    );
    let chat = Chat::new(db.pool.clone(), crypto.clone(), origin)
        .with_connections(connections.clone())
        .with_deployments(deployments.clone());
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(async move {
        let _ = server.await;
    });
    host(
        &mut tasks,
        &db.pool,
        crypto.clone(),
        &deployments,
        &chat,
        agent,
    )
    .await;
    let access = AgentAccess::new(connections.clone(), chat.clone());
    (connections, chat, access, crypto, agent, sink, tasks)
}
async fn connection(
    db: &common::Database,
    connections: &Connections,
    crypto: &Encryption,
    agent: Uuid,
    provider: &str,
    typ: &str,
) -> Uuid {
    // Ready connections to one provider need distinct slugs.
    let id = Uuid::new_v4();
    let id = connections
        .start(
            id,
            &format!("personal-{}", id.simple()),
            provider,
            typ,
            &[Assignment {
                capability: Capability::Channel,
                agent_id: agent,
                alias: None,
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
        ("inbox_id", "assistant@agentmail.to"),
        ("slack_bot_user_id", "U_BOT"),
        ("slug", "assistant"),
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
        db.pool.get().await.unwrap().execute("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)", &[&id, &key, &sealed]).await.unwrap();
    }
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE connections SET status='ready', account_label='+12025550111' WHERE id=$1",
            &[&id],
        )
        .await
        .unwrap();
    connections
        .assign(
            id,
            &Assignment {
                capability: Capability::Channel,
                agent_id: agent,
                alias: None,
            },
        )
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
    values
        .iter()
        .find_map(|value| {
            let start = value.find("http")?;
            let url = value[start..]
                .split(|c: char| c.is_whitespace() || matches!(c, '\"' | '<' | '>' | '|'))
                .next()?;
            if !url.contains("/identity/verify/") {
                return None;
            }
            url::Url::parse(url).ok()
        })
        .expect("verification URL delivered to provider")
}
#[tokio::test]
async fn private_verification_public_identity_capture_and_revocation_are_enforced() {
    let db = common::Database::new().await;
    let (connections, chat, access, crypto, agent, sink, mut server) = setup(&db).await;
    let connection = connection(&db, &connections, &crypto, agent, "whatsapp", "meta").await;
    let sender = "+12025550101";
    chat.ingest(connection, incoming("blocked", sender))
        .await
        .unwrap();
    chat.ingest(connection, incoming("blocked", sender))
        .await
        .unwrap();
    assert_eq!(
        db.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT COUNT(*) FROM chat_messages", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
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
            if let Some(row) = db
                .pool
                .get()
                .await
                .unwrap()
                .query_opt(
                    "SELECT id,run_id,thread_id FROM chat_invocations WHERE status='running'",
                    &[],
                )
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
            .is_ok()
    );
    assert!(
        chat.tokens
            .renew(runtime_token.expose_secret())
            .await
            .is_err()
    );
    assert_eq!(chat.run(run).await.unwrap().invocation_status, "canceled");
    chat.ingest(connection, incoming("disabled", sender))
        .await
        .unwrap();
    assert_eq!(
        db.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT COUNT(*) FROM chat_messages", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
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
    server.abort_all();
    db.close().await;
}
#[tokio::test]
async fn providers_deliver_supported_identity_types_and_github_rejects_private() {
    let db = common::Database::new().await;
    let (connections, _chat, access, crypto, agent, sink, mut server) = setup(&db).await;
    for (provider, typ, kind, value) in [
        (
            "agentmail",
            "inbox",
            IdentityType::Email,
            "alice@example.com",
        ),
        ("linq", "account", IdentityType::Email, "alice@example.com"),
        ("linq", "account", IdentityType::PhoneNumber, "+12025550102"),
        ("slack", "slack_app", IdentityType::Username, "U12345"),
        (
            "whatsapp",
            "meta",
            IdentityType::PhoneNumber,
            "+12025550103",
        ),
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
        let delivered = sink.0.lock().unwrap().last().unwrap().clone();
        let text = match provider {
            "agentmail" => {
                assert_eq!(delivered["subject"], "Invitation to access Assistant <&>");
                let html = delivered["html"].as_str().unwrap();
                assert!(html.contains("access Assistant &lt;&amp;&gt; from this email address"));
                assert!(html.contains(&format!("<a href=\"{}\">this link</a>", link)));
                assert!(html.contains("This link expires in 10 minutes."));
                delivered["text"].as_str().unwrap()
            }
            "slack" => {
                let text = delivered["text"].as_str().unwrap();
                assert!(text.contains("access Assistant &lt;&amp;&gt; from this Slack account"));
                assert!(text.contains(&format!("<{}|this link>", link)));
                text
            }
            "linq" => {
                let text = delivered["message"]["parts"][0]["value"].as_str().unwrap();
                assert!(text.contains(if kind == IdentityType::Email {
                    "from this email address"
                } else {
                    "from this phone number"
                }));
                text
            }
            "whatsapp" => delivered["text"]["body"].as_str().unwrap(),
            "telnyx" => delivered["whatsapp_message"]["text"]["body"]
                .as_str()
                .unwrap(),
            _ => unreachable!(),
        };
        assert!(text.starts_with("You've received an invitation to access Assistant"));
        assert!(text.contains("This link expires in 10 minutes. If you did not request access, please ignore this message."));
        if matches!(provider, "whatsapp" | "telnyx") {
            assert!(text.contains("from this WhatsApp number"));
        }
        assert!(!text.contains("No Tilde login"));
        let token = link
            .query_pairs()
            .find(|(key, _)| key == "identity_verification_token")
            .unwrap()
            .1
            .to_string();
        let verification = access.verification(id, &token).await.unwrap();
        assert_eq!(verification.value, value);
        let sender = verification.agent_identity.as_option().unwrap();
        assert_eq!(sender.agent_id, agent.to_string());
        assert_eq!(sender.connection_id, connection.to_string());
        assert_eq!(
            sender.value,
            match provider {
                "agentmail" => "assistant@agentmail.to",
                "slack" => "U_BOT",
                _ => "+12025550111",
            }
        );
        db.pool.get().await.unwrap().execute("UPDATE chat_identity_verifications SET expires_at=NOW()-INTERVAL '1 second' WHERE id=$1", &[&id]).await.unwrap();
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
    server.abort_all();
    db.close().await;
}
