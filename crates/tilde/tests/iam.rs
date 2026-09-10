use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
#[allow(dead_code)]
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
            id,
            name: "IAM fixture".into(),
            endpoint_url: None,
            webhook_signing_key: SecretString::from("test-key-with-more-than-thirty-two-bytes"),
            capabilities: caps,
        })
        .await
        .unwrap();
    id
}
async fn invocation(chat: &Chat, pool: &sqlx::PgPool, agent: Uuid) -> (Uuid, Uuid, SecretString) {
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
    sqlx::query("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1").bind(invocation).execute(pool).await.unwrap();
    let token = chat
        .tokens
        .issue(agent, invocation, thread, Uuid::parse_str(&run.id).unwrap())
        .await
        .unwrap();
    (invocation, thread, token)
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
            Reach::Only {
                ids: vec![target.to_string()],
            },
        ),
        (Capability::AgentsCreate, Reach::Any),
        (
            Capability::AgentsUpdate,
            Reach::Only {
                ids: vec![target.to_string()],
            },
        ),
        (
            Capability::AgentsGrant,
            Reach::Only {
                ids: vec![target.to_string()],
            },
        ),
        (Capability::ThreadRead, Reach::Any),
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
    let grants = json!({"grants":{"agents.delete":{"mode":"any"}}});
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
    let grants = json!({"grants":{"thread.read":{"mode":"any"}}});
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
            .permits(Capability::ThreadRead, "")
    );
    assert_eq!(
        rpc(
            &http,
            &url,
            "CreateAgent",
            token,
            json!({"name":"child","webhookSigningKey":"test-key-with-more-than-thirty-two-bytes"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(rpc(&http,&url,"CreateAgent",token,json!({"name":"child","webhookSigningKey":"test-key-with-more-than-thirty-two-bytes","capabilities":grants})).await.status(),403);
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
            json!({"name":"denied","webhookSigningKey":"test-key-with-more-than-thirty-two-bytes"})
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
    sqlx::query("UPDATE chat_invocations SET status='canceled' WHERE id=$1")
        .bind(inv)
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        rpc(&http, &url, "GetAgent", token, json!({"id":target}))
            .await
            .status(),
        401
    );
    server.abort();
    user_server.abort();
    db.close().await;
}

async fn agents_for_renewal(pool: &sqlx::PgPool, chat: &Chat, actor: Uuid, original: &str) {
    sqlx::query("UPDATE agents SET capabilities=$2 WHERE id=$1")
        .bind(actor)
        .bind(json!({"agents.delete":{"mode":"any"},"thread.read":{"mode":"none"}}))
        .execute(pool)
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
    )
    .unwrap();
    let mut oauth = OAuth::standard("https://provider.example/token");
    oauth.authorization_url = Some("https://provider.example/authorize".into());
    oauth.client_auth = ClientAuth::None;
    oauth.pkce = true;
    connections
        .register_provider(
            Provider {
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
async fn management_and_runtime_contracts_are_distinct_and_cache_stays_in_runtime() {
    let db = Database::new().await;
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(73)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let chat = Chat::new(db.pool.clone(), crypto, "http://127.0.0.1".into());
    let agent = create(
        &agents,
        Capabilities(BTreeMap::from([(Capability::ThreadRead, Reach::Any)])),
    )
    .await;
    let (_, thread, token) = invocation(&chat, &db.pool, agent).await;
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
    let (management, management_server) = serve(
        tilde::chat::rpc::management::router(chat.clone())
            .merge(tilde::agent::rpc::management::router(agents.clone())),
    )
    .await;
    let (runtime, runtime_server) = serve(tilde::iam::listeners::agent_rpc_router(
        agents,
        chat.clone(),
    ))
    .await;
    let http = reqwest::Client::new();
    let bearer = token.expose_secret();
    for (base, path) in [
        (
            &management,
            "tilde.runtime.v1.ChatService/CacheConvertedMessages",
        ),
        (&management, "tilde.runtime.v1.AgentService/ListAgents"),
        (&runtime, "tilde.management.v1.ChatService/ListThreads"),
        (&runtime, "tilde.management.v1.AgentService/ListAgents"),
        (&management, "engine.chat.v1.ChatService/GetThread"),
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
    let management_history: Value = http
        .post(format!(
            "{management}/tilde.management.v1.ChatService/ListMessages"
        ))
        .json(&json!({"threadId":thread}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(management_history.get("cachedMessages").is_none());
    assert_eq!(management_history["messages"][0]["text"], "Original");
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
    management_server.abort();
    runtime_server.abort();
    db.close().await;
}
