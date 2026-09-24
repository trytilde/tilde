mod common;
use axum::{Json, Router, routing::get};
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};
use prost::Message;
use secrecy::SecretString;
use serde_json::json;
use tilde::{
    config::SecretEnv,
    telemetry::{Destination, db, delivery::Queue, mapping, viewer::Reader},
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
                attributes: vec![attr(
                    "langfuse.observation.metadata.tilde_session_message_count",
                    "99999",
                )],
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
                        attr(
                            "langfuse.observation.metadata.tilde_session_message_count",
                            "99999",
                        ),
                        attr(
                            "langfuse.observation.metadata.tilde_session_metadata_time",
                            "2099-01-01T00:00:00Z",
                        ),
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
fn observation(data: &[u8]) -> serde_json::Value {
    let request = ExportTraceServiceRequest::decode(data).unwrap();
    let span = &request.resource_spans[0].scope_spans[0].spans[0];
    let metadata: serde_json::Map<_, _> = span
        .attributes
        .iter()
        .filter_map(|a| {
            Some((
                a.key
                    .strip_prefix("langfuse.observation.metadata.")?
                    .to_owned(),
                json_value(a.value.as_ref()?),
            ))
        })
        .collect();
    let session = span
        .attributes
        .iter()
        .find(|a| a.key == "langfuse.session.id")
        .unwrap()
        .value
        .as_ref()
        .map(json_value)
        .unwrap();
    json!({"id":hex::encode(&span.span_id),"traceId":hex::encode(&span.trace_id),"sessionId":session,"name":"model","type":"GENERATION","startTime":"2026-09-16T10:00:20Z","metadata":metadata})
}
async fn pending(pg: &common::Database) -> db::DeliveryPendingRow {
    db::delivery_pending_opt(&pg.pool.get().await.unwrap())
        .await
        .unwrap()
        .unwrap()
}
#[tokio::test]
async fn session_metadata_flows_through_existing_traces_and_only_ids_are_resolved_on_reads() {
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
    let queue = Queue::new(pg.pool.clone(), true);
    let first_bytes = payload(1, 11);
    queue.accept(&first_bytes).await.unwrap();
    let first = pending(&pg).await;
    let decoded = ExportTraceServiceRequest::decode(first.payload.as_slice()).unwrap();
    assert!(
        decoded.resource_spans[0]
            .resource
            .as_ref()
            .unwrap()
            .attributes
            .is_empty(),
        "resource-level session metadata is not trusted"
    );
    let old = observation(&first.payload);
    assert_eq!(old["metadata"]["tilde_session_message_count"], 3);
    assert_eq!(old["metadata"]["tilde_session_provider_name"], "WhatsApp");
    assert_eq!(
        old["metadata"]["tilde_session_identity_ids"],
        json!([id(21), id(22)])
    );
    assert_eq!(
        old["metadata"]["tilde_session_connection_id"],
        id(31).to_string()
    );
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
    queue.accept(&first_bytes).await.unwrap();
    queue.accept(&first.payload).await.unwrap();
    assert_eq!(
        pending(&pg).await.payload,
        first.payload,
        "retries keep the originally captured metadata"
    );
    db::delivery_complete_execute(&pg.pool.get().await.unwrap(), &first.id)
        .await
        .unwrap();
    assert!(
        db::delivery_pending_opt(&pg.pool.get().await.unwrap())
            .await
            .unwrap()
            .is_none()
    );
    queue.accept(&payload(2, 11)).await.unwrap();
    let second = pending(&pg).await;
    let latest = observation(&second.payload);
    assert_eq!(latest["metadata"]["tilde_session_message_count"], 2);
    assert_ne!(
        latest["metadata"]["tilde_session_metadata_time"],
        old["metadata"]["tilde_session_metadata_time"]
    );
    db::delivery_complete_execute(&pg.pool.get().await.unwrap(), &second.id)
        .await
        .unwrap();
    queue.accept(&payload(3, 13)).await.unwrap();
    let foreign = observation(&pending(&pg).await.payload);
    assert!(
        foreign["metadata"]
            .get("tilde_session_metadata_time")
            .is_none(),
        "a forged session snapshot cannot cross agent ownership"
    );
    let rows = json!({"data":[latest,old,foreign,{"id":"legacy","traceId":"legacy","sessionId":id(12),"metadata":{"tilde_agent_id":id(1)}}],"meta":{}});
    let upstream = Router::new()
        .route(
            "/api/public/projects",
            get(|| async { Json(json!({"data":[{"id":"project"}]})) }),
        )
        .route(
            "/api/public/v2/observations",
            get(move || {
                let rows = rows.clone();
                async move { Json(rows) }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_url = format!("http://{}", listener.local_addr().unwrap());
    let upstream = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let reader = Reader::new(
        pg.pool.clone(),
        Destination::langfuse(
            Some(upstream_url),
            None,
            Some(SecretEnv(SecretString::from("public"))),
            Some(SecretEnv(SecretString::from("secret"))),
        )
        .unwrap(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/tilde.management.v1.TracingService/ListObservations",
        listener.local_addr().unwrap()
    );
    let api_pool = pg.pool.clone();
    let api = tokio::spawn(async move {
        axum::serve(
            listener,
            common::unguarded(tilde::telemetry::viewer::router(reader), &api_pool),
        )
        .await
        .unwrap()
    });
    let response = reqwest::Client::new()
        .post(url)
        .json(&json!({"agentId":id(1),"includeSessionDetails":true}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), http::StatusCode::OK);
    let response: serde_json::Value = response.json().await.unwrap();
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
    for unknown in [12, 13] {
        let session = sessions
            .iter()
            .find(|s| s["id"] == id(unknown).to_string())
            .unwrap();
        assert!(session.get("messageCount").is_none());
        assert!(
            session["identities"]
                .as_array()
                .is_none_or(|ids| ids.is_empty())
        );
    }
    api.abort();
    upstream.abort();
    pg.close().await;
}
