//! An agent's tools across their boundaries: the agent's own choice of a connection's tools, its
//! IAM grant, the invocation catalog over ConnectRPC, encrypted credential resolution, the
//! upstream provider call, the durable tool-call record and what end users see of it.
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::{catalog::Endpoints, service::Connections},
    encryption::{Encryption, SecretBinding},
    tools::{Filter, Target, ToolSettings, Tools},
};
use uuid::Uuid;

fn framed(value: Value) -> Vec<u8> {
    let bytes = serde_json::to_vec(&value).unwrap();
    let mut out = vec![0];
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend(bytes);
    out
}
fn responses(mut bytes: &[u8]) -> Vec<Value> {
    let mut out = vec![];
    while !bytes.is_empty() {
        let len = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
        out.push(serde_json::from_slice(&bytes[5..5 + len]).unwrap());
        bytes = &bytes[5 + len..];
    }
    out
}

#[tokio::test]
async fn an_agents_tools_reach_the_provider_with_connection_credentials() {
    let db = Database::new().await;
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(21)).await.unwrap());
    // Stands in for api.tavily.com and records what the provider sent.
    let seen: Arc<Mutex<Vec<(String, String, Value)>>> = Default::default();
    let sink = seen.clone();
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_origin = format!("http://{}", upstream.local_addr().unwrap());
    let upstream_server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(
            move |uri: axum::http::Uri, headers: axum::http::HeaderMap, body: axum::Json<Value>| {
                let sink = sink.clone();
                async move {
                    let auth = headers["authorization"].to_str().unwrap().to_owned();
                    sink.lock().unwrap().push((uri.path().into(), auth, body.0));
                    axum::Json(json!({"results":[{"url":"https://docs.example/tools"}]}))
                }
            },
        );
        axum::serve(upstream, app).await.unwrap()
    });
    let connections = Connections::with_endpoints(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1:1".into(),
        "http://127.0.0.1:2".into(),
        Endpoints(BTreeMap::from([("tavily_api".into(), upstream_origin)])),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1:1".into())
        .with_connections(connections.clone());
    let tools = Tools::new(connections.clone(), String::new());

    // A ready Tavily connection, seeded as the setup broker would leave it. Tools take no
    // connection assignment: the agent's own tool source is what grants them.
    let connection = Uuid::new_v4();
    connections
        .start(connection, "prod", "tavily", "api", &[])
        .await
        .unwrap();
    let sealed = crypto
        .seal(
            SecretBinding {
                resource_kind: "connection",
                resource_id: connection,
                name: "api_key",
            },
            &SecretString::from("tvly-fixture"),
        )
        .unwrap()
        .into_bytes();
    let pg = db.pool.get().await.unwrap();
    pg.execute("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,'api_key',$2)", &[&connection, &sealed]).await.unwrap();
    pg.execute(
        "UPDATE connections SET status='ready' WHERE id=$1",
        &[&connection],
    )
    .await
    .unwrap();

    // Granted one of the connection's tools by catalog name.
    let agent = Uuid::new_v4();
    Agents::new(db.pool.clone(), crypto.clone())
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id: agent,
            name: "Researcher".into(),
            capabilities: tilde::iam::capabilities::Capabilities::from_wire(types::Capabilities {
                tools_invoke: types::TargetPermission {
                    mode: types::TargetSelection::Selected.into(),
                    ids: vec!["tavily_prod.search".into()],
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            })
            .unwrap(),
        })
        .await
        .unwrap();
    // An agent's catalog is read once per invocation, so each change is observed by a new one.
    let start = async || -> (SecretString, String) {
        let thread = chat
            .create_thread(application::CreateThread {
                title: "Research".into(),
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
                objective: "research".into(),
                idempotency_key: Uuid::new_v4().to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
        let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
        pg.execute("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1", &[&invocation]).await.unwrap();
        let token = chat
            .tokens
            .issue(
                agent,
                invocation,
                Uuid::parse_str(&thread.id).unwrap(),
                Uuid::parse_str(&run.id).unwrap(),
            )
            .await
            .unwrap();
        (token, thread.id)
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://{}/tilde.runtime.v1.ChatService",
        listener.local_addr().unwrap()
    );
    let router = tilde::chat::rpc::runtime::router(chat.clone());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let http = reqwest::Client::new();
    let names = async |token: &SecretString| -> Vec<Value> {
        let catalog: Value = http
            .post(format!("{endpoint}/ListTools"))
            .bearer_auth(token.expose_secret())
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        catalog["tools"].as_array().cloned().unwrap_or_default()
    };
    let invoke = async |token: &SecretString, input: Value| -> (String, Vec<Value>) {
        let call = Uuid::new_v4().to_string();
        let response = http
            .post(format!("{endpoint}/InvokeTool"))
            .bearer_auth(token.expose_secret())
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(framed(json!({"name":"tavily_prod.search","callId":call,"sequence":0,"inputJson":input.to_string(),"finish":true})))
            .send()
            .await
            .unwrap();
        (call, responses(&response.bytes().await.unwrap()))
    };

    assert!(
        names(&start().await.0).await.is_empty(),
        "a connection alone gives the agent nothing"
    );
    let source = |tool_names: &[&str], agent| {
        let tools = tools.clone();
        let tool_names: Vec<String> = tool_names.iter().map(|t| t.to_string()).collect();
        async move {
            tools
                .add_source(
                    tilde::tools::Owner::Agent(agent),
                    Target::Connection(connection),
                    &tool_names,
                )
                .await
        }
    };
    assert!(
        source(&["no_such_tool"], agent).await.is_err(),
        "an agent can only use tools its connection's provider ships"
    );
    let research = source(&["search", "extract"], agent).await.unwrap();
    assert_eq!(
        research.slug, "tavily_prod",
        "named after the provider and account"
    );
    assert!(
        source(&["search"], agent).await.is_err(),
        "an agent uses a connection once"
    );
    // Another agent uses the same connection with tools of its own choosing.
    let other = Uuid::new_v4();
    Agents::new(db.pool.clone(), crypto.clone())
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id: other,
            name: "Reader".into(),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    source(&["extract"], other).await.unwrap();
    let using = tools
        .sources(Filter {
            connection: Some(connection),
            ..Default::default()
        })
        .await
        .unwrap();
    let chosen = |agent| {
        using
            .iter()
            .find(|s| s.owner == tilde::tools::Owner::Agent(agent))
            .unwrap()
            .tools
            .iter()
            .map(|t| t.tool_name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        (chosen(agent), chosen(other)),
        (
            vec!["extract".to_owned(), "search".to_owned()],
            vec!["extract".to_owned()]
        )
    );

    let (token, _) = start().await;
    let listed = names(&token).await;
    assert_eq!(
        listed.len(),
        1,
        "tools_invoke filters the agent's tools: {listed:?}"
    );
    assert_eq!(listed[0]["name"], "tavily_prod.search");
    assert_eq!(listed[0]["providerId"], "tavily");
    assert_eq!(listed[0]["annotations"]["readOnly"], true);
    assert!(
        listed[0]["outputSchemaJson"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    );
    let summary = listed[0]["summary"].as_str().unwrap().to_owned();

    let (_, rejected) = invoke(&token, json!({"max_results": 3})).await;
    assert!(
        rejected.last().unwrap().get("error").is_some(),
        "{rejected:?}"
    );
    assert!(
        seen.lock().unwrap().is_empty(),
        "invalid input never reaches the provider"
    );

    let (call, result) = invoke(&token, json!({"query":"tools","max_results":3})).await;
    assert!(result.last().unwrap().get("error").is_none(), "{result:?}");
    let output: Value = serde_json::from_str(result[0]["outputJson"].as_str().unwrap()).unwrap();
    assert_eq!(output["results"][0]["url"], "https://docs.example/tools");
    assert_eq!(
        *seen.lock().unwrap(),
        vec![(
            "/search".to_owned(),
            "Bearer tvly-fixture".to_owned(),
            json!({"query":"tools","max_results":3})
        )]
    );
    let record = pg
        .query_one(
            "SELECT name,provider_id,status,summary FROM chat_tool_calls WHERE id=$1",
            &[&Uuid::parse_str(&call).unwrap()],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            record.get::<_, String>(0),
            record.get::<_, String>(1),
            record.get::<_, String>(2),
            record.get::<_, String>(3)
        ),
        (
            "tavily_prod.search".into(),
            "tavily".into(),
            "completed".into(),
            summary
        )
    );

    // The agent rewords the tool and shows only its summary to end users, from its next
    // invocation. End users watch through the Tilde chat provider; the trace keeps everything.
    // The summary is 100 characters but 300 bytes: limits count characters everywhere, so the
    // setting that saves it is also what the audit record accepts.
    let reworded = "查找资料并整理来源。".repeat(10);
    assert_eq!((reworded.chars().count(), reworded.len()), (100, 300));
    let settings = |display: types::ToolDisplay| ToolSettings {
        summary: reworded.clone(),
        description: "Search the web for sources on the user's question.".into(),
        display: display.into(),
        ..Default::default()
    };
    tools
        .set_tool(
            research.id,
            "search",
            &settings(types::ToolDisplay::Summary),
        )
        .await
        .unwrap();
    let (provider_url, provider_token, provider_server) =
        common::tilde_provider(&db.pool, crypto.clone(), chat.clone(), agent).await;
    let tool_activity = async |thread: &str| -> Vec<Value> {
        tilde::chat::audit::flush(&db.pool).await.unwrap();
        let page: Value = http
            .post(format!(
                "{provider_url}/tilde.provider.tilde.v1.ChatService/ListActivity"
            ))
            .bearer_auth(provider_token.expose_secret())
            .json(&json!({"threadId": thread, "limit": 100}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        page["events"]
            .as_array()
            .unwrap_or_else(|| panic!("{page}"))
            .iter()
            .filter(|e| e["kind"].as_str().unwrap().starts_with("tool."))
            .cloned()
            .collect()
    };
    let (token, thread) = start().await;
    let listed = names(&token).await;
    assert_eq!(
        (&listed[0]["summary"], &listed[0]["description"]),
        (
            &json!(reworded),
            &json!("Search the web for sources on the user's question.")
        )
    );
    let (call, result) = invoke(&token, json!({"query":"summary","max_results":1})).await;
    assert!(result.last().unwrap().get("error").is_none(), "{result:?}");
    let record = pg
        .query_one(
            "SELECT display,summary,input_json FROM chat_tool_calls WHERE id=$1",
            &[&Uuid::parse_str(&call).unwrap()],
        )
        .await
        .unwrap();
    assert_eq!(
        (record.get::<_, &str>(0), record.get::<_, &str>(1)),
        ("summary", reworded.as_str())
    );
    assert!(record.get::<_, &str>(2).contains("summary"));
    let events = tool_activity(&thread).await;
    let kinds: Vec<&str> = events.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["tool.started", "tool.completed"], "{events:?}");
    for event in &events {
        let tool = &event["toolCall"];
        assert_eq!(tool["display"], "TOOL_DISPLAY_SUMMARY", "{event}");
        assert_eq!(tool["summary"], reworded.as_str());
        assert!(
            tool.get("inputJson").is_none() && tool.get("outputJson").is_none(),
            "end users see no input or output: {event}"
        );
    }

    tools
        .set_tool(research.id, "search", &settings(types::ToolDisplay::Hidden))
        .await
        .unwrap();
    let (token, thread) = start().await;
    let (call, result) = invoke(&token, json!({"query":"hidden","max_results":1})).await;
    assert!(result.last().unwrap().get("error").is_none(), "{result:?}");
    assert!(
        tool_activity(&thread).await.is_empty(),
        "hidden from end users"
    );
    let record = pg
        .query_one(
            "SELECT display,status FROM chat_tool_calls WHERE id=$1",
            &[&Uuid::parse_str(&call).unwrap()],
        )
        .await
        .unwrap();
    assert_eq!(
        (record.get::<_, &str>(0), record.get::<_, &str>(1)),
        ("hidden", "completed")
    );
    assert_eq!(seen.lock().unwrap().len(), 3);

    // Dropping the tool applies from the agent's next invocation.
    tools.remove_tool(research.id, "search").await.unwrap();
    let (token, _) = start().await;
    assert!(names(&token).await.is_empty());
    let (_, revoked) = invoke(&token, json!({"query":"again"})).await;
    assert!(revoked.last().unwrap().get("error").is_some());
    assert_eq!(seen.lock().unwrap().len(), 3);

    drop(pg);
    provider_server.abort();
    server.abort();
    upstream_server.abort();
    db.close().await;
}
