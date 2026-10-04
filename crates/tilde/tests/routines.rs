//! Routines across their boundaries: a signed GitHub webhook becomes a signal that starts the
//! routine's run once per delivery, and a due cron routine is claimed, rescheduled and run.
mod common;
use hmac::{Hmac, Mac};
use secrecy::SecretString;
use serde_json::json;
use sha2::Sha256;
use std::sync::Arc;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::service::Connections,
    encryption::{Encryption, SecretBinding},
    routines::{RoutineInput, Routines, Trigger},
};
use uuid::Uuid;

#[tokio::test]
async fn signals_and_schedules_start_runs_in_new_threads() {
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(41))
            .await
            .unwrap(),
    );
    let agent = Agents::new(db.pool.clone(), crypto.clone())
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id: Uuid::new_v4(),
            name: "Triage".into(),
            capabilities: Default::default(),
        })
        .await
        .unwrap()
        .id;
    let connections = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into())
        .with_connections(connections.clone());
    let routines = Routines::new(db.pool.clone(), chat.clone());

    // A GitHub App connection used only for signals: no agent owns its chat.
    let connection = connections
        .start(Uuid::new_v4(), "acme", "github", "github_app", &[])
        .await
        .unwrap()
        .connection
        .id;
    for (key, value) in [
        ("installation_id", "1"),
        ("webhook_secret", "fixture-signing"),
    ] {
        let sealed = crypto
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
        db.pool
            .get()
            .await
            .unwrap()
            .execute(
                "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)",
                &[&connection, &key, &sealed],
            )
            .await
            .unwrap();
    }
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE connections SET status='ready' WHERE id=$1",
            &[&connection],
        )
        .await
        .unwrap();

    let signal = |signal_type: &str| Trigger::Signal {
        connection_id: connection,
        signal_type: signal_type.into(),
    };
    assert!(
        routines
            .create(
                agent,
                RoutineInput {
                    name: "Unknown".into(),
                    prompt: "Never".into(),
                    thread_title: String::new(),
                    enabled: true,
                    trigger: signal("github.push"),
                },
            )
            .await
            .is_err()
    );
    let triage = routines
        .create(
            agent,
            RoutineInput {
                name: "Triage new issues".into(),
                prompt: "Label {{ issue.title }} ({{ summary }}).".into(),
                thread_title: "{{ repository.full_name }}#{{ issue.number }}".into(),
                enabled: true,
                trigger: signal("github.issue.opened"),
            },
        )
        .await
        .unwrap();
    // A disabled routine on the same signal never fires.
    routines
        .create(
            agent,
            RoutineInput {
                name: "Paused".into(),
                prompt: "Never".into(),
                thread_title: String::new(),
                enabled: false,
                trigger: signal("github.issue.opened"),
            },
        )
        .await
        .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(
        axum::serve(
            listener,
            tilde::iam::listeners::public_event_ingress_router(chat.clone(), connections.clone()),
        )
        .into_future(),
    );
    let http = reqwest::Client::new();
    let deliver = |delivery: &'static str, sender_type: &'static str| {
        let body = serde_json::to_vec(&json!({
            "action": "opened",
            "installation": {"id": 1},
            "repository": {"full_name": "acme/app"},
            "sender": {"id": 7, "login": "octocat", "type": sender_type},
            "issue": {"number": 12, "title": "Login fails", "state": "open",
                "html_url": "https://github.com/acme/app/issues/12", "body": "Steps...", "labels": [{"name": "bug"}]},
        }))
        .unwrap();
        let mut mac = Hmac::<Sha256>::new_from_slice(b"fixture-signing").unwrap();
        mac.update(&body);
        http.post(format!("{origin}/connections/webhooks/{connection}"))
            .header("x-github-event", "issues")
            .header("x-github-delivery", delivery)
            .header(
                "x-hub-signature-256",
                format!("sha256={}", hex::encode(mac.finalize().into_bytes())),
            )
            .body(body)
            .send()
    };
    // The same delivery twice, then a bot's delivery: one run.
    for (delivery, sender) in [("d-1", "User"), ("d-1", "User"), ("d-2", "Bot")] {
        assert_eq!(deliver(delivery, sender).await.unwrap().status(), 204);
    }
    let fired = routines.get(triage.id).await.unwrap();
    assert_eq!(fired.last_error, None);
    let thread = fired.last_thread_id.expect("signal started a run");
    let runs = |sql: &'static str| {
        let pool = db.pool.clone();
        async move {
            pool.get()
                .await
                .unwrap()
                .query(sql, &[&agent])
                .await
                .unwrap()
        }
    };
    let signalled = runs(
        "SELECT r.thread_id,r.objective,t.title FROM chat_runs r JOIN chat_threads t ON t.id=r.thread_id WHERE r.agent_id=$1",
    )
    .await;
    assert_eq!(signalled.len(), 1);
    assert_eq!(signalled[0].get::<_, Uuid>(0), thread);
    let objective: String = signalled[0].get(1);
    assert!(
        objective
            .starts_with("Label Login fails (GitHub issue opened: acme/app#12 - Login fails)."),
        "{objective}"
    );
    assert!(objective.contains("`github.issue.opened`") && objective.contains("\"bug\""));
    assert_eq!(signalled[0].get::<_, String>(2), "acme/app#12");
    server.abort();

    // A due cron routine runs once and moves to its next time.
    assert!(
        routines
            .create(
                agent,
                RoutineInput {
                    name: "Bad".into(),
                    prompt: "Never".into(),
                    thread_title: String::new(),
                    enabled: true,
                    trigger: Trigger::Cron {
                        schedule: "* * * *".into()
                    },
                },
            )
            .await
            .is_err()
    );
    let digest = routines
        .create(
            agent,
            RoutineInput {
                name: "Daily digest".into(),
                prompt: "Summarise yesterday's activity.".into(),
                thread_title: String::new(),
                enabled: true,
                trigger: Trigger::Cron {
                    schedule: "0 9 * * 1-5".into(),
                },
            },
        )
        .await
        .unwrap();
    assert!(digest.next_run_at.unwrap() > chrono::Utc::now());
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE routines SET next_run_at=NOW()-INTERVAL '1 minute' WHERE id=$1",
            &[&digest.id],
        )
        .await
        .unwrap();
    routines.tick().await.unwrap();
    routines.tick().await.unwrap();
    let ran = routines.get(digest.id).await.unwrap();
    assert!(ran.next_run_at.unwrap() > chrono::Utc::now());
    assert!(ran.last_thread_id.is_some() && ran.last_error.is_none());
    let objectives = runs("SELECT objective FROM chat_runs WHERE agent_id=$1 ORDER BY objective")
        .await
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    assert_eq!(objectives.len(), 2);
    assert_eq!(objectives[1], "Summarise yesterday's activity.");

    // Disabling clears the due time; deleting the connection deletes its routines.
    let paused = routines
        .update(
            digest.id,
            RoutineInput {
                name: "Daily digest".into(),
                prompt: "Summarise yesterday's activity.".into(),
                thread_title: String::new(),
                enabled: false,
                trigger: Trigger::Cron {
                    schedule: "0 9 * * 1-5".into(),
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(paused.next_run_at, None);
    db.pool
        .get()
        .await
        .unwrap()
        .execute("DELETE FROM connections WHERE id=$1", &[&connection])
        .await
        .unwrap();
    assert_eq!(routines.list(agent).await.unwrap().len(), 1);
    db.close().await;
}

/// Sentry has no chat adapter: its issue webhooks are authenticated by the signals module with
/// the integration's client secret, and a connection without one refuses deliveries.
#[tokio::test]
async fn sentry_issue_webhooks_are_signals_verified_by_client_secret() {
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(42))
            .await
            .unwrap(),
    );
    let agent = Agents::new(db.pool.clone(), crypto.clone())
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id: Uuid::new_v4(),
            name: "Oncall".into(),
            capabilities: Default::default(),
        })
        .await
        .unwrap()
        .id;
    let connections = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into())
        .with_connections(connections.clone());
    let routines = Routines::new(db.pool.clone(), chat.clone());
    let connection = connections
        .start(Uuid::new_v4(), "acme", "sentry", "token", &[])
        .await
        .unwrap()
        .connection
        .id;
    let client = db.pool.get().await.unwrap();
    for (key, value) in [
        ("auth_token", "sntrys_x"),
        ("client_secret", "integration-secret"),
    ] {
        let sealed = crypto
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
        client
            .execute(
                "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)",
                &[&connection, &key, &sealed],
            )
            .await
            .unwrap();
    }
    client
        .execute(
            "UPDATE connections SET status='ready' WHERE id=$1",
            &[&connection],
        )
        .await
        .unwrap();
    drop(client);
    let routine = routines
        .create(
            agent,
            RoutineInput {
                name: "Investigate new issues".into(),
                prompt: "Investigate {{ data.issue.shortId }}.".into(),
                thread_title: "{{ data.issue.shortId }} {{ data.issue.title }}".into(),
                enabled: true,
                trigger: Trigger::Signal {
                    connection_id: connection,
                    signal_type: "sentry.issue.created".into(),
                },
            },
        )
        .await
        .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(
        axum::serve(
            listener,
            tilde::iam::listeners::public_event_ingress_router(chat.clone(), connections.clone()),
        )
        .into_future(),
    );
    let body = serde_json::to_vec(&json!({
        "action": "created",
        "data": {"issue": {"id": "123456", "shortId": "API-123", "title": "request failed"}},
    }))
    .unwrap();
    let sign = |key: &[u8]| {
        let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
        mac.update(&body);
        hex::encode(mac.finalize().into_bytes())
    };
    let http = reqwest::Client::new();
    let post = |signature: Option<String>| {
        let mut request = http
            .post(format!("{origin}/connections/webhooks/{connection}"))
            .header("sentry-hook-resource", "issue")
            .header("request-id", "req-1")
            .body(body.clone());
        if let Some(signature) = signature {
            request = request.header("sentry-hook-signature", signature);
        }
        request.send()
    };
    assert_eq!(post(None).await.unwrap().status(), 400);
    assert_eq!(post(Some(sign(b"wrong"))).await.unwrap().status(), 400);
    assert!(
        routines
            .get(routine.id)
            .await
            .unwrap()
            .last_run_at
            .is_none()
    );
    assert_eq!(
        post(Some(sign(b"integration-secret")))
            .await
            .unwrap()
            .status(),
        204
    );
    let fired = routines.get(routine.id).await.unwrap();
    assert_eq!(fired.last_error, None);
    let row = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT r.objective,t.title FROM chat_runs r JOIN chat_threads t ON t.id=r.thread_id WHERE t.id=$1",
            &[&fired.last_thread_id.unwrap()],
        )
        .await
        .unwrap();
    assert!(row.get::<_, String>(0).starts_with("Investigate API-123."));
    assert_eq!(row.get::<_, String>(1), "API-123 request failed");
    server.abort();
    db.close().await;
}
