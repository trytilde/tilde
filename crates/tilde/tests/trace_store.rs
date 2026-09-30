//! The span store across its boundaries: an accepted OTLP batch goes to the trace bucket and a
//! Postgres pointer, a worker writes it to ClickHouse through Rotel, and the viewer reads it
//! back under the agent scope. Needs the test ClickHouse and object store (`task test:traces`).
mod common;
use common::Database;
use envconfig::Envconfig;
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{AnyValue, KeyValue, any_value::Value as Any},
    trace::v1::{ResourceSpans, ScopeSpans, Span, Status},
};
use prost::Message;
use tilde::{
    proto::tilde::management::v1 as pb,
    telemetry::{
        spool::Spool,
        tracing::{Delivery, Sinks, store::Store, viewer::Reader},
    },
};
use uuid::Uuid;

const BASE: u64 = 1_790_000_000_000_000_000;
fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}
fn attribute(key: &str, value: &str) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: Some(AnyValue {
            value: Some(Any::StringValue(value.into())),
        }),
        ..Default::default()
    }
}
struct Draft<'a> {
    trace: u8,
    span: u8,
    parent: Option<u8>,
    name: &'a str,
    agent: Option<Uuid>,
    offset_ms: u64,
    duration_ms: u64,
    error: bool,
    attributes: Vec<(&'a str, &'a str)>,
}
fn span(draft: Draft) -> Span {
    let mut attributes: Vec<_> = draft
        .attributes
        .iter()
        .map(|(k, v)| attribute(k, v))
        .collect();
    if let Some(agent) = draft.agent {
        attributes.push(attribute("tilde.agent.id", &agent.to_string()));
        attributes.push(attribute("tilde.thread.id", &id(40).to_string()));
        attributes.push(attribute("tilde.invocation.id", &id(50).to_string()));
    }
    let start = BASE + draft.offset_ms * 1_000_000;
    Span {
        trace_id: vec![draft.trace; 16],
        span_id: vec![draft.span; 8],
        parent_span_id: draft.parent.map(|p| vec![p; 8]).unwrap_or_default(),
        name: draft.name.into(),
        start_time_unix_nano: start,
        end_time_unix_nano: start + draft.duration_ms * 1_000_000,
        status: draft.error.then(|| Status {
            code: 2,
            message: "tool failed".into(),
        }),
        attributes,
        ..Default::default()
    }
}
fn batch(spans: Vec<Span>) -> Vec<u8> {
    let mut resources = vec![ResourceSpans {
        scope_spans: vec![ScopeSpans {
            spans,
            ..Default::default()
        }],
        ..Default::default()
    }];
    // The gateway projects observation facts before anything is queued.
    tilde::telemetry::tracing::mapping::normalize(&mut resources);
    ExportTraceServiceRequest {
        resource_spans: resources,
    }
    .encode_to_vec()
}
fn window() -> pb::ObservationFilter {
    pb::ObservationFilter {
        from_time: "2026-09-21T00:00:00Z".into(),
        to_time: "2026-09-22T00:00:00Z".into(),
        ..Default::default()
    }
}
fn condition(column: &str, operator: &str, values: &[&str]) -> pb::FilterCondition {
    pb::FilterCondition {
        column: column.into(),
        operator: operator.into(),
        values: values.iter().map(|v| v.to_string()).collect(),
        ..Default::default()
    }
}
fn meta(key: &str, operator: &str, value: &str) -> pb::FilterCondition {
    pb::FilterCondition {
        key: key.into(),
        ..condition("metadata", operator, &[value])
    }
}
fn only(conditions: Vec<pb::FilterCondition>) -> pb::ObservationFilter {
    pb::ObservationFilter {
        conditions,
        ..window()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn spans_are_stored_scoped_searchable_and_paged() {
    let db = Database::new().await;
    let encryption = std::sync::Arc::new(
        tilde::encryption::Encryption::initialize(&db.pool, common::seed(81))
            .await
            .unwrap(),
    );
    let agents = tilde::agent::Agents::new(db.pool.clone(), encryption);
    for (agent, name) in [(id(1), "alpha"), (id(2), "beta")] {
        agents
            .create(tilde::agent::CreateAgent {
                description: String::new(),
                concurrency_policy: Default::default(),
                id: agent,
                name: name.into(),
                capabilities: Default::default(),
            })
            .await
            .unwrap();
    }
    // Priced from the same table as inference requests: $1/M input, $2/M output.
    db.pool.get().await.unwrap().execute(
        "INSERT INTO inference_prices(provider_id,model,input_per_m_micros,output_per_m_micros,source,updated_at) VALUES('openai','gpt-test',1000000,2000000,'manual',NOW())",
        &[],
    ).await.unwrap();
    let storage = common::Storage::load();
    let (config, database) = common::clickhouse(&storage).await;
    let store = Store(tilde::telemetry::clickhouse::Store::from_config(&config).unwrap());
    // Retention is applied on every start: a bounded window adds a TTL, indefinite retention
    // removes one that exists and leaves a table without one alone.
    let ttl = |days: Option<u32>| {
        let mut config =
            tilde::config::Config::init_from_hashmap(&std::collections::HashMap::from([
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
                    secrecy::ExposeSecret::expose_secret(&storage.clickhouse_password.0).to_owned(),
                ),
            ]))
            .unwrap();
        config.clickhouse_retention_days = days;
        let store = tilde::telemetry::clickhouse::Store::from_config(&config).unwrap();
        async move {
            store.initialize().await.unwrap();
            store
                .query(
                    include_str!("../../../queries/_logs/ttl.sql"),
                    &[("param_table".into(), "otel_logs".into())],
                )
                .await
                .unwrap()["data"][0]["has_ttl"]
                .as_u64()
                == Some(1)
        }
    };
    assert!(!ttl(None).await, "a new table has no TTL to remove");
    assert!(ttl(Some(7)).await);
    assert!(!ttl(None).await);
    let bucket = common::bucket(&storage);
    let delivery = Delivery::start(
        db.pool.clone(),
        Sinks {
            objects: bucket.clone(),
            media: Some(bucket.clone()),
            store: store.clone(),
            external: None,
        },
    )
    .unwrap();

    let mut spans = vec![
        // A gateway parent carries no agent; it is reachable only through its trace.
        span(Draft {
            trace: 1,
            span: 1,
            parent: None,
            name: "gateway.dispatch",
            agent: None,
            offset_ms: 0,
            duration_ms: 900,
            error: false,
            attributes: vec![],
        }),
        span(Draft {
            trace: 1,
            span: 2,
            parent: Some(1),
            name: "ai.generateText.doGenerate",
            agent: Some(id(1)),
            offset_ms: 10,
            duration_ms: 500,
            error: false,
            attributes: vec![
                ("ai.model.id", "gpt-test"),
                (
                    "ai.prompt.messages",
                    "[{\"role\":\"user\",\"content\":\"find the quarterly report\"}]",
                ),
                ("ai.response.text", "Here is the report"),
                ("ai.usage.promptTokens", "12"),
                ("ai.usage.completionTokens", "8"),
            ],
        }),
        span(Draft {
            trace: 1,
            span: 3,
            parent: Some(2),
            name: "ai.toolCall",
            agent: Some(id(1)),
            offset_ms: 20,
            duration_ms: 100,
            error: true,
            attributes: vec![
                ("ai.toolCall.name", "search_documents"),
                ("ai.toolCall.args", "{\"q\":\"report\"}"),
            ],
        }),
        span(Draft {
            trace: 2,
            span: 9,
            parent: None,
            name: "ai.generateText.doGenerate",
            agent: Some(id(2)),
            offset_ms: 30,
            duration_ms: 50,
            error: false,
            attributes: vec![
                ("ai.model.id", "gpt-test"),
                ("ai.response.text", "another agent's secret"),
            ],
        }),
    ];
    // A generation with an inline image and an output too large to keep on the span.
    let image = format!(
        "data:image/png;base64,{}",
        base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            [
                137u8, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1
            ]
        )
    );
    let big = "x".repeat(70 * 1024);
    let media_input = format!(
        "[{{\"role\":\"user\",\"content\":[{{\"type\":\"image\",\"image\":\"{image}\"}}]}}]"
    );
    spans.push(span(Draft {
        trace: 4,
        span: 40,
        parent: None,
        name: "ai.generateText.doGenerate",
        agent: Some(id(1)),
        offset_ms: 5000,
        duration_ms: 10,
        error: false,
        attributes: vec![
            ("ai.model.id", "vision-test"),
            ("ai.prompt.messages", media_input.as_str()),
            ("ai.response.text", big.as_str()),
        ],
    }));
    // Enough rows for a second page.
    for n in 0..60u8 {
        spans.push(span(Draft {
            trace: 3,
            span: 100 + n,
            parent: None,
            name: "bulk.step",
            agent: Some(id(1)),
            offset_ms: 1000 + n as u64,
            duration_ms: 1,
            error: false,
            attributes: vec![],
        }));
    }
    delivery.queue.accept(&batch(spans)).await.unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        delivery.queue.wait_empty(),
    )
    .await
    .expect("the pointer goes once ClickHouse accepts the batch")
    .unwrap();

    let reader = Reader::new(db.pool.clone(), store, Some(bucket.clone()));
    let agent = id(1).to_string();
    let listed = |filter: pb::ObservationFilter| {
        let (reader, agent) = (reader.clone(), agent.clone());
        async move {
            reader
                .list(&agent, filter, "", None)
                .await
                .unwrap()
                .observations
        }
    };
    // Asynchronous inserts land shortly after they are acknowledged.
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while listed(window()).await.len() < 50
            || listed(only(vec![condition("model", "=", &["vision-test"])]))
                .await
                .is_empty()
        {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    })
    .await
    .expect("spans become readable");

    // Newest first, fifty to a page, and the cursor carries on exactly where the page ended.
    let first = reader.list(&agent, window(), "", None).await.unwrap();
    assert!(first.partial && first.observations.len() == 50);
    assert!(
        first
            .observations
            .windows(2)
            .all(|w| w[0].start_time >= w[1].start_time)
    );
    let second = reader
        .list(&agent, window(), &first.next_cursor, None)
        .await
        .unwrap();
    assert_eq!(first.observations.len() + second.observations.len(), 63);
    assert!(!second.partial);
    // A cursor belongs to the filters and the date range it was issued for; an omitted bound
    // takes the frozen one.
    let other = only(vec![condition("name", "contains", &["bulk"])]);
    assert!(
        reader
            .list(&agent, other, &first.next_cursor, None)
            .await
            .is_err()
    );
    let shifted = pb::ObservationFilter {
        from_time: "2026-09-20T00:00:00Z".into(),
        ..window()
    };
    assert!(
        reader
            .list(&agent, shifted, &first.next_cursor, None)
            .await
            .is_err()
    );
    let frozen = pb::ObservationFilter {
        to_time: String::new(),
        ..window()
    };
    assert_eq!(
        reader
            .list(&agent, frozen, &first.next_cursor, None)
            .await
            .unwrap()
            .observations
            .len(),
        second.observations.len()
    );

    // Observation facts are projected from the span, and no other agent's spans are reachable.
    let generation = listed(only(vec![condition("model", "=", &["gpt-test"])])).await;
    assert_eq!(generation.len(), 1);
    let row = &generation[0];
    assert_eq!(
        (row.r#type.as_str(), row.model.as_str(), row.level.as_str()),
        ("GENERATION", "gpt-test", "DEFAULT")
    );
    assert_eq!(
        (row.input_tokens, row.output_tokens, row.total_tokens),
        (Some(12.), Some(8.), Some(20.))
    );
    assert_eq!(row.latency_seconds, Some(0.5));
    assert_eq!(
        row.cost_usd,
        Some(0.000028),
        "12 input at $1/M plus 8 output at $2/M"
    );
    assert!(row.input.contains("quarterly report") && row.output == "Here is the report");
    assert_eq!(
        (row.session_id.clone(), row.invocation_id.clone()),
        (id(40).to_string(), id(50).to_string())
    );
    assert_eq!(row.parent_id, hex::encode([1u8; 8]));

    // Every filter operator, alone and combined, including arbitrary span attributes.
    let count =
        |conditions: Vec<pb::FilterCondition>| async move { listed(only(conditions)).await.len() };
    let errors = listed(only(vec![condition("level", "=", &["ERROR"])])).await;
    assert_eq!(errors.len(), 1);
    assert_eq!(
        (errors[0].r#type.as_str(), errors[0].status_message.as_str()),
        ("TOOL", "tool failed")
    );
    assert_eq!(
        count(vec![condition("name", "contains", &["toolcall"])]).await,
        1
    );
    assert_eq!(
        count(vec![condition("name", "starts with", &["ai.tool"])]).await,
        1
    );
    assert_eq!(
        count(vec![condition("name", "ends with", &["Generate"])]).await,
        2
    );
    assert_eq!(
        count(vec![condition("name", "does not contain", &["bulk"])]).await,
        3
    );
    assert_eq!(
        count(vec![condition("type", "any of", &["generation", "tool"])]).await,
        3
    );
    assert_eq!(
        count(vec![condition("type", "none of", &["span"])]).await,
        3
    );
    assert_eq!(count(vec![condition("latency", ">", &["0.2"])]).await, 1);
    // Sixty bulk spans match; a page holds fifty.
    assert_eq!(
        count(vec![condition("latency", "<=", &["0.001"])]).await,
        50
    );
    assert_eq!(
        count(vec![condition("total_tokens", ">=", &["20"])]).await,
        1
    );
    assert_eq!(
        count(vec![condition("total_cost", ">", &["0.00001"])]).await,
        1
    );
    assert_eq!(
        count(vec![condition("input", "contains", &["QUARTERLY"])]).await,
        1
    );
    assert_eq!(
        count(vec![condition("output", "contains", &["secret"])]).await,
        0
    );
    assert_eq!(
        count(vec![condition("input", "contains", &["100%_literal"])]).await,
        0
    );
    assert_eq!(
        count(vec![meta("ai.toolCall.name", "=", "search_documents")]).await,
        1
    );
    assert_eq!(
        count(vec![meta("ai.toolCall.name", "starts with", "search")]).await,
        1
    );
    assert_eq!(
        count(vec![meta("ai.toolCall.name", "=", "it's \\ not")]).await,
        0
    );
    assert_eq!(
        count(vec![meta("gen_ai.usage.input_tokens", ">", "10")]).await,
        1
    );
    assert_eq!(
        count(vec![
            condition("model", "=", &["gpt-test"]),
            condition("session_id", "=", &[&id(40).to_string()]),
            condition("invocation_id", "=", &[&id(50).to_string()]),
        ])
        .await,
        1
    );
    for bad in [
        condition("SpanName", "=", &["x"]),
        condition("latency", "contains", &["x"]),
        condition("name", "=", &[]),
    ] {
        assert!(
            reader
                .list(&agent, only(vec![bad]), "", None)
                .await
                .is_err()
        );
    }

    // A trace opens only for an agent that took part in it, and then shows all of it.
    let trace = hex::encode([1u8; 16]);
    let whole = reader
        .list(&agent, Default::default(), "", Some(&trace))
        .await
        .unwrap();
    let mut names: Vec<_> = whole.observations.iter().map(|o| o.name.as_str()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "ai.generateText.doGenerate",
            "ai.toolCall",
            "gateway.dispatch"
        ]
    );
    assert!(
        reader
            .list(&id(2).to_string(), Default::default(), "", Some(&trace))
            .await
            .is_err()
    );
    assert!(
        reader
            .list(
                &agent,
                Default::default(),
                "",
                Some(&hex::encode([2u8; 16]))
            )
            .await
            .is_err()
    );
    assert!(
        reader
            .list(&id(9).to_string(), window(), "", None)
            .await
            .is_err(),
        "unknown agent"
    );

    // Media leaves the span as a token and payloads beyond the inline limit as a reference;
    // both resolve to signed URLs, only for the owning agent.
    let vision = listed(only(vec![condition("model", "=", &["vision-test"])])).await;
    let row = &vision[0];
    let token = row.input.split("@@@").nth(1).expect("media token");
    assert!(
        token.starts_with("tildeMedia:type=image/png|id=")
            && token.ends_with("|source=base64_data_uri")
    );
    let media_id = token
        .split("|id=")
        .nth(1)
        .unwrap()
        .split('|')
        .next()
        .unwrap();
    assert_eq!(row.output.len(), 64 * 1024);
    let reference = row
        .attributes
        .iter()
        .find(|a| a.key == "tilde.observation.output_ref")
        .expect("payload reference");
    let http = reqwest::Client::new();
    let fetch = |key: String| {
        let (reader, agent, http) = (reader.clone(), agent.clone(), http.clone());
        async move {
            let url = reader.object_url(&agent, &key).await.unwrap();
            http.get(url)
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap()
                .to_vec()
        }
    };
    assert_eq!(
        fetch(format!("media/{agent}/{media_id}")).await[..4],
        [137u8, 80, 78, 71]
    );
    assert_eq!(fetch(reference.value.clone()).await.len(), 70 * 1024);
    // The attributes the projection was copied from are bounded the same way: no full payload
    // or inline image stays anywhere on the span, and identical text shares one object.
    let attribute = |key: &str| {
        row.attributes
            .iter()
            .find(|a| a.key == key)
            .map(|a| &a.value)
    };
    assert_eq!(
        attribute("ai.response.text").map(String::len),
        Some(64 * 1024)
    );
    assert_eq!(attribute("ai.response.text_ref"), Some(&reference.value));
    let prompt = attribute("ai.prompt.messages").expect("source prompt");
    assert!(prompt.contains("@@@tildeMedia:type=image/png") && !prompt.contains("data:image"));
    assert_eq!(row.agent_id, agent);
    assert!(
        reader
            .object_url(&id(2).to_string(), &reference.value)
            .await
            .is_err()
    );
    assert!(
        reader
            .object_url(&agent, "payloads/../../secret")
            .await
            .is_err()
    );
    assert_eq!(
        listed(only(vec![condition("output", "contains", &["xxxx"])]))
            .await
            .len(),
        1,
        "the inline prefix stays searchable"
    );

    // The chart covers the whole range, whatever page the table is on, with the same filters.
    let metrics = reader.metrics(&agent, window()).await.unwrap();
    assert_eq!(metrics.buckets.len(), 24);
    assert_eq!(
        metrics
            .buckets
            .iter()
            .map(|b| b.observation_count)
            .sum::<u64>(),
        63
    );
    assert_eq!(
        metrics.buckets.iter().map(|b| b.error_count).sum::<u64>(),
        1
    );
    let searched = reader
        .metrics(
            &agent,
            only(vec![condition("input", "contains", &["quarterly"])]),
        )
        .await
        .unwrap();
    let busy: Vec<_> = searched
        .buckets
        .iter()
        .filter(|b| b.observation_count > 0)
        .collect();
    assert_eq!(
        (
            busy.len(),
            busy[0].observation_count,
            busy[0].average_latency_seconds
        ),
        (1, 1, Some(0.5))
    );

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
}

#[tokio::test]
async fn any_replica_delivers_a_batch_and_a_failed_delivery_returns() {
    let db = Database::new().await;
    let storage = common::Storage::load();
    let bucket = common::bucket(&storage);
    // Keys start with the agent, so a fresh id keeps this run apart in the shared bucket.
    let agent = Uuid::new_v4();
    let prefix = format!("traces/clickhouse/{agent}/");
    let objects = || common::object_keys(&storage, &storage.s3_bucket, &prefix);
    let replica = || {
        Spool::open(
            db.pool.clone(),
            bucket.clone(),
            "traces/clickhouse",
            1024 * 1024,
        )
    };
    let (first, second) = (replica(), replica());
    first
        .accept(agent.to_string(), b"batch".to_vec())
        .await
        .unwrap();
    // A retried upload is the same batch, not a second one. Its object is written before
    // admission and, like every object, is never deleted by the engine.
    first
        .accept(agent.to_string(), b"batch".to_vec())
        .await
        .unwrap();
    assert_eq!(first.pending().await.unwrap(), 1);
    assert_eq!(objects().await.len(), 2);

    // Another replica picks it up; while it holds the lease nobody else does.
    let (key, bytes) = second.next().await.unwrap().expect("pending batch");
    assert_eq!(bytes, b"batch");
    assert!(first.next().await.unwrap().is_none());
    // The holder died without completing: the batch comes back once the lease runs out.
    db.pool
        .get()
        .await
        .unwrap()
        .execute("UPDATE telemetry_objects SET leased_until=NOW()", &[])
        .await
        .unwrap();
    let (again, _) = first.next().await.unwrap().expect("batch is due again");
    assert_eq!(again, key);
    first.complete(again).await.unwrap();
    assert_eq!(second.pending().await.unwrap(), 0);
    // Completion removes the pointer only; the object stays for the bucket lifecycle rule.
    assert_eq!(objects().await.len(), 2);
    // Every enqueue has its own object, so a re-accepted batch never reads a completed copy.
    first
        .accept(agent.to_string(), b"batch".to_vec())
        .await
        .unwrap();
    let (requeued, bytes) = second.next().await.unwrap().expect("requeued batch");
    assert_ne!(requeued, key);
    assert_eq!(bytes, b"batch");
    second.complete(requeued).await.unwrap();

    // The per-queue limit bounds what is pending. A batch that cannot fit is refused before
    // it is uploaded.
    assert!(
        first
            .accept(agent.to_string(), vec![0; 2 * 1024 * 1024])
            .await
            .is_err()
    );
    assert_eq!(first.pending().await.unwrap(), 0);
    assert_eq!(objects().await.len(), 3);

    // A batch nobody delivers expires by age; the pointer goes and the object stays.
    first
        .accept(agent.to_string(), b"stale".to_vec())
        .await
        .unwrap();
    let client = db.pool.get().await.unwrap();
    client
        .execute(
            "UPDATE telemetry_objects SET created_at=NOW()-INTERVAL '2 days'",
            &[],
        )
        .await
        .unwrap();
    // A batch being delivered is left alone even when it is old enough to expire.
    let (leased, _) = first.next().await.unwrap().expect("stale batch is due");
    assert_eq!(
        tilde::telemetry::spool::db::expire(&client, 86400)
            .await
            .unwrap(),
        0
    );
    client
        .execute(
            "UPDATE telemetry_objects SET leased_until=NOW() WHERE object_key=$1",
            &[&leased],
        )
        .await
        .unwrap();
    assert_eq!(
        tilde::telemetry::spool::db::expire(&client, 86400)
            .await
            .unwrap(),
        1
    );
    assert_eq!(first.pending().await.unwrap(), 0);
    assert_eq!(objects().await.len(), 4);
    // Usage follows pointers through completion and expiry, so nothing is left counted.
    let usage: i64 = client
        .query_one("SELECT COUNT(*) FROM telemetry_usage", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(usage, 0);
}

/// Admission is serialized per queue: however many accepts race, the pending bytes never
/// exceed the limit, and losers leave no pointer behind.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_accepts_reserve_capacity_atomically() {
    let db = Database::new().await;
    let bucket = common::bucket(&common::Storage::load());
    let queue = Spool::open(db.pool.clone(), bucket, "traces/clickhouse", 1024 * 1024);
    // Every task holds its object written and its admission ready before any is admitted.
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    let attempts = (0..8u8)
        .map(|n| {
            let (queue, barrier) = (queue.clone(), barrier.clone());
            tokio::spawn(async move {
                barrier.wait().await;
                queue.accept(id(1).to_string(), vec![n; 300 * 1024]).await
            })
        })
        .collect::<Vec<_>>();
    let mut admitted = 0;
    for attempt in attempts {
        if attempt.await.unwrap().is_ok() {
            admitted += 1;
        }
    }
    assert_eq!(
        admitted, 3,
        "three 300 KiB batches fit in 1 MiB, never a fourth"
    );
    assert_eq!(queue.pending().await.unwrap(), 3);
    let client = db.pool.get().await.unwrap();
    let held = client
        .query_one(
            "SELECT (SELECT SUM(bytes) FROM telemetry_objects)::BIGINT, \
             (SELECT SUM(bytes) FROM telemetry_usage)::BIGINT, \
             (SELECT SUM(objects) FROM telemetry_usage)::BIGINT",
            &[],
        )
        .await
        .unwrap();
    let (pointers, usage, objects): (i64, i64, i64) = (held.get(0), held.get(1), held.get(2));
    assert!(pointers <= 1024 * 1024);
    // Admission reads usage, never the pointers, so the two must agree under concurrency.
    assert_eq!((usage, objects), (pointers, 3));
    // Everything admitted is readable; refused uploads hold no pointer.
    for _ in 0..3 {
        let (key, bytes) = queue.next().await.unwrap().expect("admitted batch");
        assert_eq!(bytes.len(), 300 * 1024);
        queue.complete(key).await.unwrap();
    }
    assert!(queue.next().await.unwrap().is_none());
}
