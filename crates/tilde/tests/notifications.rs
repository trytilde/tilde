#[allow(dead_code)]
mod common;
use common::{Database, seed};
use connectrpc::client::{ClientConfig, HttpClient};
use secrecy::SecretString;
use std::{sync::Arc, time::Duration};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, CreateThread, activity},
    database::notifications::Notifications,
    encryption::Encryption,
    proto::tilde::{management::v1::WatchThreadRequest, types::v1::ParticipantRef},
    services::tilde::management::v1::ChatServiceClient,
};
use uuid::Uuid;

#[tokio::test]
async fn committed_activity_reaches_another_instance_and_drains_multiple_pages() {
    let db = Database::new().await;
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(9)).await.unwrap());
    let agent = Agents::new(db.pool.clone(), crypto.clone())
        .create(CreateAgent {
            id: Uuid::new_v4(),
            name: "Notifications".into(),
            endpoint_url: None,
            webhook_signing_key: SecretString::from("notification-test-agent-signing-key"),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    let writer = Chat::new(db.pool.clone(), crypto.clone(), "http://localhost:1".into());
    let thread = writer
        .create_thread(CreateThread {
            title: "Cross-instance activity".into(),
            primary_agent_id: agent.id.to_string(),
            participants: vec![ParticipantRef {
                agent_id: Some(agent.id.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let thread_id = Uuid::parse_str(&thread.id).unwrap();
    let cursor = writer
        .activity_page(thread_id, 0, 100)
        .await
        .unwrap()
        .next_sequence;
    // Separate service and pool, as on another gateway instance.
    let reader_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect_with(db.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let reader = Chat::new(
        reader_pool.clone(),
        crypto.clone(),
        "http://localhost:1".into(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, tilde::chat::rpc::management::router(reader))
            .await
            .unwrap();
    });
    let client = ChatServiceClient::new(
        HttpClient::plaintext_http2_only(),
        ClientConfig::new(url.parse().unwrap()),
    );
    let mut stream = client
        .watch_thread(WatchThreadRequest {
            thread_id: thread.id,
            after_sequence: cursor,
            ..Default::default()
        })
        .await
        .unwrap();
    let mut tx = db.pool.begin().await.unwrap();
    activity(
        &mut tx,
        thread_id,
        "reasoning.delta",
        agent.id,
        "rolled back",
    )
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), stream.message())
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let mut tx = db.pool.begin().await.unwrap();
    for n in 0..205 {
        activity(
            &mut tx,
            thread_id,
            "reasoning.delta",
            agent.id,
            &n.to_string(),
        )
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        for n in 0..205 {
            let message = stream.message().await.unwrap().unwrap();
            let view = message.view();
            let event = &view.activity;
            assert_eq!(event.sequence, cursor + n + 1);
            assert_eq!(event.text_delta, n.to_string());
        }
    })
    .await
    .unwrap();
    // A later commit wakes the now-idle subscription, without polling.
    let mut tx = db.pool.begin().await.unwrap();
    activity(&mut tx, thread_id, "reasoning.delta", agent.id, "later")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let next = tokio::time::timeout(Duration::from_secs(2), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(next.view().activity.text_delta, "later");
    // After both workers have drained startup work, new messages must wake them.
    // The durable claim still permits only one invocation per message under HA.
    let user = writer.create_user("Caller").await.unwrap();
    let roster = writer
        .add_participant(tilde::chat::AddParticipant {
            thread_id: thread_id.to_string(),
            participant: Some(ParticipantRef {
                user_id: Some(user.id.clone()),
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    let participant = roster
        .participants
        .iter()
        .find(|p| p.user_id.as_deref() == Some(&user.id))
        .unwrap()
        .id
        .clone();
    let hub = Notifications::default();
    let mut changed = hub
        .subscribe(&db.pool, "tilde_chat_activity")
        .await
        .unwrap();
    let (stop, rx) = tokio::sync::watch::channel(false);
    let worker_a = tokio::spawn(writer.clone().worker(rx.clone()));
    let worker_b = tokio::spawn(
        Chat::new(reader_pool.clone(), crypto, "http://localhost:1".into()).worker(rx),
    );
    for expected in 1..=2_i64 {
        writer
            .post(tilde::chat::PostMessage {
                id: Uuid::new_v4().to_string(),
                thread_id: thread_id.to_string(),
                participant_id: participant.clone(),
                text: "Wake the agent".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                changed.borrow_and_update();
                let count: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM chat_invocations WHERE thread_id=$1 AND status='failed'",
                )
                .bind(thread_id)
                .fetch_one(&db.pool)
                .await
                .unwrap();
                // This fixture deliberately has no host endpoint, so claimed work fails.
                if count == expected {
                    break;
                }
                changed.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
    }
    let invocations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chat_invocations WHERE thread_id=$1")
            .bind(thread_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(invocations, 2);
    stop.send(true).unwrap();
    worker_a.await.unwrap();
    worker_b.await.unwrap();
    drop(stream);
    server.abort();
    let _ = server.await;
    reader_pool.close().await;
    db.close().await;
}

#[tokio::test]
async fn listener_reconnect_invalidates_subscribers_and_resumes_notifications() {
    let db = Database::new().await;
    // Distinct application name lets the test disconnect only its own listener.
    let options = db
        .pool
        .connect_options()
        .as_ref()
        .clone()
        .application_name("tilde-notification-reconnect-test");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy_with(options);
    let hub = Notifications::default();
    let mut a = hub.subscribe(&pool, "tilde_reconnect_test").await.unwrap();
    let mut b = hub.subscribe(&pool, "tilde_reconnect_test").await.unwrap();
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='tilde-notification-reconnect-test'")
        .execute(&db.pool).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), a.changed())
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), b.changed())
        .await
        .unwrap()
        .unwrap();
    // Await a new LISTEN connection using its durable observable backend state.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let listening: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name='tilde-notification-reconnect-test' AND state='idle' AND query LIKE 'LISTEN%')")
                .fetch_one(&db.pool).await.unwrap();
            if listening { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    a.borrow_and_update();
    b.borrow_and_update();
    sqlx::query("SELECT pg_notify('tilde_reconnect_test', '')")
        .execute(&db.pool)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), a.changed())
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), b.changed())
        .await
        .unwrap()
        .unwrap();
    drop(a);
    drop(b);
    drop(hub);
    pool.close().await;
    db.close().await;
}
