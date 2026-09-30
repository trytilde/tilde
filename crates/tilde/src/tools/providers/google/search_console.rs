//! Google Search Console: properties, search analytics, URL inspection and sitemaps. The tool
//! surface is the union of the upstream mcp-gsc server and Composio's schemas (`inspection_url`
//! and `feedpath` are accepted aliases); outputs are summarised rather than raw API bodies.
use super::{Api, DELETE, READ, WRITE, call, optional, text, tool};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_SEARCH, model};
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::ToolProvider;
use chrono::{Duration, Utc};
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::{Map, Value, json};

const ID: &str = "google_search_console";

pub fn definition() -> model::Provider {
    super::definition(
        ID,
        "Google Search Console",
        "Google Search Console tools for property management, search analytics, URL inspection, indexing diagnostics, and sitemap management.",
        CATEGORY_SEARCH,
        &["webmasters", "webmasters.readonly"],
    )
}

pub struct GoogleSearchConsole;

impl ToolProvider for GoogleSearchConsole {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let s = json!({"type":"string"});
        let n = json!({"type":"integer","minimum":0});
        let site = json!({"site_url":{"type":"string","description":"Exact Search Console property URL, for example `https://example.com/` or `sc-domain:example.com`."}});
        let with_site = |extra: Value| {
            let mut properties = site.clone();
            if let (Some(properties), Value::Object(extra)) = (properties.as_object_mut(), extra) {
                properties.extend(extra);
            }
            properties
        };
        let days =
            json!({"type":"integer","description":"Number of days to look back. Defaults to 28."});
        let urls = with_site(
            json!({"urls":{"type":"string","description":"URLs to inspect, one per line."}}),
        );
        let sitemap = with_site(json!({
            "sitemap_url":{"type":"string","description":"Full sitemap URL. `feedpath` is accepted as a Composio-compatible alias."},
            "feedpath":s
        }));
        let spec = |name, summary, description, hints, properties: Value, required: &[&str]| {
            tool(ID, name, summary, description, hints, properties, required)
        };
        vec![
            spec(
                "get_capabilities",
                "Get Google Search Console tool capabilities and auth status",
                "Lists available Google Search Console tools, grouped by category, and reports whether the current credential can call Search Console.",
                READ,
                json!({}),
                &[],
            ),
            spec(
                "list_properties",
                "List Google Search Console properties",
                "Lists all Google Search Console sites/properties visible to the connected Google account.",
                READ,
                json!({}),
                &[],
            ),
            spec(
                "get_site_details",
                "Get Search Console property details",
                "Gets verification, ownership, and permission details for a specific Search Console property.",
                READ,
                site.clone(),
                &["site_url"],
            ),
            spec(
                "add_site",
                "Add a Search Console property",
                "Adds a site to the connected Google Search Console account.",
                WRITE,
                site.clone(),
                &["site_url"],
            ),
            spec(
                "delete_site",
                "Delete a Search Console property",
                "Removes a site from the connected Google Search Console account.",
                DELETE,
                site.clone(),
                &["site_url"],
            ),
            spec(
                "get_search_analytics",
                "Get Search Console analytics",
                "Returns top Search Console rows for a property over a recent time window.",
                READ,
                with_site(json!({
                    "days":days,
                    "dimensions":{"type":"string","description":"Comma-separated dimensions: query, page, device, country, date."},
                    "row_limit":{"type":"integer","minimum":0,"description":"Number of rows to return. Defaults to 20 and is capped at 500."}
                })),
                &["site_url"],
            ),
            spec(
                "get_performance_overview",
                "Get Search Console performance overview",
                "Returns total Search Console performance metrics and daily trend data for a property.",
                READ,
                with_site(json!({"days":days})),
                &["site_url"],
            ),
            spec(
                "compare_search_periods",
                "Compare Search Console periods",
                "Compares clicks, impressions, CTR, and position between two date ranges.",
                READ,
                with_site(json!({
                    "period1_start":s,"period1_end":s,"period2_start":s,"period2_end":s,
                    "dimensions":s,"limit":n
                })),
                &[
                    "site_url",
                    "period1_start",
                    "period1_end",
                    "period2_start",
                    "period2_end",
                ],
            ),
            spec(
                "get_search_by_page_query",
                "Get queries for a page",
                "Returns Search Console query performance for a specific page URL.",
                READ,
                with_site(json!({"page_url":s,"days":{"type":"integer"},"row_limit":n})),
                &["site_url", "page_url"],
            ),
            spec(
                "get_advanced_search_analytics",
                "Get advanced Search Console analytics",
                "Returns Search Console analytics with date range, search type, filters, sorting, and pagination.",
                READ,
                with_site(json!({
                    "start_date":s,"end_date":s,
                    "dimensions":{"type":["string","array"],"items":{"type":"string"},"description":"Comma-separated string or array of dimensions."},
                    "search_type":s,"row_limit":n,"start_row":n,"sort_by":s,"sort_direction":s,
                    "filter_dimension":s,"filter_operator":s,"filter_expression":s,
                    "filters":{"type":"string","description":"JSON array of filter objects with dimension, operator, and expression keys."},
                    "dimension_filter_groups":{"type":"array","description":"Raw Google Search Console dimensionFilterGroups, matching Composio's schema."},
                    "aggregation_type":s,
                    "data_state":{"type":"string","description":"Data freshness: `all` or `final`."}
                })),
                &["site_url"],
            ),
            spec(
                "inspect_url_enhanced",
                "Inspect a URL in Search Console",
                "Returns URL inspection, crawl, indexing, canonical, and rich result details for one URL.",
                READ,
                with_site(json!({
                    "page_url":{"type":"string","description":"URL to inspect. `inspection_url` is accepted as a Composio-compatible alias."},
                    "inspection_url":s,
                    "language_code":{"type":"string","description":"Optional IETF BCP-47 language code for localized inspection results."}
                })),
                &["site_url"],
            ),
            spec(
                "batch_url_inspection",
                "Inspect URLs in batch",
                "Inspects up to 10 URLs and returns compact URL inspection summaries.",
                READ,
                urls.clone(),
                &["site_url", "urls"],
            ),
            spec(
                "check_indexing_issues",
                "Check URL indexing issues",
                "Inspects up to 10 URLs and groups indexing, canonical, robots, and fetch issues.",
                READ,
                urls,
                &["site_url", "urls"],
            ),
            spec(
                "list_sitemaps_enhanced",
                "List Search Console sitemaps with details",
                "Lists submitted sitemaps with status, pending state, errors, warnings, and URL counts.",
                READ,
                with_site(json!({"sitemap_index":s})),
                &["site_url"],
            ),
            spec(
                "get_sitemap_details",
                "Get Search Console sitemap details",
                "Returns detailed status and content breakdown for a specific sitemap.",
                READ,
                sitemap.clone(),
                &["site_url"],
            ),
            spec(
                "submit_sitemap",
                "Submit a sitemap to Search Console",
                "Submits or resubmits a sitemap to Google Search Console.",
                WRITE,
                sitemap.clone(),
                &["site_url"],
            ),
            spec(
                "delete_sitemap",
                "Delete a sitemap from Search Console",
                "Removes a submitted sitemap from Google Search Console.",
                DELETE,
                sitemap,
                &["site_url"],
            ),
            spec(
                "get_creator_info",
                "Get MCP-GSC creator information",
                "Returns attribution information for Amin Foroutan, creator of the upstream MCP-GSC project.",
                READ,
                json!({}),
                &[],
            ),
            spec(
                "reauthenticate",
                "Explain managed Google reauthentication",
                "Explains how to reauthenticate this managed Search Console provider in Tilde.",
                READ,
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
            let api = Api::new(
                access,
                "google_search_console_api",
                "https://searchconsole.googleapis.com",
            );
            let site_url = text(&input, "site_url");
            let site = ["webmasters", "v3", "sites", site_url];
            match name {
                "get_capabilities" => {
                    let authenticated = call(api.get(&site[..3])?).await.is_ok();
                    Ok(capabilities(authenticated))
                }
                "list_properties" => {
                    let sites = call(api.get(&site[..3])?).await?;
                    let properties: Vec<Value> = sites["siteEntry"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|site| {
                            json!({
                                "site_url": site["siteUrl"].as_str().unwrap_or("Unknown"),
                                "permission_level": site["permissionLevel"].as_str().unwrap_or("Unknown")
                            })
                        })
                        .collect();
                    Ok(json!({"count": properties.len(), "properties": properties}))
                }
                "get_site_details" => {
                    let details = call(api.get(&site)?).await?;
                    Ok(json!({
                        "site_url": site_url,
                        "permission_level": details["permissionLevel"].as_str().unwrap_or("Unknown"),
                        "verification": details.get("siteVerificationInfo"),
                        "ownership": details.get("ownershipInfo")
                    }))
                }
                "add_site" => {
                    let added = call(api.request(Method::PUT, &site)?).await?;
                    Ok(json!({
                        "message": format!("Site {site_url} has been added to Search Console."),
                        "permission_level": added["permissionLevel"].as_str()
                    }))
                }
                "delete_site" => {
                    call(api.request(Method::DELETE, &site)?).await?;
                    Ok(
                        json!({"message": format!("Site {site_url} has been removed from Search Console.")}),
                    )
                }
                "get_search_analytics" => {
                    let (days, start, end) = window(&input);
                    let dimensions = dimensions(optional(&input, "dimensions").unwrap_or("query"));
                    let response = query(
                        &api,
                        site_url,
                        &json!({
                            "startDate": start, "endDate": end, "dimensions": dimensions,
                            "rowLimit": input["row_limit"].as_u64().unwrap_or(20).clamp(1, 500),
                            "dataState": "all"
                        }),
                    )
                    .await?;
                    let rows = rows(&dimensions, &response);
                    Ok(json!({
                        "site_url": site_url,
                        "date_range": {"start": start, "end": end, "days": days},
                        "dimensions": dimensions,
                        "row_count": rows.len(),
                        "rows": rows
                    }))
                }
                "get_performance_overview" => {
                    let (days, start, end) = window(&input);
                    let total = query(
                        &api,
                        site_url,
                        &json!({"startDate": start, "endDate": end, "dimensions": [], "rowLimit": 1, "dataState": "all"}),
                    )
                    .await?;
                    let by_date = query(
                        &api,
                        site_url,
                        &json!({"startDate": start, "endDate": end, "dimensions": ["date"], "rowLimit": days, "dataState": "all"}),
                    )
                    .await?;
                    let mut trend = rows(&["date".into()], &by_date);
                    trend.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
                    Ok(json!({
                        "site_url": site_url,
                        "date_range": {"start": start, "end": end, "days": days},
                        "totals": metrics(&total["rows"][0]),
                        "daily_trend": trend
                    }))
                }
                "compare_search_periods" => compare(&api, &input).await,
                "get_search_by_page_query" => {
                    let (days, start, end) = window(&input);
                    let page = text(&input, "page_url");
                    let response = query(
                        &api,
                        site_url,
                        &json!({
                            "startDate": start, "endDate": end, "dimensions": ["query"],
                            "dimensionFilterGroups": [{"filters": [{"dimension": "page", "operator": "equals", "expression": page}]}],
                            "rowLimit": input["row_limit"].as_u64().unwrap_or(20).clamp(1, 500),
                            "orderBy": [{"metric": "CLICK_COUNT", "direction": "descending"}],
                            "dataState": "all"
                        }),
                    )
                    .await?;
                    let rows = rows(&["query".into()], &response);
                    let sum =
                        |key: &str| rows.iter().filter_map(|row| row[key].as_f64()).sum::<f64>();
                    let (clicks, impressions) = (sum("clicks"), sum("impressions"));
                    Ok(json!({
                        "site_url": site_url,
                        "page_url": page,
                        "date_range": {"start": start, "end": end, "days": days},
                        "totals": {
                            "clicks": clicks,
                            "impressions": impressions,
                            "avg_ctr": if impressions > 0.0 { round(clicks / impressions, 4) } else { 0.0 }
                        },
                        "row_count": rows.len(),
                        "rows": rows
                    }))
                }
                "get_advanced_search_analytics" => advanced(&api, &input).await,
                "inspect_url_enhanced" => {
                    let page = optional(&input, "page_url")
                        .or_else(|| optional(&input, "inspection_url"))
                        .ok_or_else(|| {
                            ConnectError::invalid_argument(
                                "inspect_url_enhanced requires page_url or inspection_url",
                            )
                        })?;
                    let result =
                        inspect(&api, site_url, page, optional(&input, "language_code")).await?;
                    Ok(inspection(site_url, page, &result))
                }
                "batch_url_inspection" => {
                    let mut results = Vec::new();
                    for page in urls(&input)? {
                        results.push(match inspect(&api, site_url, page, None).await {
                            Ok(result) => {
                                let inspection = &result["inspectionResult"];
                                let status = &inspection["indexStatusResult"];
                                json!({
                                    "url": page,
                                    "verdict": status["verdict"].as_str().unwrap_or("UNKNOWN"),
                                    "coverage_state": status["coverageState"].as_str().unwrap_or("Unknown"),
                                    "last_crawled": status["lastCrawlTime"].as_str().map(|at| datetime(at, "%Y-%m-%d")).unwrap_or_else(|| "Never".into()),
                                    "rich_results": inspection.get("richResultsResult").map(rich_results)
                                })
                            }
                            Err(error) => json!({"url": page, "error": error.message}),
                        });
                    }
                    Ok(json!({"site_url": site_url, "count": results.len(), "results": results}))
                }
                "check_indexing_issues" => indexing_issues(&api, &input).await,
                "list_sitemaps_enhanced" => {
                    let mut request = api.get(&[&site[..], &["sitemaps"]].concat())?;
                    if let Some(index) = optional(&input, "sitemap_index") {
                        request = request.query(&[("sitemapIndex", index)]);
                    }
                    let list = call(request).await?;
                    let sitemaps: Vec<Value> = list["sitemap"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(sitemap_summary)
                        .collect();
                    let pending = sitemaps
                        .iter()
                        .filter(|sitemap| sitemap["is_pending"].as_bool().unwrap_or(false))
                        .count();
                    Ok(json!({
                        "site_url": site_url,
                        "sitemap_index": optional(&input, "sitemap_index"),
                        "count": sitemaps.len(),
                        "pending_count": pending,
                        "sitemaps": sitemaps
                    }))
                }
                "get_sitemap_details" | "submit_sitemap" | "delete_sitemap" => {
                    let sitemap = optional(&input, "sitemap_url")
                        .or_else(|| optional(&input, "feedpath"))
                        .ok_or_else(|| {
                            ConnectError::invalid_argument(
                                "The sitemap action requires sitemap_url or feedpath.",
                            )
                        })?;
                    let path = [&site[..], &["sitemaps", sitemap]].concat();
                    match name {
                        "get_sitemap_details" => {
                            let details = call(api.get(&path)?).await?;
                            Ok(sitemap_details(site_url, sitemap, &details))
                        }
                        "submit_sitemap" => {
                            call(api.request(Method::PUT, &path)?).await?;
                            Ok(
                                json!({"message": format!("Successfully submitted sitemap: {sitemap}"), "site_url": site_url, "sitemap_url": sitemap}),
                            )
                        }
                        _ => {
                            call(api.request(Method::DELETE, &path)?).await?;
                            Ok(
                                json!({"message": format!("Successfully deleted sitemap: {sitemap}"), "site_url": site_url, "sitemap_url": sitemap}),
                            )
                        }
                    }
                }
                "get_creator_info" => Ok(json!({
                    "creator": "Amin Foroutan",
                    "project": "mcp-gsc",
                    "github": "https://github.com/AminForou/mcp-gsc",
                    "website": "https://aminforoutan.com/",
                    "linkedin": "https://www.linkedin.com/in/ma-foroutan/",
                    "x": "https://x.com/aminfseo",
                    "summary": "SEO consultant and creator of the upstream Google Search Console MCP server."
                })),
                "reauthenticate" => Ok(json!({
                    "message": "Reauthentication is managed by Tilde's connection credential flow. Reconnect the Google Search Console connection to switch accounts."
                })),
                _ => Err(ConnectError::not_found(
                    "Unsupported Google Search Console tool",
                )),
            }
        })
    }
}

fn capabilities(authenticated: bool) -> Value {
    json!({
        "server": "Google Search Console MCP Server",
        "auth_status": if authenticated { "authenticated" } else { "not_authenticated" },
        "getting_started": [
            "Call list_properties to get exact site_url values.",
            "Use the returned site_url for analytics, inspection, and sitemap tools."
        ],
        "tools": {
            "properties": ["list_properties", "get_site_details", "add_site", "delete_site"],
            "analytics": ["get_search_analytics", "get_performance_overview", "compare_search_periods", "get_search_by_page_query", "get_advanced_search_analytics"],
            "url_inspection": ["inspect_url_enhanced", "batch_url_inspection", "check_indexing_issues"],
            "sitemaps": ["list_sitemaps_enhanced", "get_sitemap_details", "submit_sitemap", "delete_sitemap"],
            "metadata": ["get_capabilities", "get_creator_info", "reauthenticate"]
        }
    })
}

/// `days` back from today (default 28, 1..=3650) as `(days, start, end)`.
fn window(input: &Value) -> (i64, String, String) {
    let days = input["days"].as_i64().unwrap_or(28).clamp(1, 3650);
    let end = Utc::now().date_naive();
    (
        days,
        (end - Duration::days(days)).to_string(),
        end.to_string(),
    )
}
fn dimensions(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|dimension| !dimension.is_empty())
        .map(str::to_owned)
        .collect()
}
fn urls(input: &Value) -> ToolResult<Vec<&str>> {
    let urls: Vec<&str> = text(input, "urls")
        .lines()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .collect();
    match urls.len() {
        0 => Err(ConnectError::invalid_argument(
            "No URLs provided for inspection.",
        )),
        1..=10 => Ok(urls),
        count => Err(ConnectError::invalid_argument(format!(
            "Too many URLs provided ({count}). Limit batch inspections to 10 URLs."
        ))),
    }
}

async fn query(api: &Api<'_>, site: &str, body: &Value) -> ToolResult<Value> {
    call(
        api.post(&[
            "webmasters",
            "v3",
            "sites",
            site,
            "searchAnalytics",
            "query",
        ])?
        .json(body),
    )
    .await
}
async fn inspect(
    api: &Api<'_>,
    site: &str,
    page: &str,
    language: Option<&str>,
) -> ToolResult<Value> {
    let mut body = json!({"inspectionUrl": page, "siteUrl": site});
    if let Some(language) = language {
        body["languageCode"] = json!(language);
    }
    call(
        api.post(&["v1", "urlInspection", "index:inspect"])?
            .json(&body),
    )
    .await
}

async fn advanced(api: &Api<'_>, input: &Value) -> ToolResult<Value> {
    let site_url = text(input, "site_url");
    let filters: Vec<Value> = if let Some(filters) = optional(input, "filters") {
        match serde_json::from_str(filters) {
            Ok(Value::Array(items)) => items,
            Ok(_) => {
                return Err(ConnectError::invalid_argument(
                    "Invalid filters value. Expected a JSON array.",
                ));
            }
            Err(error) => {
                return Err(ConnectError::invalid_argument(format!(
                    "Invalid filters JSON: {error}"
                )));
            }
        }
    } else if let (Some(dimension), Some(expression)) = (
        optional(input, "filter_dimension"),
        optional(input, "filter_expression"),
    ) {
        vec![json!({
            "dimension": dimension,
            "operator": optional(input, "filter_operator").unwrap_or("contains"),
            "expression": expression
        })]
    } else {
        vec![]
    };
    let today = Utc::now().date_naive();
    let dimensions = match &input["dimensions"] {
        Value::String(value) => dimensions(value),
        Value::Array(_) => super::string_list(&input["dimensions"]),
        _ => vec!["query".into()],
    };
    let state = optional(input, "data_state").unwrap_or("all");
    if !matches!(state, "all" | "final" | "hourly_all") {
        return Err(ConnectError::invalid_argument(format!(
            "Invalid data_state value '{state}'. Accepted values are 'all', 'final', or 'hourly_all'."
        )));
    }
    let limit = input["row_limit"].as_u64().unwrap_or(1000).clamp(1, 25_000);
    let start_row = input["start_row"].as_u64().unwrap_or(0);
    let mut body = json!({
        "startDate": optional(input, "start_date").map_or_else(|| (today - Duration::days(28)).to_string(), str::to_owned),
        "endDate": optional(input, "end_date").map_or_else(|| today.to_string(), str::to_owned),
        "dimensions": dimensions,
        "rowLimit": limit,
        "startRow": start_row,
        "searchType": optional(input, "search_type").unwrap_or("web"),
        "dataState": state
    });
    if let Some(aggregation) = optional(input, "aggregation_type") {
        body["aggregationType"] = json!(aggregation);
    }
    let metric = match optional(input, "sort_by").unwrap_or("clicks") {
        "clicks" => Some("CLICK_COUNT"),
        "impressions" => Some("IMPRESSION_COUNT"),
        "ctr" => Some("CTR"),
        "position" => Some("POSITION"),
        _ => None,
    };
    if let Some(metric) = metric {
        let direction = optional(input, "sort_direction")
            .unwrap_or("descending")
            .to_lowercase();
        body["orderBy"] = json!([{"metric": metric, "direction": direction}]);
    }
    if input["dimension_filter_groups"].is_array() {
        body["dimensionFilterGroups"] = input["dimension_filter_groups"].clone();
    } else if !filters.is_empty() {
        body["dimensionFilterGroups"] = json!([{"filters": filters}]);
    }
    let response = query(api, site_url, &body).await?;
    let rows = rows(&dimensions, &response);
    let more = rows.len() as u64 == limit;
    Ok(json!({
        "site_url": site_url,
        "date_range": {"start": body["startDate"], "end": body["endDate"]},
        "search_type": body["searchType"],
        "dimensions": dimensions,
        "filters_applied": filters,
        "pagination": {
            "start_row": start_row,
            "row_count": rows.len(),
            "has_more": more,
            "next_start_row": more.then_some(start_row + limit)
        },
        "rows": rows
    }))
}

/// Joins both periods' rows by dimension keys and ranks them by absolute click change.
async fn compare(api: &Api<'_>, input: &Value) -> ToolResult<Value> {
    let site_url = text(input, "site_url");
    let dimensions = dimensions(optional(input, "dimensions").unwrap_or("query"));
    let mut periods = Vec::new();
    for period in ["period1", "period2"] {
        let (start, end) = (
            text(input, &format!("{period}_start")),
            text(input, &format!("{period}_end")),
        );
        let body = json!({"startDate": start, "endDate": end, "dimensions": dimensions, "rowLimit": 1000, "dataState": "all"});
        periods.push(query(api, site_url, &body).await?);
    }
    let mut joined = std::collections::BTreeMap::<Vec<String>, (Value, Value)>::new();
    for (index, period) in periods.iter().enumerate() {
        for row in period["rows"].as_array().into_iter().flatten() {
            let key = super::string_list(&row["keys"]);
            let entry = joined.entry(key).or_insert((json!({}), json!({})));
            *if index == 0 {
                &mut entry.0
            } else {
                &mut entry.1
            } = row.clone();
        }
    }
    let mut comparison: Vec<Value> = joined
        .into_iter()
        .map(|(key, (p1, p2))| {
            let n = |row: &Value, field: &str| row[field].as_f64().unwrap_or_default();
            let pct = |old: f64, new: f64| (old > 0.0).then(|| round((new - old) / old * 100.0, 1));
            let (c1, c2) = (n(&p1, "clicks"), n(&p2, "clicks"));
            let (i1, i2) = (n(&p1, "impressions"), n(&p2, "impressions"));
            let (r1, r2) = (n(&p1, "ctr"), n(&p2, "ctr"));
            let (q1, q2) = (n(&p1, "position"), n(&p2, "position"));
            json!({
                "key": key,
                "p1_clicks": c1, "p2_clicks": c2, "click_diff": c2 - c1, "click_pct": pct(c1, c2),
                "p1_impressions": i1, "p2_impressions": i2, "imp_diff": i2 - i1, "imp_pct": pct(i1, i2),
                "p1_ctr": round(r1, 4), "p2_ctr": round(r2, 4), "ctr_diff": round(r2 - r1, 4),
                "p1_position": round(q1, 1), "p2_position": round(q2, 1), "position_diff": round(q1 - q2, 1)
            })
        })
        .collect();
    let change = |row: &Value| row["click_diff"].as_f64().unwrap_or_default().abs();
    comparison.sort_by(|a, b| change(b).total_cmp(&change(a)));
    let showing = (input["limit"].as_u64().unwrap_or(10) as usize).min(comparison.len());
    comparison.truncate(showing);
    Ok(json!({
        "site_url": site_url,
        "period1": {"start": text(input, "period1_start"), "end": text(input, "period1_end")},
        "period2": {"start": text(input, "period2_start"), "end": text(input, "period2_end")},
        "dimensions": dimensions,
        "showing": showing,
        "comparison": comparison
    }))
}

async fn indexing_issues(api: &Api<'_>, input: &Value) -> ToolResult<Value> {
    let site_url = text(input, "site_url");
    let urls = urls(input)?;
    let (mut not_indexed, mut canonical, mut robots, mut fetch, mut indexed) =
        (vec![], vec![], vec![], vec![], vec![]);
    for page in &urls {
        let result = match inspect(api, site_url, page, None).await {
            Ok(result) => result,
            Err(error) => {
                not_indexed.push(json!({"url": page, "error": error.message}));
                continue;
            }
        };
        let status = &result["inspectionResult"]["indexStatusResult"];
        let verdict = status["verdict"].as_str().unwrap_or("UNKNOWN");
        let coverage = status["coverageState"].as_str().unwrap_or("Unknown");
        let lower = coverage.to_lowercase();
        if verdict != "PASS" || lower.contains("not indexed") || lower.contains("excluded") {
            not_indexed.push(json!({"url": page, "coverage_state": coverage}));
        } else {
            indexed.push(json!(page));
        }
        let google = status["googleCanonical"].as_str().unwrap_or_default();
        let user = status["userCanonical"].as_str().unwrap_or_default();
        if !google.is_empty() && !user.is_empty() && google != user {
            canonical
                .push(json!({"url": page, "google_canonical": google, "user_canonical": user}));
        }
        if status["robotsTxtState"].as_str() == Some("BLOCKED") {
            robots.push(json!(page));
        }
        let state = status["pageFetchState"].as_str().unwrap_or_default();
        if !state.is_empty() && state != "SUCCESSFUL" {
            fetch.push(json!({"url": page, "page_fetch_state": state}));
        }
    }
    Ok(json!({
        "site_url": site_url,
        "summary": {
            "total_checked": urls.len(),
            "indexed": indexed.len(),
            "not_indexed": not_indexed.len(),
            "canonical_issues": canonical.len(),
            "robots_blocked": robots.len(),
            "fetch_issues": fetch.len()
        },
        "issues": {
            "not_indexed": not_indexed,
            "canonical_issues": canonical,
            "robots_blocked": robots,
            "fetch_issues": fetch
        },
        "indexed_urls": indexed
    }))
}

/// Each row's dimension keys by name plus its rounded metrics.
fn rows(dimensions: &[String], response: &Value) -> Vec<Value> {
    response["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            let mut entry: Map<String, Value> = dimensions
                .iter()
                .enumerate()
                .map(|(index, dimension)| {
                    (
                        dimension.clone(),
                        row["keys"].get(index).cloned().unwrap_or(Value::Null),
                    )
                })
                .collect();
            if let Value::Object(metrics) = metrics(row) {
                entry.extend(metrics);
            }
            Value::Object(entry)
        })
        .collect()
}
fn metrics(row: &Value) -> Value {
    let n = |key: &str| row[key].as_f64().unwrap_or_default();
    json!({
        "clicks": n("clicks"),
        "impressions": n("impressions"),
        "ctr": round(n("ctr"), 4),
        "position": round(n("position"), 1)
    })
}
fn round(value: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    (value * scale).round() / scale
}
fn datetime(value: &str, format: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|at| at.format(format).to_string())
        .unwrap_or_else(|_| value.to_owned())
}
const MINUTES: &str = "%Y-%m-%d %H:%M";

fn inspection(site_url: &str, page: &str, response: &Value) -> Value {
    let inspection = &response["inspectionResult"];
    let status = &inspection["indexStatusResult"];
    let referring: Vec<Value> = status["referringUrls"]
        .as_array()
        .into_iter()
        .flatten()
        .take(5)
        .cloned()
        .collect();
    json!({
        "page_url": page,
        "site_url": site_url,
        "inspection_result_link": inspection.get("inspectionResultLink"),
        "verdict": status["verdict"].as_str().unwrap_or("UNKNOWN"),
        "coverage_state": status.get("coverageState"),
        "last_crawled": status["lastCrawlTime"].as_str().map(|at| datetime(at, MINUTES)),
        "page_fetch_state": status.get("pageFetchState"),
        "robots_txt_state": status.get("robotsTxtState"),
        "indexing_state": status.get("indexingState"),
        "google_canonical": status.get("googleCanonical"),
        "user_canonical": status.get("userCanonical"),
        "crawled_as": status.get("crawledAs"),
        "referring_urls": referring,
        "rich_results": inspection.get("richResultsResult").map(rich_results)
    })
}
fn rich_results(rich: &Value) -> Value {
    let types: Vec<&str> = rich["detectedItems"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["richResultType"].as_str())
        .collect();
    let issues: Vec<Value> = rich["richResultsIssues"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|issue| json!({"severity": issue.get("severity"), "message": issue.get("message")}))
        .collect();
    json!({
        "verdict": rich["verdict"].as_str().unwrap_or("UNKNOWN"),
        "detected_types": types,
        "issues": issues
    })
}

/// Counts arrive as numbers or numeric strings.
fn count(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}
fn web_count(sitemap: &Value, field: &str) -> Option<u64> {
    sitemap["contents"]
        .as_array()?
        .iter()
        .find(|content| content["type"].as_str() == Some("web"))
        .and_then(|content| count(&content[field]))
}
fn kind(sitemap: &Value) -> &'static str {
    match sitemap["isSitemapsIndex"].as_bool().unwrap_or(false) {
        true => "Index",
        false => "Sitemap",
    }
}
fn sitemap_summary(sitemap: &Value) -> Value {
    let errors = count(&sitemap["errors"]).unwrap_or_default();
    let warnings = count(&sitemap["warnings"]).unwrap_or_default();
    json!({
        "path": sitemap["path"].as_str().unwrap_or("Unknown"),
        "last_submitted": sitemap["lastSubmitted"].as_str().map(|at| datetime(at, MINUTES)),
        "last_downloaded": sitemap["lastDownloaded"].as_str().map(|at| datetime(at, MINUTES)),
        "type": kind(sitemap),
        "is_pending": sitemap["isPending"].as_bool().unwrap_or(false),
        "url_count": web_count(sitemap, "submitted"),
        "indexed_urls": web_count(sitemap, "indexed"),
        "status": if errors > 0 { "Has errors" } else if warnings > 0 { "Has warnings" } else { "Valid" },
        "errors": errors,
        "warnings": warnings
    })
}
fn sitemap_details(site_url: &str, sitemap_url: &str, details: &Value) -> Value {
    let contents: Vec<Value> = details["contents"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|content| {
            json!({
                "type": content["type"].as_str().unwrap_or("unknown").to_uppercase(),
                "submitted": count(&content["submitted"]).unwrap_or_default(),
                "indexed": content.get("indexed")
            })
        })
        .collect();
    json!({
        "sitemap_url": sitemap_url,
        "site_url": site_url,
        "type": kind(details),
        "status": if details["isPending"].as_bool().unwrap_or(false) { "pending" } else { "processed" },
        "last_submitted": details["lastSubmitted"].as_str().map(|at| datetime(at, MINUTES)),
        "last_downloaded": details["lastDownloaded"].as_str().map(|at| datetime(at, MINUTES)),
        "errors": count(&details["errors"]).unwrap_or_default(),
        "warnings": count(&details["warnings"]).unwrap_or_default(),
        "content_breakdown": contents,
        "is_index": details["isSitemapsIndex"].as_bool().unwrap_or(false)
    })
}
