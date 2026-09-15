use tilde::proto::tilde::agent_host::v1 as host;
#[expect(
    dead_code,
    reason = "shared fixture also contains helpers for other integration targets"
)]
mod common;
use axum::{Router, extract::State, response::IntoResponse, routing::post};
use buffa::Message;
use chrono::{Duration, Utc};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use tilde::{
    agent::health::{AgentHealth, metrics},
    agent::{Agents, CreateAgent},
    encryption::Encryption,
    proto::tilde::types::v1::AgentHealthStatus,
};
use uuid::Uuid;

async fn fixture(
    State(mode): State<Arc<AtomicU8>>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    assert!(headers.contains_key("x-tilde-signature"));
    match mode.load(Ordering::SeqCst) {
        2 => return (axum::http::StatusCode::SERVICE_UNAVAILABLE, "unavailable").into_response(),
        3 => tokio::time::sleep(std::time::Duration::from_secs(20)).await,
        _ => {}
    }
    let reply = host::HealthzResponse {
        ready: mode.load(Ordering::SeqCst) == 0,
        ..Default::default()
    };
    (
        [("content-type", "application/proto")],
        reply.encode_to_vec(),
    )
        .into_response()
}
async fn create(agents: &Agents, endpoint: String) -> Uuid {
    let id = Uuid::new_v4();
    agents
        .create(CreateAgent {
            concurrency_policy: Default::default(),
            capabilities: Default::default(),
            id,
            name: "Health fixture".into(),
            endpoint_url: endpoint,
            webhook_signing_key: secrecy::SecretString::from(
                "health-fixture-signing-key-0123456789",
            ),
        })
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn health_polling_history_retention_and_shutdown() {
    let db = common::Database::new().await;
    let encryption = Arc::new(
        Encryption::initialize(&db.pool, common::seed(7))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let health = AgentHealth::new(db.pool.clone(), encryption);
    let mode = Arc::new(AtomicU8::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/tilde.agent_host.v1.AgentService/Healthz", post(fixture))
        .with_state(mode.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let id = create(&agents, endpoint.clone()).await;
    health.poll_once().await.unwrap();
    let unpolled = create(&agents, endpoint.clone()).await;
    let view = metrics(&db.pool, &[id, unpolled], Utc::now())
        .await
        .unwrap();
    assert_eq!(view[&id].health, AgentHealthStatus::Healthy);
    assert_eq!(view[&unpolled].health, AgentHealthStatus::Unknown);
    assert_eq!(view[&id].health_history.len(), 12);
    assert_eq!(
        view[&id]
            .health_history
            .iter()
            .map(|h| h.total_checks)
            .sum::<u32>(),
        1
    );
    for (flag, expected) in [(1, "not_ready"), (2, "transport"), (3, "timeout")] {
        mode.store(flag, Ordering::SeqCst);
        health.poll_once().await.unwrap();
        let code: String = sqlx::query_scalar(
            "SELECT error_code FROM agent_health WHERE agent_id=$1 ORDER BY id DESC LIMIT 1",
        )
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(code, expected);
    }
    let view = metrics(&db.pool, &[id], Utc::now()).await.unwrap();
    assert_eq!(view[&id].health, AgentHealthStatus::Unhealthy);
    assert_eq!(
        view[&id]
            .health_history
            .iter()
            .map(|h| h.failed_checks)
            .sum::<u32>(),
        3
    );
    let stale = metrics(&db.pool, &[id], Utc::now() + Duration::seconds(100))
        .await
        .unwrap();
    assert_eq!(stale[&id].health, AgentHealthStatus::Unknown);

    // A changed endpoint does not inherit the previous endpoint's health.
    sqlx::query("UPDATE agents SET endpoint_url='http://127.0.0.1:1/' WHERE id=$1")
        .bind(id)
        .execute(&db.pool)
        .await
        .unwrap();
    let changed = metrics(&db.pool, &[id], Utc::now()).await.unwrap();
    assert_eq!(changed[&id].health, AgentHealthStatus::Unknown);
    assert!(
        changed[&id]
            .health_history
            .iter()
            .all(|h| h.total_checks == 0)
    );
    let count = sqlx::query_file!(
        "../../queries/agent_health/insert.sql",
        id,
        endpoint.clone(),
        Utc::now(),
        true,
        0,
        None::<&str>
    )
    .execute(&db.pool)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(
        count, 0,
        "discard in-flight observations of a replaced endpoint"
    );
    sqlx::query("INSERT INTO agent_health(agent_id,endpoint_url,checked_at,healthy,latency_ms) VALUES($1,$2,NOW()-INTERVAL '15 days',TRUE,0),($1,$2,NOW()-INTERVAL '13 days',TRUE,0)")
        .bind(id).bind(&endpoint).execute(&db.pool).await.unwrap();
    assert_eq!(health.cleanup().await.unwrap(), 1);

    // Shutdown interrupts an in-flight timeout and releases the sweep lock.
    sqlx::query("UPDATE agents SET endpoint_url=$2 WHERE id=$1")
        .bind(id)
        .bind(&endpoint)
        .execute(&db.pool)
        .await
        .unwrap();
    let (stop, rx) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(health.clone().run(rx));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    stop.send(true).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
    mode.store(0, Ordering::SeqCst);
    health.poll_once().await.unwrap();
    agents.pause(id).await.unwrap();
    health.poll_once().await.unwrap();
    assert_eq!(
        agents.metrics(&[id]).await.unwrap()[&id].health,
        AgentHealthStatus::Healthy
    );
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_health WHERE agent_id=$1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    agents.delete(id).await.unwrap();
    health.poll_once().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_health WHERE agent_id=$1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, before, "retired agents receive no further checks");
    server.abort();
    db.close().await;
}

#[tokio::test]
async fn registry_metrics_count_sessions_and_measure_first_visible_reply() {
    let db = common::Database::new().await;
    let encryption = Arc::new(
        Encryption::initialize(&db.pool, common::seed(7))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), encryption);
    let agent = create(&agents, "http://127.0.0.1:9999".into()).await;
    let other = create(&agents, "http://127.0.0.1:9999".into()).await;
    let thread = Uuid::new_v4();
    let empty_thread = Uuid::new_v4();
    let participant = Uuid::new_v4();
    sqlx::query("INSERT INTO chat_threads(id,title,primary_agent_id) VALUES($1,'Messages',$3),($2,'Empty',$3)").bind(thread).bind(empty_thread).bind(agent).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO chat_participants(id,thread_id,agent_id) VALUES($1,$2,$3),($4,$5,$3)")
        .bind(participant)
        .bind(thread)
        .bind(agent)
        .bind(Uuid::new_v4())
        .bind(empty_thread)
        .execute(&db.pool)
        .await
        .unwrap();
    let run = Uuid::new_v4();
    let invocation = Uuid::new_v4();
    let message = Uuid::new_v4();
    sqlx::query("INSERT INTO chat_runs(id,thread_id,agent_id,objective,idempotency_key) VALUES($1,$2,$3,'Test','test')").bind(run).bind(thread).bind(agent).execute(&db.pool).await.unwrap();
    let started = Utc::now() - Duration::seconds(2);
    sqlx::query("INSERT INTO chat_invocations(id,run_id,thread_id,agent_id,status,started_at) VALUES($1,$2,$3,$4,'running',$5)").bind(invocation).bind(run).bind(thread).bind(agent).bind(started).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO chat_messages(id,thread_id,participant_id,text,status,invocation_id) VALUES($1,$2,$3,'','streaming',$4)").bind(message).bind(thread).bind(participant).bind(invocation).execute(&db.pool).await.unwrap();
    let before = metrics(&db.pool, &[agent, other], Utc::now())
        .await
        .unwrap();
    assert_eq!(before[&agent].thread_count, 2);
    assert_eq!(before[&agent].average_turns_per_thread, Some(0.0));
    assert_eq!(before[&agent].average_response_ms, None);
    assert_eq!(before[&other].thread_count, 0);
    assert_eq!(before[&other].average_turns_per_thread, None);
    sqlx::query("UPDATE chat_messages SET text='First chunk' WHERE id=$1")
        .bind(message)
        .execute(&db.pool)
        .await
        .unwrap();
    let first: chrono::DateTime<Utc> =
        sqlx::query_scalar("SELECT first_content_at FROM chat_messages WHERE id=$1")
            .bind(message)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    sqlx::query("UPDATE chat_messages SET text='Completed response',status='complete' WHERE id=$1")
        .bind(message)
        .execute(&db.pool)
        .await
        .unwrap();
    let after = metrics(&db.pool, &[agent], Utc::now()).await.unwrap();
    assert_eq!(after[&agent].average_turns_per_thread, Some(0.5));
    let expected = (first - started).num_microseconds().unwrap() as f64 / 1000.0;
    assert!((after[&agent].average_response_ms.unwrap() - expected).abs() < 0.01);
    db.close().await;
}
