//! Dynamic tool sources across the catalog, discovery, execution and audit: deferred tools are
//! absent from the listed catalog, found by words beside the agent's bundled tools, and run
//! through tools.execute as themselves. The agent's latest bundled tools stay visible to management.
mod common;
use common::invocation::{Fixture, all_tools};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tilde::{
    connections::catalog::Endpoints,
    encryption::{Encryption, SecretBinding},
};
use uuid::Uuid;

#[tokio::test]
async fn deferred_tools_are_found_by_search_and_executed_as_themselves() {
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_origin = format!("http://{}", upstream.local_addr().unwrap());
    let upstream_server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(|uri: axum::http::Uri| async move {
            axum::Json(json!({"path": uri.path()}))
        });
        axum::serve(upstream, app).await.unwrap()
    });
    let spans = common::capture_spans();
    let mut fx = Fixture::new(
        61,
        Endpoints(BTreeMap::from([("tavily_api".into(), upstream_origin)])),
        all_tools(),
    )
    .await;
    let tools = fx.chat.tools.clone().unwrap();
    let pg = fx.db.pool.get().await.unwrap();
    let connection = Uuid::new_v4();
    tavily_connection(&fx, connection).await;
    let web = tools
        .add_source(
            fx.agent,
            tilde::tools::Target::Connection(connection),
            &["search".into(), "extract".into(), "crawl".into()],
        )
        .await
        .unwrap();
    assert_eq!(web.slug, "tavily");
    tools.set_mode(fx.agent, true).await.unwrap();

    let names: Vec<String> = fx
        .tools()
        .await
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        names.iter().all(|n| !n.starts_with("tavily.")),
        "deferred tools are not listed: {names:?}"
    );
    for meta in ["tools.search", "tools.schemas", "tools.execute"] {
        assert!(
            names.iter().any(|n| n == meta),
            "{meta} missing from {names:?}"
        );
    }

    // The agent process registers its bundled tools, and registers again when they change mid-run;
    // search and schemas describe the current set beside server tools.
    let origin = fx.origin.clone();
    let register = async |token: &SecretString, tools: Value| {
        reqwest::Client::new()
            .post(format!(
                "{}/tilde.runtime.v1.ChatService/RegisterBundledTools",
                origin
            ))
            .bearer_auth(token.expose_secret())
            .json(&json!({ "tools": tools }))
            .send()
            .await
            .unwrap()
            .status()
    };
    assert!(register(&fx.token, json!([{"name":"scratch_note","description":"Keep a note.","inputSchemaJson":"{\"type\":\"object\"}"}])).await.is_success());
    assert!(register(&fx.token, json!([{"name":"read_workspace_file","description":"Read a file from the local workspace.","summary":"Read a workspace file","inputSchemaJson":"{\"type\":\"object\"}","outputSchemaJson":"{\"type\":\"object\",\"properties\":{\"text\":{\"type\":\"string\"}}}","annotations":{"readOnly":true,"idempotent":true},"display":"TOOL_DISPLAY_SUMMARY"}])).await.is_success());
    assert!(
        register(&fx.token, json!([{"name":"bad","description":"Bad.","inputSchemaJson":"{\"type\":\"object\"}","outputSchemaJson":"[]"}])).await.is_client_error(),
        "an output schema must be a JSON object"
    );

    let found = fx
        .output(
            "tools.search",
            json!({"query":"read the content of web pages"}),
        )
        .await;
    let ranked: Vec<&str> = found["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(ranked[0], "tavily.extract", "{found}");
    assert!(ranked.contains(&"read_workspace_file"), "{found}");
    assert_eq!(found["searched"], 4);
    let bundled = found["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "read_workspace_file")
        .unwrap();
    assert!(bundled["runs"].as_str().unwrap().contains("bundled"));
    assert_eq!(bundled["summary"], "Read a workspace file");
    let described = fx
        .output(
            "tools.schemas",
            json!({"names":["read_workspace_file","scratch_note"]}),
        )
        .await;
    assert_eq!(
        described["tools"].as_array().unwrap().len(),
        1,
        "the last registration replaces the set: {described}"
    );
    assert_eq!(
        (
            &described["tools"][0]["output_schema"]["properties"]["text"]["type"],
            &described["tools"][0]["read_only"],
            &described["tools"][0]["bundled"]
        ),
        (&json!("string"), &json!(true), &json!(true))
    );

    let schemas = fx
        .output("tools.schemas", json!({"names":["tavily.extract"]}))
        .await;
    assert_eq!(
        schemas["tools"][0]["input_schema"]["required"],
        json!(["urls"])
    );
    assert_eq!(schemas["tools"][0]["read_only"], true);

    let output = fx
        .output(
            "tools.execute",
            json!({"name":"tavily.extract","input":{"urls":["https://example.com"]}}),
        )
        .await;
    assert_eq!(output["path"], "/extract");
    // The trace shows tools.execute with the tool it ran beneath it.
    let outer = common::span(&spans, "execute_tool tools.execute");
    let inner = common::span(&spans, "execute_tool tavily.extract");
    assert_eq!(inner.parent_span_id, outer.span_context.span_id());
    assert_eq!(inner.span_context.trace_id(), outer.span_context.trace_id());
    let record = pg
        .query_one(
            "SELECT name,summary,status FROM chat_tool_calls WHERE provider_id='tavily'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            record.get::<_, String>(0),
            record.get::<_, String>(1),
            record.get::<_, String>(2)
        ),
        (
            "tavily.extract".into(),
            "Read web pages".into(),
            "completed".into()
        ),
        "the record is the inner call, not tools.execute"
    );
    assert!(
        fx.invoke("tools.execute", json!({"name":"tavily.extract","input":{}}))
            .await
            .get("error")
            .is_some(),
        "inner schema applies"
    );
    assert!(
        fx.invoke(
            "tools.execute",
            json!({"name":"read_workspace_file","input":{}})
        )
        .await
        .get("error")
        .is_some(),
        "bundled tools run in the agent"
    );
    assert!(
        fx.invoke("tavily.extract", json!({"urls":["https://example.com"]}))
            .await
            .get("error")
            .is_none(),
        "a known name still works directly"
    );

    // Direct mode lists the tools and withdraws the discovery tools.
    tools.set_mode(fx.agent, false).await.unwrap();
    let first = pg
        .query_one(
            "SELECT id FROM chat_invocations WHERE agent_id=$1 ORDER BY started_at DESC LIMIT 1",
            &[&fx.agent],
        )
        .await
        .unwrap()
        .get::<_, Uuid>(0);
    fx.next_invocation().await;

    // Management sees the bundled tools of the latest invocation that registered any: a newer
    // invocation that has not registered yet leaves them in place.
    let (latest, listed) = tools.bundled(fx.agent).await.unwrap().unwrap();
    assert_eq!(latest.id, first);
    assert_eq!(
        listed
            .iter()
            .map(|t| (t.name.as_str(), t.provider_id.as_str(), t.display))
            .collect::<Vec<_>>(),
        [(
            "read_workspace_file",
            "bundled",
            tilde::proto::tilde::types::v1::ToolDisplay::Summary.into()
        )]
    );
    assert!(register(&fx.token, json!([{"name":"scratch_note","description":"Keep a note.","inputSchemaJson":"{\"type\":\"object\"}"}])).await.is_success());
    let (latest, listed) = tools.bundled(fx.agent).await.unwrap().unwrap();
    assert_ne!(latest.id, first);
    assert_eq!(listed[0].name, "scratch_note");
    let names: Vec<String> = fx
        .tools()
        .await
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        names.contains(&"tavily.extract".to_owned()) && !names.contains(&"tools.search".to_owned()),
        "{names:?}"
    );
    drop(pg);
    upstream_server.abort();
    fx.close().await;
}
/// A ready installation-owned Tavily connection, seeded as the setup broker would leave it.
async fn tavily_connection(fx: &Fixture, connection: Uuid) {
    let pg = fx.db.pool.get().await.unwrap();
    pg.execute("INSERT INTO connections(id,name,provider_id,type_id,status) VALUES($1,'tavily','tavily','api','ready')", &[&connection]).await.unwrap();
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(61))
        .await
        .unwrap();
    let sealed = crypto
        .seal(
            SecretBinding {
                resource_kind: "connection",
                resource_id: connection,
                name: "api_key",
            },
            &SecretString::from("tvly-dynamic"),
        )
        .unwrap()
        .into_bytes();
    pg.execute("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,'api_key',$2)", &[&connection, &sealed]).await.unwrap();
}
