//! An MCP server as a first-class provider, across setup and invocation: a connection type names
//! the server, Tilde discovers its authorization server, registers itself as a client, runs the
//! authorization code flow with PKCE and the server as the resource, and then serves the server's
//! tools with the connection's access token.
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
        Capability, ConnectionType, CredentialSource, McpCredential, McpServer, OAuth, OAuthClient,
        OAuthGrant, Provider, ProviderKind, Values,
    },
};
use uuid::Uuid;

#[derive(Default)]
struct Seen {
    registration: Value,
    token: BTreeMap<String, String>,
}

#[tokio::test]
async fn a_dynamically_registered_mcp_server_is_set_up_and_called_with_its_own_token() {
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Seen::default()));
    let app = {
        let (o1, o2, o3) = (origin.clone(), origin.clone(), origin.clone());
        let (s1, s2) = (seen.clone(), seen.clone());
        axum::Router::new()
            .route(
                "/mcp",
                axum::routing::post(move |headers: axum::http::HeaderMap, body: axum::Json<Value>| {
                    let o1 = o1.clone();
                    async move {
                        // Unauthenticated requests learn where the server's metadata lives.
                        if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some("Bearer access-1") {
                            return (
                                axum::http::StatusCode::UNAUTHORIZED,
                                [("www-authenticate", format!("Bearer resource_metadata=\"{o1}/.well-known/oauth-protected-resource/mcp\""))],
                            )
                                .into_response();
                        }
                        let reply = |result: Value| axum::Json(json!({"jsonrpc":"2.0","id":body["id"],"result":result}));
                        match body["method"].as_str().unwrap() {
                            "initialize" => reply(json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}})).into_response(),
                            "notifications/initialized" => axum::http::StatusCode::ACCEPTED.into_response(),
                            "tools/list" => reply(json!({"tools":[{"name":"search_pages","description":"Search pages.","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}]})).into_response(),
                            "tools/call" => reply(json!({"content":[],"structuredContent":{"query":body["params"]["arguments"]["query"],"pages":1}})).into_response(),
                            _ => axum::http::StatusCode::BAD_REQUEST.into_response(),
                        }
                    }
                }),
            )
            .route(
                "/.well-known/oauth-protected-resource/mcp",
                axum::routing::get(move || {
                    let o2 = o2.clone();
                    async move { axum::Json(json!({"resource":format!("{o2}/mcp"),"authorization_servers":[o2],"scopes_supported":["read","write"]})) }
                }),
            )
            .route(
                "/.well-known/oauth-authorization-server",
                axum::routing::get(move || {
                    let o3 = o3.clone();
                    async move {
                        axum::Json(json!({"issuer":o3,"authorization_endpoint":format!("{o3}/authorize"),"token_endpoint":format!("{o3}/token"),
                            "registration_endpoint":format!("{o3}/register"),"code_challenge_methods_supported":["S256"]}))
                    }
                }),
            )
            .route(
                "/register",
                axum::routing::post(move |body: axum::Json<Value>| {
                    let s1 = s1.clone();
                    async move {
                        s1.lock().unwrap().registration = body.0;
                        axum::Json(json!({"client_id":"registered-client"}))
                    }
                }),
            )
            .route(
                "/token",
                axum::routing::post(move |form: axum::Form<BTreeMap<String, String>>| {
                    let s2 = s2.clone();
                    async move {
                        s2.lock().unwrap().token = form.0;
                        axum::Json(json!({"access_token":"access-1","refresh_token":"refresh-1","expires_in":3600,"token_type":"Bearer"}))
                    }
                }),
            )
    };
    let server = tokio::spawn(async move { axum::serve(upstream, app).await.unwrap() });
    let fx = Fixture::new(
        72,
        Endpoints(BTreeMap::from([(
            "mcp_private_origin".into(),
            origin.clone(),
        )])),
        all_tools(),
    )
    .await;
    tilde::connections::catalog::register(
        &fx.db.pool,
        Provider {
            account_name_label: None,
            icon_url: None,
            instructions: None,
            id: "wiki".into(),
            name: "Wiki".into(),
            kind: ProviderKind::BuiltIn,
            categories: vec!["documents_and_files".into()],
            connection_types: vec![ConnectionType {
                id: "oauth".into(),
                name: "Sign in with Wiki".into(),
                capabilities: vec![Capability::Tool],
                credential_source: CredentialSource::OAuth {
                    grant: OAuthGrant::AuthorizationCode,
                    configuration: OAuth {
                        client: OAuthClient::Dynamic,
                        ..OAuth::standard("")
                    }
                    .into(),
                    additional_schema: None,
                },
                mcp: Some(McpServer {
                    url: format!("{origin}/mcp"),
                    credential: McpCredential::Bearer,
                }),
            }],
        },
        true,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // Setup asks for nothing but the account name: the client is registered with the server.
    let connection = Uuid::new_v4();
    let started = fx
        .connections
        .start(connection, "Team wiki", "wiki", "oauth", &[])
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
    assert_eq!(
        view.input_schema.unwrap()["properties"],
        json!({}),
        "no client ID or secret is asked for"
    );
    let view = fx
        .connections
        .advance(setup, &token, view.action_id, Values::new())
        .await
        .unwrap();
    let registration = seen.lock().unwrap().registration.clone();
    assert_eq!(registration["token_endpoint_auth_method"], "none");
    assert_eq!(
        registration["grant_types"],
        json!(["authorization_code", "refresh_token"])
    );
    let tilde::connections::model::Action::Redirect { url: authorize } = view.action else {
        panic!("setup continues at the server's authorization endpoint");
    };
    let authorize = url::Url::parse(&authorize).unwrap();
    assert_eq!(authorize.path(), "/authorize");
    let query: BTreeMap<_, _> = authorize.query_pairs().into_owned().collect();
    assert_eq!(query["client_id"], "registered-client");
    assert_eq!(query["resource"], format!("{origin}/mcp"));
    assert_eq!(query["scope"], "read write");
    assert_eq!(query["code_challenge_method"], "S256");

    fx.connections
        .callback(
            setup,
            query["state"].split_once('.').unwrap().1,
            &Values::from([("code".into(), SecretString::from("code-1"))]),
            false,
        )
        .await
        .unwrap();
    let exchanged = seen.lock().unwrap().token.clone();
    assert_eq!(exchanged["grant_type"], "authorization_code");
    assert_eq!(exchanged["client_id"], "registered-client");
    assert_eq!(exchanged["resource"], format!("{origin}/mcp"));
    assert!(exchanged.contains_key("code_verifier") && !exchanged.contains_key("client_secret"));
    assert_eq!(
        fx.connections.get(connection).await.unwrap().status,
        "ready"
    );

    // The server's tools, discovered and called with the connection's access token.
    let tools = fx.chat.tools.clone().unwrap();
    assert_eq!(
        tools.provider_tools(connection).await.unwrap()[0].name,
        "search_pages"
    );
    let slug = fx.use_tools(connection, &["search_pages"]).await;
    assert_eq!(slug, "wiki_team_wiki");
    let result = fx
        .invoke("wiki_team_wiki.search_pages", json!({"query":"roadmap"}))
        .await;
    let output: Value = serde_json::from_str(
        result["outputJson"]
            .as_str()
            .unwrap_or_else(|| panic!("{result}")),
    )
    .unwrap();
    assert_eq!(output, json!({"query":"roadmap","pages":1}));
    server.abort();
    fx.close().await;
}

#[tokio::test]
async fn discovery_never_follows_a_server_to_a_private_address() {
    // A reachable authorization server on a private address, not the isolated test origin: it
    // would register a client if Tilde ever called it.
    let internal = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let internal_origin = format!("http://{}", internal.local_addr().unwrap());
    let reached = Arc::new(Mutex::new(0));
    let internal_app = {
        let (count, origin) = (reached.clone(), internal_origin.clone());
        axum::Router::new().fallback(move || {
            let (count, origin) = (count.clone(), origin.clone());
            async move {
                *count.lock().unwrap() += 1;
                axum::Json(json!({"issuer":origin,"authorization_endpoint":format!("{origin}/authorize"),
                    "token_endpoint":format!("{origin}/token"),"registration_endpoint":format!("{origin}/register"),
                    "code_challenge_methods_supported":["S256"],"client_id":"leaked"}))
            }
        })
    };
    let internal_server =
        tokio::spawn(async move { axum::serve(internal, internal_app).await.unwrap() });
    // The MCP server itself is the isolated test origin, and names that internal server.
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let metadata = origin.clone();
    let named = internal_origin.clone();
    let app = axum::Router::new()
        .route(
            "/mcp",
            axum::routing::post(move || {
                let metadata = metadata.clone();
                async move {
                    (
                        axum::http::StatusCode::UNAUTHORIZED,
                        [("www-authenticate", format!("Bearer resource_metadata=\"{metadata}/.well-known/oauth-protected-resource/mcp\""))],
                    )
                }
            }),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            axum::routing::get(move || {
                let named = named.clone();
                async move { axum::Json(json!({"authorization_servers":[named]})) }
            }),
        );
    let server = tokio::spawn(async move { axum::serve(upstream, app).await.unwrap() });
    let fx = Fixture::new(
        75,
        Endpoints(BTreeMap::from([(
            "mcp_private_origin".into(),
            origin.clone(),
        )])),
        all_tools(),
    )
    .await;
    tilde::connections::catalog::register(
        &fx.db.pool,
        Provider {
            account_name_label: None,
            icon_url: None,
            instructions: None,
            id: "intranet".into(),
            name: "Intranet".into(),
            kind: ProviderKind::BuiltIn,
            categories: vec!["developer_tools".into()],
            connection_types: vec![ConnectionType {
                id: "oauth".into(),
                name: "Sign in".into(),
                capabilities: vec![Capability::Tool],
                credential_source: CredentialSource::OAuth {
                    grant: OAuthGrant::AuthorizationCode,
                    configuration: OAuth {
                        client: OAuthClient::Dynamic,
                        ..OAuth::standard("")
                    }
                    .into(),
                    additional_schema: None,
                },
                mcp: Some(McpServer {
                    url: format!("{origin}/mcp"),
                    credential: McpCredential::Bearer,
                }),
            }],
        },
        true,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let started = fx
        .connections
        .start(Uuid::new_v4(), "Intranet", "intranet", "oauth", &[])
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
    let refused = fx
        .connections
        .advance(setup, &token, view.action_id, Values::new())
        .await;
    assert!(
        refused.is_err(),
        "a private authorization server is refused"
    );
    assert_eq!(*reached.lock().unwrap(), 0, "and never contacted");
    server.abort();
    internal_server.abort();
    fx.close().await;
}

#[test]
fn every_catalogue_server_is_a_valid_tool_provider() {
    let providers = tilde::connections::catalog::builtins();
    for id in [
        "notion",
        "linear",
        "hubspot",
        "vercel",
        "twilio-docs",
        "browserbase",
    ] {
        let provider = providers
            .iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("{id} is in the catalogue"));
        provider.validate().unwrap();
        assert!(provider.connection_types.iter().all(|t| t.mcp.is_some()));
    }
    // A server of a service Tilde already integrates joins that provider.
    let slack = providers.iter().find(|p| p.id == "slack").unwrap();
    slack.validate().unwrap();
    assert!(
        slack
            .connection_types
            .iter()
            .any(|t| t.id == "mcp_oauth_app" && t.mcp.is_some())
    );
}

#[tokio::test]
async fn the_catalog_lists_what_a_provider_offers_before_any_connection() {
    // Curated servers pointed at a local server: `/open` lists its tools without credentials,
    // `/closed` refuses, as most servers do.
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let open = axum::routing::post(|body: axum::Json<Value>| async move {
        let reply =
            |result: Value| axum::Json(json!({"jsonrpc":"2.0","id":body["id"],"result":result}));
        match body["method"].as_str().unwrap() {
            "initialize" => reply(json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"fixture","version":"1"}})).into_response(),
            "notifications/initialized" => axum::http::StatusCode::ACCEPTED.into_response(),
            "tools/list" => reply(json!({"tools":[{"name":"list_issues","description":"List issues.","inputSchema":{"type":"object"}}]})).into_response(),
            _ => axum::http::StatusCode::BAD_REQUEST.into_response(),
        }
    });
    let app = axum::Router::new().route("/open", open).route(
        "/closed",
        axum::routing::post(|| async { axum::http::StatusCode::UNAUTHORIZED }),
    );
    let server = tokio::spawn(async move { axum::serve(upstream, app).await.unwrap() });
    let fx = Fixture::new(
        74,
        Endpoints(BTreeMap::from([(
            "mcp_private_origin".into(),
            origin.clone(),
        )])),
        all_tools(),
    )
    .await;
    for (id, path) in [
        ("linear", "open"),
        ("notion", "closed"),
        ("stripe", "closed"),
    ] {
        let mut provider = tilde::connections::catalog::builtins()
            .into_iter()
            .find(|p| p.id == id)
            .unwrap();
        for typ in &mut provider.connection_types {
            if let Some(server) = &mut typ.mcp {
                server.url = format!("{origin}/{path}");
            }
        }
        tilde::connections::catalog::register(&fx.db.pool, provider, true, None, None, None)
            .await
            .unwrap();
    }
    let tools = fx.chat.tools.clone().unwrap();
    let names = |tools: Vec<tilde::proto::tilde::types::v1::ToolDefinition>| {
        tools.into_iter().map(|t| t.name).collect::<Vec<_>>()
    };
    // Before any connection: what the server lists without credentials, else its snapshot.
    assert_eq!(
        names(tools.catalog_tools("linear",).await.unwrap()),
        ["list_issues"]
    );
    assert!(names(tools.catalog_tools("notion",).await.unwrap()).contains(&"post-search".into()));
    assert_eq!(tools.catalog_tools("google_mail",).await.unwrap().len(), 11);
    // Built-in and MCP-served types of one provider are listed together.
    let stripe = names(tools.catalog_tools("stripe").await.unwrap());
    assert!(stripe.contains(&"process_refund".into()) && stripe.contains(&"create_refund".into()));
    server.abort();
    fx.close().await;
}
