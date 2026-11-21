//! A local agent over its ingress, as the `tilde local` hooks drive it: the harness reports a
//! session, which becomes one thread whose messages keep their own times and authors and whose
//! events are spans (a turn per trace) however often they are resent; the agent's tools are
//! served to it over MCP; only what the harness's adapter supports is served, and only to a
//! local agent's key.
mod common;
use common::invocation::Fixture;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use tilde::agent::CreateAgent;
use tilde::connections::catalog::Endpoints;
use tilde::local::Harness;
use uuid::Uuid;

async fn post(url: String, key: &str, body: Value) -> (u16, Value) {
    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
}
/// A local agent of `harness` and its application key.
async fn local_agent(fx: &Fixture, harness: Harness) -> (Uuid, SecretString) {
    let id = Uuid::new_v4();
    fx.within(async {
        fx.agents
            .create_local(
                CreateAgent {
                    description: String::new(),
                    concurrency_policy: Default::default(),
                    id,
                    name: "Local".into(),
                    capabilities: Default::default(),
                },
                harness,
            )
            .await
            .unwrap();
        (id, fx.chat.tilde_chat_key(id, false).await.unwrap())
    })
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_harness_reports_sessions_and_is_served_the_agents_tools() {
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tavily = format!("http://{}", upstream.local_addr().unwrap());
    let upstream_server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(async || axum::Json(json!({"key": {"usage": 7}})));
        axum::serve(upstream, app).await.unwrap()
    });
    let fx = Fixture::new(
        97,
        Endpoints([("tavily_api".to_owned(), tavily)].into()),
        Default::default(),
    )
    .await;
    let storage = common::Storage::load();
    let (config, database) = common::clickhouse(&storage).await;
    let traces = common::trace_delivery(&fx.db.pool, &storage, &config);
    let deployments = tilde::deployment::Deployments::new(
        fx.db.pool.clone(),
        fx.crypto.clone(),
        fx.agents.clone(),
        fx.connections.clone(),
    )
    .with_telemetry(traces.queue.clone());
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    let router = tilde::deployment::public::router(deployments, fx.chat.clone());
    let server = tokio::spawn(async move { axum::serve(socket, router).await.unwrap() });

    let (local, key) = local_agent(&fx, Harness::ClaudeCode).await;
    let key = key.expose_secret();
    let harnesses = fx
        .within(fx.agents.harnesses(&[local, fx.agent]))
        .await
        .unwrap();
    assert_eq!(harnesses.len(), 1, "a managed agent has no harness");
    assert_eq!(harnesses[&local], Harness::ClaudeCode);

    let ingest = async |agent: Uuid, key: &str, events: Value| {
        post(
            format!("{origin}/agents/{agent}/tilde.local.v1.LocalAgentService/Ingest"),
            key,
            json!({"sessionId": "session-1", "title": "Fix the build", "events": events}),
        )
        .await
    };
    let user = |id: &str, at: &str, text: &str| json!({"id": id, "turnId": id, "at": at, "user": {"text": text}});
    let agent = |id: &str, turn: &str, at: &str, text: &str| json!({"id": id, "turnId": turn, "at": at, "agent": {"text": text, "model": "claude-opus-5-5"}});
    let first = json!([
        user("u1", "2026-11-21T10:00:00Z", "Fix the build"),
        {"id": "t1", "turnId": "u1", "at": "2026-11-21T10:00:05Z",
            "tool": {"name": "Bash", "input": "{\"command\":\"cargo build\"}", "output": "error[E0308]", "failed": true,
                "startedAt": "2026-11-21T10:00:02Z"}},
        agent("a1", "u1", "2026-11-21T10:00:09Z", "A type was wrong; fixed."),
    ]);
    assert_eq!(ingest(local, key, first).await.0, 200);
    // The hook resends what it is unsure of, then goes on.
    let second = json!([
        agent(
            "a1",
            "u1",
            "2026-11-21T10:00:09Z",
            "A type was wrong; fixed."
        ),
        user("u2", "2026-11-21T10:05:00Z", "Thanks"),
        agent("a2", "u2", "2026-11-21T10:05:02Z", "Any time."),
    ]);
    assert_eq!(ingest(local, key, second).await.0, 200);

    let messages = fx
        .within(async {
            fx.db
                .pool
                .get()
                .await
                .unwrap()
                .query(
                    "SELECT m.text, p.user_id IS NOT NULL, to_char(m.created_at AT TIME ZONE 'UTC', 'HH24:MI:SS'), t.title
                     FROM chat_messages m JOIN chat_participants p ON p.id=m.participant_id
                     JOIN chat_threads t ON t.id=m.thread_id
                     WHERE t.primary_agent_id=$1 ORDER BY m.created_at",
                    &[&local],
                )
                .await
                .unwrap()
        })
        .await;
    let said: Vec<(String, bool, String, String)> = messages
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3)))
        .collect();
    let line = |text: &str, human: bool, at: &str| {
        (
            text.to_owned(),
            human,
            at.to_owned(),
            "Fix the build".to_owned(),
        )
    };
    assert_eq!(
        said,
        [
            line("Fix the build", true, "10:00:00"),
            line("A type was wrong; fixed.", false, "10:00:09"),
            line("Thanks", true, "10:05:00"),
            line("Any time.", false, "10:05:02"),
        ]
    );

    // Every event is one observation; a turn is one trace.
    let spans = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let rows: Value = reqwest::Client::new()
                .post(&storage.clickhouse_url)
                .basic_auth(
                    &storage.clickhouse_user,
                    Some(storage.clickhouse_password.0.expose_secret()),
                )
                .body(format!(
                    "SELECT SpanName AS name, ObservationType AS type, TraceId AS trace, Level AS level, Model AS model,
                        SpanAttributes['tilde.observation.input'] AS input, SpanAttributes['tilde.observation.output'] AS output
                     FROM {database}.otel_traces FINAL WHERE AgentId='{local}' ORDER BY Timestamp FORMAT JSON"
                ))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap_or(Value::Null);
            let rows = rows["data"].as_array().cloned().unwrap_or_default();
            if rows.len() >= 5 {
                break rows;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("the session's spans reach the span store");
    let summary: Vec<(&str, &str, &str, &str)> = spans
        .iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap(),
                s["type"].as_str().unwrap(),
                s["input"].as_str().unwrap(),
                s["output"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("claude_code prompt", "GENERATION", "Fix the build", ""),
            (
                "claude_code Bash",
                "TOOL",
                "{\"command\":\"cargo build\"}",
                "error[E0308]"
            ),
            (
                "claude_code response",
                "GENERATION",
                "",
                "A type was wrong; fixed."
            ),
            ("claude_code prompt", "GENERATION", "Thanks", ""),
            ("claude_code response", "GENERATION", "", "Any time."),
        ]
    );
    assert_eq!(spans[1]["level"], "ERROR");
    assert_eq!(spans[2]["model"], "claude-opus-5-5");
    assert_eq!(spans[0]["trace"], spans[2]["trace"]);
    assert_ne!(spans[0]["trace"], spans[3]["trace"]);

    // The agent's tools, over MCP: adding the source is the grant.
    let connection = Uuid::new_v4();
    fx.within(async {
        fx.connections
            .start(connection, "Search", "tavily", "api", &[])
            .await
            .unwrap();
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE connections SET status='ready' WHERE id=$1",
                &[&connection],
            )
            .await
            .unwrap();
        fx.connections
            .replace_value(connection, "api_key", &SecretString::from("key"))
            .await
            .unwrap();
        fx.chat
            .tools
            .as_ref()
            .unwrap()
            .add_source(
                tilde::tools::Owner::Agent(local),
                tilde::tools::Target::Connection(connection),
                &["usage".to_owned()],
            )
            .await
            .unwrap();
    })
    .await;
    let mcp = async |agent: Uuid, key: &str, method: &str, params: Value| {
        post(
            format!("{origin}/agents/{agent}/mcp"),
            key,
            json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params}),
        )
        .await
    };
    let (_, hello) = mcp(
        local,
        key,
        "initialize",
        json!({"protocolVersion": "2025-06-18"}),
    )
    .await;
    assert_eq!(hello["result"]["serverInfo"]["name"], "tilde", "{hello}");
    let (_, listed) = mcp(local, key, "tools/list", json!({})).await;
    let tools = listed["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1, "{listed}");
    let tool = tools[0]["name"].as_str().unwrap();
    assert!(
        tool.ends_with("_usage") && !tool.contains('.'),
        "catalog names are made MCP-safe: {tool}"
    );
    assert_eq!(tools[0]["inputSchema"]["type"], "object");
    let (_, called) = mcp(
        local,
        key,
        "tools/call",
        json!({"name": tool, "arguments": {}}),
    )
    .await;
    assert_eq!(called["id"], 7);
    assert_eq!(called["result"]["isError"], false, "{called}");
    assert!(
        called["result"]["structuredContent"].is_object(),
        "{called}"
    );
    let (_, missing) = mcp(
        local,
        key,
        "tools/call",
        json!({"name": "nope", "arguments": {}}),
    )
    .await;
    assert_eq!(missing["error"]["code"], -32602, "{missing}");

    // Tilde cannot start a local agent: nothing would serve the run.
    let thread: Uuid = fx
        .within(async {
            let client = fx.db.pool.get().await.unwrap();
            client
                .query_one(
                    "SELECT id FROM chat_threads WHERE primary_agent_id=$1",
                    &[&local],
                )
                .await
                .unwrap()
                .get(0)
        })
        .await;
    let started = fx
        .within(fx.chat.start_run(tilde::chat::StartRun {
            thread_id: thread.to_string(),
            agent_id: local.to_string(),
            objective: "carry on".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        }))
        .await;
    assert!(matches!(started, Err(tilde::chat::ChatError::Invalid(_))));

    // Another key, or a managed agent's own, opens none of it.
    let managed = fx.agent;
    let managed_key = fx
        .within(async { fx.chat.tilde_chat_key(managed, false).await.unwrap() })
        .await;
    assert_ne!(
        ingest(local, managed_key.expose_secret(), json!([]))
            .await
            .0,
        200
    );
    assert_ne!(
        ingest(managed, managed_key.expose_secret(), json!([]))
            .await
            .0,
        200
    );
    assert_ne!(
        mcp(
            managed,
            managed_key.expose_secret(),
            "tools/list",
            json!({})
        )
        .await
        .0,
        200
    );

    // Nothing is expected to serve a local agent, so the health sweep leaves it unsampled
    // (unknown) instead of recording it as down.
    let sampled: Vec<Uuid> = fx
        .within(async {
            tilde::agent::health::AgentHealth::new(fx.db.pool.clone())
                .poll_once()
                .await
                .unwrap();
            let client = fx.db.pool.get().await.unwrap();
            let rows = client
                .query("SELECT DISTINCT agent_id FROM agent_health", &[])
                .await
                .unwrap();
            rows.iter().map(|row| row.get(0)).collect()
        })
        .await;
    assert_eq!(sampled, [managed]);

    // A desktop app keeps its conversations in the vendor's account: tools only.
    let (desktop, desktop_key) = local_agent(&fx, Harness::ClaudeDesktop).await;
    let desktop_key = desktop_key.expose_secret();
    let (status, refused) = ingest(desktop, desktop_key, json!([])).await;
    assert_eq!(status, 400, "{refused}");
    let (status, listed) = mcp(desktop, desktop_key, "tools/list", json!({})).await;
    assert_eq!(status, 200);
    assert_eq!(listed["result"]["tools"], json!([]));

    server.abort();
    upstream_server.abort();
    drop(traces);
    fx.close().await;
}
