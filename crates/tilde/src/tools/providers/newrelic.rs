//! New Relic over NerdGraph (its GraphQL API), read-only: NRQL queries, alert incidents and
//! policies, and entity search, all in the connection's account. The user API key goes in
//! `API-Key`; EU accounts use `api.eu.newrelic.com`.
use super::ToolProvider;
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_DEVELOPER_TOOLS, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("New Relic account name".into()),
        icon_url: Some("/provider-icons/newrelic.svg".into()),
        instructions: Some(
            "Connect New Relic so agents can run NRQL queries, read alert incidents and policies and find entities. Create a user API key (API keys in the user menu; it starts with NRAK-) and give the account ID it should query."
                .into(),
        ),
        id: "newrelic".into(),
        name: "New Relic".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "New Relic user API key".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "account_id":{"type":"string","title":"Account ID","pattern":"^[0-9]{1,12}$"},
                    "api_key":{"type":"string","title":"User API key","description":"e.g. NRAK-...","minLength":1,"writeOnly":true},
                    "region":{"type":"string","title":"Region","enum":["us","eu"],"default":"us"}
                },"required":["account_id","api_key"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool, model::Capability::Egress],
        }],
    }
}

pub struct Newrelic;

fn tool(
    name: &str,
    summary: &str,
    description: &str,
    properties: Value,
    required: &[&str],
) -> types::ToolDefinition {
    super::rest::definition(
        "newrelic",
        name,
        summary,
        description,
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
        super::rest::Hints {
            read_only: true,
            destructive: false,
        },
    )
}

impl ToolProvider for Newrelic {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        vec![
            tool(
                "query_nrql",
                "Ran a NRQL query",
                "Run a NRQL query in the connection's account, e.g. `SELECT count(*) FROM TransactionError FACET appName SINCE 1 hour ago`.",
                json!({"nrql":{"type":"string","minLength":1,"maxLength":4096},"timeout":{"type":"integer","minimum":1,"maximum":120,"description":"Seconds; default 30."}}),
                &["nrql"],
            ),
            tool(
                "list_alert_incidents",
                "Listed New Relic alert incidents",
                "List alert incident events (opened and closed) from the NrAiIncident event type, newest first.",
                json!({"since":{"type":"string","pattern":"^[0-9]{1,4} (minute|hour|day|week)s? ago$","description":"e.g. 1 day ago (the default)."},"open_only":{"type":"boolean","description":"Only events that opened an incident."},"limit":{"type":"integer","minimum":1,"maximum":500}}),
                &[],
            ),
            tool(
                "list_alert_policies",
                "Listed New Relic alert policies",
                "List the account's alert policies.",
                json!({"name":{"type":"string","description":"Only policies whose name contains this."},"cursor":{"type":"string"}}),
                &[],
            ),
            tool(
                "search_entities",
                "Searched New Relic entities",
                "Find entities (services, hosts, dashboards) with an entity search query, e.g. `name LIKE 'checkout' AND domain = 'APM'`, with their alert severity.",
                json!({"query":{"type":"string","minLength":1,"maxLength":2048},"cursor":{"type":"string"}}),
                &["query"],
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
            let account: i64 = access
                .secret("account_id")?
                .parse()
                .map_err(|_| ConnectError::failed_precondition("Invalid New Relic account ID"))?;
            const NRQL: &str = "query($account: Int!, $nrql: Nrql!, $timeout: Seconds) { actor { account(id: $account) { nrql(query: $nrql, timeout: $timeout) { results metadata { timeWindow { since until } } } } } }";
            match name {
                "query_nrql" => {
                    let data = query(access, NRQL, json!({"account": account, "nrql": input["nrql"], "timeout": input["timeout"].as_i64().unwrap_or(30)})).await?;
                    Ok(data["actor"]["account"]["nrql"].clone())
                }
                "list_alert_incidents" => {
                    let nrql = format!(
                        "SELECT * FROM NrAiIncident {}SINCE {} LIMIT {}",
                        if input["open_only"] == true {
                            "WHERE event = 'open' "
                        } else {
                            ""
                        },
                        input["since"].as_str().unwrap_or("1 day ago"),
                        input["limit"].as_i64().unwrap_or(100),
                    );
                    let data = query(
                        access,
                        NRQL,
                        json!({"account": account, "nrql": nrql, "timeout": 30}),
                    )
                    .await?;
                    Ok(json!({"incidents": data["actor"]["account"]["nrql"]["results"]}))
                }
                "list_alert_policies" => {
                    let data = query(
                        access,
                        "query($account: Int!, $cursor: String, $criteria: AlertsPoliciesSearchCriteriaInput) { actor { account(id: $account) { alerts { policiesSearch(cursor: $cursor, searchCriteria: $criteria) { policies { id name incidentPreference } nextCursor totalCount } } } } }",
                        json!({"account": account, "cursor": input["cursor"], "criteria": input.get("name").map(|name| json!({"nameLike": name}))}),
                    )
                    .await?;
                    Ok(data["actor"]["account"]["alerts"]["policiesSearch"].clone())
                }
                "search_entities" => {
                    let data = query(
                        access,
                        "query($query: String!, $cursor: String) { actor { entitySearch(query: $query) { count results(cursor: $cursor) { nextCursor entities { guid name entityType domain type reporting permalink ... on AlertableEntityOutline { alertSeverity } } } } } }",
                        // Other accounts the key reaches stay out of the results.
                        json!({"query": format!("({}) AND accountId = {account}", input["query"].as_str().unwrap_or_default()), "cursor": input["cursor"]}),
                    )
                    .await?;
                    Ok(data["actor"]["entitySearch"].clone())
                }
                _ => Err(ConnectError::not_found("Unsupported New Relic tool")),
            }
        })
    }
}

/// One NerdGraph request; GraphQL errors fail the call with New Relic's first message.
async fn query(access: &Access, query: &str, variables: Value) -> ToolResult<Value> {
    let base = match model::optional(&access.values, "region") {
        Some("eu") => "https://api.eu.newrelic.com",
        _ => "https://api.newrelic.com",
    };
    let url = access.url("newrelic_api", base, &["graphql"])?;
    let response = access
        .json(
            access
                .http
                .client
                .post(url)
                .header("API-Key", access.secret("api_key")?)
                .json(&json!({"query": query, "variables": variables})),
        )
        .await?;
    if let Some(error) = response["errors"].as_array().and_then(|e| e.first()) {
        return Err(ConnectError::failed_precondition(format!(
            "New Relic: {}",
            error["message"].as_str().unwrap_or("request failed")
        )));
    }
    Ok(response["data"].clone())
}
