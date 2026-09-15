use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
#[allow(dead_code, reason = "Shared fixture supports other integration suites")]
mod common;
use secrecy::SecretString;
use std::{collections::BTreeMap, sync::Arc};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, Scope, tools::Registry},
    connections::{
        model::{Assignment, Capability},
        service::Connections,
    },
    encryption::Encryption,
};
use uuid::Uuid;
async fn agent(db: &common::Database, crypto: Arc<Encryption>, name: &str) -> Uuid {
    Agents::new(db.pool.clone(), crypto)
        .create(CreateAgent {
            concurrency_policy: Default::default(),
            id: Uuid::new_v4(),
            name: name.into(),
            endpoint_url: "http://127.0.0.1:9999".into(),
            webhook_signing_key: SecretString::from("test-signing-key-with-at-least-32-bytes"),
            capabilities: Default::default(),
        })
        .await
        .unwrap()
        .id
}
#[tokio::test]
async fn assignment_is_atomic_exclusive_by_chat_capability_and_paginated_with_agent_info() {
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(21))
            .await
            .unwrap(),
    );
    let a = agent(&db, crypto.clone(), "A").await;
    let b = agent(&db, crypto.clone(), "B").await;
    let service = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    service.seed().await.unwrap();
    let invalid = Uuid::new_v4();
    assert!(
        service
            .start(
                invalid,
                "Bad",
                "slack",
                "slack_app",
                &[Assignment {
                    capability: Capability::Channel,
                    agent_id: Uuid::new_v4()
                }]
            )
            .await
            .is_err()
    );
    assert!(service.get(invalid).await.is_err());
    let assignment = Assignment {
        capability: Capability::Channel,
        agent_id: a,
    };
    let first = service
        .start(
            Uuid::new_v4(),
            "Support",
            "slack",
            "slack_app",
            std::slice::from_ref(&assignment),
        )
        .await
        .unwrap();
    assert_eq!(first.connection.status, "requires_action");
    assert_eq!(first.connection.associated_agents[0].name, "A");
    let second = service
        .start(
            Uuid::new_v4(),
            "Email",
            "agentmail",
            "inbox",
            std::slice::from_ref(&assignment),
        )
        .await
        .unwrap();
    let free = service
        .start(Uuid::new_v4(), "Free", "linq", "account", &[])
        .await
        .unwrap();
    let other = Assignment {
        capability: Capability::Channel,
        agent_id: b,
    };
    let (one, two) = tokio::join!(
        service.assign(free.connection.id, &assignment),
        service.assign(free.connection.id, &other)
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let (page, cursor) = service.list(None, 1, Some(a), false).await.unwrap();
    assert_eq!(page.len(), 1);
    assert!(page[0].associated_agents.iter().any(|v| v.id == a));
    assert!(cursor.is_some());
    let (next, _) = service.list(cursor, 100, Some(a), false).await.unwrap();
    assert!(!next.iter().any(|c| c.id == page[0].id));
    assert!(next.iter().any(|c| c.id == first.connection.id) || page[0].id == first.connection.id);
    assert!(service.assign(first.connection.id, &other).await.is_err());
    service
        .unassign(first.connection.id, &assignment)
        .await
        .unwrap();
    service.assign(first.connection.id, &other).await.unwrap();
    assert_eq!(
        service
            .get(second.connection.id)
            .await
            .unwrap()
            .associated_agents[0]
            .id,
        a
    );
    let (available, _) = service.list(None, 100, None, true).await.unwrap();
    assert!(available.is_empty());
    service.disconnect(second.connection.id).await.unwrap();
    assert_eq!(
        service
            .get(second.connection.id)
            .await
            .unwrap()
            .associated_agents[0]
            .id,
        a
    );
    db.close().await;
}
#[tokio::test]
async fn audit_replays_membership_typing_attachments_and_agent_isolated_conversion_cache() {
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(22))
            .await
            .unwrap(),
    );
    let a = agent(&db, crypto.clone(), "Agent").await;
    let b = agent(&db, crypto.clone(), "Other agent").await;
    let chat = Chat::new(db.pool.clone(), crypto, "http://127.0.0.1".into());
    let human = chat.create_user("Human").await.unwrap();
    let thread = chat
        .create_thread(application::CreateThread {
            title: "Audit".into(),
            primary_agent_id: a.to_string(),
            participants: vec![
                types::ParticipantRef {
                    agent_id: Some(a.to_string()),
                    ..Default::default()
                },
                types::ParticipantRef {
                    user_id: Some(human.id),
                    ..Default::default()
                },
            ],
        })
        .await
        .unwrap();
    let tid = Uuid::parse_str(&thread.id).unwrap();
    let person = thread
        .participants
        .iter()
        .find(|p| p.user_id.is_some())
        .unwrap();
    let pid = Uuid::parse_str(&person.id).unwrap();
    let file = chat
        .upload_attachment(application::UploadAttachment {
            id: Uuid::new_v4().to_string(),
            thread_id: thread.id.clone(),
            filename: "notes.txt".into(),
            media_type: "text/plain".into(),
            content: b"private document".to_vec(),
        })
        .await
        .unwrap();
    let download = chat
        .download_attachment(tid, Uuid::parse_str(&file.id).unwrap())
        .await
        .unwrap();
    assert_eq!(download.content.as_slice(), b"private document");
    assert!(
        chat.download_attachment(Uuid::new_v4(), Uuid::parse_str(&file.id).unwrap())
            .await
            .is_err()
    );
    let sealed: Vec<u8> = sqlx::query_scalar("SELECT content FROM chat_attachments WHERE id=$1")
        .bind(Uuid::parse_str(&file.id).unwrap())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(!sealed.windows(16).any(|v| v == b"private document"));
    chat.typing(tid, pid, true).await.unwrap();
    chat.typing(tid, pid, false).await.unwrap();
    let message = chat
        .post(application::PostMessage {
            id: Uuid::new_v4().to_string(),
            thread_id: thread.id.clone(),
            participant_id: person.id.clone(),
            attachment_ids: vec![file.id.clone()],
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(message.attachments[0].filename, "notes.txt");
    let steering = chat
        .steering_message(tid, Uuid::parse_str(&message.id).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(steering.attachments[0].id, file.id);
    let scope = Scope {
        capabilities: Default::default(),
        id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        thread_id: tid,
        agent_id: a,
        participant_id: Uuid::parse_str(
            &thread
                .participants
                .iter()
                .find(|p| p.agent_id.is_some())
                .unwrap()
                .id,
        )
        .unwrap(),
    };
    chat.cache_converted_messages(
        &scope,
        vec![application::ConvertedMessage {
            message_id: message.id.clone(),
            message_json: r#"{"parts":[{"type":"text","text":"Converted attachment"}]}"#.into(),
        }],
    )
    .await
    .unwrap();
    assert_eq!(
        chat.hydrate_converted_messages(a, tid, std::slice::from_ref(&message.id))
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        chat.hydrate_converted_messages(b, tid, std::slice::from_ref(&message.id))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        chat.cache_converted_messages(
            &scope,
            vec![
                application::ConvertedMessage {
                    message_id: message.id.clone(),
                    message_json: "{}".into(),
                },
                application::ConvertedMessage {
                    message_id: Uuid::new_v4().to_string(),
                    message_json: "{}".into(),
                }
            ]
        )
        .await
        .is_err()
    );
    assert!(
        chat.hydrate_converted_messages(a, tid, std::slice::from_ref(&message.id))
            .await
            .unwrap()[0]
            .message_json
            .contains("Converted attachment")
    );
    chat.remove_participant(tid, pid).await.unwrap();
    assert!(chat.typing(tid, pid, true).await.is_err());
    chat.add_participant(application::AddParticipant {
        thread_id: thread.id.clone(),
        participant: types::ParticipantRef {
            user_id: person.user_id.clone(),
            ..Default::default()
        }
        .into(),
    })
    .await
    .unwrap();
    let mut events = vec![];
    let mut cursor = 0;
    loop {
        let page = chat.activity_page(tid, cursor, 2).await.unwrap();
        assert!(page.events.iter().all(|e| e.sequence > cursor));
        cursor = page.next_sequence;
        events.extend(page.events);
        if !page.has_more {
            break;
        }
    }
    assert!(events.iter().any(|e| e.kind == "participant.left"));
    assert_eq!(
        events.iter().filter(|e| e.kind == "typing.changed").count(),
        2
    );
    let joined = events
        .iter()
        .filter_map(|e| match &e.detail {
            Some(types::activity::Detail::Participant(p)) if p.id == person.id => Some(p.active),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(joined, vec![true, false, true]);
    let completed = events
        .iter()
        .find(|e| e.kind == "message.completed")
        .unwrap();
    let Some(types::activity::Detail::Message(snapshot)) = &completed.detail else {
        panic!("typed message snapshot")
    };
    assert_eq!(snapshot.attachments[0].id, file.id);
    assert!(
        serde_json::to_value(snapshot)
            .unwrap()
            .get("cachedAgentRepresentationJson")
            .is_none()
    );
    db.close().await;
}
#[tokio::test]
async fn dynamic_channel_catalog_tracks_ready_assignment_without_exporting_credentials() {
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(23))
            .await
            .unwrap(),
    );
    let a = agent(&db, crypto.clone(), "Agent").await;
    let connections = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let (configured, cursor) = connections.providers("", None, 100).await.unwrap();
    assert!(cursor.is_empty());
    let configured_channels: std::collections::BTreeSet<_> = configured
        .iter()
        .filter(|p| {
            p.connection_types
                .iter()
                .any(|t| t.capabilities.contains(&Capability::Channel))
        })
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(
        configured_channels,
        ["slack", "github", "agentmail", "linq", "whatsapp", "telnyx"]
            .into_iter()
            .collect(),
        "A configured chat provider needs outbound, callback and live-account coverage in this suite"
    );
    let chat = Chat::new(db.pool.clone(), crypto, "http://127.0.0.1".into())
        .with_connections(connections.clone());
    let mut capabilities = BTreeMap::new();
    capabilities.insert(
        tilde::iam::capabilities::Capability::ToolsInvoke,
        tilde::iam::capabilities::Reach::All,
    );
    let scope = Scope {
        capabilities: tilde::iam::capabilities::Capabilities(capabilities),
        id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        agent_id: a,
        participant_id: Uuid::new_v4(),
    };
    let registry = Registry::for_chat(&chat);
    let mut ids = vec![];
    for (provider, typ) in [
        ("slack", "slack_app"),
        ("github", "github_app"),
        ("agentmail", "inbox"),
        ("linq", "account"),
        ("whatsapp", "meta"),
        ("telnyx", "whatsapp"),
    ] {
        let started = connections
            .start(
                Uuid::new_v4(),
                provider,
                provider,
                typ,
                &[Assignment {
                    capability: Capability::Channel,
                    agent_id: a,
                }],
            )
            .await
            .unwrap();
        ids.push(started.connection.id);
    }
    assert_eq!(registry.list(&scope).await.unwrap().len(), 1);
    // Setup verification/credential encryption is covered by connections.rs; exercise runtime selection here.
    sqlx::query("UPDATE connections SET status='ready'")
        .execute(&db.pool)
        .await
        .unwrap();
    let tools = registry.list(&scope).await.unwrap();
    assert!(
        tools
            .iter()
            .any(|t| t.provider_id == "whatsapp" && t.name.ends_with("markRead"))
    );
    assert!(
        !tools
            .iter()
            .any(|t| t.provider_id == "telnyx" && t.name.ends_with("markRead"))
    );
    assert!(
        !tools
            .iter()
            .any(|t| t.provider_id == "agentmail" && t.name.contains("react"))
    );
    assert!(
        tools
            .iter()
            .any(|t| t.provider_id == "linq" && t.name.ends_with("reactToMessage"))
    );
    for id in ids {
        connections
            .unassign(
                id,
                &Assignment {
                    capability: Capability::Channel,
                    agent_id: a,
                },
            )
            .await
            .unwrap();
    }
    assert_eq!(registry.list(&scope).await.unwrap().len(), 1);
    db.close().await;
}

/// Stored credentials, generated invocation tools, real HTTP provider adapters and canonical audit.
#[tokio::test]
async fn slack_channel_send_and_verified_callback() {
    channel_send_and_callback("slack").await;
}
#[tokio::test]
async fn github_channel_send_and_verified_callback() {
    channel_send_and_callback("github").await;
}
#[tokio::test]
async fn agentmail_channel_send_and_verified_callback() {
    channel_send_and_callback("agentmail").await;
}
#[tokio::test]
async fn linq_channel_send_and_verified_callback() {
    channel_send_and_callback("linq").await;
}
#[tokio::test]
async fn whatsapp_channel_send_and_verified_callback() {
    channel_send_and_callback("whatsapp").await;
}
#[tokio::test]
async fn telnyx_channel_send_and_verified_callback() {
    channel_send_and_callback("telnyx").await;
}

async fn channel_send_and_callback(selected_provider: &str) {
    use axum::{
        Json,
        extract::{OriginalUri, State},
    };
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use http::HeaderMap;
    use secrecy::ExposeSecret;
    use serde_json::{Value, json};
    use sha2::Sha256;
    use tilde::connections::catalog::Endpoints;
    use tilde::encryption::SecretBinding;
    use tokio::sync::Mutex;
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(24))
            .await
            .unwrap(),
    );
    let agent_id = agent(&db, crypto.clone(), "Channels").await;
    let caps = tilde::iam::capabilities::Capabilities(BTreeMap::from([(
        tilde::iam::capabilities::Capability::ToolsInvoke,
        tilde::iam::capabilities::Reach::All,
    )]));
    Agents::new(db.pool.clone(), crypto.clone())
        .update(tilde::agent::UpdateAgent {
            concurrency_policy: None,
            id: agent_id,
            name: None,
            endpoint_url: None,
            capabilities: Some(caps),
        })
        .await
        .unwrap();
    let observed = Arc::new(Mutex::new(Vec::<(String, String, Value)>::new()));
    type Observed = Arc<Mutex<Vec<(String, String, Value)>>>;
    async fn upstream(
        State(observed): State<Observed>,
        OriginalUri(uri): OriginalUri,
        headers: HeaderMap,
        Json(input): Json<Value>,
    ) -> Result<Json<Value>, http::StatusCode> {
        let fail = input.to_string().contains("fixture-upstream-failure");
        observed.lock().await.push((
            uri.path().into(),
            headers["authorization"].to_str().unwrap().into(),
            input,
        ));
        if fail {
            return Err(http::StatusCode::BAD_GATEWAY);
        }
        Ok(Json(
            json!({"ok":true,"ts":"123.456","id":42,"message_id":"mail-1","thread_id":"mail-thread","message":{"id":"linq-1"},"messages":[{"id":"wa-1"}],"data":{"id":"telnyx-1"}}),
        ))
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_url = format!("http://{}", listener.local_addr().unwrap());
    let upstream_server = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new()
                .fallback(axum::routing::post(upstream))
                .with_state(observed.clone()),
        )
        .into_future(),
    );
    let endpoints = Endpoints(
        [
            "slack_api",
            "github_api",
            "agentmail_api",
            "linq_api",
            "meta_graph",
            "telnyx_api",
        ]
        .into_iter()
        .map(|key| (key.into(), upstream_url.clone()))
        .collect(),
    );
    let connections = Connections::with_endpoints(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1".into(),
        "https://ingress.example".into(),
        endpoints,
    )
    .unwrap();
    connections.seed().await.unwrap();
    let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into())
        .with_connections(connections.clone());
    let thread = chat
        .create_thread(application::CreateThread {
            title: "Originating conversation".into(),
            primary_agent_id: agent_id.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(agent_id.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let run = chat
        .start_run(application::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent_id.to_string(),
            objective: "Exercise provider adapters".into(),
            idempotency_key: "channels".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    sqlx::query("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1").bind(invocation).execute(&db.pool).await.unwrap();
    let token = chat
        .tokens
        .issue(
            agent_id,
            invocation,
            Uuid::parse_str(&thread.id).unwrap(),
            Uuid::parse_str(&run.id).unwrap(),
        )
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router =
        tilde::chat::rpc::runtime::router_with_tools(chat.clone(), Registry::for_chat(&chat))
            .merge(tilde::iam::listeners::public_event_ingress_router(
                chat.clone(),
                connections.clone(),
            ));
    let server = tokio::spawn(axum::serve(listener, router).into_future());
    let http = reqwest::Client::new();
    let standard_key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
    let signing = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
    let public_key =
        base64::engine::general_purpose::STANDARD.encode(signing.verifying_key().as_bytes());
    for (provider, typ, input, callback) in [
        (
            "slack",
            "slack_app",
            json!({"channelId":"C1","threadTs":"123.456","text":"Slack text"}),
            json!({"team_id":"T1","event_id":"slack-event","event":{"type":"message","channel":"C1","user":"U1","ts":"123.457","thread_ts":"123.456","text":"Slack inbound"}}),
        ),
        (
            "github",
            "github_app",
            json!({"owner":"owner","repository":"repo","number":1,"text":"**GitHub**"}),
            json!({"installation":{"id":1},"action":"created","repository":{"full_name":"owner/repo"},"issue":{"number":1},"sender":{"id":7,"login":"user","type":"User"},"comment":{"id":43,"body":"GitHub inbound"}}),
        ),
        (
            "agentmail",
            "inbox",
            json!({"to":["person@example.com"],"subject":"Subject","text":"Email text","html":"<strong>Email</strong>"}),
            json!({"event_type":"message.received","event_id":"mail-event","message":{"inbox_id":"inbox","thread_id":"mail-thread","message_id":"mail-2","from_":["person@example.com"],"text":"Email inbound"}}),
        ),
        (
            "linq",
            "account",
            json!({"chatId":"linq-chat","text":"Linq text"}),
            json!({"event_type":"message.received","event_id":"linq-event","data":{"id":"linq-2","chat":{"id":"linq-chat","owner_handle":{"handle":"+15550001111"}},"sender_handle":{"handle":"+15550002222"},"parts":[{"type":"text","value":"Linq inbound"}]}}),
        ),
        (
            "whatsapp",
            "meta",
            json!({"to":"15550002222","text":"WhatsApp text"}),
            json!({"entry":[{"changes":[{"value":{"metadata":{"phone_number_id":"phone"},"messages":[{"id":"wa-2","from":"15550002222","type":"text","text":{"body":"WhatsApp inbound"}}]}}]}]}),
        ),
        (
            "telnyx",
            "whatsapp",
            json!({"to":"+15550002222","text":"Telnyx text"}),
            json!({"data":{"id":"telnyx-event","event_type":"message.received","payload":{"id":"telnyx-2","from":{"phone_number":"+15550002222"},"to":[{"phone_number":"+15550001111"}],"text":"Telnyx inbound"}}}),
        ),
    ] {
        if provider != selected_provider {
            continue;
        }
        let started = connections
            .start(
                Uuid::new_v4(),
                provider,
                provider,
                typ,
                &[Assignment {
                    capability: Capability::Channel,
                    agent_id,
                }],
            )
            .await
            .unwrap();
        let connection = started.connection.id;
        for (key, value) in [
            ("access_token", "fixture-token"),
            ("api_key", "fixture-token"),
            ("api_token", "fixture-token"),
            ("inbox_id", "inbox"),
            ("phone_number_id", "phone"),
            ("phone_number", "+15550001111"),
            ("messaging_profile_id", "profile"),
            ("slack_team_id", "T1"),
            ("slack_bot_user_id", "BOT"),
            ("installation_id", "1"),
            ("signing_secret", "fixture-signing"),
            (
                "webhook_secret",
                if provider == "agentmail" {
                    standard_key.as_str()
                } else {
                    "fixture-signing"
                },
            ),
            ("webhook_signing_secret", standard_key.as_str()),
            ("app_secret", "fixture-signing"),
            ("public_key", public_key.as_str()),
        ] {
            let sealed = crypto
                .seal(
                    SecretBinding {
                        resource_kind: "connection",
                        resource_id: connection,
                        name: key,
                    },
                    &SecretString::from(value),
                )
                .unwrap()
                .into_bytes();
            sqlx::query("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)").bind(connection).bind(key).bind(sealed).execute(&db.pool).await.unwrap();
        }
        sqlx::query("UPDATE connections SET status='ready' WHERE id=$1")
            .bind(connection)
            .execute(&db.pool)
            .await
            .unwrap();
        tilde::chat::access::AgentAccess::new(connections.clone(), chat.clone())
            .set_mode(connection, agent_id, types::ChannelAccessMode::Public)
            .await
            .unwrap();
        let call = Uuid::new_v4();
        let name = format!("channel_{}.sendMessage", connection.simple());
        let frame=serde_json::to_vec(&json!({"name":name,"callId":call,"sequence":0,"inputJson":input.to_string(),"finish":true})).unwrap();
        let mut body = vec![0];
        body.extend_from_slice(&(frame.len() as u32).to_be_bytes());
        body.extend(frame);
        let result = http
            .post(format!("{origin}/tilde.runtime.v1.ChatService/InvokeTool"))
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(body)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        let len = u32::from_be_bytes(result[1..5].try_into().unwrap()) as usize;
        let result: Value = serde_json::from_slice(&result[5..5 + len]).unwrap();
        assert!(result.get("outputJson").is_some(), "{provider}: {result}");
        let outbound = chat.message(call).await.unwrap();
        assert_ne!(outbound.thread_id, thread.id);
        assert_eq!(outbound.delivery.connection_id, connection.to_string());
        let callback_bytes = serde_json::to_vec(&callback).unwrap();
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let mut headers = HeaderMap::new();
        let hmac = |key: &[u8], value: &[u8]| {
            let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
            mac.update(value);
            mac.finalize().into_bytes().to_vec()
        };
        match provider {
            "slack" => {
                headers.insert("x-slack-request-timestamp", timestamp.parse().unwrap());
                let mut signed = format!("v0:{timestamp}:").into_bytes();
                signed.extend_from_slice(&callback_bytes);
                headers.insert(
                    "x-slack-signature",
                    format!("v0={}", hex::encode(hmac(b"fixture-signing", &signed)))
                        .parse()
                        .unwrap(),
                );
            }
            "github" | "whatsapp" => {
                headers.insert(
                    "x-hub-signature-256",
                    format!(
                        "sha256={}",
                        hex::encode(hmac(b"fixture-signing", &callback_bytes))
                    )
                    .parse()
                    .unwrap(),
                );
                if provider == "github" {
                    headers.insert("x-github-event", "issue_comment".parse().unwrap());
                    headers.insert("x-github-delivery", "github-event".parse().unwrap());
                }
            }
            "agentmail" | "linq" => {
                let prefix = if provider == "agentmail" {
                    "svix"
                } else {
                    "webhook"
                };
                headers.insert(
                    http::header::HeaderName::from_bytes(format!("{prefix}-id").as_bytes())
                        .unwrap(),
                    "event-id".parse().unwrap(),
                );
                headers.insert(
                    http::header::HeaderName::from_bytes(format!("{prefix}-timestamp").as_bytes())
                        .unwrap(),
                    timestamp.parse().unwrap(),
                );
                let mut signed = format!("event-id.{timestamp}.").into_bytes();
                signed.extend_from_slice(&callback_bytes);
                headers.insert(
                    http::header::HeaderName::from_bytes(format!("{prefix}-signature").as_bytes())
                        .unwrap(),
                    format!(
                        "v1,{}",
                        base64::engine::general_purpose::STANDARD.encode(hmac(&[7u8; 32], &signed))
                    )
                    .parse()
                    .unwrap(),
                );
            }
            "telnyx" => {
                use ed25519_dalek::Signer;
                let mut signed = format!("{timestamp}|").into_bytes();
                signed.extend_from_slice(&callback_bytes);
                headers.insert("telnyx-timestamp", timestamp.parse().unwrap());
                headers.insert(
                    "telnyx-signature-ed25519",
                    base64::engine::general_purpose::STANDARD
                        .encode(signing.sign(&signed).to_bytes())
                        .parse()
                        .unwrap(),
                );
            }
            _ => unreachable!(),
        }
        let url = format!("{origin}/connections/webhooks/{connection}");
        assert!(
            !http
                .post(&url)
                .body(callback_bytes.clone())
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        );
        let mut tampered = callback_bytes.clone();
        tampered.push(b' '); // Still valid JSON, but no longer covered by the signature.
        assert!(
            !http
                .post(&url)
                .headers(headers.clone())
                .body(tampered)
                .send()
                .await
                .unwrap()
                .status()
                .is_success(),
            "{provider}: accepted tampered callback"
        );
        for _ in 0..2 {
            let response = http
                .post(&url)
                .headers(headers.clone())
                .body(callback_bytes.clone())
                .send()
                .await
                .unwrap();
            assert!(
                response.status().is_success(),
                "{provider}: {}",
                response.text().await.unwrap()
            );
        }
        let messages = chat
            .messages(Uuid::parse_str(&outbound.thread_id).unwrap(), 100)
            .await
            .unwrap();
        assert_eq!(
            messages.len(),
            2,
            "{provider} must preserve one conversation and deduplicate callback"
        );
        // An upstream rejection must produce a tool error without a visible sent message.
        let failed_call = Uuid::new_v4();
        let mut rejected = input.clone();
        rejected["text"] = json!("fixture-upstream-failure");
        let frame = serde_json::to_vec(&json!({"name":name,"callId":failed_call,"sequence":0,"inputJson":rejected.to_string(),"finish":true})).unwrap();
        let mut body = vec![0];
        body.extend_from_slice(&(frame.len() as u32).to_be_bytes());
        body.extend(frame);
        let response = http
            .post(format!("{origin}/tilde.runtime.v1.ChatService/InvokeTool"))
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(body)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        let len = u32::from_be_bytes(response[1..5].try_into().unwrap()) as usize;
        let failure: Value = serde_json::from_slice(&response[5..5 + len]).unwrap();
        assert!(failure.get("outputJson").is_none(), "{provider}: {failure}");
        assert!(
            chat.message(failed_call).await.is_err(),
            "{provider}: rejected send created a message"
        );
        assert_eq!(
            chat.messages(Uuid::parse_str(&outbound.thread_id).unwrap(), 100)
                .await
                .unwrap()
                .len(),
            2
        );
        connections
            .unassign(
                connection,
                &Assignment {
                    capability: Capability::Channel,
                    agent_id,
                },
            )
            .await
            .unwrap();
    }
    let observed = observed.lock().await;
    assert_eq!(observed.len(), 2);
    assert!(
        observed
            .iter()
            .all(|(_, authorization, _)| authorization == "Bearer fixture-token")
    );
    let (path, _, body) = &observed[0];
    let expected_path = match selected_provider {
        "slack" => "/chat.postMessage",
        "github" => "/repos/owner/repo/issues/1/comments",
        "agentmail" => "/inboxes/inbox/messages/send",
        "linq" => "/chats/linq-chat/messages",
        "whatsapp" => "/phone/messages",
        "telnyx" => "/messages/whatsapp",
        _ => unreachable!(),
    };
    assert_eq!(path, expected_path);
    match selected_provider {
        "slack" => {
            assert_eq!(body["channel"], "C1");
            assert_eq!(body["thread_ts"], "123.456");
        }
        "github" => assert_eq!(body["body"], "**GitHub**"),
        "agentmail" => assert_eq!(body["html"], "<strong>Email</strong>"),
        "linq" => assert_eq!(body["message"]["parts"][0]["value"], "Linq text"),
        "whatsapp" => assert_eq!(body["messaging_product"], "whatsapp"),
        "telnyx" => {
            assert_eq!(body["type"], "WHATSAPP");
            assert_eq!(body["messaging_profile_id"], "profile");
        }
        _ => unreachable!(),
    }
    server.abort();
    upstream_server.abort();
    db.close().await;
}
