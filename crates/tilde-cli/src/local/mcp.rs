//! The harness's MCP server: a stdio bridge to the agent's tools at the gateway. Each line the
//! harness writes is one JSON-RPC message, posted to the agent's MCP endpoint with the stored
//! key; the answer, if the message takes one, is written back as one line. The key never
//! enters the harness's own configuration.
use super::{Connection, Harness};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _};

pub async fn serve(harness: Harness) -> Result<()> {
    let connection = Connection::load(harness)?;
    let endpoint = format!("{}/mcp", connection.base());
    let bearer = connection.bearer()?;
    let http = reqwest::Client::builder()
        // A tool may run for minutes; the harness applies its own timeout.
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .context("Could not build an HTTP client")?;
    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let id = serde_json::from_str::<Value>(&line)
            .ok()
            .and_then(|message| message.get("id").cloned());
        let sent = http
            .post(&endpoint)
            .header(http::header::AUTHORIZATION, bearer.clone())
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(line)
            .send()
            .await;
        let answer = match sent {
            Ok(response) if response.status().is_success() => response.text().await.ok(),
            Ok(response) => Some(failure(
                &id,
                &format!("Tilde answered {}", response.status()),
            )),
            Err(_) => Some(failure(&id, "Tilde is unreachable")),
        };
        // A notification takes no answer, whatever happened to it.
        if let (Some(answer), Some(_)) = (answer.filter(|a| !a.trim().is_empty()), &id) {
            out.write_all(answer.trim().as_bytes()).await?;
            out.write_all(b"\n").await?;
            out.flush().await?;
        }
    }
    Ok(())
}

fn failure(id: &Option<Value>, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": message}}).to_string()
}
