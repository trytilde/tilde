//! Official Sentry MCP tools beyond the hand-written ones: most adapt their camelCase inputs onto
//! one REST operation, the rest compose calls or reach Sentry's documentation.
use super::{
    CATALOG, CURATED, Client, encode, invalid, is_destructive_name, is_mutating_name, openapi,
    required, resolve_issue_locator, scalar, send, text,
};
use crate::chat::tools::ToolResult;
use connectrpc::ConnectError;
use serde_json::{Map, Value, json};

const OPERATIONS: &[(&str, &str)] = &[
    ("add_team_to_project", "addProjectTeam"),
    ("analyze_issue_with_seer", "startOrganizationIssueAutofix"),
    ("create_dsn", "createProjectKey"),
    ("create_project", "createOrganizationProject"),
    ("create_team", "createOrganizationTeam"),
    ("find_dashboards", "listOrganizationDashboards"),
    ("find_dsns", "listProjectKeys"),
    ("find_monitors", "listOrganizationMonitors"),
    ("find_releases", "listOrganizationReleases"),
    ("find_teams", "listOrganizationTeams"),
    ("get_dashboard_details", "getOrganizationDashboard"),
    ("get_event_attachment", "getProjectEventAttachment"),
    ("get_issue_tag_values", "listOrganizationIssueTagValues"),
    (
        "get_latest_base_snapshot",
        "getOrganizationPreprodArtifactSnapshotLatestBase",
    ),
    ("get_monitor_details", "getOrganizationMonitor"),
    ("get_profile", "getOrganizationProfilingFlamegraph"),
    ("get_profile_details", "getProjectProfilingProfile"),
    ("get_release_details", "getOrganizationRelease"),
    ("get_replay_details", "getOrganizationReplay"),
    ("get_snapshot", "getOrganizationPreprodArtifactSnapshot"),
    (
        "get_snapshot_image",
        "getOrganizationPreprodArtifactSnapshotImage",
    ),
    ("get_span_details", "getOrganizationTrace"),
    ("get_trace_details", "getOrganizationTrace"),
    ("remove_team_from_project", "deleteProjectTeam"),
    ("search_events", "listOrganizationEvents"),
    ("update_dsn", "updateProjectKey"),
    ("update_project", "updateProject"),
];
/// Operations the hand-written tools already cover.
const CURATED_OPERATIONS: &[&str] = &[
    "listOrganizations",
    "listOrganizationProjects",
    "listOrganizationIssues",
    "getOrganizationIssue",
    "updateOrganizationIssue",
    "listOrganizationIssueEvents",
    "getOrganizationIssueEvent",
];

/// An operation represented by an official tool is not generated again, keeping one tool per
/// capability.
pub(super) fn covers(operation: &str) -> bool {
    OPERATIONS.iter().any(|(_, id)| *id == operation) || CURATED_OPERATIONS.contains(&operation)
}

pub(super) async fn invoke(client: &Client<'_>, name: &str, mut input: Value) -> ToolResult<Value> {
    if let Some(issue_url) = text(&input, "issueUrl") {
        let (organization, issue) = resolve_issue_locator(
            text(&input, "organizationSlug"),
            text(&input, "issueId"),
            Some(issue_url),
        )?;
        input["organizationSlug"] = json!(organization);
        input["issueId"] = json!(issue);
    }
    if matches!(name, "add_team_to_project" | "remove_team_from_project") {
        let operation = if name == "add_team_to_project" {
            "addProjectTeam"
        } else {
            "deleteProjectTeam"
        };
        call(client, name, operation, &input).await?;
        let teams = call(client, name, "listProjectTeams", &input).await?;
        return Ok(json!({"changed": true, "teams": teams}));
    }
    if let Some((_, operation)) = OPERATIONS.iter().find(|(tool, _)| *tool == name) {
        let value = call(client, name, operation, &input).await?;
        return Ok(match name {
            "create_dsn" => json!({"dsn": value}),
            "create_team" => json!({"team": value}),
            "find_dashboards" => json!({"dashboards": value, "nextCursor": null}),
            "find_dsns" => json!({"dsns": value}),
            "find_monitors" => json!({"monitors": value, "hasMore": false}),
            "find_releases" => json!({"releases": value, "hasMore": false}),
            "find_teams" => json!({"teams": value, "hasMore": false}),
            "get_issue_tag_values" => json!({"tag": value}),
            "update_project" => json!({"project": value}),
            _ => value,
        });
    }
    match name {
        "execute_sentry_tool" => execute(client, input).await,
        "find_alert_rules" => find_alert_rules(client, &input).await,
        "get_alert_rule" => get_alert_rule(client, &input).await,
        "get_ai_conversation_details" => ai_conversation(client, &input, false).await,
        "search_ai_conversations" => ai_conversation(client, &input, true).await,
        "get_doc" => get_doc(client, &input).await,
        "search_docs" => search_docs(client, &input).await,
        "get_issue_breadcrumbs" => {
            input["eventId"] = json!("latest");
            call(client, name, "getOrganizationIssueEvent", &input).await
        }
        "get_issue_user_reports" => {
            let organization = required(&input, "organizationSlug")?;
            let issue = required(&input, "issueId")?;
            let path = format!(
                "/api/0/organizations/{}/issues/{}/user-reports/",
                encode(organization),
                encode(issue)
            );
            get_with_query(
                client,
                &input,
                &path,
                &["organizationSlug", "issueId", "issueUrl", "regionUrl"],
            )
            .await
        }
        "get_sentry_resource" => get_resource(client, &input).await,
        "search_sentry_tools" => search_tools(&input),
        _ => Err(ConnectError::not_found("Unsupported Sentry tool")),
    }
}

/// Call `operation` with the tool's inputs renamed to the operation's parameters; inputs the
/// operation does not declare are dropped.
async fn call(
    client: &Client<'_>,
    tool: &str,
    operation: &str,
    input: &Value,
) -> ToolResult<Value> {
    let operation = openapi::operation(operation)?;
    let source = input.as_object().cloned().unwrap_or_default();
    let mut adapted = Map::new();
    for target in operation.input_schema["properties"]
        .as_object()
        .into_iter()
        .flat_map(Map::keys)
    {
        if let Some(value) = source
            .get(target)
            .cloned()
            .or_else(|| official_parameter(tool, target, &source))
        {
            adapted.insert(target.clone(), value);
        }
    }
    let value = openapi::invoke(client, operation, Value::Object(adapted)).await?;
    if tool == "get_span_details"
        && let Some(span) = text(input, "spanId").and_then(|span| find_span(&value, span))
    {
        return Ok(span.clone());
    }
    Ok(value)
}

fn official_parameter(tool: &str, target: &str, source: &Map<String, Value>) -> Option<Value> {
    let project = || {
        if source.contains_key("projectSlug") {
            "projectSlug"
        } else {
            "projectSlugOrId"
        }
    };
    let name = match target {
        "organization_id_or_slug" => "organizationSlug",
        "project_id_or_slug" | "project" => project(),
        "team_id_or_slug" => "teamSlug",
        "issue_id" => "issueId",
        "event_id" => "eventId",
        "attachment_id" => "attachmentId",
        "key_id" => "keyId",
        "workflow_id" => "ruleIdOrName",
        "dashboard_id" => "dashboardIdOrTitle",
        "monitor_id_or_slug" => "monitorSlug",
        "version" => "releaseVersion",
        "replay_id" => "replayId",
        "snapshot_id" => "snapshotId",
        "image_identifier" => "imageIdentifier",
        "app_id" => "appId",
        "profile_id" => "profileId",
        "trace_id" => "traceId",
        "key" => "tagKey",
        "per_page" => "limit",
        "statsPeriod" => "period",
        "field" => "fields",
        "user_context" if tool == "analyze_issue_with_seer" => "instruction",
        "repo_name" if tool == "create_project" => "repository",
        "rateLimit" if source.contains_key("rateLimitCount") => {
            return Some(json!({
                "count": source.get("rateLimitCount"),
                "window": source.get("rateLimitWindow")
            }));
        }
        "dynamicSdkLoaderOptions" => {
            return Some(json!({
                "hasPerformance": source.get("loaderHasPerformance"),
                "hasReplay": source.get("loaderHasReplay"),
                "hasDebug": source.get("loaderHasDebug"),
                "hasFeedback": source.get("loaderHasFeedback"),
                "hasLogs": source.get("loaderHasLogsAndMetrics")
            }));
        }
        _ => return None,
    };
    source.get(name).cloned()
}

fn find_span<'a>(value: &'a Value, span: &str) -> Option<&'a Value> {
    match value {
        Value::Object(fields) => {
            if fields
                .get("span_id")
                .or_else(|| fields.get("spanId"))
                .and_then(Value::as_str)
                == Some(span)
            {
                return Some(value);
            }
            fields.values().find_map(|value| find_span(value, span))
        }
        Value::Array(items) => items.iter().find_map(|value| find_span(value, span)),
        _ => None,
    }
}

/// Run any Sentry tool by name, or any catalog operation by its ID. The arguments are not checked
/// against the named tool's schema, as in the official server.
async fn execute(client: &Client<'_>, input: Value) -> ToolResult<Value> {
    let name = required(&input, "name")?;
    let name = name.strip_prefix("sentry_").unwrap_or(name);
    let arguments = match input.get("arguments") {
        Some(Value::Null) | None => json!({}),
        Some(arguments) => arguments.clone(),
    };
    if name == "execute_sentry_tool" {
        return Err(invalid("execute_sentry_tool cannot run itself"));
    }
    if CURATED.iter().any(|(tool, _, _)| *tool == name)
        || CATALOG.official_mcp_tools.iter().any(|t| t.name == name)
        || name == "ingest_event_via_dsn"
        || openapi::generated(name).is_some()
    {
        return super::invoke(client, name, arguments).await;
    }
    let operation = match OPERATIONS.iter().find(|(tool, _)| *tool == name) {
        Some((_, operation)) => openapi::operation(operation)?,
        None => openapi::operation(name).map_err(|_| {
            invalid(format!(
                "Tool {name} is not executable through the generic dispatcher; call it directly"
            ))
        })?,
    };
    openapi::invoke(client, operation, arguments).await
}

/// GET `path` with every other non-null input as a query parameter.
async fn get_with_query(
    client: &Client<'_>,
    input: &Value,
    path: &str,
    excluded: &[&str],
) -> ToolResult<Value> {
    let base = client.base(text(input, "regionUrl"))?;
    let query = input
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(name, value)| !excluded.contains(&name.as_str()) && !value.is_null())
        .map(|(name, value)| (name.clone(), json!(scalar(value))))
        .collect::<Vec<_>>();
    client.get(&base, path, &query).await
}

async fn find_alert_rules(client: &Client<'_>, input: &Value) -> ToolResult<Value> {
    let organization = encode(required(input, "organizationSlug")?);
    let excluded = ["organizationSlug", "regionUrl", "kind"];
    let kind = text(input, "kind").unwrap_or("all");
    let issue_rules = if kind == "metric" {
        json!([])
    } else {
        let path = format!("/api/0/organizations/{organization}/workflows/");
        get_with_query(client, input, &path, &excluded).await?
    };
    let metric_rules = if kind == "issue" {
        json!([])
    } else {
        let path = format!("/api/0/organizations/{organization}/alert-rules/");
        get_with_query(client, input, &path, &excluded).await?
    };
    Ok(json!({
        "issueRules": issue_rules,
        "metricRules": metric_rules,
        "pagination": {"nextCursor": null}
    }))
}

/// A rule by numeric ID, otherwise the first listed rule with that name.
async fn get_alert_rule(client: &Client<'_>, input: &Value) -> ToolResult<Value> {
    let organization = encode(required(input, "organizationSlug")?);
    let rule = required(input, "ruleIdOrName")?;
    let kind = text(input, "kind").unwrap_or("issue");
    let collection = if kind == "metric" {
        "alert-rules"
    } else {
        "workflows"
    };
    if rule.chars().all(|c| c.is_ascii_digit()) {
        let path = format!(
            "/api/0/organizations/{organization}/{collection}/{}/",
            encode(rule)
        );
        let base = client.base(text(input, "regionUrl"))?;
        return client.get(&base, &path, &[]).await;
    }
    let path = format!("/api/0/organizations/{organization}/{collection}/");
    let listed = get_with_query(
        client,
        input,
        &path,
        &["organizationSlug", "regionUrl", "kind", "ruleIdOrName"],
    )
    .await?;
    listed
        .as_array()
        .and_then(|items| {
            items.iter().find(|item| {
                item["name"]
                    .as_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(rule))
            })
        })
        .cloned()
        .ok_or_else(|| {
            ConnectError::not_found(format!("Sentry {kind} alert rule {rule} was not found"))
        })
}

async fn ai_conversation(client: &Client<'_>, input: &Value, search: bool) -> ToolResult<Value> {
    let organization = encode(required(input, "organizationSlug")?);
    let path = if search {
        format!("/api/0/organizations/{organization}/ai-conversations/")
    } else {
        let conversation = encode(required(input, "conversationId")?);
        format!("/api/0/organizations/{organization}/ai-conversations/{conversation}/")
    };
    get_with_query(
        client,
        input,
        &path,
        &["organizationSlug", "conversationId", "regionUrl"],
    )
    .await
}

async fn get_resource(client: &Client<'_>, input: &Value) -> ToolResult<Value> {
    let kind = required(input, "resourceType")?;
    let id = required(input, "resourceId")?;
    let organization = required(input, "organizationSlug")?;
    let mut routed = json!({"organizationSlug": organization, "regionUrl": input.get("regionUrl")});
    let (key, operation) = match kind {
        "issue" => ("issueId", "getOrganizationIssue"),
        "trace" | "span" => ("traceId", "getOrganizationTrace"),
        "replay" => ("replayId", "getOrganizationReplay"),
        "monitor" => ("monitorSlug", "getOrganizationMonitor"),
        "snapshot" => ("snapshotId", "getOrganizationPreprodArtifactSnapshot"),
        "ai_conversation" => {
            routed["conversationId"] = json!(id);
            return ai_conversation(client, &routed, false).await;
        }
        _ => return Err(invalid(format!("Unsupported Sentry resource type {kind}"))),
    };
    routed[key] = json!(id);
    call(client, "get_sentry_resource", operation, &routed).await
}

async fn get_doc(client: &Client<'_>, input: &Value) -> ToolResult<Value> {
    let path = required(input, "path")?;
    if !path.starts_with('/') || !path.ends_with(".md") || path.contains("..") {
        return Err(invalid(
            "Sentry documentation path must be an absolute .md path without traversal",
        ));
    }
    let origin = client
        .access
        .endpoints
        .0
        .get("sentry_docs")
        .map(String::as_str)
        .unwrap_or("https://docs.sentry.io");
    let url = url::Url::parse(&format!("{}{path}", origin.trim_end_matches('/')))
        .map_err(|_| invalid("Invalid Sentry documentation path"))?;
    send(
        client
            .access
            .http
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "text/markdown, text/plain"),
    )
    .await
}

async fn search_docs(client: &Client<'_>, input: &Value) -> ToolResult<Value> {
    required(input, "query")?;
    let url = client
        .access
        .endpoints
        .0
        .get("sentry_docs_search")
        .map(String::as_str)
        .unwrap_or("https://mcp.sentry.dev/api/search");
    send(client.access.http.client.post(url).json(input)).await
}

/// Lexical search over the Sentry tools, official ones first, with MCP-style annotations.
fn search_tools(input: &Value) -> ToolResult<Value> {
    let query = required(input, "query")?.to_ascii_lowercase();
    let limit = input["limit"].as_u64().unwrap_or(10).min(100) as usize;
    let mut results = Vec::new();
    for tool in &CATALOG.official_mcp_tools {
        if tool.name.to_ascii_lowercase().contains(&query)
            || tool.description.to_ascii_lowercase().contains(&query)
        {
            results.push(json!({
                "name": format!("sentry_{}", tool.name),
                "description": tool.description,
                "inputSchema": tool.input_schema,
                "annotations": {
                    "readOnlyHint": !is_mutating_name(&tool.name),
                    "destructiveHint": is_destructive_name(&tool.name),
                    "idempotentHint": false,
                    "openWorldHint": true
                }
            }));
        }
    }
    for operation in openapi::operations() {
        if results.len() >= limit {
            break;
        }
        if operation.operation_id.to_ascii_lowercase().contains(&query)
            || operation.summary.to_ascii_lowercase().contains(&query)
            || operation.description.to_ascii_lowercase().contains(&query)
        {
            results.push(json!({
                "name": format!("sentry_{}", openapi::snake_case(&operation.operation_id)),
                "description": operation.description,
                "inputSchema": operation.input_schema,
                "annotations": {
                    "readOnlyHint": operation.method == "GET",
                    "destructiveHint": operation.method == "DELETE",
                    "idempotentHint": matches!(operation.method.as_str(), "GET" | "PUT" | "DELETE"),
                    "openWorldHint": true
                }
            }));
        }
    }
    results.truncate(limit);
    Ok(json!({"query": query, "results": results}))
}
