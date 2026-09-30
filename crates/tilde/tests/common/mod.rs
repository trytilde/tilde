//! Shared fixtures. Each test binary compiles this module and uses a subset of it.
#![allow(dead_code)]
use envconfig::Envconfig;
use secrecy::ExposeSecret;
use std::str::FromStr;
use tilde::{
    config::SecretEnv,
    database::{self, Pool},
};
use uuid::Uuid;

/// The test infrastructure, typed like the application's own configuration: `task test`
/// starts Postgres, `task test:traces` and `task test:logs` add ClickHouse and MinIO. Tests
/// load this once and pass it to the helpers below; nothing reads the environment directly.
#[derive(Envconfig)]
pub struct Storage {
    #[envconfig(from = "TEST_DATABASE_URL")]
    pub database_url: Option<SecretEnv>,
    #[envconfig(from = "TEST_S3_ENDPOINT", default = "http://127.0.0.1:19000")]
    pub s3_endpoint: String,
    /// Provisioned by `minio-init` beside the development buckets; tests share it and keep
    /// apart by key prefix (every key starts with a fresh agent id).
    #[envconfig(from = "TEST_S3_BUCKET", default = "tilde-test")]
    pub s3_bucket: String,
    #[envconfig(from = "TEST_S3_ACCESS_KEY_ID", default = "minioadmin")]
    pub s3_access_key_id: SecretEnv,
    #[envconfig(from = "TEST_S3_SECRET_ACCESS_KEY", default = "minioadmin")]
    pub s3_secret_access_key: SecretEnv,
    #[envconfig(from = "TEST_CLICKHOUSE_URL", default = "http://127.0.0.1:18123")]
    pub clickhouse_url: String,
    #[envconfig(from = "TEST_CLICKHOUSE_USER", default = "tilde")]
    pub clickhouse_user: String,
    #[envconfig(from = "TEST_CLICKHOUSE_PASSWORD", default = "tilde-logs-dev")]
    pub clickhouse_password: SecretEnv,
}
impl Storage {
    pub fn load() -> Self {
        Self::init_from_env().expect("test storage configuration")
    }
}

pub struct Database {
    pub pool: Pool,
    admin: Pool,
    schema: String,
}
impl Database {
    pub async fn new() -> Self {
        let db = Self::unmigrated().await;
        database::migrate(&db.pool).await.unwrap();
        db
    }
    pub async fn unmigrated() -> Self {
        let url = Storage::load()
            .database_url
            .expect("TEST_DATABASE_URL is required; run task test for isolated Postgres");
        let url = url.0.expose_secret();
        let admin = database::pool(url, 2).unwrap();
        let schema = format!("test_{}", Uuid::new_v4().simple());
        admin
            .get()
            .await
            .unwrap()
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let mut config = tokio_postgres::Config::from_str(url).unwrap();
        config.options(format!("-c search_path={schema}"));
        let pool = database::pool_from_config(config, 4).unwrap();
        Self {
            pool,
            admin,
            schema,
        }
    }
    pub async fn close(self) {
        if !self.pool.is_closed() {
            tilde::chat::audit::flush(&self.pool).await.unwrap();
        }
        self.pool.close().await;
        // Dropping a schema locks every object in it; parallel tests dropping theirs at once
        // exhaust max_locks_per_transaction, so drops take turns. Workers a test spawned on
        // their own pools may still be querying the schema; dropping it can then lose a
        // deadlock, which is a teardown race, so the drop is retried.
        let admin = self.admin.get().await.unwrap();
        let statement = format!(
            "BEGIN; SELECT pg_advisory_xact_lock(7370616); DROP SCHEMA {} CASCADE; COMMIT;",
            self.schema
        );
        for attempt in 1.. {
            match admin.batch_execute(&statement).await {
                Ok(()) => break,
                Err(error)
                    if attempt < 5
                        && error.code()
                            == Some(&tokio_postgres::error::SqlState::T_R_DEADLOCK_DETECTED) =>
                {
                    let _ = admin.batch_execute("ROLLBACK").await;
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                Err(error) => panic!("dropping the test schema failed: {error}"),
            }
        }
        drop(admin);
        self.admin.close().await;
    }
}

pub fn seed(byte: u8) -> tilde::encryption::KeyProtection {
    use base64::Engine;
    tilde::encryption::KeyProtection::seed(secrecy::SecretString::from(
        base64::engine::general_purpose::STANDARD.encode([byte; 32]),
    ))
    .unwrap()
}

pub fn binding(id: Uuid, name: &str) -> tilde::encryption::SecretBinding<'_> {
    tilde::encryption::SecretBinding {
        resource_kind: "agent",
        resource_id: id,
        name,
    }
}

/// Register a Gateway deployment and record a ready instance for it, as the Watch and
/// heartbeat boundary would. Routing sees a live deployment; no stream exists, so a
/// wake for it has nowhere to go and fails.
pub async fn dialled_in(
    pool: &Pool,
    crypto: std::sync::Arc<tilde::encryption::Encryption>,
    agent: Uuid,
) -> Uuid {
    use tilde::proto::tilde::{agent_event_ingress::v1 as wire, types::v1 as types};
    let connections = tilde::connections::service::Connections::new(
        pool.clone(),
        crypto.clone(),
        "http://localhost:1".into(),
        "http://localhost:2".into(),
    )
    .unwrap();
    let deployments = tilde::deployment::Deployments::new(
        pool.clone(),
        crypto.clone(),
        tilde::agent::Agents::new(pool.clone(), crypto),
        connections,
    );
    let (deployment, _, _) = deployments
        .register_deployment(
            agent,
            tilde::deployment::RegisterDeployment {
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
                declarations: Default::default(),
            },
        )
        .await
        .unwrap();
    let deployment = Uuid::parse_str(&deployment.id).unwrap();
    let instance = Uuid::new_v4();
    deployments
        .register(
            agent,
            deployment,
            &wire::WatchRequest {
                instance_id: instance.to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    tilde::deployment::db::watch_open_execute(
        &pool.get().await.unwrap(),
        agent,
        instance,
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    deployments
        .heartbeat(
            agent,
            instance,
            &wire::Heartbeat {
                ready: true,
                agent_ready: true,
                agent_connected: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    deployment
}

/// Mount the actual agent-scoped Tilde provider ingress, including its credential guard.
pub async fn tilde_provider(
    pool: &Pool,
    encryption: std::sync::Arc<tilde::encryption::Encryption>,
    chat: tilde::chat::Chat,
    agent: Uuid,
) -> (String, secrecy::SecretString, tokio::task::JoinHandle<()>) {
    let connections = tilde::connections::service::Connections::new(
        pool.clone(),
        encryption.clone(),
        "http://127.0.0.1".into(),
        "http://127.0.0.1".into(),
    )
    .unwrap();
    let deployments = tilde::deployment::Deployments::new(
        pool.clone(),
        encryption.clone(),
        tilde::agent::Agents::new(pool.clone(), encryption),
        connections,
    );
    let token = deployments.issue_ingress_token(agent, None).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/agents/{agent}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            tilde::deployment::public::router(deployments, chat),
        )
        .await
        .unwrap();
    });
    (base, token, server)
}

/// These repository/control tests drive claims themselves, without a host Watch.
/// Pin the registered Gateway deployment explicitly instead of pretending that
/// an unconnected deployment is eligible for automatic traffic selection.
pub async fn pin_gateway(pool: &Pool, agent: Uuid, thread: Uuid) {
    let updated=pool.get().await.unwrap().execute(
        "UPDATE chat_participants SET deployment_id=(SELECT id FROM agent_deployments WHERE agent_id=$1 AND target='gateway' AND status='registered' ORDER BY created_at,id LIMIT 1) WHERE agent_id=$1 AND thread_id=$2",
        &[&agent,&thread],
    ).await.unwrap();
    assert_eq!(updated, 1);
}

/// The provisioned test bucket; the engine never creates buckets itself.
pub fn bucket(storage: &Storage) -> tilde::agent::avatar::ObjectStore {
    tilde::agent::avatar::ObjectStore::new(
        storage.s3_bucket.clone(),
        "us-east-1".into(),
        storage.s3_endpoint.clone(),
        None,
        Some(s3_credentials(storage)),
    )
    .unwrap()
}
fn s3_credentials(storage: &Storage) -> client_aws_sigv4::Credentials {
    client_aws_sigv4::Credentials {
        access_key_id: storage.s3_access_key_id.0.expose_secret().to_owned(),
        secret_access_key: storage.s3_secret_access_key.0.expose_secret().to_owned(),
        session_token: None,
    }
}
/// Every object key under `prefix` in a test bucket (one ListObjectsV2 page).
pub async fn object_keys(storage: &Storage, bucket: &str, prefix: &str) -> Vec<String> {
    let http = reqwest::Client::new();
    let mut request = http
        .get(format!(
            "{}/{bucket}",
            storage.s3_endpoint.trim_end_matches('/')
        ))
        .query(&[("list-type", "2"), ("prefix", prefix)])
        .build()
        .unwrap();
    client_aws_sigv4::Signer::new(s3_credentials(storage), "us-east-1", "s3")
        .sign_request_at(&mut request, &[], std::time::SystemTime::now())
        .unwrap();
    let response = http.execute(request).await.unwrap();
    assert!(response.status().is_success(), "list {bucket}");
    let body = response.text().await.unwrap();
    body.split("<Key>")
        .skip(1)
        .map(|rest| rest.split("</Key>").next().unwrap().to_owned())
        .collect()
}

/// A fresh database on the test ClickHouse, shared by the log, span and metric stores, as the
/// application configuration that selects it.
#[allow(dead_code)]
pub async fn clickhouse(storage: &Storage) -> (tilde::config::Config, String) {
    let database = format!("telemetry_test_{}", Uuid::new_v4().simple());
    let created = reqwest::Client::new()
        .post(&storage.clickhouse_url)
        .basic_auth(
            &storage.clickhouse_user,
            Some(storage.clickhouse_password.0.expose_secret()),
        )
        .body(format!("CREATE DATABASE {database}"))
        .send()
        .await
        .expect("test ClickHouse is reachable; run task test:traces or set TEST_CLICKHOUSE_URL");
    assert!(created.status().is_success());
    let config = tilde::config::Config::init_from_hashmap(&std::collections::HashMap::from([
        ("DATABASE_URL".to_owned(), "postgres://unused".to_owned()),
        (
            "ENGINE_CLICKHOUSE_URL".to_owned(),
            storage.clickhouse_url.clone(),
        ),
        ("ENGINE_CLICKHOUSE_DATABASE".to_owned(), database.clone()),
        (
            "ENGINE_CLICKHOUSE_USER".to_owned(),
            storage.clickhouse_user.clone(),
        ),
        (
            "ENGINE_CLICKHOUSE_PASSWORD".to_owned(),
            storage.clickhouse_password.0.expose_secret().to_owned(),
        ),
    ]))
    .unwrap();
    (config, database)
}
/// Trace delivery into the configured test ClickHouse, for tests that only need the tracing
/// runtime's credential checks in front of another signal.
#[allow(dead_code)]
pub fn trace_delivery(
    pool: &Pool,
    storage: &Storage,
    config: &tilde::config::Config,
) -> tilde::telemetry::tracing::Delivery {
    tilde::telemetry::tracing::Delivery::start(
        pool.clone(),
        tilde::telemetry::tracing::Sinks {
            objects: bucket(storage),
            media: None,
            store: tilde::telemetry::tracing::store::Store(
                tilde::telemetry::clickhouse::Store::from_config(config).unwrap(),
            ),
            external: None,
        },
    )
    .unwrap()
}
/// Spans ended from now on in this test binary, captured in memory: the tracer provider is global.
pub fn capture_spans() -> opentelemetry_sdk::trace::InMemorySpanExporter {
    let exporter = opentelemetry_sdk::trace::InMemorySpanExporter::default();
    opentelemetry::global::set_tracer_provider(
        opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build(),
    );
    exporter
}
/// The one finished span called `name`.
pub fn span(
    exporter: &opentelemetry_sdk::trace::InMemorySpanExporter,
    name: &str,
) -> opentelemetry_sdk::trace::SpanData {
    let mut spans: Vec<_> = exporter
        .get_finished_spans()
        .unwrap()
        .into_iter()
        .filter(|s| s.name == name)
        .collect();
    assert_eq!(spans.len(), 1, "one {name} span");
    spans.remove(0)
}
pub mod invocation;
