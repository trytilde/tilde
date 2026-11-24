//! The software factory's providers (GitLab, Jira, Entire, Datadog, Grafana, Prometheus,
//! PagerDuty, New Relic, Honeycomb, Terraform) from the catalog to an upstream call: a connection
//! is set up through the broker with credentials its catalog schema accepts, its tools are given
//! to an agent, and the agent's calls reach one fake upstream with the request each API expects
//! and the connection's credential in the place that API takes it.
mod common;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use common::invocation::{Fixture, all_tools};
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::{
    connections::{catalog::Endpoints, model::Values},
    encryption::{Encryption, SecretBinding},
};
use uuid::Uuid;

const ENTIRE_SECRET: &str =
    "ent_svc_us_01M31ZJJJ1SGSFQNK3VQKZFJNG_01M31ZQKMDEPMSG40Y4M9J0SG3_c2VjcmV0";
const PLACEMENT: &str = "01M3QSHT000000000000000000";

#[derive(Debug, Clone)]
struct Seen {
    method: String,
    uri: String,
    headers: HeaderMap,
    body: String,
}
impl Seen {
    fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
    }
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap()
    }
}
type Log = Arc<Mutex<Vec<Seen>>>;

/// What each API answers; anything else is an empty object.
fn answer(method: &str, uri: &Uri, origin: &str) -> (u16, Option<String>, Value) {
    let path = uri.path();
    let ok = |value: Value| (200, None, value);
    match (method, path) {
        ("GET", "/oauth/token/accessible-resources") => ok(json!([
            {"id":"cloud-1","url":"https://acme.atlassian.net"},
            {"id":"cloud-2","url":"https://other.atlassian.net"}
        ])),
        ("POST", "/oauth/token") => ok(json!({"access_token":"svc-token","expires_in":600})),
        ("GET", "/api/v1/repos") => ok(json!({"repos":[
            {"full_name":"acme/ontology-archive","placements":[],"primaries":{"processing":""}},
            {"full_name":"acme/ontology","placements":[
                {"id":"01M3QSHT000000000000000MRR","apiBaseUrl":"https://elsewhere.example","status":"ready"},
                {"id":PLACEMENT,"apiBaseUrl":origin,"status":"ready"}
            ],"primaries":{"processing":PLACEMENT}}
        ]})),
        ("POST", "/graphql") => ok(json!({"data":{"actor":{"account":{"nrql":{
            "results":[{"incidentId":"i-1","event":"open"}]
        }}}}})),
        ("POST", "/1/queries/api") => ok(json!({"id":"q1"})),
        ("POST", "/1/query_results/api") => ok(json!({"id":"r1","complete":false})),
        ("GET", "/1/query_results/api/r1") => {
            ok(json!({"complete":true,"data":{"results":[{"data":{"COUNT":3}}]}}))
        }
        ("GET", "/api/v2/plans/plan-1/json-output") => {
            (307, Some(format!("{origin}/archivist/plan-1")), Value::Null)
        }
        ("GET", "/archivist/plan-1") => {
            ok(json!({"resource_changes":[{"address":"aws_s3_bucket.logs"}]}))
        }
        // Prometheus refusing a malformed expression (Datadog shares the path).
        ("GET", "/api/v1/query") if uri.query().is_some_and(|q| q.contains("sum%28")) => (
            400,
            None,
            json!({"status":"error","errorType":"bad_data","error":"parse error: unexpected end of input"}),
        ),
        _ => ok(json!({})),
    }
}

async fn upstream(log: Log) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let own = origin.clone();
    let app = axum::Router::new().fallback(
        move |method: Method, uri: Uri, headers: HeaderMap, body: String| {
            let (log, origin) = (log.clone(), own.clone());
            async move {
                let (status, location, value) = answer(method.as_str(), &uri, &origin);
                log.lock().unwrap().push(Seen {
                    method: method.to_string(),
                    uri: uri.path_and_query().unwrap().to_string(),
                    headers,
                    body,
                });
                let mut response = axum::response::Response::builder()
                    .status(StatusCode::from_u16(status).unwrap())
                    .header("content-type", "application/json");
                if let Some(location) = location {
                    response = response.header("location", location);
                }
                response
                    .body(axum::body::Body::from(value.to_string()))
                    .unwrap()
            }
        },
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    origin
}

fn values(fields: &[(&str, &str)]) -> Values {
    fields
        .iter()
        .map(|(key, value)| (key.to_string(), SecretString::from(*value)))
        .collect()
}
/// Sets a connection up the way a person does: through the broker, with the fields the
/// catalog's schema asks for. Refused fields leave it unready.
async fn set_up(
    fx: &Fixture,
    provider: &str,
    typ: &str,
    fields: &[(&str, &str)],
) -> Result<Uuid, tilde::error::Error> {
    let id = Uuid::new_v4();
    let started = fx
        .connections
        .start(id, provider, provider, typ, &[])
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
        .advance(setup, &token, view.action_id, values(fields))
        .await?;
    assert_eq!(fx.connections.get(id).await.unwrap().status, "ready");
    Ok(id)
}
/// Gives the agent `tools` of a connection and moves to its next invocation, where they show.
async fn give(fx: &mut Fixture, connection: Uuid, tools: &[&str]) -> String {
    let slug = fx.use_tools(connection, tools).await;
    fx.next_invocation().await;
    slug
}
/// A ready connection holding what an OAuth consent would have stored.
async fn authorized(fx: &Fixture, provider: &str, fields: &[(&str, &str)]) -> Uuid {
    let id = Uuid::new_v4();
    fx.connections
        .start(id, provider, provider, "oauth", &[])
        .await
        .unwrap();
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(97))
        .await
        .unwrap();
    let pg = fx.db.pool.get().await.unwrap();
    for (key, value) in fields {
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: id,
                    name: key,
                },
                &SecretString::from(*value),
            )
            .unwrap()
            .into_bytes();
        pg.execute(
            "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)",
            &[&id, key, &sealed],
        )
        .await
        .unwrap();
    }
    pg.execute("UPDATE connections SET status='ready' WHERE id=$1", &[&id])
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn factory_providers_reach_their_apis_from_connections_set_up_through_the_broker() {
    let log: Log = Default::default();
    let origin = upstream(log.clone()).await;
    let endpoints = [
        "gitlab_api",
        "jira_api",
        "atlassian_api",
        "datadog_api",
        "grafana_api",
        "prometheus_api",
        "pagerduty_api",
        "newrelic_api",
        "honeycomb_api",
        "terraform_api",
        "entire_api",
        "entire_auth",
        "entire_region_api",
    ]
    .map(|key| (key.to_owned(), origin.clone()));
    let mut fx = Fixture::new(97, Endpoints(BTreeMap::from(endpoints)), all_tools()).await;
    // The requests a call made upstream, in order.
    let seen = || std::mem::take(&mut *log.lock().unwrap());

    // The catalog's schemas guard setup: a credential that could send a secret elsewhere, or
    // one that cannot work, never becomes a connection.
    for (provider, typ, fields) in [
        (
            "entire",
            "service_account",
            vec![
                ("secret", ENTIRE_SECRET),
                ("cluster", "git.attacker.example"),
            ],
        ),
        (
            "gitlab",
            "token",
            vec![("token", "glpat-x"), ("base_url", "http://gitlab.internal")],
        ),
        (
            "jira",
            "api_token",
            vec![
                ("site_url", "https://jira.attacker.example"),
                ("email", "dev@acme.test"),
                ("api_token", "x"),
            ],
        ),
        (
            "datadog",
            "api",
            vec![
                ("site", "datadoghq.attacker.example"),
                ("api_key", "a"),
                ("app_key", "b"),
            ],
        ),
        (
            "terraform",
            "api",
            vec![
                ("organization", "acme"),
                ("token", "t"),
                ("hostname", "tfe.example.com/path"),
            ],
        ),
    ] {
        assert!(
            set_up(&fx, provider, typ, &fields).await.is_err(),
            "{provider} accepted {fields:?}"
        );
    }

    // GitLab: a project in nested groups is one encoded path segment.
    let gitlab = set_up(&fx, "gitlab", "token", &[("token", "glpat-fixture")])
        .await
        .unwrap();
    let slug = give(&mut fx, gitlab, &["create_merge_request"]).await;
    fx.output(
        &format!("{slug}.create_merge_request"),
        json!({"project":"acme/platform/widgets","source_branch":"factory/fix","target_branch":"main","title":"Fix"}),
    )
    .await;
    let calls = seen();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].uri,
        "/api/v4/projects/acme%2Fplatform%2Fwidgets/merge_requests"
    );
    assert_eq!(calls[0].header("authorization"), "Bearer glpat-fixture");
    assert_eq!(
        calls[0].json(),
        json!({"source_branch":"factory/fix","target_branch":"main","title":"Fix"})
    );

    // Jira with an API token: Basic to the site, the flat input shaped into Jira's fields.
    let jira = set_up(
        &fx,
        "jira",
        "api_token",
        &[
            ("site_url", "https://acme.atlassian.net"),
            ("email", "dev@acme.test"),
            ("api_token", "jira-token"),
        ],
    )
    .await
    .unwrap();
    let slug = give(&mut fx, jira, &["create_issue", "transition_issue"]).await;
    fx.output(
        &format!("{slug}.create_issue"),
        json!({"project":"ENG","issue_type":"Bug","summary":"Checkout fails","description":"Steps","labels":["factory"]}),
    )
    .await;
    fx.output(
        &format!("{slug}.transition_issue"),
        json!({"issue":"ENG-9","transition_id":"31"}),
    )
    .await;
    let calls = seen();
    assert_eq!(calls[0].uri, "/rest/api/2/issue");
    // base64("dev@acme.test:jira-token")
    assert_eq!(
        calls[0].header("authorization"),
        "Basic ZGV2QGFjbWUudGVzdDpqaXJhLXRva2Vu"
    );
    assert_eq!(
        calls[0].json(),
        json!({"fields":{"project":{"key":"ENG"},"issuetype":{"name":"Bug"},"summary":"Checkout fails","description":"Steps","labels":["factory"]}})
    );
    assert_eq!(calls[1].uri, "/rest/api/2/issue/ENG-9/transitions");
    assert_eq!(calls[1].json(), json!({"transition":{"id":"31"}}));

    // Jira over OAuth: the grant reaches two sites; the connection's site picks the cloud ID.
    let jira = authorized(
        &fx,
        "jira",
        &[
            ("access_token", "atl-access"),
            ("site_url", "https://acme.atlassian.net"),
        ],
    )
    .await;
    let slug = give(&mut fx, jira, &["search_issues"]).await;
    fx.output(
        &format!("{slug}.search_issues"),
        json!({"jql":"project = ENG","maxResults":5}),
    )
    .await;
    let calls = seen();
    assert_eq!(calls[0].uri, "/oauth/token/accessible-resources");
    assert_eq!(calls[1].uri, "/ex/jira/cloud-1/rest/api/2/search/jql");
    assert_eq!(calls[1].header("authorization"), "Bearer atl-access");
    assert_eq!(
        calls[1].json(),
        json!({"jql":"project = ENG","maxResults":5})
    );

    // Datadog: both keys in their headers, the flat search nested as the API wants it.
    let datadog = set_up(
        &fx,
        "datadog",
        "api",
        &[
            ("site", "datadoghq.eu"),
            ("api_key", "dd-api"),
            ("app_key", "dd-app"),
        ],
    )
    .await
    .unwrap();
    let slug = give(&mut fx, datadog, &["search_logs", "query_metrics"]).await;
    fx.output(
        &format!("{slug}.search_logs"),
        json!({"query":"service:checkout status:error","from":"now-15m","limit":20}),
    )
    .await;
    fx.output(
        &format!("{slug}.query_metrics"),
        json!({"query":"avg:system.cpu.user{*}","from":1700000000,"to":1700003600}),
    )
    .await;
    let calls = seen();
    assert_eq!(calls[0].uri, "/api/v2/logs/events/search");
    assert_eq!(calls[0].header("dd-api-key"), "dd-api");
    assert_eq!(calls[0].header("dd-application-key"), "dd-app");
    assert_eq!(
        calls[0].json(),
        json!({"filter":{"query":"service:checkout status:error","from":"now-15m"},"page":{"limit":20}})
    );
    assert_eq!(
        calls[1].uri,
        "/api/v1/query?from=1700000000&query=avg%3Asystem.cpu.user%7B*%7D&to=1700003600"
    );

    // Grafana: a query goes through the data source proxy under the instance's path.
    let grafana = set_up(
        &fx,
        "grafana",
        "api",
        &[
            ("url", "https://acme.grafana.net"),
            ("token", "glsa_fixture"),
        ],
    )
    .await
    .unwrap();
    let slug = give(&mut fx, grafana, &["query_datasource"]).await;
    fx.output(
        &format!("{slug}.query_datasource"),
        json!({"datasource_uid":"prom","query":{"expr":"up"},"from":"now-5m"}),
    )
    .await;
    let calls = seen();
    assert_eq!(calls[0].uri, "/api/ds/query");
    assert_eq!(calls[0].header("authorization"), "Bearer glsa_fixture");
    assert_eq!(
        calls[0].json(),
        json!({"queries":[{"expr":"up","refId":"A","datasource":{"uid":"prom"}}],"from":"now-5m","to":"now"})
    );

    // Prometheus: no token, no Authorization; an upstream error reaches the agent to correct.
    let prometheus = set_up(
        &fx,
        "prometheus",
        "api",
        &[("url", "https://mimir.acme.test/prometheus")],
    )
    .await
    .unwrap();
    let slug = give(&mut fx, prometheus, &["find_series", "query"]).await;
    fx.output(
        &format!("{slug}.find_series"),
        json!({"match":"up{job=\"api\"}"}),
    )
    .await;
    let failed = fx
        .invoke(&format!("{slug}.query"), json!({"query":"sum("}))
        .await;
    assert!(
        failed["error"].to_string().contains("parse error"),
        "{failed}"
    );
    let calls = seen();
    assert_eq!(
        calls[0].uri,
        "/api/v1/series?match%5B%5D=up%7Bjob%3D%22api%22%7D"
    );
    assert!(
        calls
            .iter()
            .all(|call| call.header("authorization").is_empty())
    );

    // PagerDuty: list filters become `name[]`; a note is written as a named user.
    let pagerduty = set_up(&fx, "pagerduty", "api_token", &[("api_token", "pd-key")])
        .await
        .unwrap();
    let slug = give(
        &mut fx,
        pagerduty,
        &["list_incidents", "create_incident_note"],
    )
    .await;
    fx.output(
        &format!("{slug}.list_incidents"),
        json!({"statuses":["triggered","acknowledged"],"limit":5}),
    )
    .await;
    fx.output(
        &format!("{slug}.create_incident_note"),
        json!({"incident_id":"PT4KHLK","content":"Rolled back","from":"dev@acme.test"}),
    )
    .await;
    let calls = seen();
    assert_eq!(
        calls[0].uri,
        "/incidents?limit=5&statuses%5B%5D=triggered&statuses%5B%5D=acknowledged"
    );
    assert_eq!(calls[0].header("authorization"), "Token token=pd-key");
    assert_eq!(calls[1].uri, "/incidents/PT4KHLK/notes");
    assert_eq!(calls[1].header("from"), "dev@acme.test");
    assert_eq!(calls[1].json(), json!({"note":{"content":"Rolled back"}}));

    // New Relic: incidents are a NRQL query in the connection's account.
    let newrelic = set_up(
        &fx,
        "newrelic",
        "api",
        &[("account_id", "1234567"), ("api_key", "NRAK-fixture")],
    )
    .await
    .unwrap();
    let slug = give(&mut fx, newrelic, &["list_alert_incidents"]).await;
    let incidents = fx
        .output(
            &format!("{slug}.list_alert_incidents"),
            json!({"since":"2 hours ago","open_only":true}),
        )
        .await;
    assert_eq!(incidents["incidents"][0]["incidentId"], "i-1");
    let calls = seen();
    assert_eq!(calls[0].uri, "/graphql");
    assert_eq!(calls[0].header("api-key"), "NRAK-fixture");
    assert_eq!(calls[0].json()["variables"]["account"], 1234567);
    assert_eq!(
        calls[0].json()["variables"]["nrql"],
        "SELECT * FROM NrAiIncident WHERE event = 'open' SINCE 2 hours ago LIMIT 100"
    );

    // Honeycomb: one tool call creates the query, starts a result and polls it to completion.
    let honeycomb = set_up(&fx, "honeycomb", "api", &[("api_key", "hc-key")])
        .await
        .unwrap();
    let slug = give(&mut fx, honeycomb, &["run_query"]).await;
    let result = fx
        .output(
            &format!("{slug}.run_query"),
            json!({"dataset":"api","query":{"calculations":[{"op":"COUNT"}],"time_range":3600}}),
        )
        .await;
    assert_eq!(result["data"]["data"]["results"][0]["data"]["COUNT"], 3);
    let calls = seen();
    assert_eq!(
        calls
            .iter()
            .map(|c| format!("{} {}", c.method, c.uri))
            .collect::<Vec<_>>(),
        [
            "POST /1/queries/api",
            "POST /1/query_results/api",
            "GET /1/query_results/api/r1"
        ]
    );
    assert_eq!(
        calls[0].json(),
        json!({"calculations":[{"op":"COUNT"}],"time_range":3600})
    );
    assert_eq!(
        calls[1].json(),
        json!({"query_id":"q1","disable_series":true})
    );
    assert!(
        calls
            .iter()
            .all(|c| c.header("x-honeycomb-team") == "hc-key")
    );

    // Terraform: the organization is the connection's; the plan's archive is read without
    // the token.
    let terraform = set_up(
        &fx,
        "terraform",
        "api",
        &[("organization", "acme"), ("token", "tf-token")],
    )
    .await
    .unwrap();
    let slug = give(
        &mut fx,
        terraform,
        &["list_state_versions", "get_workspace", "get_plan_json"],
    )
    .await;
    fx.output(
        &format!("{slug}.list_state_versions"),
        json!({"workspace_name":"prod","page_size":5}),
    )
    .await;
    fx.output(
        &format!("{slug}.get_workspace"),
        json!({"workspace_name":"prod"}),
    )
    .await;
    let plan = fx
        .output(
            &format!("{slug}.get_plan_json"),
            json!({"plan_id":"plan-1"}),
        )
        .await;
    assert_eq!(
        plan["data"]["resource_changes"][0]["address"],
        "aws_s3_bucket.logs"
    );
    let calls = seen();
    assert_eq!(
        calls[0].uri,
        "/api/v2/state-versions?filter%5Borganization%5D%5Bname%5D=acme&filter%5Bworkspace%5D%5Bname%5D=prod&page%5Bsize%5D=5"
    );
    assert_eq!(calls[0].header("authorization"), "Bearer tf-token");
    assert_eq!(calls[1].uri, "/api/v2/organizations/acme/workspaces/prod");
    assert_eq!(calls[3].uri, "/archivist/plan-1");
    assert_eq!(calls[3].header("authorization"), "");

    // Entire: the secret is exchanged once for a service account token; only that token
    // reaches the Core, and the region API of the repository's processing placement.
    let entire = set_up(
        &fx,
        "entire",
        "service_account",
        &[
            ("secret", ENTIRE_SECRET),
            ("cluster", "aws-us-east-2.entire.io"),
        ],
    )
    .await
    .unwrap();
    let slug = give(&mut fx, entire, &["create_repository", "list_branches"]).await;
    fx.output(
        &format!("{slug}.create_repository"),
        json!({"name":"ontology","projectId":"01M31ZJJJ1SGSFQNK3VQKZFJNG"}),
    )
    .await;
    fx.output(
        &format!("{slug}.list_branches"),
        json!({"repository":"acme/ontology","search":"factory"}),
    )
    .await;
    let calls = seen();
    assert_eq!(
        calls
            .iter()
            .map(|c| format!("{} {}", c.method, c.uri))
            .collect::<Vec<_>>(),
        [
            "POST /oauth/token".to_owned(),
            "POST /api/v1/repos".to_owned(),
            "GET /api/v1/repos?q=acme%2Fontology".to_owned(),
            format!("GET /api/v1/git/repos/{PLACEMENT}/branches?search=factory"),
        ]
    );
    // base64("entire-cli:")
    assert_eq!(calls[0].header("authorization"), "Basic ZW50aXJlLWNsaTo=");
    let exchange: BTreeMap<String, String> = url::form_urlencoded::parse(calls[0].body.as_bytes())
        .into_owned()
        .collect();
    assert_eq!(exchange["subject_token"], ENTIRE_SECRET);
    assert_eq!(exchange["resource"], "svc_01M31ZJJJ1SGSFQNK3VQKZFJNG");
    assert_eq!(
        calls[1].json(),
        json!({"name":"ontology","projectId":"01M31ZJJJ1SGSFQNK3VQKZFJNG"})
    );
    assert!(
        calls[1..]
            .iter()
            .all(|call| call.header("authorization") == "Bearer svc-token")
    );
    fx.close().await;
}
