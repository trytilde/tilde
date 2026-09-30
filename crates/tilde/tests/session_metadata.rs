mod common;
use axum::{Router, routing::post};
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};
use prost::Message;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tilde::telemetry::tracing::{
    Delivery, Destination, Sinks, mapping, store::Store, viewer::Reader,
};
use uuid::Uuid;
fn id(n: u8) -> Uuid {
    Uuid::parse_str(&format!("00000000-0000-4000-8000-{n:012}")).unwrap()
}
fn attr(key: &str, value: &str) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: Some(AnyValue {
            value: Some(Value::StringValue(value.into())),
        }),
        ..Default::default()
    }
}
fn payload(span: u8, session: u8) -> Vec<u8> {
    let mut request = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: Some(opentelemetry_proto::tonic::resource::v1::Resource {
                attributes: vec![attr("tilde.session.message_count", "99999")],
                ..Default::default()
            }),
            scope_spans: vec![ScopeSpans {
                spans: vec![Span {
                    trace_id: vec![1; 16],
                    span_id: vec![span; 8],
                    name: "ai.generateText.doGenerate".into(),
                    start_time_unix_nano: 1_800_000_000_000_000_000,
                    end_time_unix_nano: 1_800_000_001_000_000_000,
                    attributes: vec![
                        attr("tilde.agent.id", &id(1).to_string()),
                        attr("tilde.thread.id", &id(session).to_string()),
                        attr("tilde.session.message_count", "99999"),
                        attr("tilde.session.metadata_time", "2099-01-01T00:00:00Z"),
                        attr("tilde.session.provider_name", "Forged provider"),
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
    };
    mapping::normalize(&mut request.resource_spans);
    request.encode_to_vec()
}
fn json_value(value: &AnyValue) -> serde_json::Value {
    match value.value.as_ref() {
        Some(Value::StringValue(v)) => json!(v),
        Some(Value::IntValue(v)) => json!(v),
        Some(Value::ArrayValue(v)) => json!(v.values.iter().map(json_value).collect::<Vec<_>>()),
        _ => serde_json::Value::Null,
    }
}
/// The session facts stamped on a delivered span.
fn session(data: &[u8]) -> serde_json::Value {
    let request = ExportTraceServiceRequest::decode(data).unwrap();
    assert!(
        request.resource_spans[0]
            .resource
            .as_ref()
            .unwrap()
            .attributes
            .is_empty(),
        "resource-level session metadata is not trusted"
    );
    let span = &request.resource_spans[0].scope_spans[0].spans[0];
    serde_json::Value::Object(
        span.attributes
            .iter()
            .filter_map(|a| {
                Some((
                    a.key.strip_prefix("tilde.session.")?.to_owned(),
                    json_value(a.value.as_ref()?),
                ))
            })
            .collect(),
    )
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn session_metadata_flows_through_stored_traces_and_only_ids_are_resolved_on_reads() {
    let pg = common::Database::new().await;
    pg.pool
        .get()
        .await
        .unwrap()
        .batch_execute(include_str!(
            "../../../queries/_tests/session_details_fixture.sql"
        ))
        .await
        .unwrap();
    let removed = pg
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            include_str!("../../../queries/_tests/session_metadata_no_projection.sql"),
            &[],
        )
        .await
        .unwrap();
    for col in ["no_queue", "no_sequence", "no_triggers"] {
        assert!(removed.get::<_, bool>(col));
    }
    // An external collector shows exactly what leaves the gateway; ClickHouse serves the reads.
    let received = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let sink = received.clone();
    let collector = Router::new().route(
        "/v1/traces",
        post(move |body: axum::body::Bytes| {
            sink.lock().unwrap().push(body.to_vec());
            async {}
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let collector_url = format!("http://{}/v1/traces", listener.local_addr().unwrap());
    let collector = tokio::spawn(async move { axum::serve(listener, collector).await.unwrap() });
    let storage = common::Storage::load();
    let (config, database) = common::clickhouse(&storage).await;
    let store = Store(tilde::telemetry::clickhouse::Store::from_config(&config).unwrap());
    let delivery = Delivery::start(
        pg.pool.clone(),
        Sinks {
            objects: common::bucket(&storage),
            media: Some(common::bucket(&storage)),
            store: store.clone(),
            external: Destination::new(Some(collector_url), None).unwrap(),
        },
    )
    .unwrap();
    let deliver = |bytes: Vec<u8>| {
        let (queue, received) = (delivery.queue.clone(), received.clone());
        async move {
            queue.accept(&bytes).await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(30), queue.wait_empty())
                .await
                .expect("delivered")
                .unwrap();
            let last = received.lock().unwrap().last().cloned().unwrap();
            session(&last)
        }
    };

    let old = deliver(payload(1, 11)).await;
    assert_eq!(old["message_count"], 3);
    assert_eq!(
        old["provider_name"], "WhatsApp",
        "forged facts are replaced"
    );
    assert_eq!(old["identity_ids"], json!([id(21), id(22)]));
    assert_eq!(old["connection_id"], id(31).to_string());
    let text = old.to_string();
    assert!(!text.contains("+447700") && !text.contains("Alice") && !text.contains("Hello"));
    pg.pool
        .get()
        .await
        .unwrap()
        .execute(
            include_str!("../../../queries/_tests/session_metadata_delete_message.sql"),
            &[&id(51)],
        )
        .await
        .unwrap();
    let latest = deliver(payload(2, 11)).await;
    assert_eq!(latest["message_count"], 2);
    assert_ne!(latest["metadata_time"], old["metadata_time"]);
    let foreign = deliver(payload(3, 13)).await;
    assert!(
        foreign.get("metadata_time").is_none(),
        "a forged session snapshot cannot cross agent ownership"
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/tilde.management.v1.TracingService/ListObservations",
        listener.local_addr().unwrap()
    );
    let reader = Reader::new(pg.pool.clone(), store, None);
    let router = tilde::telemetry::tracing::viewer::router(reader);
    let api = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let request = json!({"agentId":id(1),"includeSessionDetails":true,
        "filter":{"fromTime":"2027-01-15T00:00:00Z","toTime":"2027-01-16T00:00:00Z"}});
    let response = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let response = reqwest::Client::new()
                .post(&url)
                .json(&request)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), http::StatusCode::OK);
            let response: serde_json::Value = response.json().await.unwrap();
            if response["observations"]
                .as_array()
                .is_some_and(|rows| rows.len() == 3)
            {
                break response;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    })
    .await
    .expect("stored spans become readable");
    let sessions = response["sessions"].as_array().unwrap();
    let session = sessions
        .iter()
        .find(|s| s["id"] == id(11).to_string())
        .unwrap();
    assert_eq!(
        session["messageCount"], "2",
        "latest captured count wins even when lower, not summed/maxed over spans"
    );
    assert_eq!(
        session["identities"],
        json!(["+447700900111", "+447700900222"])
    );
    assert_eq!(session["lastTurnTime"], "2026-09-16T10:00:05+00:00");
    let unknown = sessions
        .iter()
        .find(|s| s["id"] == id(13).to_string())
        .unwrap();
    assert!(unknown.get("messageCount").is_none());
    assert!(
        unknown["identities"]
            .as_array()
            .is_none_or(|ids| ids.is_empty())
    );
    api.abort();
    collector.abort();
    reqwest::Client::new()
        .post(config.clickhouse_url.clone().unwrap())
        .basic_auth(
            &config.clickhouse_user,
            config
                .clickhouse_password
                .as_ref()
                .map(|p| secrecy::ExposeSecret::expose_secret(&p.0)),
        )
        .body(format!("DROP DATABASE {database}"))
        .send()
        .await
        .unwrap();
    pg.close().await;
}
