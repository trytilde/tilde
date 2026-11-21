//! Tavily web search, extraction, crawling, site mapping, research and usage over its REST API.
use super::ToolProvider;
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_SEARCH, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("API key name".into()),
        icon_url: Some("/provider-icons/tavily.svg".into()),
        instructions: Some(
            "Connect Tavily so agents can search and read the web. Create an API key at app.tavily.com."
                .into(),
        ),
        id: "tavily".into(),
        name: "Tavily".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_SEARCH.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Tavily API".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{"api_key":{"type":"string","title":"API key","minLength":1,"writeOnly":true}},"required":["api_key"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool],
        }],
    }
}

pub struct Tavily;

fn tool(
    name: &str,
    summary: &str,
    description: &str,
    properties: Value,
    required: &[&str],
) -> types::ToolDefinition {
    types::ToolDefinition {
        name: name.into(),
        provider_id: "tavily".into(),
        description: description.into(),
        summary: summary.into(),
        input_schema_json: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}).to_string(),
        output_schema_json: json!({"type":"object"}).to_string(),
        // Every Tavily tool reads the public web and changes nothing upstream.
        annotations: types::ToolAnnotations { read_only: true, idempotent: true, open_world: true, ..Default::default() }.into(),
        ..Default::default()
    }
}
fn urls() -> Value {
    json!({"type":"array","items":{"type":"string","minLength":1,"maxLength":2048},"minItems":1,"maxItems":20})
}
fn patterns() -> Value {
    json!({"type":"array","items":{"type":"string","minLength":1,"maxLength":512},"maxItems":32})
}
fn crawl_properties() -> serde_json::Map<String, Value> {
    json!({
        "url":{"type":"string","minLength":1,"maxLength":2048,"description":"Root URL to begin from."},
        "max_depth":{"type":"integer","minimum":1,"maximum":5},
        "max_breadth":{"type":"integer","minimum":1,"maximum":500},
        "limit":{"type":"integer","minimum":1,"maximum":500,"description":"Total links to process before stopping."},
        "instructions":{"type":"string","maxLength":2000,"description":"Natural language guidance for selecting pages."},
        "select_paths":patterns(),"select_domains":patterns(),
        "allow_external":{"type":"boolean"}
    }).as_object().cloned().unwrap_or_default()
}
impl ToolProvider for Tavily {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let mut crawl = crawl_properties();
        crawl.insert("extract_depth".into(), json!({"enum":["basic","advanced"]}));
        crawl.insert("format".into(), json!({"enum":["markdown","text"]}));
        crawl.insert("include_favicon".into(), json!({"type":"boolean"}));
        vec![
            tool(
                "search",
                "Searched the web",
                "Run a web search and return ranked results with optional answer, raw content and images.",
                json!({
                    "query":{"type":"string","minLength":1,"maxLength":400},
                    "search_depth":{"enum":["ultra-fast","fast","basic","advanced"]},
                    "max_results":{"type":"integer","minimum":1,"maximum":20},
                    "topic":{"enum":["general","news","finance"]},
                    "include_answer":{"enum":[true,false,"basic","advanced"]},
                    "include_raw_content":{"enum":[true,false,"markdown","text"]},
                    "include_images":{"type":"boolean"},
                    "include_image_descriptions":{"type":"boolean"},
                    "include_favicon":{"type":"boolean"},
                    "include_usage":{"type":"boolean","description":"Include the credits the search used."},
                    "include_domains":patterns(),"exclude_domains":patterns(),
                    "time_range":{"enum":["day","week","month","year","d","w","m","y"]},
                    "start_date":{"type":"string","pattern":"^\\d{4}-\\d{2}-\\d{2}$"},
                    "end_date":{"type":"string","pattern":"^\\d{4}-\\d{2}-\\d{2}$"},
                    "country":{"type":"string","maxLength":64,"description":"Full country name to boost; general topic only."},
                    "exact_match":{"type":"boolean","description":"Only return results containing the query's quoted phrases."}
                }),
                &["query"],
            ),
            tool(
                "extract",
                "Read web pages",
                "Extract the content of one or more URLs as markdown or text.",
                json!({
                    "urls":urls(),
                    "extract_depth":{"enum":["basic","advanced"]},
                    "format":{"enum":["markdown","text"]},
                    "include_images":{"type":"boolean"},
                    "include_favicon":{"type":"boolean"},
                    "query":{"type":"string","maxLength":400,"description":"Reranks extracted chunks by relevance."}
                }),
                &["urls"],
            ),
            tool(
                "crawl",
                "Crawled a website",
                "Crawl a site from a root URL and return the content of the pages found.",
                Value::Object(crawl),
                &["url"],
            ),
            tool(
                "map",
                "Mapped a website",
                "Discover the URLs of a site from a root URL without extracting content.",
                Value::Object(crawl_properties()),
                &["url"],
            ),
            tool(
                "research",
                "Researched a topic",
                "Run a multi-step research task and return its report with sources. It can take many minutes; prefer an async function.",
                json!({
                    "input":{"type":"string","minLength":1,"maxLength":10000,"description":"A full description of the research task."},
                    "model":{"enum":["mini","pro","auto"]}
                }),
                &["input"],
            ),
            tool(
                "usage",
                "Checked Tavily usage",
                "Return the API key's and account's credit usage and limits.",
                json!({}),
                &[],
            ),
        ]
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            if name == "research" {
                return research(access, input).await;
            }
            if name == "usage" {
                let url = access.url("tavily_api", "https://api.tavily.com", &["usage"])?;
                return access
                    .json(
                        access
                            .http
                            .client
                            .get(url)
                            .bearer_auth(access.secret("api_key")?),
                    )
                    .await;
            }
            if !["search", "extract", "crawl", "map"].contains(&name) {
                return Err(ConnectError::not_found("Unsupported Tavily tool"));
            }
            let url = access.url("tavily_api", "https://api.tavily.com", &[name])?;
            access.json(access.post(url, "api_key")?.json(&input)).await
        })
    }
}

/// Research is a job: start it, then poll until Tavily reports an outcome. It can take many
/// minutes, so agents should use it as an async (background) tool.
async fn research(access: &Access, input: Value) -> ToolResult<Value> {
    let start = access.url("tavily_api", "https://api.tavily.com", &["research"])?;
    let started = access
        .json(access.post(start, "api_key")?.json(&input))
        .await?;
    let request = started["request_id"]
        .as_str()
        .ok_or_else(|| ConnectError::unknown("Tavily returned no research request"))?;
    let mut wait = std::time::Duration::from_secs(2);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(900);
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(wait).await;
        wait = (wait * 3 / 2).min(std::time::Duration::from_secs(10));
        let url = access.url(
            "tavily_api",
            "https://api.tavily.com",
            &["research", request],
        )?;
        let poll = access
            .json(
                access
                    .http
                    .client
                    .get(url)
                    .bearer_auth(access.secret("api_key")?),
            )
            .await?;
        match poll["status"].as_str() {
            Some("completed") => return Ok(poll),
            Some("failed") => return Err(ConnectError::unknown("Tavily research failed")),
            _ => {}
        }
    }
    Err(ConnectError::deadline_exceeded(
        "Tavily research did not finish in time",
    ))
}
