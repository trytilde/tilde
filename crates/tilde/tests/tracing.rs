#[allow(dead_code)]
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
    (
        http::StatusCode::OK,
        ExportTraceServiceResponse::default().encode_to_vec(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rotel_persists_before_ack_deduplicates_and_forwards_independently() {
    let db = Database::new().await;
    sqlx::raw_sql(include_str!("sql/tracing_fixture.sql"))
        .execute(&db.pool)
        .await
        .unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(19)).await.unwrap());
    let tokens = Tokens::new(db.pool.clone(), crypto.clone());
    let token = tokens.issue(id(1), id(5), id(2), id(4)).await.unwrap();
    let collector = Collector {
        available: Arc::new(AtomicBool::new(false)),
        attempts: Arc::new(AtomicUsize::new(0)),
        received: Arc::new(AtomicUsize::new(0)),
    };
    let (remote, remote_task) = serve(
        Router::new()
            .route("/v1/traces", post(collect))
            .with_state(collector.clone()),
    )
    .await;
    let destination = Destination::new(
        Some(format!("{remote}/v1/traces")),
        Some(tilde::config::SecretEnv(SecretString::from(
            "authorization=Bearer collector-test",
        ))),
    )
    .unwrap();
    let runtime = Runtime::start(db.pool.clone(), tokens.clone(), destination, 7);
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
    // No success before the asynchronous database transaction can commit.
    let mut lock = db.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE tracing_spans IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let request = client
        .post(&endpoint)
        .bearer_auth(token.expose_secret())
        .header("content-type", "application/x-protobuf")
        .body(spans(1536).encode_to_vec());
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert!(!pending.is_finished());
    lock.commit().await.unwrap();
    assert_eq!(pending.await.unwrap().status(), 200);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tracing_spans")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1536);
    assert_eq!(collector.received.load(Ordering::SeqCst), 0);
    // Retry after an ambiguous response must not overwrite or duplicate stored spans.
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
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tracing_spans")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1536);
    let (otlp, owner): (Vec<u8>, Uuid) =
        sqlx::query_as("SELECT otlp,invocation_id FROM tracing_spans LIMIT 1")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(owner, id(5));
    let decoded = ExportTraceServiceRequest::decode(otlp.as_slice()).unwrap();
    let prompt = &decoded.resource_spans[0].scope_spans[0].spans[0].attributes;
    assert!(prompt.iter().any(|a| a.key == "prompt"
        && a.value.as_ref().unwrap().value
            == Some(Value::StringValue("private test prompt".into()))));
    assert_eq!(
        decoded.resource_spans[0].schema_url,
        "https://example.com/resource-schema"
    );
    assert_eq!(
        decoded.resource_spans[0].scope_spans[0].schema_url,
        "https://example.com/scope-schema"
    );
    let attributes = &decoded.resource_spans[0].scope_spans[0].spans[0].attributes;
    assert!(attributes.iter().any(|a| a.key == "tilde.invocation.id"
        && a.value.as_ref().unwrap().value == Some(Value::StringValue(id(5).to_string()))));
    // A platform span joins the same trace through the provider -> Rotel path.
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
    collector.available.store(true, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let pending: i64 =
                sqlx::query_scalar("SELECT count(*) FROM tracing_spans WHERE delivery='pending'")
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            if pending == 0 && collector.received.load(Ordering::SeqCst) == 1537 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    // Termination revokes actions immediately, while the identical token uploads final spans.
    sqlx::query("UPDATE chat_invocations SET status='canceled',ended_at=NOW() WHERE id=$1")
        .bind(id(5))
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(tokens.verify(token.expose_secret()).await.is_err());
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
    let key_row: Vec<u8> = sqlx::query_scalar("SELECT sealed FROM iam_signing_key")
        .fetch_one(&db.pool)
        .await
        .unwrap();
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
    claims.iat = claims.exp - 600;
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
    sqlx::query("UPDATE chat_invocations SET ended_at=NOW()-INTERVAL '2 seconds' WHERE id=$1")
        .bind(id(5))
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(tokens.verify_trace(expired.expose_secret()).await.is_ok());
    assert!(tokens.verify(expired.expose_secret()).await.is_err());
    drop(expired);
    sqlx::query("UPDATE chat_invocations SET ended_at=NOW()-INTERVAL '301 seconds' WHERE id=$1")
        .bind(id(5))
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(tokens.verify_trace(token.expose_secret()).await.is_err());
    collector.available.store(false, Ordering::SeqCst);
    let mut last = runtime
        .provider
        .tracer("test-platform")
        .start("shutdown.partial");
    opentelemetry::trace::Span::end(&mut last);
    server.abort();
    runtime.shutdown().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tracing_spans")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        count, 1538,
        "shutdown must flush the provider and partial Rotel batch"
    );
    let destination = Destination::new(
        Some(format!("{remote}/v1/traces")),
        Some(tilde::config::SecretEnv(SecretString::from(
            "authorization=Bearer collector-test",
        ))),
    )
    .unwrap();
    collector.available.store(true, Ordering::SeqCst);
    let restarted = Runtime::start(db.pool.clone(), tokens, destination, 7);
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let pending: i64 =
                sqlx::query_scalar("SELECT count(*) FROM tracing_spans WHERE delivery='pending'")
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            if pending == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    restarted.shutdown().await;
    assert!(collector.received.load(Ordering::SeqCst) >= 1538);
    remote_task.abort();
    db.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn request_context_survives_durable_invocation_dispatch() {
    use opentelemetry::trace::TraceContextExt;
    let db = Database::new().await;
    sqlx::raw_sql(include_str!("sql/tracing_fixture.sql"))
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE chat_invocations SET status='canceled',ended_at=NOW() WHERE id=$1")
        .bind(id(5))
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO chat_users(id,name) VALUES('00000000-0000-4000-8000-000000000006','Human'); INSERT INTO chat_participants(id,thread_id,user_id) VALUES('00000000-0000-4000-8000-000000000007','00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000006');").execute(&db.pool).await.unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(29)).await.unwrap());
    let chat = tilde::chat::Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1:1".into());
    let runtime = Runtime::start(db.pool.clone(), chat.tokens.clone(), None, 7);
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
   let row:Option<(String,String,String,String,Uuid)>=sqlx::query_as("SELECT i.traceparent,i.tracestate,i.execution_traceparent,i.status,i.id FROM chat_invocations i JOIN chat_runs r ON r.id=i.run_id WHERE r.idempotency_key=$1").bind(&message_id).fetch_optional(&db.pool).await.unwrap();
   // The fixture has no runtime endpoint. Dispatch must still retain request
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
    let rows: Vec<(Vec<u8>, Vec<u8>, Option<Uuid>)> = sqlx::query_as(
        "SELECT span_id,parent_span_id,invocation_id FROM tracing_spans WHERE trace_id=$1",
    )
    .bind(vec![3u8; 16])
    .fetch_all(&db.pool)
    .await
    .unwrap();
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
    sqlx::raw_sql(include_str!("sql/tracing_fixture.sql"))
        .execute(&db.pool)
        .await
        .unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(27)).await.unwrap());
    let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into());
    let agent_token = chat.tokens.issue(id(1), id(5), id(2), id(4)).await.unwrap();
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let connections =
        Connections::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into()).unwrap();
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
    sqlx::query("INSERT INTO iam_users(id,issuer,subject) VALUES($1,$2,$3)")
        .bind(id(10))
        .bind("http://127.0.0.1:5556")
        .bind("trace-operator")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO iam_sessions(token_hash,user_id,expires_at) VALUES($1,$2,NOW()+INTERVAL '1 hour')")
        .bind(Sha256::digest(management_token.expose_secret().as_bytes()).to_vec())
        .bind(id(10)).execute(&db.pool).await.unwrap();
    let telemetry = Runtime::start(db.pool.clone(), chat.tokens.clone(), None, 7);
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
            "{management_url}/tilde.management.v1.ChatService/GetThread"
        ))
        .bearer_auth(management_token.expose_secret())
        .json(&serde_json::json!({"id":id(2)}))
        .send()
        .await
        .unwrap()
        .status(),
        200
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
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracing_spans")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        0
    );
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
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracing_spans")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query("UPDATE chat_invocations SET status='stopped',ended_at=NOW() WHERE id=$1")
        .bind(id(5))
        .execute(&db.pool)
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
        401
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
    sqlx::query("UPDATE chat_invocations SET ended_at=NOW()-INTERVAL '6 minutes' WHERE id=$1")
        .bind(id(5))
        .execute(&db.pool)
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
    db.close().await;
}
