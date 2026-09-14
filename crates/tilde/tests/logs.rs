#[allow(dead_code)]
mod common;
use axum::{Router, body::Bytes, extract::State, routing::post};
use common::{Database, seed};
use envconfig::Envconfig;
use opentelemetry_proto::tonic::{
    collector::logs::v1::ExportLogsServiceRequest,
    common::v1::{AnyValue, KeyValue, any_value::Value},
    logs::v1::{LogRecord, ResourceLogs, ScopeLogs},
};
use prost::Message;
use secrecy::ExposeSecret;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tilde::{
    config::Config,
    encryption::Encryption,
    iam::tokens::Tokens,
    logs::{Runtime, spool::Spool},
    proto::tilde::management::v1 as pb,
};
use uuid::Uuid;
fn id(n: u8) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{n:012}")).unwrap()
}
fn config(path: &std::path::Path) -> Config {
    Config::init_from_hashmap(&HashMap::from([
        ("DATABASE_URL".into(), "postgres://unused".into()),
        ("LOGS_QUEUE_DIR".into(), path.to_string_lossy().into_owned()),
    ]))
    .unwrap()
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move { axum::serve(l, router).await.unwrap() }),
    )
}
fn payload(count: usize) -> ExportLogsServiceRequest {
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap() as u64;
    ExportLogsServiceRequest {
        resource_logs: vec![ResourceLogs {
            scope_logs: vec![ScopeLogs {
                log_records: (0..count)
                    .map(|i| LogRecord {
                        time_unix_nano: now + i as u64,
                        observed_time_unix_nano: now,
                        trace_id: vec![1; 16],
                        span_id: vec![3; 8],
                        severity_number: 17,
                        severity_text: "ERROR".into(),
                        body: Some(AnyValue {
                            value: Some(Value::StringValue(format!(
                                "test log {i} <script>not executable</script>"
                            ))),
                        }),
                        attributes: vec![KeyValue {
                            key: "tilde.agent.id".into(),
                            value: Some(AnyValue {
                                value: Some(Value::StringValue(id(99).to_string())),
                            }),
                            ..Default::default()
                        }],
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }],
    }
}
#[tokio::test]
async fn disk_queue_is_bounded_recovers_and_resets_destination() {
    let path = std::env::temp_dir().join(format!("tilde-logs-spool-{}", Uuid::new_v4()));
    let lock = tilde::logs::spool::lock(&path).unwrap();
    assert!(tilde::logs::spool::lock(&path).is_err());
    let queue = Spool::open(&path, "external", "http://collector-a", 16).unwrap();
    queue.accept(id(1).to_string(), vec![1; 8]).await.unwrap();
    queue.accept(id(1).to_string(), vec![1; 8]).await.unwrap();
    assert!(queue.accept(id(1).to_string(), vec![2; 9]).await.is_err());
    drop(queue);
    let queue = Spool::open(&path, "external", "http://collector-a", 16).unwrap();
    let (key, bytes) = queue.next().await.unwrap().unwrap();
    assert_eq!(bytes, vec![1; 8]);
    queue.complete(key).await.unwrap();
    assert!(queue.next().await.unwrap().is_none());
    queue.accept(id(1).to_string(), vec![4; 8]).await.unwrap();
    drop(queue);
    let queue = Spool::open(&path, "external", "http://collector-b", 16).unwrap();
    assert!(queue.next().await.unwrap().is_none());
    drop(queue);
    drop(lock);
    std::fs::remove_dir_all(path).unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn disabled_logs_validate_and_authenticate_without_delivery() {
    let db = Database::new().await;
    sqlx::raw_sql(include_str!("sql/tracing_fixture.sql"))
        .execute(&db.pool)
        .await
        .unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(61)).await.unwrap());
    let tokens = Tokens::new(db.pool.clone(), crypto);
    let token = tokens.issue(id(1), id(5), id(2), id(4)).await.unwrap();
    let traces = tilde::telemetry::Runtime::start(db.pool.clone(), tokens, None);
    let path = std::env::temp_dir().join(format!("tilde-logs-disabled-{}", Uuid::new_v4()));
    let runtime = Runtime::start(db.pool.clone(), &config(&path)).unwrap();
    let (url, server) = serve(runtime.router(&traces.tracing)).await;
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .post(format!("{url}/v1/logs"))
            .body(payload(1).encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    for (body, status) in [(payload(1).encode_to_vec(), 200), (vec![255], 400)] {
        assert_eq!(
            client
                .post(format!("{url}/v1/logs"))
                .bearer_auth(token.expose_secret())
                .header("content-type", "application/x-protobuf")
                .body(body)
                .send()
                .await
                .unwrap()
                .status(),
            status
        );
    }
    let mut invalid = payload(1);
    invalid.resource_logs[0].scope_logs[0].log_records[0].trace_id = vec![9; 16];
    assert_eq!(
        client
            .post(format!("{url}/v1/logs"))
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(invalid.encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 1);
    server.abort();
    runtime.shutdown().await;
    traces.shutdown().await;
    std::fs::remove_dir_all(path).unwrap();
}
#[derive(Clone, Default)]
struct Remote {
    down: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<ExportLogsServiceRequest>>>,
}
async fn remote(State(state): State<Remote>, body: Bytes) -> http::StatusCode {
    if state.down.load(Ordering::SeqCst) {
        return http::StatusCode::SERVICE_UNAVAILABLE;
    }
    state
        .requests
        .lock()
        .unwrap()
        .push(ExportLogsServiceRequest::decode(body).unwrap());
    http::StatusCode::OK
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires LOGS_TEST_CLICKHOUSE_URL; task test:logs runs an isolated ClickHouse database"]
async fn clickhouse_ingress_pagination_isolation_and_independent_replay() {
    let db = Database::new().await;
    sqlx::raw_sql(include_str!("sql/tracing_fixture.sql"))
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agents(id,name,endpoint_url,webhook_signing_key) SELECT $1,'Other','http://unused',webhook_signing_key FROM agents LIMIT 1").bind(id(99)).execute(&db.pool).await.unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(62)).await.unwrap());
    let tokens = Tokens::new(db.pool.clone(), crypto);
    let token = tokens.issue(id(1), id(5), id(2), id(4)).await.unwrap();
    let traces = tilde::telemetry::Runtime::start(db.pool.clone(), tokens, None);
    let path = std::env::temp_dir().join(format!("tilde-logs-live-{}", Uuid::new_v4()));
    let mut config = config(&path);
    config.logs_clickhouse_url = Some(std::env::var("LOGS_TEST_CLICKHOUSE_URL").unwrap());
    config.logs_clickhouse_user =
        std::env::var("LOGS_TEST_CLICKHOUSE_USER").unwrap_or("tilde".into());
    config.logs_clickhouse_password = Some(
        std::env::var("LOGS_TEST_CLICKHOUSE_PASSWORD")
            .unwrap_or("tilde-logs-dev".into())
            .parse()
            .unwrap(),
    );
    config.logs_clickhouse_database = format!("logs_test_{}", Uuid::new_v4().simple());
    let client = reqwest::Client::new();
    let admin = |sql: String| {
        client
            .post(config.logs_clickhouse_url.clone().unwrap())
            .basic_auth(
                config.logs_clickhouse_user.clone(),
                config
                    .logs_clickhouse_password
                    .as_ref()
                    .map(|p| p.0.expose_secret().to_owned()),
            )
            .body(sql)
    };
    assert!(
        admin(format!(
            "CREATE DATABASE {}",
            config.logs_clickhouse_database
        ))
        .send()
        .await
        .unwrap()
        .status()
        .is_success()
    );
    let state = Remote::default();
    state.down.store(true, Ordering::SeqCst);
    let (external, remote_server) = serve(
        Router::new()
            .route("/v1/logs", post(remote))
            .with_state(state.clone()),
    )
    .await;
    config.logs_otlp_endpoint = Some(format!("{external}/v1/logs"));
    let runtime = Runtime::start(db.pool.clone(), &config).unwrap();
    let (url, server) = serve(runtime.router(&traces.tracing)).await;
    let data = payload(101).encode_to_vec();
    for _ in 0..2 {
        assert_eq!(
            client
                .post(format!("{url}/v1/logs"))
                .bearer_auth(token.expose_secret())
                .header("content-type", "application/x-protobuf")
                .body(data.clone())
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    assert_eq!(
        client
            .post(format!("{url}/v1/logs"))
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(Vec::<u8>::new())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let json_data: ExportLogsServiceRequest =
        ExportLogsServiceRequest::decode(data.as_slice()).unwrap();
    assert_eq!(
        client
            .post(format!("{url}/v1/logs"))
            .bearer_auth(token.expose_secret())
            .json(&json_data)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let request = pb::ListLogsRequest {
        agent_id: id(1).to_string(),
        ..Default::default()
    };
    let page = tokio::time::timeout(Duration::from_secs(35), async {
        loop {
            if let Ok(page) = runtime.reader.list(request.clone()).await
                && page.records.len() == 100
            {
                break page;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("ClickHouse delivery");
    assert!(page.partial);
    assert!(!page.next_cursor.is_empty());
    assert_eq!(page.records[0].invocation_id, id(5).to_string());
    assert!(
        page.records[0]
            .attributes
            .iter()
            .any(|a| a.key == "tilde.agent.id" && a.value == id(1).to_string())
    );
    let next = runtime
        .reader
        .list(pb::ListLogsRequest {
            cursor: page.next_cursor.clone(),
            ..request.clone()
        })
        .await
        .unwrap();
    assert_eq!(next.records.len(), 1);
    assert!(!next.partial);
    let other = runtime
        .reader
        .list(pb::ListLogsRequest {
            agent_id: id(99).to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(other.records.is_empty());
    assert!(
        runtime
            .reader
            .list(pb::ListLogsRequest {
                agent_id: id(99).to_string(),
                cursor: page.next_cursor,
                ..Default::default()
            })
            .await
            .is_err()
    );
    assert!(state.requests.lock().unwrap().is_empty());
    server.abort();
    runtime.shutdown().await;
    state.down.store(false, Ordering::SeqCst);
    let restarted = Runtime::start(db.pool.clone(), &config).unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        while state.requests.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("External replay after restart");
    assert_eq!(
        state.requests.lock().unwrap()[0].resource_logs[0].scope_logs[0]
            .log_records
            .len(),
        101
    );
    let (url, server) = serve(restarted.router(&traces.tracing)).await;
    sqlx::query("UPDATE chat_invocations SET status='stopped',ended_at=NOW()-INTERVAL '2 minutes' WHERE id=$1").bind(id(5)).execute(&db.pool).await.unwrap();
    assert_eq!(
        client
            .post(format!("{url}/v1/logs"))
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(payload(1).encode_to_vec())
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
        client
            .post(format!("{url}/v1/logs"))
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/x-protobuf")
            .body(payload(1).encode_to_vec())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.abort();
    remote_server.abort();
    restarted.shutdown().await;
    traces.shutdown().await;
    assert!(
        admin(format!("DROP DATABASE {}", config.logs_clickhouse_database))
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    std::fs::remove_dir_all(path).unwrap();
}
