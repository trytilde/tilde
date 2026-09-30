//! Metrics across their boundaries: an authenticated OTLP upload is stamped with ownership,
//! queued in the metrics bucket, written to ClickHouse by Rotel and forwarded to a collector.
mod common;
use axum::{Router, routing::post};
use common::{Database, seed};
use envconfig::Envconfig;
use opentelemetry_proto::tonic::{
    collector::metrics::v1::ExportMetricsServiceRequest,
    common::v1::{AnyValue, KeyValue, any_value::Value},
    metrics::v1::{Gauge, Metric, NumberDataPoint, ResourceMetrics, ScopeMetrics, metric::Data},
};
use prost::Message;
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tilde::{
    config::Config, encryption::Encryption, iam::tokens::Tokens, telemetry::metrics::Runtime,
};
use uuid::Uuid;

fn id(n: u8) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{n:012}")).unwrap()
}
fn payload(agent_claim: Uuid) -> ExportMetricsServiceRequest {
    ExportMetricsServiceRequest {
        resource_metrics: vec![ResourceMetrics {
            scope_metrics: vec![ScopeMetrics {
                metrics: vec![Metric {
                    name: "agent.queue.depth".into(),
                    unit: "1".into(),
                    data: Some(Data::Gauge(Gauge {
                        data_points: vec![NumberDataPoint {
                            time_unix_nano: 1_790_000_000_000_000_000,
                            value: Some(
                                opentelemetry_proto::tonic::metrics::v1::number_data_point::Value::AsInt(7),
                            ),
                            attributes: vec![KeyValue {
                                // A forged owner is replaced by the authenticated one.
                                key: "tilde.agent.id".into(),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue(agent_claim.to_string())),
                                }),
                                ..Default::default()
                            }],
                            ..Default::default()
                        }],
                    })),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn metrics_are_owned_stored_and_forwarded() {
    let db = Database::new().await;
    db.pool
        .get()
        .await
        .unwrap()
        .batch_execute(include_str!("sql/tracing_fixture.sql"))
        .await
        .unwrap();
    let deployment_token = "deployment-metric-test-credential";
    db.pool.get().await.unwrap().execute("INSERT INTO agent_deployments(id,agent_id,source,target,token_hash) VALUES($1,$2,'manual','gateway',$3)", &[&(id(80)), &(id(1)), &(Sha256::digest(deployment_token.as_bytes()).to_vec())]).await.unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(63)).await.unwrap());
    let tokens = Tokens::new(db.pool.clone(), crypto);
    let received = Arc::new(Mutex::new(Vec::<ExportMetricsServiceRequest>::new()));
    let sink = received.clone();
    let collector = Router::new().route(
        "/v1/metrics",
        post(move |body: axum::body::Bytes| {
            sink.lock()
                .unwrap()
                .push(ExportMetricsServiceRequest::decode(body).unwrap());
            async { http::StatusCode::OK }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let collector_url = format!("http://{}/v1/metrics", listener.local_addr().unwrap());
    let collector = tokio::spawn(async move { axum::serve(listener, collector).await.unwrap() });
    let storage = common::Storage::load();
    let (mut config, database) = common::clickhouse(&storage).await;
    config.metrics_otlp_endpoint = Some(collector_url);
    let traces = tilde::telemetry::tracing::Runtime::start(
        db.pool.clone(),
        tokens,
        common::trace_delivery(&db.pool, &storage, &config),
    );
    let runtime = Runtime::start(db.pool.clone(), &config, common::bucket(&storage)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = runtime.router(&traces.tracing);
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let post = |token: Option<&str>| {
        let mut request = client
            .post(format!("{url}/v1/metrics"))
            .header("content-type", "application/x-protobuf")
            .body(payload(id(99)).encode_to_vec());
        if let Some(token) = token {
            request = request
                .bearer_auth(token)
                .header("x-tilde-log-scope", "deployment");
        }
        request.send()
    };
    assert_eq!(post(None).await.unwrap().status(), 401);
    assert_eq!(post(Some(deployment_token)).await.unwrap().status(), 200);

    // The collector receives the batch with ownership rewritten to the authenticated agent.
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        while received.lock().unwrap().is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("forwarded");
    let forwarded = received.lock().unwrap().remove(0);
    let Some(Data::Gauge(gauge)) = &forwarded.resource_metrics[0].scope_metrics[0].metrics[0].data
    else {
        panic!("gauge")
    };
    let attributes: HashMap<_, _> = gauge.data_points[0]
        .attributes
        .iter()
        .map(|a| {
            (
                a.key.clone(),
                match &a.value.as_ref().unwrap().value {
                    Some(Value::StringValue(s)) => s.clone(),
                    _ => String::new(),
                },
            )
        })
        .collect();
    assert_eq!(attributes["tilde.agent.id"], id(1).to_string());
    assert_eq!(attributes["tilde.deployment.id"], id(80).to_string());

    // ClickHouse holds the point in the gauge table under the owning agent.
    let query = |sql: String| {
        client
            .post(config.clickhouse_url.clone().unwrap())
            .query(&[("database", database.clone())])
            .basic_auth(
                config.clickhouse_user.clone(),
                config
                    .clickhouse_password
                    .as_ref()
                    .map(|p| p.0.expose_secret().to_owned()),
            )
            .body(sql)
            .send()
    };
    let stored = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let text = query(format!(
                "SELECT MetricName, Value FROM otel_metrics_gauge WHERE AgentId='{}' FORMAT TSV",
                id(1)
            ))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
            if text.contains("agent.queue.depth") {
                break text;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    })
    .await
    .expect("stored");
    assert_eq!(stored.trim(), "agent.queue.depth\t7");
    server.abort();
    collector.abort();
    runtime.shutdown().await;
    traces.shutdown().await;
    let _ = Config::init_from_hashmap(&HashMap::new());
    query(format!("DROP DATABASE {database}")).await.unwrap();
}
