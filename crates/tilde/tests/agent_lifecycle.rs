#[expect(dead_code, reason = "shared test fixtures")]
mod common;
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, CreateThread, StartRun},
    encryption::Encryption,
    error::Error,
    proto::tilde::types::v1 as types,
};
use uuid::Uuid;

#[tokio::test]
async fn pause_revokes_running_work_fences_dispatch_and_preserves_history() {
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(72))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let chat = Chat::new(db.pool.clone(), crypto, "http://127.0.0.1:1".into());
    let endpoint = "http://127.0.0.1:1".to_owned();
    let agent = Uuid::new_v4();
    agents
        .create(CreateAgent {
            concurrency_policy: Default::default(),
            id: agent,
            name: "Pause fixture".into(),
            endpoint_url: endpoint,
            webhook_signing_key: SecretString::from("fixture-shared-key-0123456789abcdef"),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    let mut runs = Vec::new();
    for index in 0..2 {
        let thread = chat
            .create_thread(CreateThread {
                title: format!("Thread {index}"),
                primary_agent_id: agent.to_string(),
                participants: vec![types::ParticipantRef {
                    agent_id: Some(agent.to_string()),
                    ..Default::default()
                }],
            })
            .await
            .unwrap();
        runs.push(
            chat.start_run(StartRun {
                thread_id: thread.id,
                agent_id: agent.to_string(),
                objective: "Do work".into(),
                idempotency_key: "initial".into(),
                ..Default::default()
            })
            .await
            .unwrap(),
        );
    }
    let running = Uuid::parse_str(&runs[0].invocation_id).unwrap();
    let pending = Uuid::parse_str(&runs[1].invocation_id).unwrap();
    let claim = sqlx::query_file!("../../queries/chat/invocation_claim.sql", running)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(claim.generation, 0);
    let token = chat
        .tokens
        .issue(agent, running, claim.thread_id, claim.run_id)
        .await
        .unwrap();
    assert!(chat.tokens.verify(token.expose_secret()).await.is_ok());
    assert!(matches!(
        agents.delete(agent).await,
        Err(Error::AgentNotPaused)
    ));
    let paused = agents.pause(agent).await.unwrap();
    assert!(paused.paused);
    assert!(chat.tokens.verify(token.expose_secret()).await.is_err());
    assert_eq!(
        chat.run(claim.run_id).await.unwrap().invocation_status,
        "canceled"
    );
    assert!(
        sqlx::query_file!("../../queries/chat/invocation_claim.sql", pending)
            .fetch_optional(&db.pool)
            .await
            .unwrap()
            .is_none()
    );
    assert!(chat.resume_run(claim.run_id).await.is_err());
    assert!(
        chat.start_run(StartRun {
            thread_id: runs[0].thread_id.clone(),
            agent_id: agent.to_string(),
            objective: "Blocked".into(),
            idempotency_key: "blocked".into(),
            ..Default::default()
        })
        .await
        .is_err()
    );
    // Retrying pause reuses the fence; resume advances past it.
    assert!(agents.pause(agent).await.unwrap().paused);
    assert!(!agents.resume(agent).await.unwrap().paused);
    let resumed = sqlx::query_file!("../../queries/chat/invocation_claim.sql", pending)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(resumed.generation, 2);
    agents.pause(agent).await.unwrap();
    agents.delete(agent).await.unwrap();
    agents.delete(agent).await.unwrap();
    assert!(matches!(agents.get(agent).await, Err(Error::NotFound)));
    assert!(agents.list(10, "").await.unwrap().agents.is_empty());
    assert!(agents.resume(agent).await.is_err());
    let historical = chat.thread(claim.thread_id).await.unwrap();
    assert_eq!(historical.participants[0].name, "Pause fixture");
    assert!(!historical.participants[0].active);
    assert_eq!(
        chat.run(claim.run_id).await.unwrap().invocation_status,
        "canceled"
    );
    let key: Vec<u8> = sqlx::query_scalar("SELECT webhook_signing_key FROM agents WHERE id=$1")
        .bind(agent)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(key.is_empty());
    db.close().await;
}

#[tokio::test]
async fn messages_received_while_paused_wait_for_resume_notification() {
    use std::time::Duration;
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(73))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let chat = Chat::new(db.pool.clone(), crypto, "http://127.0.0.1:1".into());
    let agent = Uuid::new_v4();
    agents
        .create(CreateAgent {
            concurrency_policy: Default::default(),
            id: agent,
            name: "Queue fixture".into(),
            endpoint_url: "http://127.0.0.1:9999".into(),
            webhook_signing_key: SecretString::from("fixture-shared-key-0123456789abcdef"),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    let user = chat.create_user("Caller").await.unwrap();
    let thread = chat
        .create_thread(CreateThread {
            title: "Paused messages".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![
                types::ParticipantRef {
                    agent_id: Some(agent.to_string()),
                    ..Default::default()
                },
                types::ParticipantRef {
                    user_id: Some(user.id.clone()),
                    ..Default::default()
                },
            ],
        })
        .await
        .unwrap();
    agents.pause(agent).await.unwrap();
    let message = chat
        .post(tilde::chat::PostMessage {
            id: Uuid::new_v4().to_string(),
            thread_id: thread.id.clone(),
            participant_id: thread
                .participants
                .iter()
                .find(|p| p.user_id.as_ref() == Some(&user.id))
                .unwrap()
                .id
                .clone(),
            text: "Wait for resume".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(
        sqlx::query_file!("../../queries/chat/route_pending.sql")
            .fetch_all(&db.pool)
            .await
            .unwrap()
            .is_empty()
    );
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(chat.clone().worker(receiver));
    agents.resume(agent).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let routed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_message_dispatch WHERE message_id=$1 AND agent_id=$2)").bind(Uuid::parse_str(&message.id).unwrap()).bind(agent).fetch_one(&db.pool).await.unwrap();
            if routed { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    shutdown.send(true).unwrap();
    worker.await.unwrap();
    assert_eq!(
        chat.message(Uuid::parse_str(&message.id).unwrap())
            .await
            .unwrap()
            .text,
        "Wait for resume"
    );
    db.close().await;
}
