//! Egress to the API providers agents read from sandboxes: Linear and New Relic (GraphQL), Sentry,
//! Jira, PagerDuty, Datadog, Grafana, Prometheus, Honeycomb and HCP Terraform (REST). A sandbox's
//! client is pointed at the route and presents the egress token where it would send its key; the
//! gateway checks the request and attaches the connection's credential as the provider takes it
//! (`mints::stored`: bearer, the provider's key headers, Basic or `Token token=`; an OAuth access
//! token the connection keeps refreshed). None of these providers can mint a narrower credential
//! per request, so the rules here and a grant's config are the narrowing.
//!
//! Upstreams are the provider's or read from the connection, never from a grant: a self-hosted
//! Sentry's `base_url`, the Datadog site, the Grafana or Prometheus URL, the Jira site, the
//! PagerDuty, New Relic or Honeycomb region, the Terraform Enterprise hostname.
//!
//! Pointing a client at the route (`TILDE_EGRESS_<SLUG>_URL` is the route, `TILDE_EGRESS_<SLUG>_TOKEN`
//! the token, and each provider's own variables are set as listed in its `wiring`):
//! - Linear: `new LinearClient({ apiKey: process.env.LINEAR_API_KEY, apiUrl: process.env.LINEAR_API_URL })`
//!   (`LINEAR_API_URL` is the route's `/graphql`).
//! - Sentry: `sentry-cli` reads `SENTRY_URL` and `SENTRY_AUTH_TOKEN`; REST calls go to
//!   `$SENTRY_URL/api/0/...` with `Authorization: Bearer $SENTRY_AUTH_TOKEN`.
//! - Jira: `$JIRA_URL/rest/api/3/...` with Basic auth, any user and `$JIRA_API_TOKEN` as the password.
//! - PagerDuty: `$TILDE_EGRESS_<SLUG>_URL/incidents` with `Authorization: Token token=$TILDE_EGRESS_<SLUG>_TOKEN`.
//! - Datadog: `DD_API_KEY`, `DD_APP_KEY` and `DATADOG_HOST` (datadogpy, `dog`); the
//!   datadog-api-client libraries take the route as `configuration.host`.
//! - Grafana: `GRAFANA_URL` and `GRAFANA_SERVICE_ACCOUNT_TOKEN` (mcp-grafana, grafanactl).
//! - Prometheus: `promtool query instant "$PROMETHEUS_URL" 'up'` needs a bearer; curl
//!   `$PROMETHEUS_URL/api/v1/query` with `Authorization: Bearer $TILDE_EGRESS_<SLUG>_TOKEN`.
//! - New Relic: `$TILDE_EGRESS_<SLUG>_URL/graphql` with `API-Key: $NEW_RELIC_API_KEY`.
//! - Honeycomb: `HONEYCOMB_API_ENDPOINT` and `HONEYCOMB_API_KEY`.
//! - Terraform: `$TILDE_EGRESS_<SLUG>_URL/api/v2/...` with `Authorization: Bearer $TFE_TOKEN`
//!   (go-tfe replaces an address's path, so `TFE_ADDRESS` cannot name the route).
use crate::chat::providers::Access;
use crate::connections::egress::{
    Decision, Egress, EgressHook, EgressProtocol, EgressRequest, EnvTemplate, forge::strings,
    graphql, rules,
};
use crate::connections::model;
use crate::error::Error;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// One provider's egress: its definition, where its upstream is, and its grant check.
pub struct Api {
    egress: Egress,
    upstream: fn(&Access) -> Result<String, Error>,
    check: fn(&EgressRequest<'_>, &Value) -> Decision,
}

static HOOKS: LazyLock<BTreeMap<&'static str, Api>> = LazyLock::new(|| {
    BTreeMap::from([
        ("linear", linear()),
        ("sentry", sentry()),
        ("jira", jira()),
        ("pagerduty", pagerduty()),
        ("datadog", datadog()),
        ("grafana", grafana()),
        ("prometheus", prometheus()),
        ("newrelic", newrelic()),
        ("honeycomb", honeycomb()),
        ("terraform", terraform()),
    ])
});
pub fn hook(provider: &str) -> Option<&'static Api> {
    HOOKS.get(provider)
}

#[async_trait::async_trait]
impl EgressHook for Api {
    fn definition(&self) -> &Egress {
        &self.egress
    }
    fn check(&self, request: &EgressRequest<'_>, config: &Value) -> Decision {
        match rules::check(request) {
            Decision::Allow => (self.check)(request, config),
            denied => denied,
        }
    }
    fn upstream(&self, _protocol: &EgressProtocol, access: &Access) -> Result<String, Error> {
        (self.upstream)(access)
    }
    async fn setup_query(
        &self,
        _access: &Access,
        name: &str,
        _input: Value,
    ) -> Result<Value, Error> {
        Err(Error::Invalid(format!("Unknown setup query {name}")))
    }
}

fn http(
    upstream: &'static str,
    allow: Vec<&'static str>,
    deny: Vec<&'static str>,
    wiring: &[(&'static str, &'static str)],
) -> Vec<EgressProtocol> {
    vec![EgressProtocol::Http {
        upstream,
        allow,
        deny,
        wiring: wiring
            .iter()
            .map(|&(name, value)| EnvTemplate { name, value })
            .collect(),
    }]
}
fn schema(properties: Value) -> Value {
    json!({"type": "object", "properties": properties, "additionalProperties": false})
}
fn egress(protocols: Vec<EgressProtocol>, config_schema: Value) -> Egress {
    Egress {
        protocols,
        config_schema,
        config_version: 1,
        setup_queries: vec![],
    }
}
fn flag(config: &Value, key: &str) -> bool {
    config.get(key).and_then(Value::as_bool).unwrap_or(false)
}
fn reads(request: &EgressRequest<'_>) -> bool {
    matches!(request, EgressRequest::Http { method, .. } if method.eq_ignore_ascii_case("GET"))
}
fn body<'a>(request: &EgressRequest<'a>) -> &'a [u8] {
    match request {
        EgressRequest::Http { body, .. } => body,
        EgressRequest::Git { .. } => &[],
    }
}
/// The rules alone: read-only providers, whose allowlist has no writes.
fn allowed(_request: &EgressRequest<'_>, _config: &Value) -> Decision {
    Decision::Allow
}
fn https(url: Option<&str>) -> Result<String, Error> {
    let url = url.ok_or_else(|| Error::Invalid("The connection names no URL".into()))?;
    crate::tools::providers::rest::https(url)
        .map(str::to_owned)
        .map_err(|e| Error::Invalid(e.message.unwrap_or_default()))
}
/// The values of every query parameter whose key starts with `prefix` (`include`, `include[]`),
/// split at commas and lowercased.
fn query_values(request: &EgressRequest<'_>, prefix: &str) -> Vec<String> {
    let EgressRequest::Http { query, .. } = request else {
        return vec![];
    };
    url::form_urlencoded::parse(query.unwrap_or_default().as_bytes())
        .filter(|(k, _)| k.starts_with(prefix))
        .flat_map(|(_, v)| {
            v.split(',')
                .map(|v| v.trim().to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .collect()
}
/// GraphQL endpoints take the document in the body only: servers that also read `query`,
/// `variables` or `extensions` from the URL would run what the check never saw.
fn graphql_body_only(request: &EgressRequest<'_>) -> Option<Decision> {
    matches!(request, EgressRequest::Http { query: Some(_), .. })
        .then(|| Decision::deny("GraphQL requests carry no query string"))
}
fn regional(access: &Access, us: &str, eu: &str) -> String {
    match model::optional(&access.values, "region") {
        Some("eu") => eu.to_owned(),
        _ => us.to_owned(),
    }
}

/// Linear's GraphQL API: queries, and the mutations the grant names (by default creating and
/// updating issues and commenting). Linear cannot narrow a key per request or a query to teams;
/// a key restricted to teams when it is created narrows that.
fn linear() -> Api {
    Api {
        egress: egress(
            http(
                "https://api.linear.app",
                vec!["POST /graphql"],
                vec![],
                &[
                    ("LINEAR_API_KEY", "{token}"),
                    ("LINEAR_API_URL", "{url}/graphql"),
                ],
            ),
            schema(json!({
                "mutations": {
                    "type": "array",
                    "title": "Mutations",
                    "items": {"type": "string", "pattern": "^[a-z][A-Za-z]{1,80}$"},
                    "uniqueItems": true,
                    "default": ["issueCreate", "issueUpdate", "commentCreate"],
                    "description": "GraphQL mutations sandboxes may run, beside queries. Empty: read-only"
                }
            })),
        ),
        upstream: |_| Ok("https://api.linear.app".into()),
        check: |request, config| {
            graphql_body_only(request).unwrap_or_else(|| {
                graphql::check(body(request), &strings(config.get("mutations")), &[])
            })
        },
    }
}

/// New Relic's NerdGraph, read-only: queries (NRQL among them), no mutations, and nothing under
/// `apiAccess` (its API keys).
fn newrelic() -> Api {
    Api {
        egress: egress(
            http(
                "https://api.newrelic.com",
                vec!["POST /graphql"],
                vec![],
                &[("NEW_RELIC_API_KEY", "{token}")],
            ),
            schema(json!({})),
        ),
        upstream: |access| {
            Ok(regional(
                access,
                "https://api.newrelic.com",
                "https://api.eu.newrelic.com",
            ))
        },
        // `apiAccess` reads the account's API keys.
        check: |request, _| {
            graphql_body_only(request)
                .unwrap_or_else(|| graphql::check(body(request), &[], &["apiAccess"]))
        },
    }
}

/// Sentry's REST API under an organization: an allowlist of reads (the organization, its issues,
/// events, releases, projects, teams, environments, tags, traces, replays, monitors and stats),
/// and updating an issue (resolve, ignore, assign) when the grant allows it. A grant may narrow to organizations (slugs); paths that
/// name none are refused then. Organization-wide paths (issues by ID, search) are not narrowed
/// further by project.
fn sentry() -> Api {
    Api {
        egress: egress(
            http(
                "https://sentry.io",
                vec![
                    "GET /api/0",
                    "GET /api/0/organizations/{organization}",
                    "GET /api/0/organizations/{organization}/issues/**",
                    "GET /api/0/organizations/{organization}/events/**",
                    "GET /api/0/organizations/{organization}/events-stats",
                    "GET /api/0/organizations/{organization}/eventids/*",
                    "GET /api/0/organizations/{organization}/shortids/*",
                    "GET /api/0/organizations/{organization}/projects",
                    "GET /api/0/organizations/{organization}/teams",
                    "GET /api/0/organizations/{organization}/environments",
                    "GET /api/0/organizations/{organization}/releases/**",
                    "GET /api/0/organizations/{organization}/tags/**",
                    "GET /api/0/organizations/{organization}/trace/**",
                    "GET /api/0/organizations/{organization}/replays/**",
                    "GET /api/0/organizations/{organization}/monitors/**",
                    "GET /api/0/organizations/{organization}/stats_v2",
                    "GET /api/0/projects/{organization}/{project}",
                    "GET /api/0/projects/{organization}/{project}/issues/**",
                    "GET /api/0/projects/{organization}/{project}/events/**",
                    "GET /api/0/projects/{organization}/{project}/releases/**",
                    "GET /api/0/projects/{organization}/{project}/environments",
                    "GET /api/0/projects/{organization}/{project}/tags/**",
                    "PUT /api/0/organizations/{organization}/issues/*",
                ],
                vec![],
                &[("SENTRY_URL", "{url}"), ("SENTRY_AUTH_TOKEN", "{token}")],
            ),
            schema(json!({
                "organizations": {
                    "type": "array",
                    "title": "Organizations",
                    "items": {"type": "string", "pattern": "^[a-z0-9_-]{1,64}$"},
                    "uniqueItems": true,
                    "description": "Organization slugs. Empty: every organization the connection reaches"
                },
                "issue_writes": {
                    "type": "boolean",
                    "title": "Update issues",
                    "default": true,
                    "description": "Resolve, ignore and assign issues"
                }
            })),
        ),
        upstream: |access| match model::optional(&access.values, "base_url") {
            Some(url) => https(Some(url)),
            None => Ok("https://sentry.io".into()),
        },
        check: |request, config| {
            let EgressRequest::Http { path, .. } = request else {
                return Decision::deny("Sentry is reached over HTTP");
            };
            let organizations = strings(config.get("organizations"));
            let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
            match segments.get(2..4) {
                Some(["organizations" | "projects", organization]) => {
                    if !organizations.is_empty() && !organizations.contains(organization) {
                        return Decision::deny(format!("{organization} is not granted"));
                    }
                }
                _ if !organizations.is_empty() && segments.len() > 2 => {
                    return Decision::deny("The grant is limited to organizations: name one");
                }
                _ => {}
            }
            if !reads(request) && !flag(config, "issue_writes") {
                return Decision::deny("Updating issues is not granted");
            }
            Decision::Allow
        },
    }
}

/// Jira Cloud's REST API with an API token (the OAuth type reaches Jira through
/// `api.atlassian.com` by a cloud ID it looks up, so it offers tools only): reads and JQL search,
/// and creating, updating, commenting on and transitioning issues when the grant allows it.
fn jira() -> Api {
    Api {
        egress: egress(
            http(
                "https://atlassian.net",
                vec![
                    "GET /rest/api/*/**",
                    "GET /rest/agile/1.0/**",
                    "POST /rest/api/*/search",
                    "POST /rest/api/*/search/jql",
                    "POST /rest/api/*/issue",
                    "PUT /rest/api/*/issue/*",
                    "POST /rest/api/*/issue/*/comment",
                    "PUT /rest/api/*/issue/*/comment/*",
                    "POST /rest/api/*/issue/*/transitions",
                ],
                vec!["* /rest/api/*/webhook/**"],
                &[("JIRA_URL", "{url}"), ("JIRA_API_TOKEN", "{token}")],
            ),
            schema(json!({
                "writes": {
                    "type": "boolean",
                    "title": "Write issues",
                    "default": true,
                    "description": "Create, update, comment on and transition issues"
                }
            })),
        ),
        upstream: |access| https(model::optional(&access.values, "site_url")),
        check: |request, config| {
            let search =
                matches!(request, EgressRequest::Http { path, .. } if path.contains("/search"));
            if !reads(request) && !search && !flag(config, "writes") {
                return Decision::deny("Writing issues is not granted");
            }
            Decision::Allow
        },
    }
}

/// PagerDuty's REST API: incidents, services (never their integrations, whose keys they carry),
/// on-calls and schedules, adding notes to incidents and, when the grant allows it,
/// acknowledging and resolving them.
fn pagerduty() -> Api {
    Api {
        egress: egress(
            http(
                "https://api.pagerduty.com",
                vec![
                    "GET /incidents/**",
                    "GET /services",
                    "GET /services/*",
                    "GET /oncalls",
                    "GET /schedules/**",
                    "GET /escalation_policies/**",
                    "GET /log_entries/**",
                    "GET /teams/**",
                    "GET /priorities",
                    "GET /change_events/**",
                    "GET /users/me",
                    "POST /incidents/*/notes",
                    "PUT /incidents/*",
                ],
                vec![
                    // Integration keys.
                    "* /services/*/integrations/**",
                    "* /users/*/contact_methods/**",
                    "* /users/*/notification_rules/**",
                    "* /webhook_subscriptions/**",
                    "* /extensions/**",
                ],
                &[],
            ),
            schema(json!({
                "notes": {
                    "type": "boolean",
                    "title": "Incident notes",
                    "default": true,
                    "description": "Add notes to incidents"
                },
                "incident_updates": {
                    "type": "boolean",
                    "title": "Update incidents",
                    "default": false,
                    "description": "Acknowledge, resolve and reassign incidents"
                }
            })),
        ),
        upstream: |access| {
            Ok(regional(
                access,
                "https://api.pagerduty.com",
                "https://api.eu.pagerduty.com",
            ))
        },
        check: |request, config| match request {
            // Some servers split query parameters at `;` as well as `&`.
            EgressRequest::Http { query: Some(q), .. } if q.contains(';') => {
                Decision::deny("Query strings are separated with &")
            }
            // `include[]=integrations` embeds integration keys.
            _ if query_values(request, "include")
                .iter()
                .any(|v| v.contains("integration")) =>
            {
                Decision::deny("Service integrations are not readable")
            }
            EgressRequest::Http { method, .. } if method.eq_ignore_ascii_case("POST") => {
                if flag(config, "notes") {
                    Decision::Allow
                } else {
                    Decision::deny("Adding incident notes is not granted")
                }
            }
            _ if !reads(request) && !flag(config, "incident_updates") => {
                Decision::deny("Updating incidents is not granted")
            }
            _ => Decision::Allow,
        },
    }
}

/// Datadog's API, read-only: monitors, metrics, events, logs, spans, dashboards, SLOs, incidents.
fn datadog() -> Api {
    Api {
        egress: egress(
            http(
                "https://api.datadoghq.com",
                vec![
                    "GET /api/v1/validate",
                    "GET /api/v1/monitor/**",
                    "GET /api/v1/query",
                    "POST /api/v2/query/timeseries",
                    "POST /api/v2/query/scalar",
                    "GET /api/v1/metrics/**",
                    "GET /api/v2/metrics/**",
                    "GET /api/v1/events/**",
                    "GET /api/v2/events/**",
                    "POST /api/v2/events/search",
                    "GET /api/v2/logs/events/**",
                    "POST /api/v2/logs/events/search",
                    "GET /api/v2/spans/events/**",
                    "POST /api/v2/spans/events/search",
                    "GET /api/v1/dashboard/**",
                    "GET /api/v1/slo/**",
                    "GET /api/v2/incidents/**",
                    "GET /api/v1/hosts",
                    "GET /api/v1/downtime/**",
                ],
                vec![
                    "* /api/v1/api_key/**",
                    "* /api/v1/application_key/**",
                    "* /api/v2/api_keys/**",
                    "* /api/v2/application_keys/**",
                    "* /api/v2/current_user/application_keys/**",
                ],
                &[
                    ("DD_API_KEY", "{token}"),
                    ("DD_APP_KEY", "{token}"),
                    ("DATADOG_HOST", "{url}"),
                ],
            ),
            schema(json!({})),
        ),
        upstream: |access| {
            let site = model::optional(&access.values, "site").unwrap_or("datadoghq.com");
            if !crate::tools::providers::datadog::SITES.contains(&site) {
                return Err(Error::Invalid("Unknown Datadog site".into()));
            }
            Ok(format!("https://api.{site}"))
        },
        check: allowed,
    }
}

/// Grafana's HTTP API, read-only apart from data source queries (`POST /api/ds/query`).
fn grafana() -> Api {
    Api {
        egress: egress(
            http(
                "https://grafana.net",
                vec![
                    "GET /api/health",
                    "GET /api/user",
                    "GET /api/search",
                    "GET /api/dashboards/**",
                    "GET /api/folders/**",
                    "GET /api/datasources",
                    "GET /api/datasources/uid/*",
                    "GET /api/datasources/proxy/**",
                    "POST /api/ds/query",
                    "GET /api/annotations",
                    // Alerts and silences only: an alertmanager's config holds receiver secrets.
                    "GET /api/alertmanager/*/api/v2/alerts/**",
                    "GET /api/alertmanager/*/api/v2/silences/**",
                    "GET /api/prometheus/**",
                    "GET /api/v1/provisioning/alert-rules/**",
                ],
                vec![
                    "* /api/auth/keys/**",
                    "* /api/serviceaccounts/**",
                    "* /api/admin/**",
                ],
                &[
                    ("GRAFANA_URL", "{url}"),
                    ("GRAFANA_SERVICE_ACCOUNT_TOKEN", "{token}"),
                ],
            ),
            schema(json!({})),
        ),
        upstream: |access| https(model::optional(&access.values, "url")),
        check: allowed,
    }
}

/// The Prometheus HTTP API (and Mimir, Thanos, VictoriaMetrics), read-only.
fn prometheus() -> Api {
    Api {
        egress: egress(
            http(
                "https://prometheus.example.com",
                vec![
                    "GET /api/v1/**",
                    "POST /api/v1/query",
                    "POST /api/v1/query_range",
                    "POST /api/v1/query_exemplars",
                    "POST /api/v1/series",
                    "POST /api/v1/labels",
                ],
                vec!["* /api/v1/admin/**"],
                &[("PROMETHEUS_URL", "{url}")],
            ),
            schema(json!({})),
        ),
        upstream: |access| https(model::optional(&access.values, "url")),
        check: allowed,
    }
}

/// Honeycomb's API: triggers, SLOs, boards, columns, markers, and running queries.
fn honeycomb() -> Api {
    Api {
        egress: egress(
            http(
                "https://api.honeycomb.io",
                vec![
                    "GET /1/auth",
                    "GET /1/datasets/**",
                    "GET /1/columns/**",
                    "GET /1/triggers/**",
                    "GET /1/slos/**",
                    "GET /1/burn_alerts/**",
                    "GET /1/boards/**",
                    "GET /1/markers/**",
                    "GET /1/queries/**",
                    "POST /1/queries/*",
                    "GET /1/query_results/**",
                    "POST /1/query_results/*",
                ],
                vec![],
                &[
                    ("HONEYCOMB_API_ENDPOINT", "{url}"),
                    ("HONEYCOMB_API_KEY", "{token}"),
                ],
            ),
            schema(json!({})),
        ),
        upstream: |access| {
            Ok(regional(
                access,
                "https://api.honeycomb.io",
                "https://api.eu1.honeycomb.io",
            ))
        },
        check: allowed,
    }
}

/// HCP Terraform's (or Terraform Enterprise's) API, read-only, an allowlist: organizations,
/// projects, workspaces, a run's events, redacted plans and applies. No state, outputs,
/// variables, assessments, plan exports or tokens, including through `include`. Runs themselves
/// are not readable (they carry their variables' values), nor embeddable with `include`; a run's
/// events, plan and apply are.
fn terraform() -> Api {
    Api {
        egress: egress(
            http(
                "https://app.terraform.io",
                vec![
                    "GET /api/v2/organizations/*",
                    "GET /api/v2/organizations/*/workspaces",
                    "GET /api/v2/organizations/*/workspaces/*",
                    "GET /api/v2/organizations/*/projects",
                    "GET /api/v2/projects/*",
                    "GET /api/v2/workspaces/*",
                    "GET /api/v2/runs/*/run-events",
                    "GET /api/v2/runs/*/plan",
                    "GET /api/v2/runs/*/plan/json-output-redacted",
                    "GET /api/v2/runs/*/apply",
                    "GET /api/v2/plans/*",
                    "GET /api/v2/plans/*/json-output-redacted",
                    "GET /api/v2/applies/*",
                ],
                vec![],
                &[("TFE_TOKEN", "{token}")],
            ),
            schema(json!({})),
        ),
        upstream: |access| {
            Ok(format!(
                "https://{}",
                model::optional(&access.values, "hostname").unwrap_or("app.terraform.io")
            ))
        },
        // `include` would embed state versions or their outputs (and signed download URLs). A
        // `.json` suffix names the same route upstream, so no segment may carry a dot (names
        // and IDs have none).
        check: |request, _| {
            let EgressRequest::Http { path, .. } = request else {
                return Decision::deny("Terraform is reached over HTTP");
            };
            if path.contains('.') {
                return Decision::deny("Terraform paths have no dots");
            }
            // Rails splits query parameters at `;` as well as `&`.
            if matches!(request, EgressRequest::Http { query: Some(q), .. } if q.contains(';')) {
                return Decision::deny("Query strings are separated with &");
            }
            let embeds = query_values(request, "include")
                .iter()
                .any(|v| v.contains("output") || v.contains("state") || v.contains("run"));
            if embeds {
                return Decision::deny("State versions, outputs and runs cannot be included");
            }
            Decision::Allow
        },
    }
}
