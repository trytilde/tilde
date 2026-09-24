use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    encryption::Encryption,
    iam::{
        capabilities::{Capabilities, Capability, Reach},
        oidc::Oidc,
    },
};
use uuid::Uuid;

async fn create(agents: &Agents, caps: Capabilities) -> Uuid {
    let id = Uuid::new_v4();
    agents
        .create(CreateAgent {
            concurrency_policy: Default::default(),
            id,
            name: "IAM fixture".into(),
            capabilities: caps,
        })
        .await
        .unwrap();
    id
}
async fn invocation(
    chat: &Chat,
    pool: &tilde::database::Pool,
    agent: Uuid,
) -> (Uuid, Uuid, SecretString) {
    let thread = chat
        .create_thread(application::CreateThread {
            title: "IAM scope".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(agent.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let run = chat
        .start_run(application::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "IAM test".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    let thread = Uuid::parse_str(&thread.id).unwrap();
    pool.get().await.unwrap().execute("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1", &[&invocation]).await.unwrap();
    let token = chat
        .tokens
        .issue(agent, invocation, thread, Uuid::parse_str(&run.id).unwrap())
        .await
        .unwrap();
    (invocation, thread, token)
}
#[tokio::test]
async fn scope_uses_five_minute_claims_and_renewal_requires_live_state() {
    let db = Database::new().await;
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(72)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into());
    let agent = create(&agents, Capabilities::default()).await;
    let (inv, thread, token) = invocation(&chat, &db.pool, agent).await;
    let (_, other_thread, other_token) = invocation(&chat, &db.pool, agent).await;
    let scope = chat.scope(token.expose_secret()).await.unwrap();
    let other_scope = chat.scope(other_token.expose_secret()).await.unwrap();
    assert_ne!(scope.participant_id, other_scope.participant_id);
    let roster = chat.thread(thread).await.unwrap();
    assert!(roster.participants.iter().any(|p| {
        p.id == scope.participant_id.to_string() && p.agent_id == Some(agent.to_string())
    }));

    let claims = chat.tokens.verify(token.expose_secret()).await.unwrap();
    assert_eq!(claims.exp - claims.iat, 300);
    assert_eq!(claims.participant_id, scope.participant_id);
    assert!(
        chat.tokens
            .issue(agent, inv, other_thread, scope.run_id)
            .await
            .is_err()
    );

    let sealed: Vec<u8> = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT sealed FROM iam_signing_key", &[])
        .await
        .unwrap()
        .get(0);
    let key = crypto
        .open(
            tilde::encryption::SecretBinding {
                resource_kind: "iam",
                resource_id: Uuid::nil(),
                name: "agent_token_signing_key",
            },
            tilde::encryption::SealedSecret::from_bytes(&sealed).unwrap(),
        )
        .unwrap();
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    header.typ = Some("tilde-agent-connect+jwt".into());
    let mut near_expiry = claims.clone();
    near_expiry.iat -= 240;
    near_expiry.exp -= 240;
    let near_expiry_token = SecretString::from(
        jsonwebtoken::encode(
            &header,
            &near_expiry,
            &jsonwebtoken::EncodingKey::from_secret(key.expose_secret().as_bytes()),
        )
        .unwrap(),
    );
    let renewed = chat
        .tokens
        .renew(near_expiry_token.expose_secret())
        .await
        .unwrap();
    let renewed_claims = chat.tokens.verify(renewed.expose_secret()).await.unwrap();
    assert!(renewed_claims.exp > near_expiry.exp);
    assert_eq!(renewed_claims.exp - renewed_claims.iat, 300);
    assert_eq!(renewed_claims.participant_id, scope.participant_id);
    let mut expired_claims = claims;
    expired_claims.exp = chrono::Utc::now().timestamp() - 1;
    expired_claims.iat = expired_claims.exp - 300;
    let expired = SecretString::from(
        jsonwebtoken::encode(
            &header,
            &expired_claims,
            &jsonwebtoken::EncodingKey::from_secret(key.expose_secret().as_bytes()),
        )
        .unwrap(),
    );
    drop(key);
    assert!(chat.scope(expired.expose_secret()).await.is_err());
    assert!(chat.tokens.renew(expired.expose_secret()).await.is_err());
    drop(expired);

    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_participants SET active=false WHERE id=$1",
            &[&scope.participant_id],
        )
        .await
        .unwrap();
    assert!(chat.scope(token.expose_secret()).await.is_ok());
    assert!(chat.tokens.renew(token.expose_secret()).await.is_err());
    assert!(chat.scope(other_token.expose_secret()).await.is_ok());
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_participants SET active=true WHERE id=$1",
            &[&scope.participant_id],
        )
        .await
        .unwrap();

    // An issued token remains valid; only renewal consults current authority.
    for (deny, restore, id) in [
        (
            "UPDATE agents SET paused=true WHERE id=$1",
            "UPDATE agents SET paused=false WHERE id=$1",
            agent,
        ),
        (
            "UPDATE chat_invocations SET lease_expires_at=NOW()-INTERVAL '1 second' WHERE id=$1",
            "UPDATE chat_invocations SET lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1",
            inv,
        ),
        (
            "UPDATE chat_invocations SET status='canceled' WHERE id=$1",
            "UPDATE chat_invocations SET status='running' WHERE id=$1",
            inv,
        ),
        (
            "UPDATE chat_runs SET status='completed' WHERE id=$1",
            "UPDATE chat_runs SET status='active' WHERE id=$1",
            scope.run_id,
        ),
    ] {
        db.pool
            .get()
            .await
            .unwrap()
            .execute(deny, &[&id])
            .await
            .unwrap();
        assert!(chat.scope(token.expose_secret()).await.is_ok());
        assert!(chat.tokens.renew(token.expose_secret()).await.is_err());
        db.pool
            .get()
            .await
            .unwrap()
            .execute(restore, &[&id])
            .await
            .unwrap();
        assert_eq!(
            chat.scope(token.expose_secret())
                .await
                .unwrap()
                .participant_id,
            scope.participant_id
        );
        assert!(chat.tokens.renew(token.expose_secret()).await.is_ok());
    }
    db.pool
        .get()
        .await
        .unwrap()
        .execute("UPDATE agents SET deleted_at=NOW() WHERE id=$1", &[&agent])
        .await
        .unwrap();
    assert!(chat.scope(token.expose_secret()).await.is_ok());
    assert!(chat.tokens.renew(token.expose_secret()).await.is_err());
    // Signature validation and scope resolution still work with the database unavailable.
    db.pool.close().await;
    assert!(chat.scope(token.expose_secret()).await.is_ok());
    db.close().await;
}

#[tokio::test]
async fn message_pages_keep_history_bounds_cursor_validation_and_agent_cache_isolation() {
    let db = Database::new().await;
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(73)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let chat = Chat::new(db.pool.clone(), crypto, "http://127.0.0.1".into());
    let a = create(&agents, Capabilities::default()).await;
    let b = create(&agents, Capabilities::default()).await;
    let (inv, thread, token) = invocation(&chat, &db.pool, a).await;
    let scope = chat.scope(token.expose_secret()).await.unwrap();
    let user = chat.create_user("History reader").await.unwrap();
    let roster = chat
        .add_participant(application::AddParticipant {
            thread_id: thread.to_string(),
            participant: Some(types::ParticipantRef {
                user_id: Some(user.id.clone()),
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    let sender = roster
        .participants
        .iter()
        .find(|p| p.user_id.as_deref() == Some(&user.id))
        .unwrap();
    let ids = [Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)];
    for id in ids {
        chat.post(application::PostMessage {
            id: id.to_string(),
            thread_id: thread.to_string(),
            participant_id: sender.id.clone(),
            text: id.to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    }
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_messages SET created_at='2026-01-01T00:00:00Z' WHERE thread_id=$1",
            &[&thread],
        )
        .await
        .unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET history_through_message_id=$2 WHERE id=$1",
            &[&inv, &(ids[1])],
        )
        .await
        .unwrap();
    for (agent_id, label) in [(a, "A"), (b, "B")] {
        let owner = application::Scope {
            agent_id,
            ..scope.clone()
        };
        chat.cache_converted_messages(
            &owner,
            ids.iter()
                .map(|id| application::ConvertedMessage {
                    message_id: id.to_string(),
                    message_json: json!({"owner":label}).to_string(),
                })
                .collect(),
        )
        .await
        .unwrap();
    }
    let first = chat
        .invocation_message_page(thread, None, 1, Some(inv))
        .await
        .unwrap();
    assert_eq!(first.messages[0].id, ids[1].to_string());
    assert_eq!(first.next_page_token, ids[1].to_string());
    assert_eq!(first.cached_messages.len(), 1);
    assert_eq!(first.cached_messages[0].message_id, ids[1].to_string());
    assert_eq!(first.cached_messages[0].message_json, "{\"owner\":\"A\"}");
    let second = chat
        .invocation_message_page(thread, Some(ids[1]), 1, Some(inv))
        .await
        .unwrap();
    assert_eq!(second.messages[0].id, ids[0].to_string());
    assert_eq!(second.cached_messages[0].message_id, ids[0].to_string());
    assert!(second.next_page_token.is_empty());
    let empty = chat
        .invocation_message_page(thread, Some(ids[0]), 1, Some(inv))
        .await
        .unwrap();
    assert!(empty.messages.is_empty() && empty.cached_messages.is_empty());
    assert!(matches!(
        chat.invocation_message_page(thread, Some(Uuid::new_v4()), 1, Some(inv))
            .await,
        Err(application::ChatError::NotFound)
    ));
    assert!(matches!(
        chat.message_page(Uuid::new_v4(), None, 1).await,
        Err(application::ChatError::NotFound)
    ));
    let (_, other_thread, other_token) = invocation(&chat, &db.pool, b).await;
    let _other = chat.scope(other_token.expose_secret()).await.unwrap();
    let other_roster = chat
        .add_participant(application::AddParticipant {
            thread_id: other_thread.to_string(),
            participant: Some(types::ParticipantRef {
                user_id: Some(user.id.clone()),
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    let other_sender = other_roster
        .participants
        .iter()
        .find(|p| p.user_id.as_deref() == Some(&user.id))
        .unwrap();
    let foreign = chat
        .post(application::PostMessage {
            id: Uuid::new_v4().to_string(),
            thread_id: other_thread.to_string(),
            participant_id: other_sender.id.clone(),
            text: "Other thread".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(matches!(
        chat.invocation_message_page(
            thread,
            Some(Uuid::parse_str(&foreign.id).unwrap()),
            1,
            Some(inv)
        )
        .await,
        Err(application::ChatError::NotFound)
    ));
    let management = chat.message_page(thread, None, 100).await.unwrap();
    assert_eq!(management.messages.len(), 3);
    assert!(management.cached_messages.is_empty());
    // An invocation may read its own later replies beyond its incoming-message cutoff.
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_messages SET invocation_id=$2,participant_id=$3 WHERE id=$1",
            &[&(ids[2]), &inv, &scope.participant_id],
        )
        .await
        .unwrap();
    let own = chat
        .invocation_message_page(thread, None, 100, Some(inv))
        .await
        .unwrap();
    assert_eq!(own.messages.len(), 3);
    assert_eq!(own.cached_messages.len(), 3);
    db.close().await;
}

// The fixture passes its database explicitly rather than exposing the domain pool.
async fn serve(router: axum::Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() }),
    )
}
async fn rpc(
    http: &reqwest::Client,
    url: &str,
    method: &str,
    token: &str,
    body: Value,
) -> reqwest::Response {
    http.post(format!("{url}/tilde.runtime.v1.AgentService/{method}"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap()
}
#[tokio::test]
async fn listeners_enforce_scopes_grants_and_live_invocation_state() {
    let db = Database::new().await;
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(71)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into());
    let target = create(&agents, Capabilities::default()).await;
    let caps = Capabilities(BTreeMap::from([
        (
            Capability::AgentsRead,
            Reach::Selected {
                ids: vec![target.to_string()],
            },
        ),
        (Capability::AgentsCreate, Reach::Yes),
        (
            Capability::AgentsUpdate,
            Reach::Selected {
                ids: vec![target.to_string()],
            },
        ),
        (
            Capability::AgentsGrant,
            Reach::Selected {
                ids: vec![target.to_string()],
            },
        ),
        (Capability::ThreadRead, Reach::Yes),
    ]));
    let actor = create(&agents, caps.clone()).await;
    let (inv, thread, token) = invocation(&chat, &db.pool, actor).await;
    let token = token.expose_secret();
    let (url, server) = serve(tilde::iam::listeners::agent_rpc_router(
        agents.clone(),
        chat.clone(),
    ))
    .await;
    let http = reqwest::Client::new();
    assert_eq!(
        rpc(&http, &url, "GetAgent", token, json!({"id":target}))
            .await
            .status(),
        200
    );
    assert_eq!(
        rpc(&http, &url, "GetAgent", token, json!({"id":actor}))
            .await
            .status(),
        403
    );
    let list: Value = rpc(&http, &url, "ListAgents", token, json!({}))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(list["agents"].as_array().unwrap().len(), 1);
    assert_eq!(
        rpc(&http, &url, "DeleteAgent", token, json!({"id":target}))
            .await
            .status(),
        403
    );
    let grants = json!({"agentsDelete":{"mode":"TARGET_SELECTION_ALL"}});
    assert_eq!(
        rpc(
            &http,
            &url,
            "UpdateAgent",
            token,
            json!({"id":target,"capabilities":grants})
        )
        .await
        .status(),
        403
    );
    let grants = json!({"threadRead":"BINARY_PERMISSION_YES"});
    assert_eq!(
        rpc(
            &http,
            &url,
            "UpdateAgent",
            token,
            json!({"id":target,"capabilities":grants})
        )
        .await
        .status(),
        200
    );
    assert!(
        agents
            .get(target)
            .await
            .unwrap()
            .capabilities
            .0
            .permits(Capability::ThreadRead, "")
    );
    assert_eq!(
        rpc(&http, &url, "CreateAgent", token, json!({"name":"child"}))
            .await
            .status(),
        200
    );
    assert_eq!(
        rpc(
            &http,
            &url,
            "CreateAgent",
            token,
            json!({"name":"child","capabilities":grants})
        )
        .await
        .status(),
        403
    );
    for path in [
        "/auth/login",
        "/auth/callback",
        "/auth/session",
        "/tilde.management.v1.ConnectionsService/ListConnections",
        "/tilde.management.v1.ChatService/CreateUser",
    ] {
        assert_eq!(
            http.get(format!("{url}{path}"))
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    let scoped: Value = http
        .post(format!("{url}/tilde.runtime.v1.ChatService/GetThread"))
        .bearer_auth(token)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(scoped["thread"]["id"], thread.to_string());
    let empty_agent = create(&agents, Capabilities::default()).await;
    let (_, _, empty) = invocation(&chat, &db.pool, empty_agent).await;
    assert_eq!(
        rpc(
            &http,
            &url,
            "CreateAgent",
            empty.expose_secret(),
            json!({"name":"denied"})
        )
        .await
        .status(),
        403
    );
    let tools: Value = http
        .post(format!("{url}/tilde.runtime.v1.ChatService/ListTools"))
        .bearer_auth(empty.expose_secret())
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        tools
            .get("tools")
            .is_none_or(|tools| tools.as_array().unwrap().is_empty())
    );
    // The user listener accepts only a live user session, never an agent token or missing credentials.
    let oidc = Oidc::new(
        db.pool.clone(),
        crypto,
        "http://127.0.0.1:5556".into(),
        "test".into(),
        SecretString::from("test-secret"),
        "http://127.0.0.1".into(),
        true,
    )
    .unwrap();
    let user_router = tilde::agent::rpc::management::router(agents)
        .layer(axum::middleware::from_fn_with_state(
            oidc.clone(),
            tilde::iam::oidc::management_guard,
        ))
        .merge(oidc.router());
    let (user_url, user_server) = serve(user_router).await;
    assert_eq!(
        http.get(format!("{user_url}/auth/session"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        http.post(format!(
            "{user_url}/tilde.management.v1.AgentService/GetAgent"
        ))
        .bearer_auth(token)
        .json(&json!({"id":target}))
        .send()
        .await
        .unwrap()
        .status(),
        401
    );
    let mut altered = token.to_owned();
    let index = altered.len() - 4;
    altered.replace_range(
        index..index + 1,
        if &altered[index..index + 1] == "a" {
            "b"
        } else {
            "a"
        },
    );
    assert_eq!(
        rpc(&http, &url, "GetAgent", &altered, json!({"id":target}))
            .await
            .status(),
        401
    );
    // Renewal intersects current grants with the original credential; newly added grants cannot widen it.
    agents_for_renewal(&db.pool, &chat, actor, token).await;
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET status='canceled' WHERE id=$1",
            &[&inv],
        )
        .await
        .unwrap();
    assert_eq!(
        rpc(&http, &url, "GetAgent", token, json!({"id":target}))
            .await
            .status(),
        200
    );
    assert!(chat.tokens.renew(token).await.is_err());
    server.abort();
    user_server.abort();
    db.close().await;
}

async fn agents_for_renewal(
    pool: &tilde::database::Pool,
    chat: &Chat,
    actor: Uuid,
    original: &str,
) {
    pool.get()
        .await
        .unwrap()
        .execute(
            "UPDATE agents SET capabilities=$2 WHERE id=$1",
            &[
                &actor,
                &(json!({"agents.delete":{"mode":"all"},"thread.read":{"mode":"no"}})),
            ],
        )
        .await
        .unwrap();
    let renewed = chat.tokens.renew(original).await.unwrap();
    let claims = chat.tokens.verify(renewed.expose_secret()).await.unwrap();
    assert!(!claims.capabilities.permits(Capability::AgentsDelete, ""));
    assert!(!claims.capabilities.permits(Capability::ThreadRead, ""));
}

/// Provider redirects do not carry a management session; OAuth state is their credential.
#[tokio::test]
async fn connection_callback_bypasses_management_auth_but_requires_valid_state() {
    use tilde::connections::{model::*, service::Connections};
    let db = Database::new().await;
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(8)).await.unwrap());
    let connections = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    let mut oauth = OAuth::standard("https://provider.example/token");
    oauth.authorization_url = Some("https://provider.example/authorize".into());
    oauth.client_auth = ClientAuth::None;
    oauth.pkce = true;
    connections
        .register_provider(
            Provider {
                account_name_label: None,
                icon_url: None,
                instructions: None,
                id: "custom/callback-test".into(),
                name: "Callback test".into(),
                kind: ProviderKind::Configured,
                categories: vec!["other".into()],
                connection_types: vec![ConnectionType {
                    id: "oauth".into(),
                    name: "OAuth".into(),
                    capabilities: vec![],
                    credential_source: CredentialSource::OAuth {
                        grant: OAuthGrant::AuthorizationCode,
                        configuration: oauth.into(),
                        additional_schema: None,
                    },
                }],
            },
            None,
        )
        .await
        .unwrap();
    let started = connections
        .start(
            Uuid::new_v4(),
            "Callback test",
            "custom/callback-test",
            "oauth",
            &[],
        )
        .await
        .unwrap();
    let url = url::Url::parse(&started.brokering_url).unwrap();
    let setup_id = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let connection_setup_token = url
        .query_pairs()
        .find(|(key, _)| key == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    let form = connections
        .view(setup_id, &connection_setup_token)
        .await
        .unwrap();
    let consent = connections
        .advance(
            setup_id,
            &connection_setup_token,
            form.action_id,
            BTreeMap::from([("client_id".into(), SecretString::from("client"))]),
        )
        .await
        .unwrap();
    let Action::Redirect { url } = consent.action else {
        panic!("expected OAuth consent")
    };
    let authorization = url::Url::parse(&url).unwrap();
    let state = authorization
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    let oidc = Oidc::new(
        db.pool.clone(),
        crypto,
        "http://127.0.0.1:5556".into(),
        "test".into(),
        SecretString::from("test-secret"),
        "http://127.0.0.1".into(),
        true,
    )
    .unwrap();
    let router = tilde::connections::rpc::management::router(connections.clone())
        .layer(axum::middleware::from_fn_with_state(
            oidc,
            tilde::iam::oidc::management_guard,
        ))
        .merge(tilde::connections::rpc::setup::router(connections.clone()));
    let (origin, server) = serve(router).await;
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let callback = format!("{origin}{}", tilde::connections::CALLBACK_PATH);
    let invalid = http
        .get(&callback)
        .query(&[
            ("state", format!("{setup_id}.forged")),
            ("error", "access_denied".into()),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 400);
    assert_eq!(
        connections
            .view(setup_id, &connection_setup_token)
            .await
            .unwrap()
            .step,
        "oauth_consent"
    );
    // A legitimate provider denial completes callback handling without a bearer token or cookie.
    let response = http
        .get(&callback)
        .query(&[("state", state), ("error", "access_denied".into())])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 303);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        connections
            .view(setup_id, &connection_setup_token)
            .await
            .unwrap()
            .step,
        "failed"
    );
    let protected = http
        .post(format!(
            "{origin}/tilde.management.v1.ConnectionsService/ListConnections"
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(protected.status(), 401);
    server.abort();
    db.close().await;
}

#[tokio::test]
async fn provider_management_and_runtime_contracts_are_distinct_and_cache_stays_in_runtime() {
    let db = Database::new().await;
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(73)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into());
    let agent = create(
        &agents,
        Capabilities(BTreeMap::from([(Capability::ThreadRead, Reach::Yes)])),
    )
    .await;
    let (invocation_id, thread, token) = invocation(&chat, &db.pool, agent).await;
    let user = chat.create_user("Reader").await.unwrap();
    let roster = chat
        .add_participant(tilde::chat::AddParticipant {
            thread_id: thread.to_string(),
            participant: Some(types::ParticipantRef {
                user_id: Some(user.id),
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    let participant = roster
        .participants
        .iter()
        .find(|p| p.user_id.is_some())
        .unwrap();
    let message = chat
        .post(tilde::chat::PostMessage {
            id: Uuid::new_v4().to_string(),
            thread_id: thread.to_string(),
            participant_id: participant.id.clone(),
            text: "Original".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    // This handcrafted invocation fixture is triggered by the message created above.
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET history_through_message_id=$1 WHERE id=$2",
            &[&(Uuid::parse_str(&message.id).unwrap()), &invocation_id],
        )
        .await
        .unwrap();
    let (management, management_server) = serve(common::unguarded(
        tilde::agent::rpc::management::router(agents.clone()),
        &db.pool,
    ))
    .await;
    let (provider, provider_token, provider_server) =
        common::tilde_provider(&db.pool, crypto, chat.clone(), agent).await;
    let (runtime, runtime_server) = serve(tilde::iam::listeners::agent_rpc_router(
        agents,
        chat.clone(),
    ))
    .await;
    let http = reqwest::Client::new();
    let bearer = token.expose_secret();
    for credential in [None, Some(bearer)] {
        let mut request = http
            .post(format!(
                "{provider}/tilde.provider.tilde.v1.ChatService/ListMessages"
            ))
            .json(&json!({"threadId":thread}));
        if let Some(credential) = credential {
            request = request.bearer_auth(credential);
        }
        assert_eq!(request.send().await.unwrap().status(), 403);
    }
    for (base, path) in [
        (
            &management,
            "tilde.runtime.v1.ChatService/CacheConvertedMessages",
        ),
        (&management, "tilde.runtime.v1.AgentService/ListAgents"),
        (&runtime, "tilde.management.v1.ChatService/ListThreads"),
        (&runtime, "tilde.management.v1.AgentService/ListAgents"),
        (&management, "engine.chat.v1.ChatService/GetThread"),
        (&management, "tilde.management.v1.ChatService/ListMessages"),
        (
            &management,
            "tilde.provider.tilde.v1.ChatService/ListMessages",
        ),
        (&runtime, "tilde.provider.tilde.v1.ChatService/ListMessages"),
    ] {
        assert_eq!(
            http.post(format!("{base}/{path}"))
                .bearer_auth(bearer)
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    let response = http
        .post(format!(
            "{runtime}/tilde.runtime.v1.ChatService/CacheConvertedMessages"
        ))
        .bearer_auth(bearer)
        .json(&json!({"messages":[{"messageId":message.id,"messageJson":"{\"converted\":true}"}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let runtime_history: Value = http
        .post(format!(
            "{runtime}/tilde.runtime.v1.ChatService/ListMessages"
        ))
        .bearer_auth(bearer)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        runtime_history["cachedMessages"][0]["messageId"],
        message.id
    );
    assert!(
        runtime_history["messages"][0]
            .get("cachedAgentRepresentationJson")
            .is_none()
    );
    let provider_history: Value = http
        .post(format!(
            "{provider}/tilde.provider.tilde.v1.ChatService/ListMessages"
        ))
        .bearer_auth(provider_token.expose_secret())
        .json(&json!({"threadId":thread}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(provider_history.get("cachedMessages").is_none());
    assert_eq!(provider_history["messages"][0]["text"], "Original");
    // Even raw clients cannot make runtime readers select a foreign thread.
    let response = http
        .post(format!("{runtime}/tilde.runtime.v1.ChatService/GetThread"))
        .bearer_auth(bearer)
        .json(&json!({"id":Uuid::new_v4()}))
        .send()
        .await
        .unwrap();
    if response.status().is_success() {
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["thread"]["id"], thread.to_string());
    } else {
        assert_eq!(response.status(), 400);
    }
    provider_server.abort();
    management_server.abort();
    runtime_server.abort();
    db.close().await;
}
