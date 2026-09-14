use tilde::chat as application;
use tilde::proto::tilde::management::v1 as management;
use tilde::proto::tilde::types::v1 as types;
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tilde::{
    agent::{Agents, CreateAgent, UpdateAgent},
    encryption::{Encryption, SealedSecret},
    error::Error,
};
use uuid::Uuid;

fn input(id: Uuid, name: &str) -> CreateAgent {
    CreateAgent {
        capabilities: Default::default(),
        id,
        name: name.into(),
        webhook_signing_key: SecretString::from(KEY),
        endpoint_url: "http://127.0.0.1:3000/agent".into(),
    }
}
const KEY: &str = "tilde_whsec_0123456789abcdef0123456789abcdef";

async fn agents(db: &Database) -> Agents {
    Agents::new(
        db.pool.clone(),
        Arc::new(Encryption::initialize(&db.pool, seed(7)).await.unwrap()),
    )
}

#[tokio::test]
async fn central_migrations_and_agent_lifecycle() {
    let db = Database::new().await;
    tilde::database::migrate(&db.pool).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        count,
        sqlx::migrate!("../../migrations").iter().count() as i64
    );
    let service = agents(&db).await;
    let id = Uuid::new_v4();
    let created = service.create(input(id, "Ada")).await.unwrap();
    let replay = service.create(input(id, "Ada")).await.unwrap();
    assert_eq!(created.created_at, replay.created_at);
    assert!(matches!(
        service.create(input(id, "Different")).await,
        Err(Error::Conflict)
    ));
    let mut different_key = input(id, "Ada");
    different_key.webhook_signing_key =
        SecretString::from("a-different-caller-generated-signing-key");
    assert!(matches!(
        service.create(different_key).await,
        Err(Error::Conflict)
    ));
    let updated = service
        .update(UpdateAgent {
            capabilities: None,
            id,
            name: Some("Grace".into()),
            endpoint_url: Some("https://grace.example.com".into()),
        })
        .await
        .unwrap();
    assert_eq!(updated.name, "Grace");
    assert_eq!(
        updated.endpoint_url.as_deref(),
        Some("https://grace.example.com/")
    );
    assert!(matches!(
        service
            .update(UpdateAgent {
                capabilities: None,
                id,
                name: None,
                endpoint_url: Some(String::new())
            })
            .await,
        Err(Error::Invalid(_))
    ));
    let untouched = service
        .update(UpdateAgent {
            capabilities: None,
            id,
            name: None,
            endpoint_url: None,
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
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agents WHERE deleted_at IS NULL")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let old_table: Option<String> = sqlx::query_scalar("SELECT to_regclass('agent_secrets')::text")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(old_table.is_none());
    db.close().await;
}

#[tokio::test]
async fn signing_keys_are_encrypted_persistent_and_bound_to_the_agent_column() {
    let db = Database::new().await;
    let service = agents(&db).await;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    service.create(input(first, "A")).await.unwrap();
    service.create(input(second, "B")).await.unwrap();
    let stored: Vec<u8> = sqlx::query_scalar("SELECT webhook_signing_key FROM agents WHERE id=$1")
        .bind(first)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let other: Vec<u8> = sqlx::query_scalar("SELECT webhook_signing_key FROM agents WHERE id=$1")
        .bind(second)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(
        !stored
            .windows(KEY.len())
            .any(|window| window == KEY.as_bytes())
    );
    assert_ne!(
        &stored[18..30],
        &other[18..30],
        "fresh nonce for each encryption"
    );
    drop(service);
    let restarted = agents(&db).await;
    // A replay after restart decrypts the persisted column and verifies the original key.
    restarted.create(input(first, "A")).await.unwrap();
    let crypto = Encryption::initialize(&db.pool, seed(7)).await.unwrap();
    assert_eq!(
        crypto
            .open(
                common::binding(first, "webhook_signing_key"),
                SealedSecret::from_bytes(&stored).unwrap()
            )
            .unwrap()
            .expose_secret(),
        KEY
    );
    assert!(
        crypto
            .open(
                common::binding(first, "another_column"),
                SealedSecret::from_bytes(&stored).unwrap()
            )
            .is_err()
    );
    for truncated in [0, 1, 17, 29, 45] {
        assert!(SealedSecret::from_bytes(&stored[..truncated]).is_err());
    }
    for index in [0, 2, 18, 30] {
        let mut tampered = stored.clone();
        tampered[index] ^= 1;
        assert!(
            crypto
                .open(
                    common::binding(first, "webhook_signing_key"),
                    SealedSecret::from_bytes(&tampered).unwrap()
                )
                .is_err()
        );
    }
    sqlx::query("UPDATE agents SET webhook_signing_key=$1 WHERE id=$2")
        .bind(stored)
        .bind(second)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(matches!(
        restarted.create(input(second, "B")).await,
        Err(Error::Encryption)
    ));
    db.close().await;
}

#[tokio::test]
async fn wrong_seed_fails_without_overwriting_the_key() {
    let db = Database::new().await;
    let crypto = Encryption::initialize(&db.pool, seed(1)).await.unwrap();
    let before: Vec<u8> = sqlx::query_scalar("SELECT wrapped_key FROM encryption_keys")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(matches!(
        Encryption::initialize(&db.pool, seed(2)).await,
        Err(Error::Encryption)
    ));
    let after: Vec<u8> = sqlx::query_scalar("SELECT wrapped_key FROM encryption_keys")
        .fetch_one(&db.pool)
        .await
        .unwrap();
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
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM encryption_keys")
        .fetch_one(&db.pool)
        .await
        .unwrap();
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
    sqlx::query("UPDATE agents SET created_at='2026-01-01T00:00:00Z'")
        .execute(&db.pool)
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
    let search = service.list_filtered(1, "", "b").await.unwrap();
    assert_eq!(search.agents.len(), 1);
    assert_eq!(search.agents[0].name, "B");
    assert!(search.next_page_token.is_empty());
    assert!(service.create(input(Uuid::new_v4(), "   ")).await.is_err());
    let mut bad = input(Uuid::new_v4(), "Credentials in URL");
    bad.endpoint_url = "https://example.com/?token=secret".into();
    assert!(service.create(bad).await.is_err());
    let mut missing = input(Uuid::new_v4(), "Missing endpoint");
    missing.endpoint_url.clear();
    assert!(matches!(
        service.create(missing).await,
        Err(Error::Invalid(_))
    ));
    for key in [
        String::new(),
        "too-short".into(),
        " ".repeat(40),
        "é".repeat(40),
        "x".repeat(1025),
    ] {
        let mut bad = input(Uuid::new_v4(), "Invalid key");
        bad.webhook_signing_key = SecretString::from(key);
        assert!(matches!(service.create(bad).await, Err(Error::Invalid(_))));
    }
    db.close().await;
}

#[test]
fn generated_debug_output_redacts_secret_values() {
    let request = management::CreateAgentRequest {
        webhook_signing_key: "do-not-print-this".into(),
        ..Default::default()
    };
    let debug = format!("{request:?}");
    assert!(!debug.contains("do-not-print-this"));
    assert!(debug.contains("REDACTED"));
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
            capabilities: Default::default(),
            id: Uuid::new_v4(),
            name: "Recovery".into(),
            endpoint_url: "http://127.0.0.1:9999".into(),
            webhook_signing_key: SecretString::from("tilde_whsec_0123456789abcdef0123456789abcdef"),
        })
        .await
        .unwrap();
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
    sqlx::query("UPDATE chat_invocations SET status='running',started_at=NOW()-INTERVAL '2 minutes',lease_expires_at=NOW()-INTERVAL '1 minute' WHERE id=$1").bind(invocation).execute(&db.pool).await.unwrap();
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
    let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM chat_activity WHERE entity_id=$1 AND kind='invocation.ended' AND text_delta='failed'").bind(invocation).fetch_one(&db.pool).await.unwrap();
    assert_eq!(count, 1);
    let resumed = chat.resume_run(run_id).await.unwrap();
    assert_eq!(resumed.status, "active");
    assert_eq!(resumed.invocation_status, "pending");
    assert_ne!(resumed.invocation_id, run.invocation_id);
    db.close().await;
}
