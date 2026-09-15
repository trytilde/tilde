#![cfg(debug_assertions)]
#[expect(dead_code, reason = "shared fixture helpers")]
mod common;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{sync::Arc, time::Duration};
use tilde::{agent::health::AgentHealth, encryption::Encryption};
use tokio::io::{AsyncBufReadExt, BufReader};

struct Process(tokio::process::Child);
impl Drop for Process {
    fn drop(&mut self) {
        if let Some(id) = self.0.id() {
            // Isolated process group: clean up the SDK child even if an assertion panics.
            let _ = std::process::Command::new("kill")
                .args(["-TERM", "--", &format!("-{id}")])
                .status();
        }
    }
}

#[tokio::test]
async fn dev_registration_reuses_encrypted_key_and_hosts_a_signed_sdk_agent() {
    let db = common::Database::new().await;
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let mut url = url::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let encryption = Arc::new(
        Encryption::initialize(&db.pool, common::seed(91))
            .await
            .unwrap(),
    );
    let health = AgentHealth::new(db.pool.clone(), encryption);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut original_key = None;
    for _ in 0..2 {
        let mut child = Process(
            tokio::process::Command::new(env!("CARGO_BIN_EXE_tilde"))
                .arg("dev-agent")
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap())
                .env("DATABASE_URL", url.as_str())
                .env("ENGINE_ENCRYPTION_KEY", STANDARD.encode([91u8; 32]))
                .env("ENGINE_SERVE", "runtime")
                .env("ENGINE_WEB_ENABLED", "false")
                .env("DEV_AGENT_PORT", port.to_string())
                .env("OPENAI_API_KEY", "fixture-unused-key")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .process_group(0)
                .spawn()
                .unwrap(),
        );
        let mut lines = BufReader::new(child.0.stdout.take().unwrap()).lines();
        tokio::time::timeout(Duration::from_secs(20), async {
            while let Some(line) = lines.next_line().await.unwrap() {
                assert!(!line.contains("fixture-unused-key"));
                if line.contains("Example Agent 1 is ready") {
                    return;
                }
            }
            panic!("Example agent exited before listening");
        })
        .await
        .unwrap();
        health.poll_once().await.unwrap();
        let ready: bool =
            sqlx::query_scalar("SELECT healthy FROM agent_health ORDER BY checked_at DESC LIMIT 1")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert!(
            ready,
            "The SDK host accepted the registry's signed Healthz request"
        );
        let keys: Vec<Vec<u8>> = sqlx::query_scalar("SELECT webhook_signing_key FROM agents")
            .fetch_all(&db.pool)
            .await
            .unwrap();
        assert_eq!(keys.len(), 1, "Restarts reuse the agent registration");
        if let Some(original) = &original_key {
            assert!(
                original == &keys[0],
                "Signing ciphertext is retained across restarts"
            );
        }
        original_key = Some(keys[0].clone());
        std::process::Command::new("kill")
            .args(["-TERM", &child.0.id().unwrap().to_string()])
            .status()
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(10), child.0.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
    }
    db.close().await;
}
