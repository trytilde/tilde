#[allow(dead_code)]
mod common;
use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    routing::{get, post},
};
use common::{Database, seed};
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};
use prost::Message;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value as Json, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tilde::{
    config::SecretEnv,
    encryption::Encryption,
    iam::tokens::Tokens,
    proto::tilde::management::v1 as pb,
    telemetry::{Destination, Runtime, viewer::Reader},
};
use uuid::Uuid;
fn id(n: u8) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{n:012}")).unwrap()
}
#[derive(Clone, Default)]
struct Remote {
    down: Arc<AtomicBool>,
    exports: Arc<Mutex<Vec<ExportTraceServiceRequest>>>,
    reads: Arc<AtomicUsize>,
    filters: Arc<Mutex<Vec<Json>>>,
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move { axum::serve(l, router).await.unwrap() }),
    )
}
fn auth(headers: &http::HeaderMap) {
    assert_eq!(
        headers["authorization"],
        "Basic cGstZml4dHVyZTpzay1maXh0dXJl"
    );
}
async fn ingest(
    State(state): State<Remote>,
    headers: http::HeaderMap,
    body: Bytes,
) -> (
    http::StatusCode,
    [(http::HeaderName, &'static str); 1],
    Vec<u8>,
) {
    auth(&headers);
    assert_eq!(headers["x-langfuse-ingestion-version"], "4");
    if state.down.load(Ordering::SeqCst) {
        return (
            http::StatusCode::SERVICE_UNAVAILABLE,
            [(http::header::CONTENT_TYPE, "application/json")],
            vec![],
        );
    }
    state
        .exports
        .lock()
        .unwrap()
        .push(ExportTraceServiceRequest::decode(body).unwrap());
    (
        http::StatusCode::OK,
        [(http::header::CONTENT_TYPE, "application/json")],
        br#"{"name":"otel-ingestion-job","id":"accepted"}"#.to_vec(),
    )
}
async fn projects(
    State(state): State<Remote>,
    headers: http::HeaderMap,
) -> (http::StatusCode, axum::Json<Json>) {
    auth(&headers);
    state.reads.fetch_add(1, Ordering::SeqCst);
    if state.down.load(Ordering::SeqCst) {
        return (http::StatusCode::UNAUTHORIZED, axum::Json(json!({})));
    }
    (
        http::StatusCode::OK,
        axum::Json(json!({"data":[{"id":"project-fixture","name":"Fixture"}]})),
    )
}
async fn observations(
    State(state): State<Remote>,
    headers: http::HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> axum::Json<Json> {
    auth(&headers);
    assert!(
        !query.contains_key("cursor"),
        "first page must omit cursor rather than send an empty value"
    );
    let filter: Json = serde_json::from_str(&query["filter"]).unwrap();
    assert!(
        filter
            .as_array()
            .unwrap()
            .iter()
            .any(
                |f| (f["key"] == "tilde_agent_id" && f["value"] == id(1).to_string())
                    || f["column"] == "traceId"
            )
    );
    state.filters.lock().unwrap().push(filter);
    state.reads.fetch_add(1, Ordering::SeqCst);
    let mut response = json!({"data":[{"id":"span-1","traceId":"trace-1","name":"model call","type":"GENERATION","level":"DEFAULT","sessionId":"session / one","startTime":"2026-09-11T10:00:00Z","input":"{\"text\":\"hello\"}","output":"hi","metadata":{"tilde_agent_id":id(1),"tilde_invocation_id":id(5)},"totalUsage":12,"totalCost":"0.001"},{"id":"foreign","metadata":{"tilde_agent_id":id(99)}}],"meta":{"cursor":"next-page"}});
    response["data"].as_array_mut().unwrap().push(json!({"id":"root","traceId":"trace-1","name":"Gateway trigger","type":"SPAN","metadata":{}}));
    let duplicate = response["data"][0].clone();
    response["data"].as_array_mut().unwrap().push(duplicate);
    axum::Json(response)
}
async fn remote() -> (Remote, String, tokio::task::JoinHandle<()>) {
    let state = Remote::default();
    let (url, server) = serve(
        Router::new()
            .route("/lf/api/public/projects", get(projects))
            .route("/lf/api/public/v2/observations", get(observations))
            .route("/lf/api/public/otel/v1/traces", post(ingest))
            .with_state(state.clone()),
    )
    .await;
    (state, format!("{url}/lf"), server)
}
fn destination(url: &str) -> Option<Destination> {
    Destination::langfuse(
        Some(url.into()),
        Some("https://viewer.example/prefix".into()),
        Some(SecretEnv(SecretString::from("pk-fixture"))),
        Some(SecretEnv(SecretString::from("sk-fixture"))),
    )
    .unwrap()
}
fn payload(n: u64) -> Vec<u8> {
    ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            scope_spans: vec![ScopeSpans {
                spans: vec![Span {
                    trace_id: vec![1; 16],
                    span_id: n.to_be_bytes().to_vec(),
                    parent_span_id: vec![2; 8],
                    name: "ai.generateText.doGenerate".into(),
                    start_time_unix_nano: 10,
                    end_time_unix_nano: 20,
                    attributes: vec![KeyValue {
                        key: "ai.prompt.messages".into(),
                        value: Some(AnyValue {
                            value: Some(Value::StringValue("hello".into())),
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
    }
    .encode_to_vec()
}
async fn empty_queue(pool: &sqlx::PgPool) {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let n: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM telemetry_delivery WHERE payload IS NOT NULL",
            )
            .fetch_one(pool)
            .await
            .unwrap();
            if n == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn durable_delivery_replays_then_disabled_ingestion_blackholes() {
    let db = Database::new().await;
    sqlx::raw_sql(include_str!("sql/tracing_fixture.sql"))
        .execute(&db.pool)
        .await
        .unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(37)).await.unwrap());
    let tokens = Tokens::new(db.pool.clone(), crypto.clone());
    let token = tokens.issue(id(1), id(5), id(2), id(4)).await.unwrap();
    let (remote, url, remote_server) = remote().await;
    remote.down.store(true, Ordering::SeqCst);
    let runtime = Runtime::start(db.pool.clone(), tokens.clone(), destination(&url));
    let (local, server) = serve(tilde::iam::listeners::agent_runtime_router(
        tilde::agent::Agents::new(db.pool.clone(), crypto.clone()),
        tilde::chat::Chat::new(db.pool.clone(), crypto.clone(), "http://unused".into()),
        &runtime.tracing,
    ))
    .await;
    let client = reqwest::Client::new();
    let endpoint = format!("{local}/v1/traces");
    for _ in 0..2 {
        assert_eq!(
            client
                .post(&endpoint)
                .bearer_auth(token.expose_secret())
                .header("content-type", "application/x-protobuf")
                .body(payload(1))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry_delivery")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let data: Vec<u8> = sqlx::query_scalar("SELECT payload FROM telemetry_delivery")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let request = ExportTraceServiceRequest::decode(data.as_slice()).unwrap();
    let attrs = &request.resource_spans[0].scope_spans[0].spans[0].attributes;
    assert!(
        attrs
            .iter()
            .any(|a| a.key == "langfuse.observation.metadata.tilde_agent_id")
    );
    assert!(attrs.iter().any(|a| a.key == "langfuse.session.id"));
    server.abort();
    runtime.shutdown().await;
    remote.down.store(false, Ordering::SeqCst);
    let restarted = Runtime::start(db.pool.clone(), tokens.clone(), destination(&url));
    empty_queue(&db.pool).await;
    assert_eq!(remote.exports.lock().unwrap().len(), 1);
    restarted.queue.accept(&data).await.unwrap();
    empty_queue(&db.pool).await;
    assert_eq!(
        remote.exports.lock().unwrap().len(),
        1,
        "receipt prevents HA/restart replay"
    );
    restarted.shutdown().await;
    let disabled = Runtime::start(db.pool.clone(), tokens.clone(), None);
    let (local, server) = serve(tilde::iam::listeners::agent_runtime_router(
        tilde::agent::Agents::new(db.pool.clone(), crypto.clone()),
        tilde::chat::Chat::new(db.pool.clone(), crypto, "http://unused".into()),
        &disabled.tracing,
    ))
    .await;
    let endpoint = format!("{local}/v1/traces");
    assert_eq!(
        client
            .post(&endpoint)
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(payload(2))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .post(&endpoint)
            .header("content-type", "application/x-protobuf")
            .body(payload(2))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .post(&endpoint)
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body("invalid")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(remote.exports.lock().unwrap().len(), 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry_delivery")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    server.abort();
    disabled.shutdown().await;
    remote_server.abort();
    db.close().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn viewer_scopes_filters_coalesces_reads_and_builds_public_links() {
    let db = Database::new().await;
    sqlx::raw_sql(include_str!("sql/tracing_fixture.sql"))
        .execute(&db.pool)
        .await
        .unwrap();
    let (remote, url, server) = remote().await;
    let reader = Reader::new(db.pool.clone(), destination(&url));
    let agent = id(1).to_string();
    let (a, b) = tokio::join!(reader.status(&agent), reader.status(&agent));
    assert_eq!(a.unwrap().state, pb::TracingState::Ready);
    assert_eq!(b.unwrap().state, pb::TracingState::Ready);
    assert_eq!(remote.reads.load(Ordering::SeqCst), 1);
    let filter = pb::ObservationFilter {
        from_time: "2026-09-10T00:00:00Z".into(),
        to_time: "2026-09-12T00:00:00Z".into(),
        input_search: "hello \"world\"".into(),
        r#type: "GENERATION".into(),
        ..Default::default()
    };
    let page = reader
        .list(&id(1).to_string(), filter, "", None, false)
        .await
        .unwrap();
    assert_eq!(page.observations.len(), 1);
    assert!(page.partial);
    assert_eq!(page.next_cursor, "next-page");
    let row = &page.observations[0];
    assert_eq!(row.total_tokens, Some(12.));
    assert_eq!(row.cost_usd, Some(0.001));
    assert_eq!(
        row.trace_url,
        "https://viewer.example/prefix/project/project-fixture/traces/trace-1"
    );
    assert!(row.session_url.contains("session%20%2F%20one"));
    assert!(!row.trace_url.contains("sk-fixture"));
    assert!(
        remote.filters.lock().unwrap()[0]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["column"] == "input" && f["value"] == "hello \"world\"")
    );
    assert!(
        reader
            .list(
                &id(99).to_string(),
                pb::ObservationFilter::default(),
                "",
                None,
                false
            )
            .await
            .is_err()
    );
    let context = reader
        .list(
            &id(1).to_string(),
            pb::ObservationFilter::default(),
            "",
            Some("trace-1"),
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        context.observations.len(),
        2,
        "authorized trace includes its gateway parent"
    );
    assert!(
        reader
            .list(
                &id(1).to_string(),
                pb::ObservationFilter::default(),
                "",
                Some("unrelated-trace"),
                false
            )
            .await
            .is_err()
    );
    let disabled = Reader::new(db.pool.clone(), None);
    assert_eq!(
        disabled.status(&id(1).to_string()).await.unwrap().state,
        pb::TracingState::Disabled
    );
    let unavailable = Reader::new(db.pool.clone(), destination(&url));
    remote.down.store(true, Ordering::SeqCst);
    assert_eq!(
        unavailable.status(&id(1).to_string()).await.unwrap().state,
        pb::TracingState::Unavailable
    );
    server.abort();
    db.close().await;
}
