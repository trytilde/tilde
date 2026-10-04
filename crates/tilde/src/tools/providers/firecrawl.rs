//! Firecrawl scraping, search, crawling, extraction, monitors, browser interaction and research
//! over its v2 REST API. Requests carry Firecrawl's origin marker (header and body) so usage is
//! attributed to Tilde.
use super::{
    ToolProvider,
    rest::{self, Hints, Rest, Verb},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_SEARCH, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Map, Value, json};

const BASE: &str = "https://api.firecrawl.dev";
const ORIGIN: &str = "tilde-managed-provider";

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("API key name".into()),
        icon_url: Some("/provider-icons/firecrawl.svg".into()),
        instructions: Some(
            "Connect Firecrawl so agents can scrape, search, crawl and monitor the web. Create an API key (fc-...) at firecrawl.dev/app/api-keys."
                .into(),
        ),
        id: "firecrawl".into(),
        name: "Firecrawl".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_SEARCH.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Firecrawl API".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{"api_key":{"type":"string","title":"API key","minLength":1,"writeOnly":true},"webhook_secret":{"type":"string","title":"Webhook secret","description":"Optional. To receive monitor results as signals, give monitors this connection's webhook URL and paste the webhook secret from your Firecrawl account settings here.","writeOnly":true}},"required":["api_key"],"additionalProperties":false}),
            },
            // Monitor webhooks, signed with the account's webhook secret.
            capabilities: vec![model::Capability::Tool, model::Capability::Signal],
        }],
    }
}

pub struct Firecrawl;

const fn tool(
    name: &'static str,
    summary: &'static str,
    description: &'static str,
    verb: Verb,
    path: &'static str,
) -> Rest {
    Rest {
        name,
        summary,
        description,
        verb,
        path,
        query: &[],
    }
}
use Verb::{Delete, Get, Patch, Post};
const TOOLS: &[Rest] = &[
    tool(
        "scrape",
        "Scrape a URL",
        "Scrape one URL and return clean markdown, JSON extraction, screenshots, links, branding data, or other requested formats.",
        Post,
        "/v2/scrape",
    ),
    tool(
        "map",
        "Map a website",
        "Discover URLs on a site before deciding what to scrape or crawl.",
        Post,
        "/v2/map",
    ),
    tool(
        "search",
        "Search the web",
        "Search the web and optionally scrape the results.",
        Post,
        "/v2/search",
    ),
    tool(
        "search_feedback",
        "Send search feedback",
        "Submit feedback for a previous search result.",
        Post,
        "/v2/search/{searchId}/feedback",
    ),
    tool(
        "feedback",
        "Send endpoint feedback",
        "Submit feedback on a scrape, parse, map or search job.",
        Post,
        "/v2/feedback",
    ),
    tool(
        "crawl",
        "Crawl a website",
        "Start a crawl and wait for it to finish (up to `timeout` seconds, default 600). A crawl still running at the deadline can be followed with check_crawl_status.",
        Post,
        "/v2/crawl",
    ),
    tool(
        "check_crawl_status",
        "Get crawl status",
        "Fetch the status and available results of a crawl.",
        Get,
        "/v2/crawl/{id}",
    ),
    tool(
        "extract",
        "Extract structured data",
        "Extract structured information from one or more URLs.",
        Post,
        "/v2/extract",
    ),
    tool(
        "agent",
        "Start a research agent",
        "Start an asynchronous autonomous web research agent job; poll it with agent_status.",
        Post,
        "/v2/agent",
    ),
    tool(
        "agent_status",
        "Get agent status",
        "Fetch the status and results of an agent job.",
        Get,
        "/v2/agent/{id}",
    ),
    tool(
        "interact",
        "Interact with a page",
        "Run a prompt or code against a page in a browser session: give `url` to scrape and open a new session, or `scrapeId` to reuse one. Returns the session's scrapeId.",
        Post,
        "/v2/scrape/{scrapeId}/interact",
    ),
    tool(
        "interact_stop",
        "Stop interact session",
        "Stop the browser session of a scrape.",
        Delete,
        "/v2/scrape/{scrapeId}/interact",
    ),
    tool(
        "parse",
        "Parse an uploaded file",
        "Parse a file already uploaded to Firecrawl, named by its upload reference. Local file paths cannot be read.",
        Post,
        "/v2/parse",
    ),
    tool(
        "monitor_create",
        "Create monitor",
        "Create a recurring monitor from a full `body`, or from shorthand fields: a goal plus pages to scrape or queries to search.",
        Post,
        "/v2/monitor",
    ),
    tool(
        "monitor_list",
        "List monitors",
        "List the account's monitors.",
        Get,
        "/v2/monitor",
    ),
    tool(
        "monitor_get",
        "Get monitor",
        "Fetch a monitor.",
        Get,
        "/v2/monitor/{id}",
    ),
    tool(
        "monitor_update",
        "Update monitor",
        "Change a monitor with the fields in `body`.",
        Patch,
        "/v2/monitor/{id}",
    ),
    tool(
        "monitor_delete",
        "Delete monitor",
        "Delete a monitor.",
        Delete,
        "/v2/monitor/{id}",
    ),
    tool(
        "monitor_run",
        "Run monitor now",
        "Trigger a monitor check immediately.",
        Post,
        "/v2/monitor/{id}/run",
    ),
    tool(
        "monitor_checks",
        "List monitor checks",
        "List the check runs of a monitor.",
        Get,
        "/v2/monitor/{id}/checks",
    ),
    tool(
        "monitor_check",
        "Get monitor check",
        "Fetch one monitor check, optionally filtering its pages by status.",
        Get,
        "/v2/monitor/{id}/checks/{checkId}",
    ),
    tool(
        "research_search_papers",
        "Search research papers",
        "Search the research-paper index by natural-language topic.",
        Get,
        "/v2/search/research/papers",
    ),
    tool(
        "research_inspect_paper",
        "Inspect paper",
        "Fetch canonical metadata for one research paper.",
        Get,
        "/v2/search/research/papers/{id}",
    ),
    tool(
        "research_related_papers",
        "Find related papers",
        "Find papers related to one or more seed paper ids.",
        Get,
        "/v2/search/research/papers/{paperId}/similar",
    ),
    tool(
        "research_read_paper",
        "Read paper passages",
        "Read the full-text passages of one paper most relevant to a question.",
        Get,
        "/v2/search/research/papers/{paperId}",
    ),
    tool(
        "research_search_github",
        "Search GitHub research index",
        "Search the indexed GitHub issue, pull request and README history.",
        Get,
        "/v2/search/research/github",
    ),
];

fn object(properties: Value, required: &[&str], open: bool) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":open})
}
fn strings() -> Value {
    json!({"type":"array","items":{"type":"string","minLength":1}})
}
fn text() -> Value {
    json!({"type":"string","minLength":1})
}
fn integer() -> Value {
    json!({"type":"integer","minimum":0})
}
/// Tools whose option sets are Firecrawl's own stay open: unlisted fields pass through as sent.
fn schema(name: &str) -> Value {
    match name {
        "scrape" => object(
            json!({
                "url":text(),
                "formats":{"type":"array","description":"markdown, html, rawHtml, links, images, screenshot, summary, json, query, changeTracking or branding; json, query and screenshot take jsonOptions, queryOptions and screenshotOptions."},
                "onlyMainContent":{"type":"boolean"},
                "includeTags":strings(),"excludeTags":strings(),
                "waitFor":{"type":"integer","minimum":0,"description":"Milliseconds to wait before scraping."},
                "mobile":{"type":"boolean"},
                "jsonOptions":{"type":"object","description":"prompt and/or schema for the json format."},
                "queryOptions":{"type":"object"},"screenshotOptions":{"type":"object"},
                "parsers":{"type":"array","description":"e.g. [\"pdf\"]; pdf takes pdfOptions."},
                "pdfOptions":{"type":"object"},
                "location":{"type":"object","description":"country and languages."},
                "maxAge":{"type":"integer","minimum":0,"description":"Accept a cached copy this many milliseconds old."}
            }),
            &["url"],
            true,
        ),
        "map" => object(
            json!({"url":text(),"search":{"type":"string"},"limit":integer(),"includeSubdomains":{"type":"boolean"},"sitemap":{"enum":["include","skip","only"]}}),
            &["url"],
            true,
        ),
        "search" => object(
            json!({
                "query":text(),"limit":integer(),
                "sources":{"type":"array","description":"web, news or images."},
                "includeDomains":strings(),"excludeDomains":strings(),
                "tbs":{"type":"string","description":"Time filter, e.g. qdr:d."},
                "location":{"type":"string"},
                "scrapeOptions":{"type":"object","description":"Scrape options for each result, as for scrape."}
            }),
            &["query"],
            true,
        ),
        "search_feedback" => object(
            json!({"searchId":text(),"rating":text()}),
            &["searchId", "rating"],
            true,
        ),
        "feedback" => object(
            json!({"endpoint":{"enum":["scrape","parse","map","search"]},"jobId":text(),"rating":text()}),
            &["endpoint", "jobId", "rating"],
            true,
        ),
        "crawl" => object(
            json!({
                "url":text(),"limit":integer(),"maxDiscoveryDepth":integer(),
                "includePaths":strings(),"excludePaths":strings(),
                "allowExternalLinks":{"type":"boolean"},"allowSubdomains":{"type":"boolean"},
                "crawlEntireDomain":{"type":"boolean"},
                "prompt":{"type":"string","description":"Natural language guidance for the crawl."},
                "scrapeOptions":{"type":"object","description":"Scrape options for each page, as for scrape."},
                "pollInterval":{"type":"number","minimum":1,"maximum":60,"description":"Seconds between status checks; default 2."},
                "timeout":{"type":"number","minimum":1,"maximum":600,"description":"Seconds to wait for completion; default 600."}
            }),
            &["url"],
            true,
        ),
        "extract" => object(
            json!({"urls":{"type":"array","items":text(),"minItems":1},"prompt":{"type":"string"},"schema":{"type":"object"},"enableWebSearch":{"type":"boolean"},"allowExternalLinks":{"type":"boolean"},"includeSubdomains":{"type":"boolean"}}),
            &["urls"],
            true,
        ),
        "agent" => object(
            json!({"prompt":text(),"urls":strings(),"schema":{"type":"object"}}),
            &["prompt"],
            false,
        ),
        "interact" => object(
            json!({
                "url":text(),"scrapeId":text(),
                "prompt":{"type":"string"},"code":{"type":"string"},
                "scrapeOptions":{"type":"object","description":"Scrape options when opening a session from url."}
            }),
            &[],
            true,
        ),
        "interact_stop" => object(json!({"scrapeId":text()}), &["scrapeId"], false),
        "parse" => object(json!({"uploadRef":text()}), &["uploadRef"], true),
        "monitor_create" => object(
            json!({
                "body":{"type":"object","description":"A complete monitor definition; the shorthand fields are ignored when given."},
                "name":{"type":"string"},"goal":{"type":"string"},
                "page":{"type":"string"},"pages":strings(),"queries":strings(),
                "searchWindow":{"type":"string"},"maxResults":integer(),
                "includeDomains":strings(),"excludeDomains":strings(),
                "scheduleText":{"type":"string","description":"e.g. every 30 minutes (the default)."},
                "timezone":{"type":"string"},
                "email":{"type":"string"},"webhookUrl":{"type":"string"},"includeDiffs":{"type":"boolean"}
            }),
            &[],
            false,
        ),
        "monitor_update" => object(
            json!({"id":text(),"body":{"type":"object"}}),
            &["id", "body"],
            false,
        ),
        "monitor_list" => object(json!({"limit":integer(),"offset":integer()}), &[], false),
        "monitor_checks" => object(
            json!({"id":text(),"limit":integer(),"offset":integer(),"status":{"type":"string"}}),
            &["id"],
            false,
        ),
        "monitor_check" => object(
            json!({"id":text(),"checkId":text(),"limit":integer(),"offset":integer(),"pageStatus":{"type":"string"}}),
            &["id", "checkId"],
            false,
        ),
        "research_search_papers" => object(
            json!({"query":text(),"k":integer(),"authors":strings(),"categories":strings(),"from":{"type":"string"},"to":{"type":"string"}}),
            &["query"],
            false,
        ),
        "research_related_papers" => object(
            json!({"seedIds":{"type":"array","items":text(),"minItems":1},"intent":{"type":"string"},"mode":{"type":"string"},"k":integer(),"rerank":{"type":"boolean"}}),
            &["seedIds"],
            false,
        ),
        "research_read_paper" => object(
            json!({"paperId":text(),"query":{"type":"string"},"question":{"type":"string","description":"Used as query when query is absent."},"k":integer()}),
            &["paperId"],
            false,
        ),
        "research_search_github" => {
            object(json!({"query":text(),"k":integer()}), &["query"], false)
        }
        _ => object(json!({"id":text()}), &["id"], false),
    }
}

impl ToolProvider for Firecrawl {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            .map(|spec| {
                // These post their parameters but only read the web.
                let hints = if ["scrape", "map", "search", "crawl", "extract", "parse"]
                    .contains(&spec.name)
                {
                    Hints {
                        read_only: true,
                        destructive: false,
                    }
                } else {
                    spec.verb.hints()
                };
                rest::definition(
                    "firecrawl",
                    spec.name,
                    spec.summary,
                    spec.description,
                    schema(spec.name),
                    hints,
                )
            })
            .collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let spec = spec(name)?;
            let mut input = match input {
                Value::Object(input) => input,
                _ => Map::new(),
            };
            match name {
                "crawl" => return crawl(access, input).await,
                "interact" => return interact(access, input).await,
                "scrape" | "parse" => scrape_options(&mut input),
                "search" => {
                    let include = strings_of(input.remove("includeDomains"));
                    let exclude = strings_of(input.remove("excludeDomains"));
                    // Firecrawl search filters domains through the query's site: operators.
                    if let Some(Value::String(query)) = input.get_mut("query") {
                        if !include.is_empty() {
                            let sites: Vec<String> =
                                include.iter().map(|d| format!("site:{d}")).collect();
                            *query = format!("{query} ({})", sites.join(" OR "));
                        } else if !exclude.is_empty() {
                            let sites: Vec<String> =
                                exclude.iter().map(|d| format!("-site:{d}")).collect();
                            *query = format!("{query} {}", sites.join(" "));
                        }
                    }
                    nested_scrape_options(&mut input);
                }
                "monitor_create" => input = monitor(input)?,
                "monitor_update" => {
                    let id = input.remove("id");
                    input = match input.remove("body") {
                        Some(Value::Object(body)) => body,
                        _ => Map::new(),
                    };
                    input.insert("id".into(), id.unwrap_or_default());
                }
                "research_related_papers" => {
                    let mut seeds = strings_of(input.remove("seedIds")).into_iter();
                    input.insert("paperId".into(), json!(seeds.next()));
                    input.insert("anchor".into(), json!(seeds.collect::<Vec<_>>()));
                }
                "research_read_paper" => {
                    if let Some(question) = input.remove("question")
                        && !input.contains_key("query")
                    {
                        input.insert("query".into(), question);
                    }
                }
                _ => {}
            }
            if spec.verb == Post && !name.starts_with("monitor_") {
                input.insert("origin".into(), json!(ORIGIN));
            }
            call(access, spec, input).await
        })
    }
}

fn spec(name: &str) -> ToolResult<&'static Rest> {
    TOOLS
        .iter()
        .find(|spec| spec.name == name)
        .ok_or_else(|| ConnectError::not_found("Unsupported Firecrawl tool"))
}
async fn call(access: &Access, spec: &Rest, input: Map<String, Value>) -> ToolResult<Value> {
    let request = rest::request(access, "firecrawl_api", BASE, spec, Value::Object(input))?
        .bearer_auth(access.secret("api_key")?)
        .header("X-Origin", ORIGIN);
    rest::send(request).await
}

/// A crawl is a job: start it, then poll its status until it ends or the caller's deadline.
async fn crawl(access: &Access, mut input: Map<String, Value>) -> ToolResult<Value> {
    let every = input
        .remove("pollInterval")
        .and_then(|v| v.as_f64())
        .unwrap_or(2.0);
    let limit = input
        .remove("timeout")
        .and_then(|v| v.as_f64())
        .unwrap_or(600.0);
    nested_scrape_options(&mut input);
    input.insert("origin".into(), json!(ORIGIN));
    let started = call(access, spec("crawl")?, input).await?;
    let Some(id) = started["data"]["id"]
        .as_str()
        .or_else(|| started["data"]["data"]["id"].as_str())
        .map(str::to_owned)
    else {
        return Ok(started);
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs_f64(limit);
    loop {
        let status = call(
            access,
            spec("check_crawl_status")?,
            Map::from_iter([("id".to_owned(), json!(id))]),
        )
        .await?;
        if matches!(
            status["data"]["status"].as_str(),
            Some("completed" | "failed" | "cancelled")
        ) {
            return Ok(status);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(ConnectError::deadline_exceeded(format!(
                "Firecrawl crawl {id} is still running; follow it with check_crawl_status"
            )));
        }
        tokio::time::sleep(std::time::Duration::from_secs_f64(every)).await;
    }
}

async fn interact(access: &Access, mut input: Map<String, Value>) -> ToolResult<Value> {
    if !input.contains_key("prompt") && !input.contains_key("code") {
        return Err(ConnectError::invalid_argument("Give prompt or code"));
    }
    let scrape = match (input.remove("url"), input.remove("scrapeId")) {
        (None, Some(Value::String(id))) => id,
        (Some(url), None) => {
            let mut options = match input.remove("scrapeOptions") {
                Some(Value::Object(options)) => options,
                _ => Map::new(),
            };
            options.insert("url".into(), url);
            options.insert("origin".into(), json!(ORIGIN));
            scrape_options(&mut options);
            let scraped = call(access, spec("scrape")?, options).await?;
            scraped["data"]
                .pointer("/metadata/scrapeId")
                .or_else(|| scraped["data"].pointer("/data/metadata/scrapeId"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| ConnectError::unknown("Firecrawl returned no scrapeId"))?
        }
        _ => {
            return Err(ConnectError::invalid_argument(
                "Give exactly one of url or scrapeId",
            ));
        }
    };
    input.insert("scrapeId".into(), json!(scrape));
    input.insert("origin".into(), json!(ORIGIN));
    clean(&mut input);
    let mut output = call(access, spec("interact")?, input).await?;
    output["scrapeId"] = json!(scrape);
    Ok(output)
}

/// Firecrawl takes json, query and screenshot formats and the pdf parser as typed objects
/// carrying their options; the tools accept them as names plus a sibling options object.
fn scrape_options(body: &mut Map<String, Value>) {
    for (list, typed) in [
        ("formats", &["json", "query", "screenshot"][..]),
        ("parsers", &["pdf"][..]),
    ] {
        let Some(Value::Array(items)) = body.get(list) else {
            continue;
        };
        let items: Vec<Value> = items
            .iter()
            .map(|item| match item.as_str() {
                Some(kind) if typed.contains(&kind) => {
                    match body.get(&format!("{kind}Options")) {
                        Some(Value::Object(options)) => {
                            let mut typed = options.clone();
                            typed.insert("type".into(), json!(kind));
                            Value::Object(typed)
                        }
                        // json and query need an object even without options.
                        _ if kind == "json" || kind == "query" => json!({"type":kind}),
                        _ => item.clone(),
                    }
                }
                _ => item.clone(),
            })
            .collect();
        body.insert(list.into(), Value::Array(items));
    }
    for options in [
        "jsonOptions",
        "queryOptions",
        "screenshotOptions",
        "pdfOptions",
    ] {
        body.remove(options);
    }
    clean(body);
}
fn nested_scrape_options(body: &mut Map<String, Value>) {
    if let Some(Value::Object(options)) = body.get_mut("scrapeOptions") {
        scrape_options(options);
    }
    clean(body);
}
/// Firecrawl rejects some empty values; unset optional fields are dropped instead.
fn clean(body: &mut Map<String, Value>) {
    body.retain(|_, value| match value {
        Value::Null => false,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        _ => true,
    });
}
fn strings_of(value: Option<Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => vec![],
    }
}

fn monitor(mut input: Map<String, Value>) -> ToolResult<Map<String, Value>> {
    if let Some(Value::Object(body)) = input.remove("body") {
        return Ok(body);
    }
    let text = |input: &mut Map<String, Value>, key: &str| match input.remove(key) {
        Some(Value::String(value)) => Some(value),
        _ => None,
    };
    let goal = text(&mut input, "goal").ok_or_else(|| {
        ConnectError::invalid_argument("Give body, or goal with page, pages or queries")
    })?;
    let mut urls: Vec<String> = text(&mut input, "page").into_iter().collect();
    urls.extend(strings_of(input.remove("pages")));
    let queries = strings_of(input.remove("queries"));
    let mut target = Map::new();
    if !queries.is_empty() {
        target.insert("type".into(), json!("search"));
        target.insert("queries".into(), json!(queries));
        for key in [
            "searchWindow",
            "maxResults",
            "includeDomains",
            "excludeDomains",
        ] {
            if let Some(value) = input.remove(key) {
                target.insert(key.into(), value);
            }
        }
    } else if !urls.is_empty() {
        target.insert("type".into(), json!("scrape"));
        target.insert("urls".into(), json!(urls));
    } else {
        return Err(ConnectError::invalid_argument(
            "A monitor needs page, pages or queries",
        ));
    }
    let mut body = Map::new();
    body.insert(
        "name".into(),
        json!(text(&mut input, "name").unwrap_or_else(|| "Firecrawl monitor".into())),
    );
    body.insert(
        "schedule".into(),
        json!({
            "text": text(&mut input, "scheduleText").unwrap_or_else(|| "every 30 minutes".into()),
            "timezone": text(&mut input, "timezone").unwrap_or_else(|| "UTC".into()),
        }),
    );
    body.insert("goal".into(), json!(goal));
    body.insert("targets".into(), json!([target]));
    if let Some(email) = text(&mut input, "email") {
        let diffs = input.remove("includeDiffs").and_then(|v| v.as_bool()) == Some(true);
        body.insert(
            "notification".into(),
            json!({"email":{"enabled":true,"recipients":[email],"includeDiffs":diffs}}),
        );
    }
    if let Some(url) = text(&mut input, "webhookUrl") {
        body.insert(
            "webhook".into(),
            json!({"url":url,"events":["monitor.page","monitor.check.completed"]}),
        );
    }
    Ok(body)
}
