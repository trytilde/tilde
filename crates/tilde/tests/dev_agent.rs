#![cfg(debug_assertions)]
mod common;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{sync::Arc, time::Duration};
use tilde::{
    agent::{Agents, health::AgentHealth},
    chat::Chat,
    connections::service::Connections,
    deployment::Deployments,
    encryption::Encryption,
};
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

/// `tilde dev-agent` registers the example and its Gateway deployment, then starts the
/// SDK process, which dials in to the gateway with the issued token. A restart reuses
/// both registrations and rotates the token.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dev_registration_is_reused_and_the_sdk_agent_dials_in_with_a_rotated_token() {
    let db = common::Database::new().await;
    let schema: String = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT current_schema()", &[])
        .await
        .unwrap()
        .get(0);
    let mut url = url::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(91))
            .await
            .unwrap(),
    );
    // The gateway the example dials: the same agent listener the engine serves.
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gateway = format!("http://{}", listener.local_addr().unwrap());
    let connections = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        gateway.clone(),
        gateway.clone(),
    )
    .unwrap();
    let deployments = Deployments::new(
        db.pool.clone(),
        crypto.clone(),
        agents.clone(),
        connections.clone(),
    );
    let chat = Chat::new(db.pool.clone(), crypto, gateway.clone())
        .with_connections(connections)
        .with_deployments(deployments);
    let router = tilde::iam::listeners::agent_rpc_router(agents, chat);
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let health = AgentHealth::new(db.pool.clone());
    let healthy = async || -> bool {
        health.poll_once().await.unwrap();
        db.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT healthy FROM agent_health ORDER BY checked_at DESC LIMIT 1",
                &[],
            )
            .await
            .unwrap()
            .get(0)
    };
    let mut original_token = None;
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
                .env("ENGINE_RUNTIME_PUBLIC_URL", &gateway)
                // Required at startup; dev-agent never reads telemetry, so none is running.
                .env("ENGINE_CLICKHOUSE_URL", "http://127.0.0.1:9")
                .env("ENGINE_LOGS_S3_BUCKET", "unused-logs")
                .env("ENGINE_TRACES_S3_BUCKET", "unused-traces")
                .env("ENGINE_METRICS_S3_BUCKET", "unused-metrics")
                .env("DEV_AGENTS", "vercel-ai")
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
            panic!("Example agent exited before dialling in");
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            while !healthy().await {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("the dialled-in example heartbeats ready");
        let tokens: Vec<Vec<u8>> = db
            .pool
            .get()
            .await
            .unwrap()
            .query(
                "SELECT d.token_hash FROM agents a JOIN agent_deployments d ON d.agent_id=a.id",
                &[],
            )
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.get(0))
            .collect();
        assert_eq!(
            tokens.len(),
            1,
            "Restarts reuse the agent and its deployment"
        );
        assert_ne!(
            original_token.as_ref(),
            Some(&tokens[0]),
            "Each start rotates the deployment token"
        );
        original_token = Some(tokens[0].clone());
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
        tokio::time::timeout(Duration::from_secs(10), async {
            while healthy().await {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("a stopped example no longer serves");
    }
    server.abort();
    db.close().await;
}
