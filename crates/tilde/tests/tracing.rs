mod common;
use axum::{Router, body::Bytes, extract::State, routing::post};
use common::{Database, seed};
use opentelemetry::{
    KeyValue,
    trace::{Tracer, TracerProvider},
};
use opentelemetry_proto::tonic::{
    collector::trace::v1::{ExportTraceServiceRequest, ExportTraceServiceResponse},
    common::v1::{AnyValue, KeyValue as OtelValue, any_value::Value},
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};
use prost::Message;
use secrecy::{ExposeSecret, SecretString};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tilde::{
    encryption::{Encryption, SealedSecret, SecretBinding},
    iam::tokens::Tokens,
    telemetry::{Destination, Runtime},
};
use uuid::Uuid;

fn id(n: u8) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{n:012}")).unwrap()
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() }),
    )
}
fn spans(count: u64) -> ExportTraceServiceRequest {
    ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            schema_url: "https://example.com/resource-schema".into(),
            scope_spans: vec![ScopeSpans {
                schema_url: "https://example.com/scope-schema".into(),
                spans: (1..=count)
                    .map(|n| Span {
                        trace_id: vec![1; 16],
                        span_id: n.to_be_bytes().to_vec(),
                        parent_span_id: vec![2; 8],
                        name: "agent.model".into(),
                        start_time_unix_nano: 10,
                        end_time_unix_nano: 20,
                        attributes: vec![
                            OtelValue {
                                key: "tilde.client_sdk.version".into(),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue("1.2.3-client".into())),
                                }),
                                ..Default::default()
                            },
                            OtelValue {
                                key: "tilde.gateway.version".into(),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue("forged-gateway".into())),
                                }),
                                ..Default::default()
                            },
                            OtelValue {
                                key: "tilde.sidecar.version".into(),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue("forged-sidecar".into())),
                                }),
                                ..Default::default()
                            },
                            OtelValue {
                                key: "prompt".into(),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue("private test prompt".into())),
                                }),
                                ..Default::default()
                            },
                            OtelValue {
                                key: "tilde.invocation.id".into(),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue(id(99).to_string())),
                                }),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }],
    }
}
#[derive(Clone)]
struct Collector {
    available: Arc<AtomicBool>,
    attempts: Arc<AtomicUsize>,
    received: Arc<AtomicUsize>,
    payloads: Arc<std::sync::Mutex<Vec<ResourceSpans>>>,
}
async fn collect(
    State(state): State<Collector>,
    headers: http::HeaderMap,
    body: Bytes,
) -> (http::StatusCode, Vec<u8>) {
    assert_eq!(headers["authorization"], "Bearer collector-test");
    let request = ExportTraceServiceRequest::decode(body).unwrap();
    state.attempts.fetch_add(1, Ordering::SeqCst);
    if !state.available.load(Ordering::SeqCst) {
        return (http::StatusCode::SERVICE_UNAVAILABLE, vec![]);
    }
    state.received.fetch_add(
        request
            .resource_spans
            .iter()
            .flat_map(|r| &r.scope_spans)
            .map(|s| s.spans.len())
            .sum::<usize>(),
        Ordering::SeqCst,
    );
    state
        .payloads
        .lock()
        .unwrap()
        .extend(request.resource_spans);
    (
        http::StatusCode::OK,
        ExportTraceServiceResponse::default().encode_to_vec(),
    )
}

async fn collector() -> (Collector, String, tokio::task::JoinHandle<()>) {
    let state = Collector {
        available: Arc::new(AtomicBool::new(true)),
        attempts: Arc::new(AtomicUsize::new(0)),
        received: Arc::new(AtomicUsize::new(0)),
        payloads: Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let (url, server) = serve(
        Router::new()
            .route("/v1/traces", post(collect))
            .with_state(state.clone()),
    )
    .await;
    (state, url, server)
}
fn destination(url: &str) -> Option<Destination> {
    Destination::new(
        Some(format!("{url}/v1/traces")),
        Some(tilde::config::SecretEnv(SecretString::from(
            "authorization=Bearer collector-test",
        ))),
    )
    .unwrap()
}
async fn wait_count(collector: &Collector, count: usize) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while collector.received.load(Ordering::SeqCst) < count {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn durable_trace_relay_retries_collector_outages() {
    let db = Database::new().await;
    db.pool
        .get()
        .await
        .unwrap()
        .batch_execute(include_str!("sql/tracing_fixture.sql"))
        .await
        .unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(19)).await.unwrap());
    let tokens = Tokens::new(db.pool.clone(), crypto.clone());
    let token = tokens.issue(id(1), id(5), id(2), id(4)).await.unwrap();
    let (collector, remote, remote_task) = collector().await;
    collector.available.store(false, Ordering::SeqCst);
    let runtime = Runtime::start(db.pool.clone(), tokens.clone(), destination(&remote));
    let (url, server) = serve(tilde::iam::listeners::agent_runtime_router(
        tilde::agent::Agents::new(db.pool.clone(), crypto.clone()),
        tilde::chat::Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into()),
        &runtime.tracing,
    ))
    .await;
    let client = reqwest::Client::new();
    let endpoint = format!("{url}/v1/traces");
    assert_eq!(
        client
            .post(&endpoint)
            .body(spans(1).encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let mut wrong = spans(1);
    wrong.resource_spans[0].scope_spans[0].spans[0].trace_id = vec![9; 16];
    assert_eq!(
        client
            .post(&endpoint)
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(wrong.encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    // Queue acceptance is independent of downstream availability; a request may span several batches.
    assert_eq!(
        client
            .post(&endpoint)
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(spans(1536).encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while collector.attempts.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(collector.received.load(Ordering::SeqCst), 0);
    collector.available.store(true, Ordering::SeqCst);
    wait_count(&collector, 1536).await;
    assert!(
        db.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT to_regclass('tracing_spans') IS NULL", &[])
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    {
        let resources = collector.payloads.lock().unwrap();
        assert_eq!(
            resources[0].schema_url,
            "https://example.com/resource-schema"
        );
        let scope = &resources[0].scope_spans[0];
        assert_eq!(scope.schema_url, "https://example.com/scope-schema");
        for span in resources
            .iter()
            .flat_map(|r| &r.scope_spans)
            .flat_map(|s| &s.spans)
        {
            for (key, value) in [
                ("tilde.client_sdk.version", "1.2.3-client"),
                ("tilde.gateway.version", env!("CARGO_PKG_VERSION")),
                (
                    "langfuse.trace.metadata.tilde_client_sdk_version",
                    "1.2.3-client",
                ),
                (
                    "langfuse.observation.metadata.tilde_gateway_version",
                    env!("CARGO_PKG_VERSION"),
                ),
            ] {
                assert!(
                    span.attributes.iter().any(|a| a.key == key
                        && a.value.as_ref().unwrap().value
                            == Some(Value::StringValue(value.into()))),
                    "missing {key}"
                );
            }
            assert!(
                !span
                    .attributes
                    .iter()
                    .any(|a| a.key == "tilde.sidecar.version")
            );
        }
        let attrs = &scope.spans[0].attributes;
        assert!(attrs.iter().any(|a| a.key == "prompt"));
        assert!(attrs.iter().any(|a| a.key == "tilde.invocation.id"
            && a.value.as_ref().unwrap().value == Some(Value::StringValue(id(5).to_string()))));
    }
    let parent = tilde::telemetry::context::restore(
        "00-01010101010101010101010101010101-0202020202020202-01",
        "",
    );
    let mut platform = runtime
        .provider
        .tracer("test-platform")
        .start_with_context("platform.callback", &parent);
    opentelemetry::trace::Span::set_attribute(
        &mut platform,
        KeyValue::new("tilde.invocation.id", id(5).to_string()),
    );
    opentelemetry::trace::Span::end(&mut platform);
    let provider = runtime.provider.clone();
    tokio::task::spawn_blocking(move || provider.force_flush())
        .await
        .unwrap()
        .unwrap();
    wait_count(&collector, 1537).await;
    // Termination blocks renewal; the issued token remains valid and uploads final spans.
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET status='canceled',ended_at=NOW() WHERE id=$1",
            &[&(id(5))],
        )
        .await
        .unwrap();
    assert!(tokens.verify(token.expose_secret()).await.is_ok());
    assert!(tokens.renew(token.expose_secret()).await.is_err());
    assert!(tokens.verify_trace(token.expose_secret()).await.is_ok());
    assert_eq!(
        client
            .post(&endpoint)
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(spans(1).encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    // Exercise the case where a JWT expires during the terminal upload window.
    let key_row: Vec<u8> = db
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
            SecretBinding {
                resource_kind: "iam",
                resource_id: Uuid::nil(),
                name: "agent_token_signing_key",
            },
            SealedSecret::from_bytes(&key_row).unwrap(),
        )
        .unwrap();
    let mut claims = tokens.verify_trace(token.expose_secret()).await.unwrap();
    claims.exp = chrono::Utc::now().timestamp() - 1;
    claims.iat = claims.exp - 300;
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    header.typ = Some("tilde-agent-connect+jwt".into());
    let expired = SecretString::from(
        jsonwebtoken::encode(
            &header,
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(key.expose_secret().as_bytes()),
        )
        .unwrap(),
    );
    drop(key);
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET ended_at=NOW()-INTERVAL '2 seconds' WHERE id=$1",
            &[&(id(5))],
        )
        .await
        .unwrap();
    assert!(tokens.verify_trace(expired.expose_secret()).await.is_ok());
    assert!(tokens.verify(expired.expose_secret()).await.is_err());
    drop(expired);
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET ended_at=NOW()-INTERVAL '301 seconds' WHERE id=$1",
            &[&(id(5))],
        )
        .await
        .unwrap();
    assert!(tokens.verify_trace(token.expose_secret()).await.is_err());

    let mut last = runtime
        .provider
        .tracer("test-platform")
        .start("shutdown.partial");
    opentelemetry::trace::Span::end(&mut last);
    server.abort();
    runtime.shutdown().await;
    assert_eq!(collector.received.load(Ordering::SeqCst), 1539);
    assert!(
        db.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT to_regclass('sidecar_traces') IS NULL", &[])
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    assert_eq!(
        db.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM telemetry_delivery WHERE payload IS NOT NULL",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    remote_task.abort();
    db.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn request_context_survives_durable_invocation_dispatch() {
    use opentelemetry::trace::TraceContextExt;
    let db = Database::new().await;
    db.pool
        .get()
        .await
        .unwrap()
        .batch_execute(include_str!("sql/tracing_fixture.sql"))
        .await
        .unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET status='canceled',ended_at=NOW() WHERE id=$1",
            &[&(id(5))],
        )
        .await
        .unwrap();
    db.pool.get().await.unwrap().batch_execute("INSERT INTO chat_users(id,name) VALUES('00000000-0000-4000-8000-000000000006','Human'); INSERT INTO chat_participants(id,thread_id,user_id) VALUES('00000000-0000-4000-8000-000000000007','00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000006');").await.unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(29)).await.unwrap());
    common::dialled_in(&db.pool, crypto.clone(), id(1)).await;
    let chat = tilde::chat::Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1:1".into());
    let (collector, remote, remote_server) = collector().await;
    let runtime = Runtime::start(db.pool.clone(), chat.tokens.clone(), destination(&remote));
    opentelemetry::global::set_tracer_provider(runtime.provider.clone());
    async fn trigger(State(chat): State<tilde::chat::Chat>) -> String {
        chat.post(tilde::chat::PostMessage {
            thread_id: id(2).to_string(),
            id: id(8).to_string(),
            participant_id: id(7).to_string(),
            text: "trace dispatch".into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .id
    }
    let (url, server) = serve(
        Router::new()
            .route("/trigger", post(trigger))
            .with_state(chat.clone())
            .layer(axum::middleware::from_fn(
                tilde::telemetry::context::request,
            )),
    )
    .await;
    let response = reqwest::Client::new()
        .post(format!("{url}/trigger"))
        .header(
            "traceparent",
            "00-03030303030303030303030303030303-0404040404040404-01",
        )
        .header("tracestate", "test=upstream")
        .send()
        .await
        .unwrap();
    let message_id = response.text().await.unwrap();
    let (stop, rx) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(chat.worker(rx));
    let (parent,state,execution,invocation):(String,String,String,Uuid)=tokio::time::timeout(Duration::from_secs(10),async {loop {
   let row:Option<(String,String,String,String,Uuid)>=db.pool.get().await.unwrap().query_opt("SELECT i.traceparent,i.tracestate,i.execution_traceparent,i.status,i.id FROM chat_invocations i JOIN chat_runs r ON r.id=i.run_id WHERE r.idempotency_key=$1", &[&(&message_id)]).await.map(|row| row.map(|row| (row.get::<_, String>(0),row.get::<_, String>(1),row.get::<_, String>(2),row.get::<_, String>(3),row.get::<_, Uuid>(4)))).unwrap();
   // No host has dialled in for the fixture. Dispatch must still retain request
   // ancestry and export its failure span, rather than create an unrelated trace.
   if let Some(row)=row && row.3=="failed" {break(row.0,row.1,row.2,row.4);}tokio::time::sleep(Duration::from_millis(50)).await;
 }}).await.unwrap();
    assert_eq!(state, "test=upstream");
    assert!(parent.starts_with("00-03030303030303030303030303030303-"));
    assert!(execution.starts_with("00-03030303030303030303030303030303-"));
    assert_ne!(parent, execution);
    stop.send(true).unwrap();
    worker.await.unwrap();
    server.abort();
    runtime.shutdown().await;
    let rows: Vec<_> = collector
        .payloads
        .lock()
        .unwrap()
        .iter()
        .flat_map(|r| &r.scope_spans)
        .flat_map(|s| &s.spans)
        .map(|span| {
            let owner = span
                .attributes
                .iter()
                .find(|a| a.key == "tilde.invocation.id")
                .and_then(|a| a.value.as_ref())
                .and_then(|v| match &v.value {
                    Some(Value::StringValue(v)) => v.parse::<Uuid>().ok(),
                    _ => None,
                });
            (span.span_id.clone(), span.parent_span_id.clone(), owner)
        })
        .collect();
    remote_server.abort();
    let request_span = hex::decode(parent.split('-').nth(2).unwrap()).unwrap();
    assert!(
        rows.iter()
            .any(|(span, parent, owner)| *span == request_span
                && *parent == vec![4u8; 8]
                && owner.is_none())
    );
    assert!(
        rows.iter()
            .any(|(_, parent, owner)| *parent == request_span && *owner == Some(invocation))
    );
    assert!(
        !opentelemetry::Context::current()
            .span()
            .span_context()
            .is_valid(),
        "request context must not leak to unrelated tasks"
    );
    db.close().await;
}

/// Exercise the production listener composition, including telemetry's independent late-flush auth.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn agent_trace_capture_is_runtime_only_and_rejects_management_credentials() {
    use sha2::{Digest, Sha256};
    use tilde::{
        agent::Agents,
        chat::Chat,
        connections::service::Connections,
        iam::{listeners, oidc::Oidc},
    };

    let db = Database::new().await;
    db.pool
        .get()
        .await
        .unwrap()
        .batch_execute(include_str!("sql/tracing_fixture.sql"))
        .await
        .unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(27)).await.unwrap());
    let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into());
    let agent_token = chat.tokens.issue(id(1), id(5), id(2), id(4)).await.unwrap();
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let connections = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    let oidc = Oidc::new(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1:5556".into(),
        "trace-boundary".into(),
        SecretString::from("fixture-oidc-secret"),
        "http://127.0.0.1".into(),
        true,
    )
    .unwrap();
    let management_token = SecretString::from("fixture-management-session-for-trace-boundary");
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO iam_users(id,issuer,subject) VALUES($1,$2,$3)",
            &[&(id(10)), &("http://127.0.0.1:5556"), &("trace-operator")],
        )
        .await
        .unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO iam_group_members(group_id,user_id) VALUES('tilde_system:admin',$1)",
            &[&(id(10))],
        )
        .await
        .unwrap();
    db.pool.get().await.unwrap().execute("INSERT INTO iam_sessions(token_hash,user_id,expires_at) VALUES($1,$2,NOW()+INTERVAL '1 hour')", &[&(Sha256::digest(management_token.expose_secret().as_bytes()).to_vec()), &(id(10))]).await.unwrap();
    let (collector, remote, remote_server) = collector().await;
    let telemetry = Runtime::start(db.pool.clone(), chat.tokens.clone(), destination(&remote));
    let (runtime_url, runtime_server) = serve(listeners::agent_runtime_router(
        agents.clone(),
        chat.clone(),
        &telemetry.tracing,
    ))
    .await;
    let (management_url, management_server) = serve(listeners::management_router(
        agents,
        chat,
        connections,
        oidc,
    ))
    .await;
    let http = reqwest::Client::new();
    // Establish that this is a real valid management session, not merely an invalid token rejection.
    assert_eq!(
        http.post(format!(
            "{management_url}/tilde.management.v1.AgentService/ListAgents"
        ))
        .bearer_auth(management_token.expose_secret())
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    // The control plane never accepts an agent's invocation token, and a management RPC
    // reached with it does not even resolve a caller.
    assert_eq!(
        http.post(format!(
            "{management_url}/tilde.management.v1.AgentService/ListAgents"
        ))
        .bearer_auth(agent_token.expose_secret())
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap()
        .status(),
        401
    );
    for token in [
        None,
        Some(management_token.expose_secret()),
        Some(agent_token.expose_secret()),
    ] {
        let mut request = http
            .post(format!("{management_url}/v1/traces"))
            .header("content-type", "application/x-protobuf")
            .body(spans(1).encode_to_vec());
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        assert_eq!(request.send().await.unwrap().status(), 404);
    }
    let endpoint = format!("{runtime_url}/v1/traces");
    for token in [None, Some(management_token.expose_secret())] {
        let mut request = http
            .post(&endpoint)
            .header("content-type", "application/x-protobuf")
            .header("x-tilde-trace-invocation", id(5).to_string())
            .header(
                "x-tilde-trace-parent",
                "00-01010101010101010101010101010101-0202020202020202-01",
            )
            .body(spans(1).encode_to_vec());
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        assert_eq!(request.send().await.unwrap().status(), 401);
    }
    assert_eq!(collector.received.load(Ordering::SeqCst), 0);
    for path in ["/v1/metrics", "/v1/logs"] {
        assert_eq!(
            http.post(format!("{runtime_url}{path}"))
                .bearer_auth(agent_token.expose_secret())
                .header("content-type", "application/x-protobuf")
                .body(Vec::new())
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    assert_eq!(
        http.post(&endpoint)
            .bearer_auth(agent_token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(spans(1).encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    wait_count(&collector, 1).await;
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET status='stopped',ended_at=NOW() WHERE id=$1",
            &[&(id(5))],
        )
        .await
        .unwrap();
    assert_eq!(
        http.post(format!(
            "{runtime_url}/tilde.runtime.v1.ChatService/ListTools"
        ))
        .bearer_auth(agent_token.expose_secret())
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    assert_eq!(
        http.post(&endpoint)
            .bearer_auth(agent_token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(spans(2).encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET ended_at=NOW()-INTERVAL '6 minutes' WHERE id=$1",
            &[&(id(5))],
        )
        .await
        .unwrap();
    assert_eq!(
        http.post(&endpoint)
            .bearer_auth(agent_token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(spans(3).encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    runtime_server.abort();
    management_server.abort();
    telemetry.shutdown().await;
    remote_server.abort();
    db.close().await;
}
