use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tilde::{
    agent::{Agents, CreateAgent, UpdateAgent},
    encryption::Encryption,
    error::Error,
};
use uuid::Uuid;

fn input(id: Uuid, name: &str) -> CreateAgent {
    CreateAgent {
        description: String::new(),
        concurrency_policy: Default::default(),
        capabilities: Default::default(),
        id,
        name: name.into(),
    }
}
/// For tests that do not open the key themselves; the seed must not collide with theirs.
async fn agents(db: &Database) -> Agents {
    let encryption = Encryption::initialize(&db.pool, seed(11)).await.unwrap();
    Agents::new(db.pool.clone(), std::sync::Arc::new(encryption))
}

#[tokio::test]
async fn central_migrations_and_agent_lifecycle() {
    let db = Database::new().await;
    tilde::database::migrate(&db.pool).await.unwrap();
    let count: i64 = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT COUNT(*) FROM _tilde_migrations", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, tilde::database::MIGRATIONS.len() as i64);
    let service = agents(&db).await;
    let id = Uuid::new_v4();
    let created = service.create(input(id, "Ada")).await.unwrap();
    let replay = service.create(input(id, "Ada")).await.unwrap();
    assert_eq!(created.created_at, replay.created_at);
    assert!(matches!(
        service.create(input(id, "Different")).await,
        Err(Error::Conflict)
    ));
    let updated = service
        .update(UpdateAgent {
            description: None,
            concurrency_policy: None,
            capabilities: None,
            id,
            name: Some("Grace".into()),
        })
        .await
        .unwrap();
    assert_eq!(updated.name, "Grace");
    let untouched = service
        .update(UpdateAgent {
            description: None,
            concurrency_policy: None,
            capabilities: None,
            id,
            name: None,
        })
        .await
        .unwrap();
    assert_eq!(untouched.name, "Grace");
    assert!(matches!(
        service.delete(id).await,
        Err(Error::AgentNotPaused)
    ));
    service.pause(id).await.unwrap();
    service.delete(id).await.unwrap();
    service.delete(id).await.unwrap();
    assert!(matches!(service.get(id).await, Err(Error::NotFound)));
    let count: i64 = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT COUNT(*) FROM agents WHERE deleted_at IS NULL", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    let old_table: Option<String> = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT to_regclass('agent_secrets')::text", &[])
        .await
        .unwrap()
        .get(0);
    assert!(old_table.is_none());
    db.close().await;
}

#[tokio::test]
async fn wrong_seed_fails_without_overwriting_the_key() {
    let db = Database::new().await;
    let crypto = Encryption::initialize(&db.pool, seed(1)).await.unwrap();
    let before: Vec<u8> = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT wrapped_key FROM encryption_keys", &[])
        .await
        .unwrap()
        .get(0);
    assert!(matches!(
        Encryption::initialize(&db.pool, seed(2)).await,
        Err(Error::Encryption)
    ));
    let after: Vec<u8> = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT wrapped_key FROM encryption_keys", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(before, after);
    let record = crypto
        .seal(
            common::binding(Uuid::nil(), "token"),
            &SecretString::from("test-value"),
        )
        .unwrap();
    let reopened = Encryption::initialize(&db.pool, seed(1)).await.unwrap();
    assert_eq!(
        reopened
            .open(common::binding(Uuid::nil(), "token"), record.clone())
            .unwrap()
            .expose_secret(),
        "test-value"
    );
    assert!(
        reopened
            .open(common::binding(Uuid::nil(), "other"), record.clone())
            .is_err()
    );
    assert!(
        reopened
            .open(
                tilde::encryption::SecretBinding {
                    resource_kind: "other",
                    ..common::binding(Uuid::nil(), "token")
                },
                record.clone()
            )
            .is_err()
    );
    let mut modified = record;
    modified.ciphertext[0] ^= 1;
    assert!(
        reopened
            .open(common::binding(Uuid::nil(), "token"), modified)
            .is_err()
    );
    db.close().await;
}

#[tokio::test]
async fn concurrent_bootstrap_uses_one_data_key() {
    let db = Database::new().await;
    let (a, b) = tokio::join!(
        Encryption::initialize(&db.pool, seed(3)),
        Encryption::initialize(&db.pool, seed(3))
    );
    let a = a.unwrap();
    let b = b.unwrap();
    let value = a
        .seal(
            common::binding(Uuid::nil(), "token"),
            &SecretString::from("shared-key"),
        )
        .unwrap();
    assert_eq!(
        b.open(common::binding(Uuid::nil(), "token"), value)
            .unwrap()
            .expose_secret(),
        "shared-key"
    );
    let count: i64 = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT COUNT(*) FROM encryption_keys", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    db.close().await;
}

#[tokio::test]
async fn pagination_keeps_equal_timestamps_and_input_validation_prevents_bad_records() {
    let db = Database::new().await;
    let service = agents(&db).await;
    for name in ["A", "B", "C"] {
        service.create(input(Uuid::new_v4(), name)).await.unwrap();
    }
    db.pool
        .get()
        .await
        .unwrap()
        .execute("UPDATE agents SET created_at='2026-01-01T00:00:00Z'", &[])
        .await
        .unwrap();
    let a = service.list(2, "").await.unwrap();
    assert_eq!(a.agents.len(), 2);
    assert!(!a.next_page_token.is_empty());
    let b = service.list(2, &a.next_page_token).await.unwrap();
    assert_eq!(b.agents.len(), 1);
    assert!(b.next_page_token.is_empty());
    assert!(!a.agents.iter().any(|x| x.id == b.agents[0].id));
    assert!(service.list(20, "invalid").await.is_err());
    let search = service
        .list_filtered(
            1,
            "",
            &tilde::agent::AgentFilter {
                search: "b",
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(search.agents.len(), 1);
    assert_eq!(search.agents[0].name, "B");
    assert!(search.next_page_token.is_empty());
    assert!(service.create(input(Uuid::new_v4(), "   ")).await.is_err());
    db.close().await;
}

#[tokio::test]
async fn expired_invocation_fails_atomically_and_can_be_explicitly_resumed() {
    use tilde::chat::Chat;
    let db = common::Database::new().await;
    let encryption = Arc::new(
        Encryption::initialize(&db.pool, common::seed(6))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let agent = agents
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            capabilities: Default::default(),
            id: Uuid::new_v4(),
            name: "Recovery".into(),
        })
        .await
        .unwrap();
    common::dialled_in(&db.pool, encryption.clone(), agent.id).await;
    let chat = Chat::new(db.pool.clone(), encryption, "http://127.0.0.1:1".into());
    let user = chat.create_user("Alice").await.unwrap();
    let thread = chat
        .create_thread(application::CreateThread {
            title: "Recovery".into(),
            primary_agent_id: agent.id.to_string(),
            participants: vec![
                types::ParticipantRef {
                    user_id: Some(user.id),
                    ..Default::default()
                },
                types::ParticipantRef {
                    agent_id: Some(agent.id.to_string()),
                    ..Default::default()
                },
            ],
        })
        .await
        .unwrap();
    common::pin_gateway(&db.pool, agent.id, Uuid::parse_str(&thread.id).unwrap()).await;
    let run = chat
        .start_run(application::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.id.to_string(),
            objective: "Recover without repeating uncertain effects".into(),
            idempotency_key: "recovery".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let run_id = Uuid::parse_str(&run.id).unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    db.pool.get().await.unwrap().execute("UPDATE chat_invocations SET status='running',started_at=NOW()-INTERVAL '2 minutes',lease_expires_at=NOW()-INTERVAL '1 minute' WHERE id=$1", &[&invocation]).await.unwrap();
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(chat.clone().worker(receiver));
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let state = chat.run(run_id).await.unwrap();
            if state.invocation_status == "failed" {
                assert_eq!(state.status, "failed");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    shutdown.send(true).unwrap();
    worker.await.unwrap();
    tilde::chat::audit::flush(&db.pool).await.unwrap();
    let count:i64=db.pool.get().await.unwrap().query_one("SELECT COUNT(*) FROM chat_activity WHERE entity_id=$1 AND kind='invocation.ended' AND text_delta='failed'", &[&invocation]).await.unwrap().get(0);
    assert_eq!(count, 1);
    let resumed = chat.resume_run(run_id).await.unwrap();
    assert_eq!(resumed.status, "active");
    assert_eq!(resumed.invocation_status, "pending");
    assert_ne!(resumed.invocation_id, run.invocation_id);
    db.close().await;
}
