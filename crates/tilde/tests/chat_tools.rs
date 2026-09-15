use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
#[allow(
    dead_code,
    reason = "The shared fixture also provides crypto-only helpers."
)]
mod common;
use futures::{StreamExt, future::BoxFuture};
use secrecy::ExposeSecret;
use secrecy::SecretString;
use serde_json::{Value, json};
use std::sync::Arc;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{
        Chat, Scope,
        tools::{Context, InputStream, Provider, Registry, ToolResult},
    },
    encryption::Encryption,
};
use uuid::Uuid;

/// A provider with its own sendMessage format; no sendMessage trait method exists.
struct MarkdownProvider {
    thread: Uuid,
}
impl Provider for MarkdownProvider {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            if scope.thread_id != self.thread {
                return Ok(vec![]);
            }
            Ok(vec![types::ToolDefinition{name:"sendMessage".into(),provider_id:"fixture-markdown".into(),description:"Send provider markdown with a heading.".into(),input_schema_json:json!({"type":"object","properties":{"heading":{"type":"string"}},"required":["heading"],"additionalProperties":false}).to_string(),chunk_schema_json:json!({"type":"object","properties":{"markdown":{"type":"string"}},"required":["markdown"],"additionalProperties":false}).to_string(),..Default::default()},types::ToolDefinition{name:"inspect".into(),provider_id:"fixture-markdown".into(),description:"Inspect provider formats without sending anything.".into(),input_schema_json:json!({"type":"object","additionalProperties":false}).to_string(),..Default::default()}])
        })
    }
    fn invoke<'a>(
        &'a self,
        ctx: Context,
        name: &'a str,
        input: Value,
        mut chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            if name == "inspect" {
                return Ok(json!({"formats":["markdown"]}));
            }
            assert_eq!(name, "sendMessage");
            let heading = input
                .get("heading")
                .and_then(Value::as_str)
                .ok_or_else(|| connectrpc::ConnectError::invalid_argument("Heading is required"))?;
            let mut events = ctx.begin_message(ctx.call_id, &[], None).await?;
            events.append(&format!("# {heading}\n")).await?;
            while let Some(chunk) = chunks.next().await {
                let chunk = chunk?;
                let markdown = chunk
                    .get("markdown")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        connectrpc::ConnectError::invalid_argument("Markdown is required")
                    })?;
                events.append(markdown).await?;
            }
            let message = events.finish().await?;
            Ok(json!({"format":"markdown","messageId":message.id}))
        })
    }
}
fn framed(values: Vec<Value>) -> Vec<u8> {
    let mut out = vec![];
    for value in values {
        let bytes = serde_json::to_vec(&value).unwrap();
        out.push(0);
        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        out.extend(bytes);
    }
    out
}
fn responses(mut bytes: &[u8]) -> Vec<Value> {
    let mut out = vec![];
    while !bytes.is_empty() {
        assert!(bytes.len() >= 5);
        let len = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
        out.push(serde_json::from_slice(&bytes[5..5 + len]).unwrap());
        bytes = &bytes[5 + len..];
    }
    out
}

#[tokio::test]
async fn scoped_catalog_dispatches_provider_formats_and_publishes_messages() {
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(13))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let agent = agents
        .create(CreateAgent {
            concurrency_policy: Default::default(),
            capabilities: tilde::iam::capabilities::Capabilities(std::collections::BTreeMap::from(
                [(
                    tilde::iam::capabilities::Capability::ToolsInvoke,
                    tilde::iam::capabilities::Reach::All,
                )],
            )),
            id: Uuid::new_v4(),
            name: "Tool fixture".into(),
            endpoint_url: "http://127.0.0.1:9999".into(),
            webhook_signing_key: SecretString::from("fixture-signing-key-with-at-least-32-bytes"),
        })
        .await
        .unwrap();
    let chat = Chat::new(db.pool.clone(), crypto, "http://127.0.0.1".into());
    let user = chat.create_user("User").await.unwrap();
    let thread = chat
        .create_thread(application::CreateThread {
            title: "Custom provider".into(),
            primary_agent_id: agent.id.to_string(),
            participants: vec![
                types::ParticipantRef {
                    agent_id: Some(agent.id.to_string()),
                    ..Default::default()
                },
                types::ParticipantRef {
                    user_id: Some(user.id),
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
            objective: "Exercise provider tools".into(),
            idempotency_key: "fixture".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    sqlx::query("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1").bind(invocation).execute(&db.pool).await.unwrap();
    let token = chat
        .tokens
        .issue(
            agent.id,
            invocation,
            Uuid::parse_str(&thread.id).unwrap(),
            Uuid::parse_str(&run.id).unwrap(),
        )
        .await
        .unwrap();
    let capability = token.expose_secret();
    let thread_id = Uuid::parse_str(&thread.id).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = tilde::chat::rpc::runtime::router_with_tools(
        chat.clone(),
        Registry::new(vec![Arc::new(MarkdownProvider { thread: thread_id })]),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let http = reqwest::Client::new();
    let endpoint = format!("{origin}/tilde.runtime.v1.ChatService");
    let denied = http
        .post(format!("{endpoint}/ListTools"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);
    let catalog: Value = http
        .post(format!("{endpoint}/ListTools"))
        .bearer_auth(capability)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(catalog["tools"][0]["providerId"], "fixture-markdown");
    assert!(
        catalog["tools"][0]["inputSchemaJson"]
            .as_str()
            .unwrap()
            .contains("heading")
    );
    let call = Uuid::new_v4().to_string();
    let body = framed(vec![
        json!({"name":"sendMessage","callId":call,"sequence":0,"inputJson":"{\"heading\":\"Provider heading\"}"}),
        json!({"name":"sendMessage","callId":call,"sequence":1,"chunkJson":"{\"markdown\":\"**Provider body**\"}"}),
        json!({"name":"sendMessage","callId":call,"sequence":2,"finish":true}),
    ]);
    let response = http
        .post(format!("{endpoint}/InvokeTool"))
        .bearer_auth(capability)
        .header("content-type", "application/connect+json")
        .header("connect-protocol-version", "1")
        .body(body)
        .send()
        .await
        .unwrap();
    let result = responses(&response.bytes().await.unwrap());
    assert!(result.last().unwrap().get("error").is_none(), "{result:?}");
    let output: Value = serde_json::from_str(result[0]["outputJson"].as_str().unwrap()).unwrap();
    assert_eq!(output["format"], "markdown");
    let message = chat.message(Uuid::parse_str(&call).unwrap()).await.unwrap();
    assert_eq!(message.text, "# Provider heading\n**Provider body**");
    assert_eq!(message.thread_id, thread.id);
    let kinds: Vec<String> =
        sqlx::query_scalar("SELECT kind FROM chat_activity WHERE entity_id=$1 ORDER BY sequence")
            .bind(Uuid::parse_str(&call).unwrap())
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        kinds,
        vec![
            "tool.started",
            "message.started",
            "message.delta",
            "tool.input.delta",
            "message.delta",
            "message.completed",
            "tool.completed"
        ]
    );
    let inspection = http.post(format!("{endpoint}/InvokeTool")).bearer_auth(capability)
        .header("content-type", "application/connect+json").header("connect-protocol-version", "1")
        .body(framed(vec![json!({"name":"inspect","callId":Uuid::new_v4().to_string(),"sequence":0,"inputJson":"{}","finish":true})]))
        .send().await.unwrap();
    let inspection = responses(&inspection.bytes().await.unwrap());
    assert!(inspection.last().unwrap().get("error").is_none());
    let value: Value = serde_json::from_str(inspection[0]["outputJson"].as_str().unwrap()).unwrap();
    assert_eq!(value["formats"], json!(["markdown"]));
    assert_eq!(
        chat.messages(thread_id, 10).await.unwrap().len(),
        1,
        "Tool results must not implicitly publish messages"
    );
    // A provider may expose no tools for another invocation scope; a send method is not mandatory.
    let mut scope = chat.scope(capability).await.unwrap();
    scope.thread_id = Uuid::new_v4();
    let registry = Registry::new(vec![Arc::new(MarkdownProvider { thread: thread_id })]);
    assert!(registry.list(&scope).await.unwrap().is_empty());
    sqlx::query("UPDATE chat_invocations SET status='canceled' WHERE id=$1")
        .bind(invocation)
        .execute(&db.pool)
        .await
        .unwrap();
    let denied = http
        .post(format!("{endpoint}/ListTools"))
        .bearer_auth(capability)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);
    server.abort();
    db.close().await;
}
