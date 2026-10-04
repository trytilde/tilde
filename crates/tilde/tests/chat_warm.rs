//! The gateway's warm cache across its boundaries: priming an invocation fills the channel
//! catalog and decrypted credentials from Postgres, the wake's history page is served back to
//! the invocation, local writes and configuration changes (through Postgres notifications)
//! invalidate, and the invocation's end evicts.
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::{model::*, service::Connections},
    encryption::{Encryption, SecretBinding},
};
use uuid::Uuid;

async fn eventually(mut probe: impl AsyncFnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    while !probe().await {
        assert!(tokio::time::Instant::now() < deadline, "timed out");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn priming_serves_tools_credentials_and_history_from_memory_until_invalidated() {
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(11)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
        "http://127.0.0.1:2".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let chat = Chat::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
    )
    .with_connections(connections.clone());
    let (stop, shutdown) = tokio::sync::watch::channel(false);
    let connection_worker = tokio::spawn(connections.clone().worker(shutdown.clone()));
    let warm_worker = tokio::spawn(chat.warm.clone().worker(db.pool.clone(), shutdown));
    let agent = Uuid::new_v4();
    agents
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id: agent,
            name: "Warm".into(),
            capabilities: tilde::iam::capabilities::Capabilities::from_wire(types::Capabilities {
                tools_invoke: types::TargetPermission {
                    mode: types::TargetSelection::All.into(),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .unwrap(),
        })
        .await
        .unwrap();
    // A ready AgentMail channel connection, seeded directly as the broker would leave it.
    let connection = Uuid::new_v4();
    connections
        .start(
            connection,
            "inbox",
            "agentmail",
            "inbox",
            &[Assignment {
                capability: Capability::Channel,
                agent_id: agent,
                alias: None,
            }],
        )
        .await
        .unwrap();
    let mut values = vec![];
    for (key, value) in [
        ("inbox_id", "warm@agentmail.to"),
        ("api_key", "am-key"),
        ("webhook_secret", "whs"),
    ] {
        let sealed = encryption
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: connection,
                    name: key,
                },
                &SecretString::from(value),
            )
            .unwrap()
            .into_bytes();
        values.push((key, sealed));
    }
    let store = |values: Vec<(&'static str, Vec<u8>)>| {
        let pool = db.pool.clone();
        async move {
            for (key, sealed) in values {
                pool.get().await.unwrap().execute("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)", &[&connection, &key, &sealed]).await.unwrap();
            }
        }
    };
    store(values.clone()).await;
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE connections SET status='ready', account_label='warm@agentmail.to' WHERE id=$1",
            &[&connection],
        )
        .await
        .unwrap();
    // Give the notification from that update time to land before measuring cache state.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let thread = chat
        .create_thread(application::CreateThread {
            title: "Warm".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(agent.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let run = chat
        .start_run(application::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "warm".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let thread = Uuid::parse_str(&thread.id).unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    db.pool.get().await.unwrap().execute("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1", &[&invocation]).await.unwrap();
    assert!(chat.warm.channels(agent, thread).is_none());
    // Prime as the claim does. The run's own activity notification can land after priming and
    // invalidate it, later under load, so prime until the catalog stays primed.
    let rows = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            chat.clone().prime(agent, thread).await;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if let Some(rows) = chat.warm.channels(agent, thread) {
                break rows;
            }
        }
    })
    .await
    .expect("catalog primed");
    assert!(rows.iter().any(|r| r.id == Some(connection)));

    // Credentials were decrypted by priming: the ciphertext can go and resolve still answers.
    // Notifications are database-wide, so another test's connection change can clear the cache
    // between priming and the check; then the ciphertext is restored and the round repeated.
    let values = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            chat.clone().prime(agent, thread).await;
            db.pool
                .get()
                .await
                .unwrap()
                .execute(
                    "DELETE FROM connection_values WHERE connection_id=$1",
                    &[&connection],
                )
                .await
                .unwrap();
            let resolved = connections.resolve(connection).await.unwrap();
            if resolved.contains_key("api_key") {
                break resolved;
            }
            store(values.clone()).await;
        }
    })
    .await
    .expect("credentials primed");
    assert_eq!(values["api_key"].expose_secret(), "am-key");
    connections.forget_credentials();
    assert!(
        connections.resolve(connection).await.unwrap().is_empty(),
        "nothing left to decrypt"
    );

    // The wake's first page is served back to that invocation; a reply from this process
    // forgets it, and the fresh read includes the reply.
    let page = chat
        .invocation_message_page(thread, None, 100, Some(invocation))
        .await
        .unwrap();
    assert!(chat.warm.history(thread, invocation).is_some());
    let again = chat
        .invocation_message_page(thread, None, 100, Some(invocation))
        .await
        .unwrap();
    assert_eq!(again.messages.len(), page.messages.len());
    let scope = chat
        .tokens
        .scope(
            chat.tokens
                .issue(agent, invocation, thread, Uuid::parse_str(&run.id).unwrap())
                .await
                .unwrap()
                .expose_secret(),
        )
        .await
        .unwrap();
    let registry = tilde::chat::tools::Registry::for_chat(&chat);
    let tools = registry.list(&scope).await.unwrap();
    assert!(tools.iter().any(|t| t.name == "sendMessage"));
    assert!(
        tools.iter().any(|t| t.name.starts_with("channel_")),
        "channel tools from the primed catalog"
    );
    chat.warm.forget_history(thread);
    assert!(chat.warm.history(thread, invocation).is_none());

    // Unassigning fires the configuration notification; the listener empties the catalog.
    connections
        .unassign(
            connection,
            &Assignment {
                capability: Capability::Channel,
                agent_id: agent,
                alias: None,
            },
        )
        .await
        .unwrap();
    eventually(async || chat.warm.channels(agent, thread).is_none()).await;
    // Re-read: the catalog no longer offers the connection's tools.
    let tools = registry.list(&scope).await.unwrap();
    assert!(!tools.iter().any(|t| t.name.starts_with("channel_")));
    chat.warm.forget_invocation(agent, thread, invocation);
    assert!(chat.warm.channels(agent, thread).is_none());

    let _ = stop.send(true);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), connection_worker).await;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), warm_worker).await;
    db.close().await;
}
