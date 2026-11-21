//! The harness's MCP server: a stdio bridge to the agent's tools at the gateway. Each line the
//! harness writes is one JSON-RPC message, posted to the agent's MCP endpoint with the stored
//! key; the answer, if the message takes one, is written back as one line. The key never
//! enters the harness's own configuration.
//!
//! Messages are forwarded as they arrive, each on its own task, so a slow tool call holds up
//! neither a ping nor the calls a harness makes beside it; answers are written whole, one at a
//! time, in the order they finish.
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
    let (answers, mut finished) = tokio::sync::mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        let mut out = tokio::io::stdout();
        while let Some(answer) = finished.recv().await {
            if out
                .write_all(format!("{answer}\n").as_bytes())
                .await
                .is_err()
                || out.flush().await.is_err()
            {
                break;
            }
        }
    });
    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request = http
            .post(&endpoint)
            .header(http::header::AUTHORIZATION, bearer.clone())
            .header(http::header::CONTENT_TYPE, "application/json");
        let answers = answers.clone();
        tokio::spawn(async move {
            if let Some(answer) = forward(request, line).await {
                let _ = answers.send(answer);
            }
        });
    }
    // The harness closed stdin: the writer ends once every call in flight has answered.
    drop(answers);
    let _ = writer.await;
    Ok(())
}

/// Forward one message and return the gateway's answer. A request (it has an id) is always
/// answered, with an error when the gateway gave none; a notification is sent and never is.
async fn forward(request: reqwest::RequestBuilder, line: String) -> Option<String> {
    let id = serde_json::from_str::<Value>(&line)
        .ok()
        .and_then(|message| message.get("id").cloned());
    let sent = request.body(line).send().await;
    let id = id?;
    let failure = |message: String| {
        json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": message}})
            .to_string()
    };
    Some(match sent {
        Ok(response) if response.status().is_success() => match response.text().await {
            Ok(answer) if !answer.trim().is_empty() => answer.trim().to_owned(),
            _ => failure("Tilde gave no answer".into()),
        },
        Ok(response) => failure(format!("Tilde answered {}", response.status())),
        Err(_) => failure("Tilde is unreachable".into()),
    })
}
