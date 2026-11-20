//! `tilde iac types`: types for the organization's own connections. It reads every connection
//! the caller can see (MCP servers, tool hosts' instances and ontology environments are
//! connections too) and the tools each offers, and writes a `.d.ts` that augments
//! `@trytilde/iac`'s `Connections`, so `connection.ref("acme_mcp/prod")` takes that connection's
//! type and tool names. Slugs it does not list keep taking plain strings.
use crate::api::{Client, failed, options};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::{fmt::Write as _, path::PathBuf};
use tilde_contracts::proto::tilde::{management::v1 as wire, types::v1 as types};

#[derive(Args)]
pub struct Iac {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Write a .d.ts typing connection.ref(...) with the organization's connections and tools.
    Types(Types),
}
#[derive(Args)]
struct Types {
    #[arg(long, env = "TILDE_URL")]
    url: String,
    /// Required by Tilde Cloud; open-source Tilde needs none.
    #[arg(long, env = "TILDE_API_KEY", hide_env_values = true)]
    api_key: Option<String>,
    /// Where to write the declarations; include it in the project's tsconfig.
    #[arg(long, default_value = "tilde-connections.d.ts")]
    out: PathBuf,
}

/// One connection as the declarations type it: its tools (empty: it serves none), `None` when
/// none could be listed, which leaves its tool names plain strings.
pub struct Typed {
    pub slug: String,
    pub type_id: String,
    pub tools: Option<Vec<String>>,
}

pub async fn run(args: Iac) -> Result<()> {
    let Command::Types(args) = args.command;
    let client = Client::new(&args.url, args.api_key.clone())?;
    let typed = collect(&client).await?;
    std::fs::write(&args.out, render(&typed))
        .with_context(|| format!("Could not write {}", args.out.display()))?;
    eprintln!(
        "Wrote {} connection(s) to {}",
        typed.len(),
        args.out.display()
    );
    Ok(())
}

/// The ready connections (slugs are unique among those) and the tools each offers.
pub async fn collect(client: &Client) -> Result<Vec<Typed>> {
    let mut connections = Vec::new();
    let mut page_token = String::new();
    loop {
        let page = client
            .connections
            .list_connections_with_options(
                wire::ListConnectionsRequest {
                    page_size: 100,
                    page_token: page_token.clone(),
                    status: Some("ready".into()),
                    ..Default::default()
                },
                options(),
            )
            .await
            .map_err(|error| failed("ListConnections", error))?
            .into_owned();
        connections.extend(page.connections);
        if page.next_page_token.is_empty() {
            break;
        }
        page_token = page.next_page_token;
    }
    let mut typed = Vec::new();
    // Each agent's own Tilde chat connection is not referenced from specs.
    for connection in connections.into_iter().filter(|c| c.provider_id != "tilde") {
        let serves_tools = connection
            .capabilities
            .iter()
            .any(|c| c.as_known() == Some(types::Capability::Tool));
        // A connection that serves tools but lists none now (an MCP server not yet discovered,
        // a listing that failed) keeps plain-string tool names rather than failing the file.
        let tools = if serves_tools {
            let listed = client
                .tools
                .list_provider_tools_with_options(
                    wire::ListProviderToolsRequest {
                        connection_id: connection.id.clone(),
                        ..Default::default()
                    },
                    options(),
                )
                .await
                .map(|listed| listed.into_owned().tools);
            match listed {
                Ok(tools) if !tools.is_empty() => Some(tools.into_iter().map(|t| t.name).collect()),
                Ok(_) => {
                    eprintln!(
                        "{}: lists no tools yet; its tool names stay plain strings",
                        connection.slug
                    );
                    None
                }
                Err(error) => {
                    eprintln!(
                        "{}: tools not listed ({}); its tool names stay plain strings",
                        connection.slug,
                        error.message.unwrap_or_default()
                    );
                    None
                }
            }
        } else {
            Some(Vec::new())
        };
        typed.push(Typed {
            slug: connection.slug,
            type_id: connection.type_id,
            tools,
        });
    }
    Ok(typed)
}

/// The declarations: one `Connections` entry per connection, sorted by slug.
pub fn render(connections: &[Typed]) -> String {
    let quote = |value: &str| serde_json::to_string(value).expect("a string serialises");
    let mut out = String::from(
        "// Generated by `tilde iac types` from the organization's connections. Do not edit:\n\
         // rerun it when connections or their tools change.\n\
         import \"@trytilde/iac\";\n\n\
         declare module \"@trytilde/iac\" {\n  interface Connections {\n",
    );
    let mut sorted: Vec<&Typed> = connections.iter().collect();
    sorted.sort_by(|a, b| a.slug.cmp(&b.slug));
    for connection in sorted {
        let tools = match &connection.tools {
            None => String::new(),
            Some(names) if names.is_empty() => "; tools: never".into(),
            Some(names) => {
                let mut names = names.clone();
                names.sort();
                names.dedup();
                let union: Vec<String> = names.iter().map(|n| quote(n)).collect();
                format!("; tools: {}", union.join(" | "))
            }
        };
        let _ = writeln!(
            out,
            "    {}: {{ type: {}{tools} }};",
            quote(&connection.slug),
            quote(&connection.type_id)
        );
    }
    out.push_str("  }\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Bytes,
        http::{StatusCode, header},
        response::{IntoResponse, Response},
        routing::post,
    };
    use buffa::Message;

    fn proto<M: Message>(message: M) -> Response {
        (
            [(header::CONTENT_TYPE, "application/proto")],
            message.encode_to_vec(),
        )
            .into_response()
    }
    fn connection(id: &str, slug: &str, tools: bool) -> types::Connection {
        let (provider, _) = slug.split_once('/').unwrap();
        types::Connection {
            id: id.into(),
            slug: slug.into(),
            provider_id: provider.into(),
            type_id: "api".into(),
            status: "ready".into(),
            capabilities: if tools {
                vec![types::Capability::Tool.into()]
            } else {
                Vec::new()
            },
            ..Default::default()
        }
    }

    /// Against a stand-in management API: two pages of connections, each agent's Tilde chat
    /// connection left out, a failed and an empty tool listing kept as plain strings, a
    /// connection without tools offering none, and only ready connections, so a slug whose
    /// earlier setup failed is not listed twice.
    #[tokio::test]
    async fn collects_ready_connections_and_their_tools_across_pages() {
        let app = Router::new()
            .route(
                "/tilde.management.v1.ConnectionsService/ListConnections",
                post(|body: Bytes| async move {
                    let request = wire::ListConnectionsRequest::decode_from_slice(&body).unwrap();
                    let mut connections = Vec::new();
                    let next_page_token = if request.page_token.is_empty() {
                        connections.push(connection("c-app", "tilde/agent-app", false));
                        connections.push(connection("c-linear", "linear/acme", true));
                        "page-2".to_owned()
                    } else {
                        connections.push(connection("c-mcp", "acme_mcp/prod", true));
                        connections.push(connection("c-empty", "empty_mcp/new", true));
                        connections.push(connection("c-openai", "openai/prod", false));
                        String::new()
                    };
                    // Slugs are unique among ready connections only.
                    if request.status.as_deref() != Some("ready") {
                        connections.push(types::Connection {
                            status: "failed".into(),
                            ..connection("c-linear-old", "linear/acme", true)
                        });
                    }
                    proto(wire::ListConnectionsResponse {
                        connections,
                        next_page_token,
                        ..Default::default()
                    })
                }),
            )
            .route(
                "/tilde.management.v1.ToolService/ListProviderTools",
                post(|body: Bytes| async move {
                    let request = wire::ListProviderToolsRequest::decode_from_slice(&body).unwrap();
                    match request.connection_id.as_str() {
                        "c-linear" => proto(wire::ListProviderToolsResponse {
                            tools: vec![wire::ProviderTool {
                                name: "get_issue".into(),
                                ..Default::default()
                            }],
                            ..Default::default()
                        }),
                        "c-empty" => proto(wire::ListProviderToolsResponse::default()),
                        _ => (
                            StatusCode::SERVICE_UNAVAILABLE,
                            [(header::CONTENT_TYPE, "application/json")],
                            r#"{"code":"unavailable","message":"server down"}"#,
                        )
                            .into_response(),
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let client = Client::new(&url, None).unwrap();
        let mut typed = collect(&client).await.unwrap();
        typed.sort_by(|a, b| a.slug.cmp(&b.slug));
        let summary: Vec<(&str, Option<Vec<String>>)> = typed
            .iter()
            .map(|t| (t.slug.as_str(), t.tools.clone()))
            .collect();
        assert_eq!(
            summary,
            [
                ("acme_mcp/prod", None),
                ("empty_mcp/new", None),
                ("linear/acme", Some(vec!["get_issue".to_owned()])),
                ("openai/prod", Some(Vec::new())),
            ]
        );
        let text = render(&typed);
        assert_eq!(text.matches("\"linear/acme\"").count(), 1, "{text}");
        assert!(
            text.contains("\"empty_mcp/new\": { type: \"api\" };"),
            "{text}"
        );
    }

    #[test]
    fn declarations_augment_connections_with_each_slugs_type_and_tools() {
        let text = render(&[
            Typed {
                slug: "linear/acme".into(),
                type_id: "api_key".into(),
                tools: Some(vec!["search_issues".into(), "get_issue".into()]),
            },
            Typed {
                slug: "acme_mcp/prod".into(),
                type_id: "oauth".into(),
                tools: None,
            },
            Typed {
                slug: "openai/prod".into(),
                type_id: "api".into(),
                tools: Some(Vec::new()),
            },
        ]);
        assert!(text.contains("declare module \"@trytilde/iac\""), "{text}");
        assert!(
            text.contains(
                "\"linear/acme\": { type: \"api_key\"; tools: \"get_issue\" | \"search_issues\" };"
            ),
            "{text}"
        );
        // Unlisted tools stay plain strings; a connection without tools offers none.
        assert!(
            text.contains("\"acme_mcp/prod\": { type: \"oauth\" };"),
            "{text}"
        );
        assert!(
            text.contains("\"openai/prod\": { type: \"api\"; tools: never };"),
            "{text}"
        );
        assert!(
            text.find("acme_mcp").unwrap() < text.find("linear/acme").unwrap(),
            "sorted"
        );
    }
}
