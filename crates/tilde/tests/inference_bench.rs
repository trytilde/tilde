//! Benchmark fixture, not a test: serves a release-mode gateway runtime listener with one
//! agent, an `openai/mock` connection pointed at the shared mock upstream (and `openai/real`
//! when OPENAI_API_KEY is set), and writes the base URL plus a live invocation token to
//! `TILDE_BENCH_OUT`. It renews the token while `TILDE_BENCH_STOP` does not exist.
//! Run through `task bench:inference`.
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::{model::*, service::Connections},
    encryption::Encryption,
    inference::{self, Gateway, audit},
};
use uuid::Uuid;

async fn connect(connections: &Connections, agent: Uuid, account: &str, fields: &[(&str, &str)]) {
    let id = Uuid::new_v4();
    let started = connections
        .start(
            id,
            "OpenAI",
            "openai",
            "api",
            &[Assignment {
                capability: Capability::Inference,
                agent_id: agent,
                alias: None,
            }],
        )
        .await
        .unwrap();
    let url = url::Url::parse(&started.brokering_url).unwrap();
    let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let token = url
        .query_pairs()
        .find(|(k, _)| k == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    let view = connections.view(setup, &token).await.unwrap();
    let view = connections
        .set_connection_name(setup, &token, view.action_id, account)
        .await
        .unwrap();
    let values: Values = fields
        .iter()
        .map(|(k, v)| (k.to_string(), SecretString::from(*v)))
        .collect();
    connections
        .advance(setup, &token, view.action_id, values)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "benchmark fixture; run through task bench:inference"]
async fn serve_bench_gateway() {
    let out = std::env::var("TILDE_BENCH_OUT")
        .unwrap_or_else(|_| "/tmp/tilde-inference-bench.json".into());
    let stop = std::env::var("TILDE_BENCH_STOP")
        .unwrap_or_else(|_| "/tmp/tilde-inference-bench.stop".into());
    let listen = std::env::var("TILDE_BENCH_LISTEN").unwrap_or_else(|_| "127.0.0.1:18400".into());
    let mock =
        std::env::var("TILDE_BENCH_MOCK").unwrap_or_else(|_| "http://127.0.0.1:18300/v1".into());
    let _ = std::fs::remove_file(&stop);
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(5)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
        "http://127.0.0.1:2".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let agent = Uuid::new_v4();
    agents
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id: agent,
            name: "Bench".into(),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    connect(
        &connections,
        agent,
        "mock",
        &[("api_key", "sk-mock"), ("base_url", mock.as_str())],
    )
    .await;
    if let Ok(key) = std::env::var("OPENAI_API_KEY")
        && !key.is_empty()
    {
        connect(&connections, agent, "real", &[("api_key", key.as_str())]).await;
    }
    let loader = inference::gateway::Loader::new(db.pool.clone(), connections.clone());
    loader.load().await.unwrap();
    let chat = Chat::new(
        db.pool.clone(),
        encryption.clone(),
        format!("http://{listen}"),
    )
    .with_connections(connections.clone())
    .with_inference(Gateway {
        upstreams: loader.upstreams.clone(),
        audit: audit::Audit::start(audit::Sink::Postgres(db.pool.clone())),
    });
    let router = tilde::iam::listeners::agent_rpc_router(agents.clone(), chat.clone());
    let listener = tokio::net::TcpListener::bind(&listen).await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    use axum::serve::ListenerExt;
    let listener = listener.tap_io(|stream| {
        let _ = stream.set_nodelay(true);
    });
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    // One live invocation whose token the load generator uses; renewed before it expires.
    let thread = chat
        .create_thread(application::CreateThread {
            title: "Bench".into(),
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
            objective: "bench".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    db.pool.get().await.unwrap().execute("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '1 hour' WHERE id=$1", &[&invocation]).await.unwrap();
    let mut token = chat
        .tokens
        .issue(
            agent,
            invocation,
            Uuid::parse_str(&thread.id).unwrap(),
            Uuid::parse_str(&run.id).unwrap(),
        )
        .await
        .unwrap();
    let write = |token: &SecretString| {
        std::fs::write(&out, serde_json::json!({"url": url, "token": token.expose_secret(), "slugs": loader.upstreams.slugs()}).to_string()).unwrap();
    };
    write(&token);
    eprintln!("bench gateway ready at {url}; stop by creating {stop}");
    let started = std::time::Instant::now();
    while !std::path::Path::new(&stop).exists()
        && started.elapsed() < std::time::Duration::from_secs(1800)
    {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if started.elapsed().as_secs().is_multiple_of(180) {
            db.pool.get().await.unwrap().execute("UPDATE chat_invocations SET lease_expires_at=NOW()+INTERVAL '1 hour' WHERE id=$1", &[&invocation]).await.unwrap();
            token = chat.tokens.renew(token.expose_secret()).await.unwrap();
            write(&token);
        }
    }
    server.abort();
    db.close().await;
}
