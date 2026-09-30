//! The inference gateway end to end on the gateway runtime listener: a provider connection
//! set up through the broker, assigned to an agent, routed by slug with the invocation token,
//! bytes relayed untouched both ways, usage accounted off the request path, and access
//! revoked through token renewal rather than any per-request lookup.
mod common;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;
use std::sync::{Arc, Mutex};
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

struct Captured {
    path: String,
    headers: HeaderMap,
    body: Vec<u8>,
}
type Seen = Arc<Mutex<Vec<Captured>>>;
const SSE: &str = "data: {\"id\":\"c1\",\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\ndata: {\"id\":\"c1\",\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\ndata: {\"id\":\"c1\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":2}}}\n\ndata: [DONE]\n\n";

/// A stand-in provider speaking OpenAI's wire format, recording what it receives.
async fn provider() -> (String, Seen, tokio::task::JoinHandle<()>) {
    let seen: Seen = Arc::default();
    async fn record(seen: &Seen, path: &str, headers: HeaderMap, body: Bytes) {
        seen.lock().unwrap().push(Captured {
            path: path.into(),
            headers,
            body: body.to_vec(),
        });
    }
    async fn chat(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
        record(&seen, "/v1/chat/completions", headers, body).await;
        let frames = SSE
            .split_inclusive("\n\n")
            .map(|f| Ok::<_, std::io::Error>(Bytes::from(f.to_owned())))
            .collect::<Vec<_>>();
        let stream = futures::stream::iter(frames).then(|frame| async {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            frame
        });
        (
            [
                ("content-type", "text/event-stream"),
                ("x-request-id", "req-1"),
            ],
            Body::from_stream(stream),
        )
            .into_response()
    }
    async fn embeddings(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
        record(&seen, "/v1/embeddings", headers, body).await;
        axum::Json(json!({"object":"list","data":[],"usage":{"prompt_tokens":7,"total_tokens":7}}))
            .into_response()
    }
    async fn limited(State(seen): State<Seen>, headers: HeaderMap, body: Bytes) -> Response {
        record(&seen, "/v1/responses", headers, body).await;
        (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(json!({"error":{"message":"slow down","type":"rate_limit"}})),
        )
            .into_response()
    }
    use futures::StreamExt;
    let router = Router::new()
        .route("/v1/chat/completions", post(chat))
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/responses", post(limited))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (url, seen, task)
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (url, task)
}
async fn create_agent(agents: &Agents) -> Uuid {
    let id = Uuid::new_v4();
    agents
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id,
            name: "Inference fixture".into(),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    id
}
/// A live invocation and its connect token, as an agent process would hold them.
async fn invocation(chat: &Chat, pool: &tilde::database::Pool, agent: Uuid) -> SecretString {
    let thread = chat
        .create_thread(application::CreateThread {
            title: "Inference".into(),
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
            objective: "Answer".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    pool.get().await.unwrap().execute("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1", &[&invocation]).await.unwrap();
    chat.tokens
        .issue(
            agent,
            invocation,
            Uuid::parse_str(&thread.id).unwrap(),
            Uuid::parse_str(&run.id).unwrap(),
        )
        .await
        .unwrap()
}
fn brokering(url: &str) -> (Uuid, String) {
    let url = url::Url::parse(url).unwrap();
    (
        Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap(),
        url.query_pairs()
            .find(|(key, _)| key == "connection_setup_token")
            .unwrap()
            .1
            .into_owned(),
    )
}
/// Connect a provider through the broker exactly as the setup iframe does.
async fn connect(
    connections: &Connections,
    provider: &str,
    account: &str,
    agent: Uuid,
    fields: &[(&str, &str)],
) -> Uuid {
    let id = Uuid::new_v4();
    let started = connections
        .start(
            id,
            "OpenAI",
            provider,
            "api",
            &[Assignment {
                capability: Capability::Inference,
                agent_id: agent,
                alias: None,
            }],
        )
        .await
        .unwrap();
    let (setup, token) = brokering(&started.brokering_url);
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
    id
}

#[tokio::test]
async fn forwards_provider_traffic_by_slug_and_accounts_usage_off_the_hot_path() {
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(9)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let (upstream_url, seen, upstream_task) = provider().await;
    let agent = create_agent(&agents).await;
    let outsider = create_agent(&agents).await;

    // Account names must be URL-path safe: they are the slug.
    let id = Uuid::new_v4();
    let started = connections
        .start(id, "OpenAI", "openai", "api", &[])
        .await
        .unwrap();
    let (setup, token) = brokering(&started.brokering_url);
    let view = connections.view(setup, &token).await.unwrap();
    assert!(
        connections
            .set_connection_name(setup, &token, view.action_id, "prod key")
            .await
            .is_err()
    );
    // A key that does not form an upstream is refused at setup, not at first use.
    let bad = connections
        .set_connection_name(setup, &token, view.action_id, "broken")
        .await
        .unwrap();
    assert!(
        connections
            .advance(
                setup,
                &token,
                bad.action_id,
                [
                    ("api_key".to_string(), SecretString::from("k")),
                    ("base_url".to_string(), SecretString::from("ftp://x"))
                ]
                .into_iter()
                .collect(),
            )
            .await
            .is_err()
    );

    let connection = connect(
        &connections,
        "openai",
        "prod",
        agent,
        &[
            ("api_key", "sk-test-key"),
            ("base_url", upstream_url.as_str()),
        ],
    )
    .await;
    let ready = connections.get(connection).await.unwrap();
    assert_eq!(
        (ready.status.as_str(), ready.slug().as_str()),
        ("ready", "openai/prod")
    );
    assert!(ready.inference_capable && !ready.channel_capable);
    // A slug is unique among ready assignments to the same agent and capability.
    let dup = Uuid::new_v4();
    let started = connections
        .start(
            dup,
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
    let (setup, token) = brokering(&started.brokering_url);
    let view = connections.view(setup, &token).await.unwrap();
    assert!(
        connections
            .set_connection_name(setup, &token, view.action_id, "prod")
            .await
            .is_err()
    );
    connections
        .unassign(
            dup,
            &Assignment {
                capability: Capability::Inference,
                agent_id: agent,
                alias: None,
            },
        )
        .await
        .unwrap();
    // Several agents may share one key; chat's single-owner rule does not apply.
    connections
        .assign(
            connection,
            &Assignment {
                capability: Capability::Inference,
                agent_id: outsider,
                alias: None,
            },
        )
        .await
        .unwrap();
    connections
        .unassign(
            connection,
            &Assignment {
                capability: Capability::Inference,
                agent_id: outsider,
                alias: None,
            },
        )
        .await
        .unwrap();
    let (listed, _) = connections
        .list(None, 10, None, Some(Capability::Inference))
        .await
        .unwrap();
    // Every inference-capable connection is offered, ready or not, so a half-configured
    // key can still be picked and finished; the route map below only holds ready ones.
    assert!(
        listed.iter().all(|c| c.inference_capable) && listed.iter().any(|c| c.id == connection)
    );
    let (assigned, _) = connections
        .list(None, 10, Some(agent), Some(Capability::Inference))
        .await
        .unwrap();
    assert_eq!(
        assigned.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![connection]
    );

    // An agent may alias a connection; aliases share the slug charset and cannot be provider ids.
    let alias = |alias: &str| Assignment {
        capability: Capability::Inference,
        agent_id: agent,
        alias: Some(alias.into()),
    };
    assert!(
        connections
            .assign(connection, &alias("openai"))
            .await
            .is_err()
    );
    assert!(
        connections
            .assign(connection, &alias("fast model"))
            .await
            .is_err()
    );
    connections
        .assign(connection, &alias("fast"))
        .await
        .unwrap();
    assert_eq!(
        connections
            .get(connection)
            .await
            .unwrap()
            .associated_agents
            .0
            .iter()
            .find(|a| a.id == agent)
            .and_then(|a| a.alias.clone())
            .as_deref(),
        Some("fast")
    );
    // The gateway's credential map is loaded once and refreshed on change, never per request.
    let loader = inference::gateway::Loader::new(db.pool.clone(), connections.clone());
    loader.load().await.unwrap();
    assert_eq!(loader.upstreams.slugs(), vec!["openai/prod".to_string()]);
    let gateway = Gateway {
        upstreams: loader.upstreams.clone(),
        audit: audit::Audit::start(audit::Sink::Postgres(db.pool.clone())),
    };
    let chat = Chat::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
    )
    .with_inference(gateway.clone());
    let (url, server) = serve(tilde::iam::listeners::agent_rpc_router(
        agents.clone(),
        chat.clone(),
    ))
    .await;
    let token = invocation(&chat, &db.pool, agent).await;
    let http = reqwest::Client::new();
    let request =
        json!({"model":"gpt-5","stream":true,"messages":[{"role":"user","content":"hi"}]});
    let body = serde_json::to_vec(&request).unwrap();

    // Streaming chat: bytes in and out are exactly the provider's; only credentials change.
    let response = http
        .post(format!("{url}/inference/openai/prod/chat/completions"))
        .bearer_auth(token.expose_secret())
        .header("content-type", "application/json")
        .header("x-api-key", "placeholder-from-provider-sdk")
        .header("x-custom", "kept")
        .header("x-tilde-internal", "dropped")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["x-request-id"], "req-1");
    assert_eq!(response.text().await.unwrap(), SSE);
    {
        let seen = seen.lock().unwrap();
        let call = seen.last().unwrap();
        assert_eq!(call.path, "/v1/chat/completions");
        assert_eq!(call.body, body);
        assert_eq!(call.headers["authorization"], "Bearer sk-test-key");
        assert_eq!(call.headers["x-custom"], "kept");
        assert_eq!(call.headers["content-type"], "application/json");
        assert!(!call.headers.contains_key("x-api-key"));
        assert!(!call.headers.contains_key("x-tilde-internal"));
        assert!(!call.headers.contains_key("accept-encoding"));
    }
    // The alias routes to the same upstream for this agent only.
    let aliased = http
        .post(format!("{url}/inference/fast/chat/completions"))
        .bearer_auth(token.expose_secret())
        .header("content-type", "application/json")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(aliased.status(), 200);
    assert_eq!(aliased.text().await.unwrap(), SSE);
    // Non-streaming JSON, a provider error, and paths that are not inference.
    let embeddings = http
        .post(format!("{url}/inference/openai/prod/embeddings?user=abc"))
        .bearer_auth(token.expose_secret())
        .json(&json!({"model":"text-embedding-3-small","input":"hi"}))
        .send()
        .await
        .unwrap();
    assert_eq!(embeddings.status(), 200);
    assert_eq!(
        embeddings.json::<serde_json::Value>().await.unwrap()["usage"]["prompt_tokens"],
        7
    );
    let limited = http
        .post(format!("{url}/inference/openai/prod/responses"))
        .bearer_auth(token.expose_secret())
        .json(&json!({"model":"gpt-5","input":"hi"}))
        .send()
        .await
        .unwrap();
    assert_eq!(limited.status(), 429);
    assert_eq!(
        limited.json::<serde_json::Value>().await.unwrap()["error"]["type"],
        "rate_limit"
    );
    for (path, status) in [
        ("openai/prod/models", 404),
        ("openai/prod/files", 404),
        ("openai/staging/chat/completions", 404),
        ("anthropic/prod/messages", 404),
        ("slow/chat/completions", 404),
    ] {
        let response = http
            .post(format!("{url}/inference/{path}"))
            .bearer_auth(token.expose_secret())
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{path}");
    }
    assert_eq!(
        http.post(format!("{url}/inference/openai/prod/chat/completions"))
            .body("{}")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        seen.lock().unwrap().len(),
        4,
        "refused calls never reach the provider"
    );

    // Accounting happened off the request path, once per forwarded call, with parsed usage.
    gateway.audit.flush().await.unwrap();
    let mut rows = inference::db::requests_for_agent_all(&db.pool.get().await.unwrap(), agent)
        .await
        .unwrap();
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(rows.len(), 4);
    rows.dedup_by(|a, b| a.path == b.path);
    assert_eq!(
        rows.len(),
        3,
        "the aliased call is accounted like the slug call"
    );
    let chat_row = &rows[0];
    assert_eq!(
        (
            chat_row.kind.as_str(),
            chat_row.path.as_str(),
            chat_row.model.as_deref(),
            chat_row.status
        ),
        ("chat", "chat/completions", Some("gpt-5"), 200)
    );
    assert_eq!(
        (
            chat_row.input_tokens,
            chat_row.output_tokens,
            chat_row.cached_input_tokens,
            chat_row.usage.as_str()
        ),
        (Some(10), Some(5), Some(2), "parsed")
    );
    assert_eq!(chat_row.request_bytes, body.len() as i64);
    assert_eq!(chat_row.response_bytes, SSE.len() as i64);
    assert!(
        chat_row.first_byte_ms.is_some() && chat_row.latency_ms >= chat_row.first_byte_ms.unwrap()
    );
    assert_eq!(chat_row.connection_id, connection);
    assert_eq!(
        (
            rows[1].kind.as_str(),
            rows[1].input_tokens,
            rows[1].output_tokens,
            rows[1].usage.as_str()
        ),
        ("embeddings", Some(7), None, "parsed")
    );
    assert_eq!(
        (
            rows[2].kind.as_str(),
            rows[2].status,
            rows[2].usage.as_str()
        ),
        ("chat", 429, "error")
    );

    // Another agent's valid token cannot use a connection it is not assigned.
    let other = invocation(&chat, &db.pool, outsider).await;
    let response = http
        .post(format!("{url}/inference/openai/prod/chat/completions"))
        .bearer_auth(other.expose_secret())
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let response = http
        .post(format!("{url}/inference/fast/chat/completions"))
        .bearer_auth(other.expose_secret())
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404, "aliases are per agent");

    // Revocation removes this agent's route; the next renewed token also drops the grant.
    // Nothing on the request path consults Postgres.
    connections
        .unassign(
            connection,
            &Assignment {
                capability: Capability::Inference,
                agent_id: agent,
                alias: None,
            },
        )
        .await
        .unwrap();
    loader.load().await.unwrap();
    assert!(loader.upstreams.slugs().is_empty());
    let renewed = chat.tokens.renew(token.expose_secret()).await.unwrap();
    let response = http
        .post(format!("{url}/inference/openai/prod/chat/completions"))
        .bearer_auth(renewed.expose_secret())
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    // Disconnecting removes the upstream itself.
    connections.disconnect(connection).await.unwrap();
    loader.load().await.unwrap();
    assert!(loader.upstreams.slugs().is_empty());

    server.abort();
    upstream_task.abort();
    db.close().await;
}

#[test]
fn paths_map_to_inference_kinds_and_nothing_else() {
    use inference::Kind;
    for (path, kind) in [
        ("chat/completions", Some(Kind::Chat)),
        ("responses", Some(Kind::Chat)),
        ("messages", Some(Kind::Chat)),
        (
            "models/gemini-2.5-pro:streamGenerateContent",
            Some(Kind::Chat),
        ),
        ("model/anthropic.claude-3/converse-stream", Some(Kind::Chat)),
        ("embeddings", Some(Kind::Embeddings)),
        (
            "models/text-embedding-004:embedContent",
            Some(Kind::Embeddings),
        ),
        ("rerank", Some(Kind::Rerank)),
        ("messages/count_tokens", Some(Kind::CountTokens)),
        ("models/gemini-2.5-pro:countTokens", Some(Kind::CountTokens)),
        ("images/generations", Some(Kind::Image)),
        ("audio/transcriptions", Some(Kind::Transcription)),
        ("audio/speech", Some(Kind::Speech)),
        ("models", None),
        ("files", None),
        ("fine_tuning/jobs", None),
        ("speech", None),
        ("generations", None),
    ] {
        assert_eq!(Kind::of(path), kind, "{path}");
    }
}

#[tokio::test]
async fn identical_provider_ids_route_to_each_agents_own_credentials() {
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(44)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1".into(),
        "http://127.0.0.1".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let (upstream_url, seen, upstream_task) = provider().await;
    let a = create_agent(&agents).await;
    let b = create_agent(&agents).await;
    let ca = connect(
        &connections,
        "openai",
        "prod",
        a,
        &[("api_key", "key-for-a"), ("base_url", &upstream_url)],
    )
    .await;
    let cb = connect(
        &connections,
        "openai",
        "prod",
        b,
        &[("api_key", "key-for-b"), ("base_url", &upstream_url)],
    )
    .await;
    assert!(
        connections
            .assign(
                cb,
                &Assignment {
                    capability: Capability::Inference,
                    agent_id: a,
                    alias: None
                }
            )
            .await
            .is_err()
    );
    let loader = inference::gateway::Loader::new(db.pool.clone(), connections.clone());
    loader.load().await.unwrap();
    assert_eq!(
        loader
            .upstreams
            .get(a, "openai", "prod")
            .unwrap()
            .connection,
        ca
    );
    assert_eq!(
        loader
            .upstreams
            .get(b, "openai", "prod")
            .unwrap()
            .connection,
        cb
    );
    let gateway = Gateway {
        upstreams: loader.upstreams.clone(),
        audit: audit::Audit::start(audit::Sink::Postgres(db.pool.clone())),
    };
    let chat = Chat::new(db.pool.clone(), encryption, "http://127.0.0.1".into())
        .with_connections(connections)
        .with_inference(gateway);
    let (origin, server) = serve(tilde::iam::listeners::agent_rpc_router(
        agents,
        chat.clone(),
    ))
    .await;
    for agent in [a, b] {
        let token = invocation(&chat, &db.pool, agent).await;
        let response = reqwest::Client::new()
            .post(format!("{origin}/inference/openai/prod/embeddings"))
            .bearer_auth(token.expose_secret())
            .json(&json!({"model":"fixture","input":"test"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        response.bytes().await.unwrap();
    }
    {
        let requests = seen.lock().unwrap();
        assert_eq!(requests[0].headers["authorization"], "Bearer key-for-a");
        assert_eq!(requests[1].headers["authorization"], "Bearer key-for-b");
    }
    server.abort();
    upstream_task.abort();
    db.close().await;
}
