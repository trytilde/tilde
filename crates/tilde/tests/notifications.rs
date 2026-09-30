mod common;
use common::{Database, seed};
use connectrpc::client::{ClientConfig, HttpClient};
use secrecy::ExposeSecret;
use std::{sync::Arc, time::Duration};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, CreateThread, activity},
    database::notifications::Notifications,
    encryption::Encryption,
    proto::tilde::{provider::tilde::v1::WatchThreadRequest, types::v1::ParticipantRef},
    services::tilde::provider::tilde::v1::ChatServiceClient,
};
use uuid::Uuid;

#[tokio::test]
async fn committed_activity_reaches_another_instance_and_drains_multiple_pages() {
    let db = Database::new().await;
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(9)).await.unwrap());
    let agent = Agents::new(db.pool.clone(), crypto.clone())
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id: Uuid::new_v4(),
            name: "Notifications".into(),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    common::dialled_in(&db.pool, crypto.clone(), agent.id).await;
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
    common::pin_gateway(&db.pool, agent.id, thread_id).await;
    tilde::chat::audit::flush(&db.pool).await.unwrap();
    let sequence = writer
        .activity_page(thread_id, 0, 100)
        .await
        .unwrap()
        .next_sequence;
    let cursor = writer.ingress_activity(thread_id, "", 100).await.unwrap().1;
    // Separate service and pool, as on another gateway instance.
    let reader_pool =
        tilde::database::pool_from_config(db.pool.config().as_ref().clone(), 2).unwrap();
    let reader = Chat::new(
        reader_pool.clone(),
        crypto.clone(),
        "http://localhost:1".into(),
    );
    let (url, token, server) =
        common::tilde_provider(&reader_pool, crypto.clone(), reader, agent.id).await;
    let mut headers = http::HeaderMap::new();
    headers.insert(
        http::header::AUTHORIZATION,
        format!("Bearer {}", token.expose_secret()).parse().unwrap(),
    );
    let client = ChatServiceClient::new(
        HttpClient::plaintext_http2_only(),
        ClientConfig::new(url.parse().unwrap()).with_default_headers(headers),
    );
    let mut stream = client
        .watch_thread(WatchThreadRequest {
            thread_id: thread.id,
            after_cursor: cursor,
            ..Default::default()
        })
        .await
        .unwrap();
    let mut tx_client = db.pool.get().await.unwrap();
    let tx = tx_client.transaction().await.unwrap();
    activity(
        &db.pool,
        &tx,
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
    drop(tx_client);
    let mut tx_client = db.pool.get().await.unwrap();
    let tx = tx_client.transaction().await.unwrap();
    for n in 0..205 {
        activity(
            &db.pool,
            &tx,
            thread_id,
            "reasoning.delta",
            agent.id,
            &n.to_string(),
        )
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    drop(tx_client);
    tokio::time::timeout(Duration::from_secs(5), async {
        for n in 0..205 {
            let message = stream.message().await.unwrap().unwrap();
            let view = message.view();
            let event = &view.activity;
            assert_eq!(event.sequence, sequence + n + 1);
            assert_eq!(event.text_delta, n.to_string());
        }
    })
    .await
    .unwrap();
    // A later commit wakes the now-idle subscription, without polling.
    let mut tx_client = db.pool.get().await.unwrap();
    let tx = tx_client.transaction().await.unwrap();
    activity(
        &db.pool,
        &tx,
        thread_id,
        "reasoning.delta",
        agent.id,
        "later",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(tx_client);
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
                let count: i64 = db.pool.get().await.unwrap().query_one("SELECT count(*) FROM chat_invocations WHERE thread_id=$1 AND status='failed'", &[&thread_id]).await.unwrap().get(0);
                // No host has dialled in for this fixture, so claimed work fails.
                if count == expected {
                    break;
                }
                changed.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
    }
    let invocations: i64 = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM chat_invocations WHERE thread_id=$1",
            &[&thread_id],
        )
        .await
        .unwrap()
        .get(0);
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
    let mut options = db.pool.config().as_ref().clone();
    options.application_name("tilde-notification-reconnect-test");
    let pool = tilde::database::pool_from_config(options, 1).unwrap();
    let hub = Notifications::default();
    let mut a = hub.subscribe(&pool, "tilde_reconnect_test").await.unwrap();
    let mut b = hub.subscribe(&pool, "tilde_reconnect_test").await.unwrap();
    db.pool.get().await.unwrap().execute("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='tilde-notification-reconnect-test'", &[]).await.unwrap();
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
            let listening: bool = db.pool.get().await.unwrap().query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name='tilde-notification-reconnect-test' AND state='idle' AND query LIKE 'LISTEN%')", &[]).await.unwrap().get(0);
            if listening { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    a.borrow_and_update();
    b.borrow_and_update();
    db.pool
        .get()
        .await
        .unwrap()
        .execute("SELECT pg_notify('tilde_reconnect_test', '')", &[])
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
