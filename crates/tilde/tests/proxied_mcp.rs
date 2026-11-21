//! An MCP server added by URL, across provider registration, connection setup, discovery,
//! an agent's tools and invocation: the server is a registered provider of its own, its tools come from
//! the server, are named by functions, and are called through Tilde over streamable HTTP with
//! the connection's credential and the MCP session.
mod common;
use axum::response::IntoResponse;
use common::invocation::{Fixture, all_tools};
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::connections::{
    catalog::Endpoints,
    model::{
        Capability, ConnectionType, CredentialSource, McpCredential, McpServer, Provider,
        ProviderKind, Values,
    },
};
use uuid::Uuid;

#[tokio::test]
async fn discovered_mcp_tools_are_called_through_tilde_with_the_connection_credential() {
    let tools: Arc<Mutex<Value>> = Arc::new(Mutex::new(json!([
        {"name":"get_issue","description":"Fetch an issue.","inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]},"annotations":{"readOnlyHint":true}},
        {"name":"undescribed","inputSchema":{"type":"object"}}
    ])));
    let served = tools.clone();
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/mcp",
            axum::routing::post(move |headers: axum::http::HeaderMap, body: axum::Json<Value>| {
                let served = served.clone();
                async move {
                    // Without credentials it refuses, as real servers do; with them, the
                    // connection's own credential travels on every request.
                    let Some(key) = headers.get("x-api-key") else {
                        return axum::http::StatusCode::UNAUTHORIZED.into_response();
                    };
                    assert_eq!(key, "mcp-secret", "the connection credential travels on every request");
                    let method = body["method"].as_str().unwrap();
                    if method != "initialize" {
                        assert_eq!(headers["mcp-session-id"], "session-1");
                    }
                    let reply = |result: Value| json!({"jsonrpc":"2.0","id":body["id"],"result":result});
                    match method {
                        "initialize" => ([("mcp-session-id", "session-1")], axum::Json(reply(json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}})))).into_response(),
                        "notifications/initialized" => axum::http::StatusCode::ACCEPTED.into_response(),
                        // Servers may answer over an event stream, with unrelated messages first.
                        "tools/list" => (
                            [("content-type", "text/event-stream")],
                            format!("event: message\ndata: {}\n\nevent: message\ndata: {}\n\n",
                                json!({"jsonrpc":"2.0","method":"notifications/message","params":{}}),
                                reply(json!({"tools": served.lock().unwrap().clone()}))),
                        ).into_response(),
                        "tools/call" if body["params"]["arguments"]["id"] == "404" => axum::Json(reply(json!({"isError":true,"content":[{"type":"text","text":"Issue not found"}]}))).into_response(),
                        "tools/call" => axum::Json(reply(json!({"content":[{"type":"text","text":"ok"}],"structuredContent":{"id":body["params"]["arguments"]["id"],"tool":body["params"]["name"]}}))).into_response(),
                        _ => axum::http::StatusCode::BAD_REQUEST.into_response(),
                    }
                }
            }),
        );
        axum::serve(upstream, app).await.unwrap()
    });
    let mut fx = Fixture::new(
        71,
        Endpoints(BTreeMap::from([(
            "mcp_private_origin".into(),
            origin.clone(),
        )])),
        all_tools(),
    )
    .await;
    let agent_tools = fx.chat.tools.clone().unwrap();
    let server_type = |capabilities: Vec<Capability>| ConnectionType {
        id: "api_key".into(),
        name: "API key".into(),
        capabilities,
        credential_source: CredentialSource::Static {
            schema: json!({"type":"object","properties":{"api_key":{"type":"string","title":"API key","writeOnly":true}},"required":["api_key"],"additionalProperties":false}),
        },
        mcp: Some(McpServer {
            url: format!("{origin}/mcp"),
            credential: McpCredential::Header {
                name: "x-api-key".into(),
                prefix: String::new(),
            },
        }),
    };
    let tracker = |capabilities| Provider {
        account_name_label: None,
        icon_url: None,
        instructions: None,
        id: "tracker".into(),
        name: "Tracker".into(),
        kind: ProviderKind::Configured,
        categories: vec!["developer_tools".into()],
        connection_types: vec![server_type(capabilities)],
    };
    // The MCP server is the adapter that lets a registered provider serve tools; nothing else can.
    assert!(
        fx.connections
            .register_provider(tracker(vec![Capability::Tool, Capability::Channel]), None)
            .await
            .is_err()
    );
    let mut plain = tracker(vec![Capability::Tool]);
    plain.connection_types[0].mcp = None;
    assert!(fx.connections.register_provider(plain, None).await.is_err());
    fx.connections
        .register_provider(tracker(vec![Capability::Tool]), None)
        .await
        .unwrap();

    let connection = Uuid::new_v4();
    let started = fx
        .connections
        .start(connection, "Team tracker", "tracker", "api_key", &[])
        .await
        .unwrap();
    let url = url::Url::parse(&started.brokering_url).unwrap();
    let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let token = url
        .query_pairs()
        .find(|(key, _)| key == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    let view = fx.connections.view(setup, &token).await.unwrap();
    fx.connections
        .advance(
            setup,
            &token,
            view.action_id,
            Values::from([("api_key".into(), SecretString::from("mcp-secret"))]),
        )
        .await
        .unwrap();
    assert_eq!(
        fx.connections.get(connection).await.unwrap().status,
        "ready"
    );

    let discovered = agent_tools.provider_tools(connection).await.unwrap();
    assert_eq!(
        discovered
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["get_issue"],
        "a tool without a description cannot be offered"
    );
    // The provider's catalog entry lists what its connections discovered.
    assert_eq!(
        agent_tools.catalog_tools("tracker",).await.unwrap()[0].name,
        "get_issue"
    );
    assert!(
        !agent_tools
            .refresh_connection_tools(connection)
            .await
            .unwrap()
            .1,
        "an unchanged server is a no-op"
    );
    let source = agent_tools
        .add_source(
            tilde::tools::Owner::Agent(fx.agent),
            tilde::tools::Target::Connection(connection),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(source.slug, "tracker_team_tracker");
    assert!(
        agent_tools
            .set_tool(source.id, "undescribed", &Default::default())
            .await
            .is_err(),
        "a tool without a description cannot be offered"
    );
    agent_tools
        .set_tool(source.id, "get_issue", &Default::default())
        .await
        .unwrap();

    let listed = fx.tools().await;
    let tool = listed
        .iter()
        .find(|t| t["name"] == "tracker_team_tracker.get_issue")
        .expect("listed");
    assert_eq!(
        (
            tool["providerId"].clone(),
            tool["annotations"]["readOnly"].clone()
        ),
        (json!("tracker"), json!(true))
    );
    assert_eq!(
        fx.output("tracker_team_tracker.get_issue", json!({"id":"42"}))
            .await,
        json!({"id":"42","tool":"get_issue"})
    );
    assert!(
        fx.invoke("tracker_team_tracker.get_issue", json!({}))
            .await
            .get("error")
            .is_some(),
        "the discovered schema is enforced before the server is called"
    );
    let failed = fx
        .invoke("tracker_team_tracker.get_issue", json!({"id":"404"}))
        .await;
    assert_eq!(failed["error"]["message"], "Issue not found");

    // The server drops the tool: rediscovery withdraws it from the catalog.
    *tools.lock().unwrap() = json!([]);
    assert!(
        agent_tools
            .refresh_connection_tools(connection)
            .await
            .unwrap()
            .1
    );
    fx.next_invocation().await;
    assert!(
        fx.tools()
            .await
            .iter()
            .all(|t| t["name"] != "tracker_team_tracker.get_issue")
    );

    // Health beside tool hosts: a server asking for credentials is up; one that doesn't answer
    // is a failed check.
    let hour = |health: std::collections::BTreeMap<String, Vec<_>>| {
        let hours: Vec<tilde::proto::tilde::types::v1::AgentHealthHour> = health["tracker"].clone();
        let last = hours.last().unwrap();
        (hours.len(), last.total_checks, last.failed_checks)
    };
    agent_tools.sample_mcp_health().await.unwrap();
    assert_eq!(
        hour(agent_tools.mcp_health(&["tracker".into()]).await.unwrap()),
        (12, 1, 0)
    );
    server.abort();
    let _ = server.await;
    agent_tools.sample_mcp_health().await.unwrap();
    assert_eq!(
        hour(agent_tools.mcp_health(&["tracker".into()]).await.unwrap()),
        (12, 2, 1)
    );
    fx.close().await;
}

/// A server's tool descriptions can be account-specific. The provider catalog shows only what
/// installation connections discovered; a personal connection's discovery stays its owner's.
#[tokio::test]
async fn the_provider_catalog_never_shows_a_personal_connections_discovery() {
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/mcp",
            axum::routing::post(|headers: axum::http::HeaderMap, body: axum::Json<Value>| async move {
                // Without credentials it refuses, so the catalog cannot fall back to a probe.
                let Some(key) = headers.get("x-api-key") else {
                    return axum::http::StatusCode::UNAUTHORIZED.into_response();
                };
                let reply = |result: Value| json!({"jsonrpc":"2.0","id":body["id"],"result":result});
                match body["method"].as_str().unwrap() {
                    "initialize" => axum::Json(reply(json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}}))).into_response(),
                    "notifications/initialized" => axum::http::StatusCode::ACCEPTED.into_response(),
                    "tools/list" => axum::Json(reply(json!({"tools": if key == "alice-key" {
                        json!([{"name":"list_boards","description":"Boards of Alice's account: Project Nightingale.",
                            "inputSchema":{"type":"object","properties":{"board":{"type":"string","enum":["project-nightingale"]}}}}])
                    } else {
                        json!([{"name":"get_issue","description":"Fetch an issue.","inputSchema":{"type":"object"}}])
                    }}))).into_response(),
                    _ => axum::http::StatusCode::BAD_REQUEST.into_response(),
                }
            }),
        );
        axum::serve(upstream, app).await.unwrap()
    });
    let fx = Fixture::new(
        72,
        Endpoints(BTreeMap::from([(
            "mcp_private_origin".into(),
            origin.clone(),
        )])),
        all_tools(),
    )
    .await;
    let agent_tools = fx.chat.tools.clone().unwrap();
    fx.connections
        .register_provider(
            Provider {
                account_name_label: None,
                icon_url: None,
                instructions: None,
                id: "tracker".into(),
                name: "Tracker".into(),
                kind: ProviderKind::Configured,
                categories: vec!["developer_tools".into()],
                connection_types: vec![ConnectionType {
                    id: "api_key".into(),
                    name: "API key".into(),
                    capabilities: vec![Capability::Tool],
                    credential_source: CredentialSource::Static {
                        schema: json!({"type":"object","properties":{"api_key":{"type":"string","title":"API key","writeOnly":true}},"required":["api_key"],"additionalProperties":false}),
                    },
                    mcp: Some(McpServer {
                        url: format!("{origin}/mcp"),
                        credential: McpCredential::Header {
                            name: "x-api-key".into(),
                            prefix: String::new(),
                        },
                    }),
                }],
            },
            None,
        )
        .await
        .unwrap();
    let connect = |key: &'static str| {
        let connections = fx.connections.clone();
        async move {
            let id = Uuid::new_v4();
            let started = connections
                .start(id, key, "tracker", "api_key", &[])
                .await
                .unwrap();
            let url = url::Url::parse(&started.brokering_url).unwrap();
            let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
            let token = url
                .query_pairs()
                .find(|(key, _)| key == "connection_setup_token")
                .unwrap()
                .1
                .into_owned();
            let view = connections.view(setup, &token).await.unwrap();
            connections
                .advance(
                    setup,
                    &token,
                    view.action_id,
                    Values::from([("api_key".into(), SecretString::from(key))]),
                )
                .await
                .unwrap();
            id
        }
    };
    let team = connect("team-key").await;
    agent_tools.refresh_connection_tools(team).await.unwrap();
    // Alice's connection discovers her boards, then belongs to her alone.
    let personal = connect("alice-key").await;
    agent_tools
        .refresh_connection_tools(personal)
        .await
        .unwrap();
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE connections SET owner_user_id=$2::TEXT::UUID WHERE id=$1",
            &[&personal, &fx.user_id],
        )
        .await
        .unwrap();

    let names = |tools: Vec<tilde::proto::tilde::types::v1::ToolDefinition>| {
        assert!(
            tools.iter().all(|t| !t.description.contains("Nightingale")
                && !t.input_schema_json.contains("nightingale")),
            "a personal account's schema leaked"
        );
        tools.into_iter().map(|t| t.name).collect::<Vec<_>>()
    };
    assert_eq!(
        names(agent_tools.catalog_tools("tracker",).await.unwrap()),
        ["get_issue"]
    );
    server.abort();
    fx.close().await;
}

/// MCP responses may be larger than an audit record holds. Such an output fails the call with a
/// terminal record saying so, and a background call still tells the agent.
#[tokio::test]
async fn an_output_too_large_to_record_fails_the_call_and_is_still_delivered() {
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/mcp",
            axum::routing::post(|body: axum::Json<Value>| async move {
                let reply = |result: Value| json!({"jsonrpc":"2.0","id":body["id"],"result":result});
                match body["method"].as_str().unwrap() {
                    "initialize" => axum::Json(reply(json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}}))).into_response(),
                    "notifications/initialized" => axum::http::StatusCode::ACCEPTED.into_response(),
                    "tools/list" => axum::Json(reply(json!({"tools":[{"name":"export","description":"Export everything.","inputSchema":{"type":"object"}}]}))).into_response(),
                    // Within the MCP response limit, beyond what a tool call may return.
                    "tools/call" => axum::Json(reply(json!({"content":[],"structuredContent":{"rows":"x".repeat(2 * 1024 * 1024)}}))).into_response(),
                    _ => axum::http::StatusCode::BAD_REQUEST.into_response(),
                }
            }),
        );
        axum::serve(upstream, app).await.unwrap()
    });
    let mut fx = Fixture::new(
        74,
        Endpoints(BTreeMap::from([(
            "mcp_private_origin".into(),
            origin.clone(),
        )])),
        all_tools(),
    )
    .await;
    let agent_tools = fx.chat.tools.clone().unwrap();
    fx.connections
        .register_provider(
            Provider {
                account_name_label: None,
                icon_url: None,
                instructions: None,
                id: "warehouse".into(),
                name: "Warehouse".into(),
                kind: ProviderKind::Configured,
                categories: vec!["developer_tools".into()],
                connection_types: vec![ConnectionType {
                    id: "open".into(),
                    name: "Open".into(),
                    capabilities: vec![Capability::Tool],
                    credential_source: CredentialSource::Static {
                        schema: tilde::connections::schema::empty(),
                    },
                    mcp: Some(McpServer {
                        url: format!("{origin}/mcp"),
                        credential: McpCredential::None,
                    }),
                }],
            },
            None,
        )
        .await
        .unwrap();
    let connection = Uuid::new_v4();
    // Nothing to enter: the connection is ready as soon as it is started.
    fx.connections
        .start(connection, "Main", "warehouse", "open", &[])
        .await
        .unwrap();
    assert_eq!(
        fx.connections.get(connection).await.unwrap().status,
        "ready"
    );
    let source = agent_tools
        .add_source(
            tilde::tools::Owner::Agent(fx.agent),
            tilde::tools::Target::Connection(connection),
            &["export".into()],
        )
        .await
        .unwrap();
    let name = format!("{}.export", source.slug);
    let pg = fx.db.pool.get().await.unwrap();
    let record = async |call: Uuid| -> (String, String) {
        let row = pg
            .query_one(
                "SELECT status,error FROM chat_tool_calls WHERE id=$1",
                &[&call],
            )
            .await
            .unwrap();
        (row.get(0), row.get(1))
    };

    let result = fx.invoke(&name, json!({})).await;
    assert!(
        result["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("larger than 1 MiB")),
        "{result}"
    );
    let call: Uuid = pg
        .query_one("SELECT id FROM chat_tool_calls WHERE name=$1", &[&name])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        record(call).await,
        ("failed".into(), tilde::tools::OUTPUT_TOO_LARGE.into())
    );

    // In the background: the ticket now, then the failure delivered to the agent.
    agent_tools
        .set_tool(
            source.id,
            "export",
            &tilde::tools::ToolSettings {
                is_async: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    fx.next_invocation().await;
    let accepted = fx.output(&name, json!({})).await;
    let ticket = Uuid::parse_str(accepted["ticket"].as_str().unwrap()).unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let woken = loop {
        if let Some(row) = pg
            .query_opt("SELECT text FROM chat_inputs WHERE id=$1", &[&ticket])
            .await
            .unwrap()
        {
            break row.get::<_, String>(0);
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the agent was never told"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    assert!(
        woken.contains("failed") && woken.contains(tilde::tools::OUTPUT_TOO_LARGE),
        "{woken}"
    );
    assert_eq!(
        record(ticket).await,
        ("failed".into(), tilde::tools::OUTPUT_TOO_LARGE.into())
    );
    drop(pg);
    server.abort();
    fx.close().await;
}
