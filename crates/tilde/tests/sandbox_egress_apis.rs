//! Sandbox egress to API providers end to end: a sandbox enrolls with grants on Linear, Sentry,
//! Jira, PagerDuty, Datadog, Grafana, Prometheus, New Relic, Honeycomb, HCP Terraform and AWS,
//! and calls each through the gateway route the way that provider's clients do (the egress token
//! where the client sends its key: bare, Bearer, Basic, `Token token=`, the provider's key header,
//! or an AWS SigV4 access key ID). One fake upstream stands in for every provider. Each allowed
//! request reaches it with the connection's credential as the provider takes it; each denied one
//! (outside the rules or narrowed by the grant) never does. The connection's credentials never
//! reach the sandbox and the egress token never reaches upstream. AWS requests arrive re-signed
//! with the connection's keys for the service and region the client signed for, verified by the
//! fake; a Linear OAuth connection's expired access token is refreshed on the way.
mod common;
use axum::{
    Router,
    extract::{Request, State},
    response::{IntoResponse, Response},
};
use base64::Engine;
use client_aws_sigv4::{Credentials, Signer};
use common::invocation::{Fixture, all_tools, drain, framed};
use futures::StreamExt;
use reqwest::{Method, StatusCode};
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::connections::catalog::Endpoints;
use uuid::Uuid;

const AWS_KEY: &str = "AKIAEGRESSFIXTURE";
const AWS_SECRET: &str = "aws_secret_fixture";

/// One request the fake upstream received.
#[derive(Debug, Clone)]
struct Seen {
    route: String,
    headers: BTreeMap<String, String>,
    /// For a SigV4-signed request: whether its signature verifies with the connection's keys.
    signed: Option<bool>,
}
#[derive(Default)]
struct Fake {
    seen: Mutex<Vec<Seen>>,
    refreshes: Mutex<Vec<String>>,
}

async fn fake() -> (String, Arc<Fake>) {
    let state = Arc::new(Fake::default());
    let app = Router::new().fallback(serve).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (origin, state)
}
async fn serve(State(fake): State<Arc<Fake>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    if parts.uri.path() == "/oauth/token" {
        fake.refreshes
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(&body).into_owned());
        return axum::Json(json!({"access_token": "lin_oauth_new", "refresh_token": "lin_refresh_2", "expires_in": 3600, "token_type": "Bearer"}))
            .into_response();
    }
    let headers: BTreeMap<String, String> = parts
        .headers
        .iter()
        .map(|(n, v)| {
            (
                n.as_str().to_owned(),
                v.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    let signed = headers
        .get("authorization")
        .filter(|a| a.starts_with("AWS4-HMAC-SHA256"))
        .map(|_| verifies(&parts, &headers, &body));
    let path_and_query = parts.uri.path_and_query().unwrap().as_str().to_owned();
    fake.seen.lock().unwrap().push(Seen {
        route: format!("{} {path_and_query}", parts.method),
        headers,
        signed,
    });
    axum::Json(json!({"ok": true})).into_response()
}
/// Signs the request again as AWS would check it: its signed headers, method, URL and body with
/// the connection's keys at its `x-amz-date`, for the region and service of its scope.
fn verifies(
    parts: &axum::http::request::Parts,
    headers: &BTreeMap<String, String>,
    body: &[u8],
) -> bool {
    let authorization = &headers["authorization"];
    let field = |name: &str| {
        authorization
            .split([' ', ','])
            .find_map(|part| part.strip_prefix(name))
            .unwrap()
            .to_owned()
    };
    let scope: Vec<String> = field("Credential=").split('/').map(str::to_owned).collect();
    let date = chrono::NaiveDateTime::parse_from_str(&headers["x-amz-date"], "%Y%m%dT%H%M%SZ")
        .unwrap()
        .and_utc();
    let url = format!(
        "http://{}{}",
        headers["host"],
        parts.uri.path_and_query().unwrap()
    );
    let mut request = reqwest::Client::new()
        .request(parts.method.clone(), url)
        .build()
        .unwrap();
    for name in field("SignedHeaders=").split(';') {
        if !["host", "x-amz-date", "x-amz-content-sha256"].contains(&name) {
            request.headers_mut().insert(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                headers[name].parse().unwrap(),
            );
        }
    }
    let credentials = Credentials {
        access_key_id: AWS_KEY.into(),
        secret_access_key: AWS_SECRET.into(),
        session_token: None,
    };
    Signer::new(credentials, scope[2].clone(), scope[3].clone())
        .sign_request_at(&mut request, body, date.into())
        .unwrap();
    scope[0] == AWS_KEY && request.headers()["authorization"].to_str().unwrap() == authorization
}

/// The status of a request written as is, path unresolved.
async fn raw(host: &str, method: &str, path: &str, authorization: &str) -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(host).await.unwrap();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nhost: {host}\r\nauthorization: {authorization}\r\ncontent-type: application/x-amz-json-1.0\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{{}}"
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = vec![];
    stream.read_to_end(&mut response).await.unwrap();
    String::from_utf8_lossy(&response)[9..12].parse().unwrap()
}

/// A ready connection holding `values`.
async fn connection(fx: &Fixture, provider: &str, typ: &str, values: &[(&str, &str)]) -> Uuid {
    let id = Uuid::new_v4();
    fx.connections
        .start(id, provider, provider, typ, &[])
        .await
        .unwrap();
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute("UPDATE connections SET status='ready' WHERE id=$1", &[&id])
        .await
        .unwrap();
    for (key, value) in values {
        fx.connections
            .replace_value(id, key, &SecretString::from(*value))
            .await
            .unwrap();
    }
    id
}

/// What the sandbox process receives when it enrolls: its egress token and wiring.
async fn connect(fx: &Fixture, enrollment: &str) -> Value {
    let connected = reqwest::Client::new()
        .post(format!(
            "{}/tilde.sandbox.v1.SandboxService/Connect",
            fx.origin
        ))
        .bearer_auth(enrollment)
        .header("content-type", "application/connect+json")
        .header("connect-protocol-version", "1")
        .body(framed(json!({})))
        .send()
        .await
        .unwrap();
    assert_eq!(connected.status(), 200);
    let mut stream = connected.bytes_stream();
    let mut buffer = vec![];
    loop {
        buffer.extend_from_slice(&stream.next().await.unwrap().unwrap());
        if let Some(frame) = drain(&mut buffer).into_iter().next() {
            break frame["registered"].clone();
        }
    }
}

/// A grant: slug, provider, connection type, the connection's values and the grant's config.
type Grant = (
    &'static str,
    &'static str,
    &'static str,
    Vec<(&'static str, &'static str)>,
    Value,
);

/// An AWS request naming conflicting actions: service, path and query, headers, body.
type Confused = (
    &'static str,
    &'static str,
    Vec<(&'static str, &'static str)>,
    &'static str,
);

/// How a client presents the egress token.
#[derive(Clone, Copy)]
enum Present {
    Bare,
    Bearer,
    Basic,
    PagerDuty,
    Header(&'static str),
    Sigv4(&'static str, &'static str),
}

// Every provider's credential model in one sandbox: what reaches upstream for an allowed request,
// and that a denied one stops at the gateway.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sandbox_reaches_api_providers_through_egress_within_their_grants() {
    let (origin, upstream) = fake().await;
    let providers = [
        "linear",
        "sentry",
        "jira",
        "pagerduty",
        "datadog",
        "grafana",
        "prometheus",
        "newrelic",
        "honeycomb",
        "terraform",
        "aws",
    ];
    let endpoints = Endpoints(
        providers
            .iter()
            .map(|p| (format!("{p}_api"), origin.clone()))
            .collect(),
    );
    let fx = Fixture::new(76, endpoints, all_tools()).await;
    // (slug, provider, type, values, grant config)
    let grants: Vec<Grant> = vec![
        (
            "linear",
            "linear",
            "api_key",
            vec![("api_key", "lin_api_fixture")],
            json!({}),
        ),
        (
            "linear_oauth",
            "linear",
            "oauth",
            vec![
                ("client_id", "linear-client"),
                ("client_secret", "linear-secret"),
                ("access_token", "lin_oauth_old"),
                ("refresh_token", "lin_refresh_1"),
            ],
            json!({"mutations": []}),
        ),
        (
            "sentry",
            "sentry",
            "token",
            vec![("auth_token", "sntrys_fixture")],
            json!({"organizations": ["acme"], "issue_writes": false}),
        ),
        (
            "jira",
            "jira",
            "api_token",
            vec![
                ("site_url", "https://acme.atlassian.net"),
                ("email", "dev@acme.com"),
                ("api_token", "jira_fixture"),
            ],
            json!({"writes": false}),
        ),
        (
            "pagerduty",
            "pagerduty",
            "api_token",
            vec![("api_token", "pd_fixture")],
            json!({}),
        ),
        (
            "datadog",
            "datadog",
            "api",
            vec![
                ("site", "datadoghq.eu"),
                ("api_key", "dd_api_fixture"),
                ("app_key", "dd_app_fixture"),
            ],
            json!({}),
        ),
        (
            "grafana",
            "grafana",
            "api",
            vec![
                ("url", "https://acme.grafana.net"),
                ("token", "glsa_fixture"),
            ],
            json!({}),
        ),
        (
            "prometheus",
            "prometheus",
            "api",
            vec![("url", "https://prometheus.acme.com")],
            json!({}),
        ),
        (
            "newrelic",
            "newrelic",
            "api",
            vec![
                ("account_id", "1234"),
                ("api_key", "NRAK-fixture"),
                ("region", "eu"),
            ],
            json!({}),
        ),
        (
            "honeycomb",
            "honeycomb",
            "api",
            vec![("api_key", "hcaik_fixture")],
            json!({}),
        ),
        (
            "terraform",
            "terraform",
            "api",
            vec![("organization", "acme"), ("token", "tf_fixture")],
            json!({}),
        ),
        (
            "aws",
            "aws",
            "iam",
            vec![
                ("access_key_id", AWS_KEY),
                ("secret_access_key", AWS_SECRET),
                ("region", "us-east-1"),
            ],
            json!({"services": ["logs", "sts", "ssm", "s3", "lambda", "monitoring", "ec2", "ecs"], "regions": ["us-east-1"]}),
        ),
    ];
    let secrets: Vec<&str> = grants
        .iter()
        .flat_map(|g| g.3.iter().map(|(_, v)| *v))
        .filter(|v| v.contains("fixture") || v.starts_with("lin_") || v.contains("secret"))
        .collect();
    let sandboxes = fx.chat.tools.as_ref().unwrap().sandboxes.clone();
    let enrollment = "tilde_sbx_enrollment_apis";
    let oauth = fx
        .within(async {
            let e2b = connection(&fx, "e2b", "api", &[("api_key", "e2b_fixture")]).await;
            let blueprint = sandboxes
                .create_blueprint(
                    "observer",
                    e2b,
                    "tilde-base",
                    "thread",
                    Default::default(),
                )
                .await
                .unwrap();
            let mut oauth = Uuid::nil();
            for (slug, provider, typ, values, config) in &grants {
                let id = connection(&fx, provider, typ, values).await;
                if *slug == "linear_oauth" {
                    oauth = id;
                }
                sandboxes
                    .set_egress(blueprint.id, slug, id, config, true)
                    .await
                    .unwrap_or_else(|e| panic!("{slug}: {e:?}"));
            }
            // A grant names supported services only.
            let aws = connection(&fx, "aws", "iam", &[("access_key_id", AWS_KEY), ("secret_access_key", AWS_SECRET)]).await;
            let refused = sandboxes
                .set_egress(blueprint.id, "aws_glue", aws, &json!({"services": ["glue"]}), true)
                .await;
            assert!(refused.is_err(), "a grant on an unsupported service was saved");
            let pg = fx.db.pool.get().await.unwrap();
            let hash = <sha2::Sha256 as sha2::Digest>::digest(enrollment.as_bytes());
            pg.execute(
                "INSERT INTO sandboxes(id,blueprint_id,reuse,connection_id,agent_id,thread_id,status,enrollment_hash) VALUES($1,$2,'thread',$3,$4,$5,'running',$6)",
                &[&Uuid::new_v4(), &blueprint.id, &e2b, &fx.agent, &fx.thread, &hash.to_vec()],
            )
            .await
            .unwrap();
            // The OAuth connection's access token has expired, and its token endpoint is the fake.
            pg.execute(
                "UPDATE connections SET token_expires_at = now() - interval '1 minute' WHERE id=$1",
                &[&oauth],
            )
            .await
            .unwrap();
            oauth
        })
        .await;
    let token_url = async {
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE connection_types SET token_url=$1 WHERE provider_id='linear' AND type_id='oauth'",
                &[&format!("{origin}/oauth/token")],
            )
            .await
            .unwrap()
    };
    token_url.await;

    let registered = connect(&fx, enrollment).await;
    let registration = registered.to_string();
    for secret in &secrets {
        assert!(
            !registration.contains(secret),
            "{secret} reached the sandbox"
        );
    }
    let token = registered["egressToken"].as_str().unwrap().to_owned();
    let env: BTreeMap<String, String> = registered["egressEnv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["name"].as_str().unwrap().into(),
                v["value"].as_str().unwrap().into(),
            )
        })
        .collect();
    let route = |slug: &str| format!("{}/sandboxes/egress/{slug}", fx.origin);
    // What standard clients read, pointed at the route (the runtime's public URL).
    let wired = |slug: &str| env[&format!("TILDE_EGRESS_{}_URL", slug.to_uppercase())].clone();
    assert!(wired("sentry").ends_with("/sandboxes/egress/sentry"));
    // Two Linear grants set the same variables; the later one's stand.
    assert_eq!(
        env["LINEAR_API_URL"],
        format!("{}/graphql", wired("linear_oauth"))
    );
    assert_eq!(env["LINEAR_API_KEY"], "{token}");
    assert_eq!(env["SENTRY_URL"], wired("sentry"));
    assert_eq!(env["SENTRY_AUTH_TOKEN"], "{token}");
    assert_eq!(env["AWS_ENDPOINT_URL"], wired("aws"));
    assert_eq!(env["AWS_ACCESS_KEY_ID"], "{token}");
    assert_eq!(env["TILDE_EGRESS_PAGERDUTY_TOKEN"], "{token}");
    assert_eq!(env["HONEYCOMB_API_ENDPOINT"], wired("honeycomb"));

    let graphql = |query: &str| json!({ "query": query }).to_string();
    let read = graphql("{ issues(first: 5) { nodes { id title } } }");
    let http = reqwest::Client::new();
    let call = |slug: &'static str,
                method: Method,
                path: &'static str,
                body: Option<String>,
                present: Present| {
        let http = http.clone();
        let url = format!("{}{path}", route(slug));
        let token = token.clone();
        async move {
            let mut request = http.request(method.clone(), &url);
            request = match present {
                Present::Bare => request.header("authorization", &token),
                Present::Bearer => request.bearer_auth(&token),
                Present::Basic => request.basic_auth("dev@acme.com", Some(&token)),
                Present::PagerDuty => {
                    request.header("authorization", format!("Token token={token}"))
                }
                Present::Header(name) => request.header(name, &token),
                Present::Sigv4(..) => request,
            };
            let body = body.unwrap_or_default();
            let mut request = request
                .header("content-type", "application/json")
                .body(body.clone())
                .build()
                .unwrap();
            if let Present::Sigv4(region, service) = present {
                if let Some(target) = path.strip_prefix("/#") {
                    *request.url_mut() = format!("{}/", url.split("/#").next().unwrap())
                        .parse()
                        .unwrap();
                    request
                        .headers_mut()
                        .insert("x-amz-target", target.parse().unwrap());
                    request.headers_mut().insert(
                        "content-type",
                        "application/x-amz-json-1.1".parse().unwrap(),
                    );
                }
                // As an AWS SDK signs with the egress token as its access key ID.
                let credentials = Credentials {
                    access_key_id: token.clone(),
                    secret_access_key: "tilde-egress".into(),
                    session_token: None,
                };
                Signer::new(credentials, region, service)
                    .sign_request_at(&mut request, body.as_bytes(), std::time::SystemTime::now())
                    .unwrap();
            }
            let response = http.execute(request).await.unwrap();
            let status = response.status();
            let text = response.text().await.unwrap();
            (status, text)
        }
    };

    // (slug, method, path, body, how the client presents the token, expected status, the
    // credential headers upstream must see)
    type Case = (
        &'static str,
        Method,
        &'static str,
        Option<String>,
        Present,
        StatusCode,
        Vec<(&'static str, String)>,
    );
    let basic = |user: &str, password: &str| {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
        )
    };
    let ok = StatusCode::OK;
    let no = StatusCode::FORBIDDEN;
    let cases: Vec<Case> = vec![
        // Linear: queries and the granted mutations; the API key goes bare, as Linear takes it.
        (
            "linear",
            Method::POST,
            "/graphql",
            Some(read.clone()),
            Present::Bare,
            ok,
            vec![("authorization", "lin_api_fixture".into())],
        ),
        (
            "linear",
            Method::POST,
            "/graphql",
            Some(graphql(
                "mutation { commentCreate(input: {issueId: \"x\", body: \"hi\"}) { success } }",
            )),
            Present::Bare,
            ok,
            vec![("authorization", "lin_api_fixture".into())],
        ),
        (
            "linear",
            Method::POST,
            "/graphql",
            Some(graphql("mutation { teamDelete(id: \"x\") { success } }")),
            Present::Bare,
            no,
            vec![],
        ),
        (
            "linear",
            Method::GET,
            "/oauth/authorize",
            None,
            Present::Bare,
            no,
            vec![],
        ),
        // The OAuth connection's expired token is refreshed before the request goes out.
        (
            "linear_oauth",
            Method::POST,
            "/graphql",
            Some(read.clone()),
            Present::Bearer,
            ok,
            vec![("authorization", "Bearer lin_oauth_new".into())],
        ),
        (
            "linear_oauth",
            Method::POST,
            "/graphql",
            Some(graphql("mutation { issueCreate(input: {}) { success } }")),
            Present::Bearer,
            no,
            vec![],
        ),
        // Sentry: within the granted organization, reads only (issue writes are off).
        (
            "sentry",
            Method::GET,
            "/api/0/organizations/acme/issues/",
            None,
            Present::Bearer,
            ok,
            vec![("authorization", "Bearer sntrys_fixture".into())],
        ),
        (
            "sentry",
            Method::GET,
            "/api/0/organizations/other/issues/",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "sentry",
            Method::PUT,
            "/api/0/organizations/acme/issues/42/",
            Some("{\"status\":\"resolved\"}".into()),
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "sentry",
            Method::GET,
            "/api/0/organizations/acme/org-auth-tokens/",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        // Jira: Basic as the account's email and API token; writes are off.
        (
            "jira",
            Method::GET,
            "/rest/api/3/issue/ENG-1",
            None,
            Present::Basic,
            ok,
            vec![("authorization", basic("dev@acme.com", "jira_fixture"))],
        ),
        (
            "jira",
            Method::POST,
            "/rest/api/3/search/jql",
            Some("{\"jql\":\"project = ENG\"}".into()),
            Present::Basic,
            ok,
            vec![("authorization", basic("dev@acme.com", "jira_fixture"))],
        ),
        (
            "jira",
            Method::POST,
            "/rest/api/3/issue",
            Some("{}".into()),
            Present::Basic,
            no,
            vec![],
        ),
        // PagerDuty: `Token token=`; notes yes, incident updates no.
        (
            "pagerduty",
            Method::GET,
            "/incidents",
            None,
            Present::PagerDuty,
            ok,
            vec![("authorization", "Token token=pd_fixture".into())],
        ),
        (
            "pagerduty",
            Method::POST,
            "/incidents/P1/notes",
            Some("{}".into()),
            Present::PagerDuty,
            ok,
            vec![("authorization", "Token token=pd_fixture".into())],
        ),
        (
            "pagerduty",
            Method::PUT,
            "/incidents/P1",
            Some("{}".into()),
            Present::PagerDuty,
            no,
            vec![],
        ),
        // Datadog: both keys in its own headers; keys are never reachable.
        (
            "datadog",
            Method::GET,
            "/api/v1/monitor",
            None,
            Present::Header("dd-api-key"),
            ok,
            vec![
                ("dd-api-key", "dd_api_fixture".into()),
                ("dd-application-key", "dd_app_fixture".into()),
            ],
        ),
        (
            "datadog",
            Method::GET,
            "/api/v2/api_keys",
            None,
            Present::Header("dd-api-key"),
            no,
            vec![],
        ),
        (
            "datadog",
            Method::DELETE,
            "/api/v1/monitor/1",
            None,
            Present::Header("dd-api-key"),
            no,
            vec![],
        ),
        (
            "grafana",
            Method::GET,
            "/api/search",
            None,
            Present::Bearer,
            ok,
            vec![("authorization", "Bearer glsa_fixture".into())],
        ),
        (
            "grafana",
            Method::DELETE,
            "/api/dashboards/uid/abc",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        // A Prometheus without authentication gets none: neither a credential nor the egress token.
        (
            "prometheus",
            Method::GET,
            "/api/v1/query?query=up",
            None,
            Present::Bearer,
            ok,
            vec![],
        ),
        (
            "prometheus",
            Method::POST,
            "/api/v1/admin/tsdb/delete_series",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "newrelic",
            Method::POST,
            "/graphql",
            Some(graphql("{ actor { user { name } } }")),
            Present::Header("api-key"),
            ok,
            vec![("api-key", "NRAK-fixture".into())],
        ),
        (
            "newrelic",
            Method::POST,
            "/graphql",
            Some(graphql(
                "mutation { alertsPolicyDelete(accountId: 1, id: 2) { id } }",
            )),
            Present::Header("api-key"),
            no,
            vec![],
        ),
        (
            "honeycomb",
            Method::GET,
            "/1/triggers/checkout",
            None,
            Present::Header("x-honeycomb-team"),
            ok,
            vec![("x-honeycomb-team", "hcaik_fixture".into())],
        ),
        (
            "honeycomb",
            Method::DELETE,
            "/1/triggers/checkout/t1",
            None,
            Present::Header("x-honeycomb-team"),
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/organizations/acme/workspaces",
            None,
            Present::Bearer,
            ok,
            vec![("authorization", "Bearer tf_fixture".into())],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/workspaces/ws-1/vars",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        // State reached through `include`, and unredacted plans; the redacted plan is readable.
        (
            "terraform",
            Method::GET,
            "/api/v2/organizations/acme/workspaces?include=current_state_version",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/plans/plan-1/json-output",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/plans/plan-1/json-output-redacted",
            None,
            Present::Bearer,
            ok,
            vec![("authorization", "Bearer tf_fixture".into())],
        ),
        // NerdGraph's API keys, at any depth.
        (
            "newrelic",
            Method::POST,
            "/graphql",
            Some(graphql(
                "{ actor { apiAccess { keySearch(query: {scope: {}}) { keys { key } } } } }",
            )),
            Present::Header("api-key"),
            no,
            vec![],
        ),
        // SSM parameter reads return decrypted secrets.
        // Third review: REST services read by method alone, run variables, alertmanager config.
        (
            "aws",
            Method::GET,
            "/reports/2026.csv",
            None,
            Present::Sigv4("us-east-1", "s3"),
            ok,
            vec![],
        ),
        (
            "aws",
            Method::PUT,
            "/reports/2026.csv?Action=GetObject",
            Some("data".into()),
            Present::Sigv4("us-east-1", "s3"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/2015-03-31/functions/f/invocations?Action=GetFunction",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "lambda"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#AWSCognitoIdentityProviderService.DescribeUserPoolClient",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "cognito-idp"),
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/runs/run-1",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/runs/run-1?fields%5Brun%5D=status,message",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/workspaces/ws-1/runs",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        // Fourth review: `;` separates parameters too; unknown services are REST.
        (
            "terraform",
            Method::GET,
            "/api/v2/workspaces/ws-1?a=1;include=outputs",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "pagerduty",
            Method::GET,
            "/services?a=1;include%5B%5D=integrations",
            None,
            Present::PagerDuty,
            no,
            vec![],
        ),
        (
            "aws",
            Method::DELETE,
            "/clusters/prod?Action=ListClusters",
            None,
            Present::Sigv4("us-east-1", "eks"),
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/workspaces/ws-1/runs?fields%5Brun%5D=status,variables",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/workspaces/ws-1?include=current_run",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "grafana",
            Method::GET,
            "/api/alertmanager/grafana/api/v2/alerts",
            None,
            Present::Bearer,
            ok,
            vec![("authorization", "Bearer glsa_fixture".into())],
        ),
        (
            "grafana",
            Method::GET,
            "/api/alertmanager/grafana/config/api/v1/alerts",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        // Second review: allowlists, query strings, and the remaining credential reads.
        (
            "linear",
            Method::POST,
            "/graphql?query=%7Bviewer%7Bid%7D%7D",
            Some(r#"{"query":"{ viewer { id } }"}"#.into()),
            Present::Bare,
            no,
            vec![],
        ),
        (
            "newrelic",
            Method::POST,
            "/graphql?variables=%7B%7D",
            Some(r#"{"query":"{ actor { user { name } } }"}"#.into()),
            Present::Header("api-key"),
            no,
            vec![],
        ),
        (
            "sentry",
            Method::GET,
            "/api/0/organizations/acme/api-keys/",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "sentry",
            Method::GET,
            "/api/0/projects/acme/web/plugins/",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "pagerduty",
            Method::GET,
            "/services",
            None,
            Present::PagerDuty,
            ok,
            vec![("authorization", "Token token=pd_fixture".into())],
        ),
        (
            "pagerduty",
            Method::GET,
            "/services/S1/integrations/I1",
            None,
            Present::PagerDuty,
            no,
            vec![],
        ),
        (
            "pagerduty",
            Method::GET,
            "/services?include%5B%5D=integrations",
            None,
            Present::PagerDuty,
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/runs/run-1/plan/json-output-redacted",
            None,
            Present::Bearer,
            ok,
            vec![("authorization", "Bearer tf_fixture".into())],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/workspaces/ws-1/assessment-results",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/organizations/acme/workspaces.json",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/workspaces/ws-1?include%5B%5D=outputs",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#Logs_20140328.StartQuery",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "logs"),
            ok,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#AmazonAthena.StartQueryExecution",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "athena"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#AWSGlue.GetConnection",
            Some(r#"{"Name":"warehouse"}"#.into()),
            Present::Sigv4("us-east-1", "glue"),
            no,
            vec![],
        ),
        // Sixth review: reads that return secrets, Logs-only query reads, Query bodies are forms.
        (
            "aws",
            Method::POST,
            "/?Action=DescribeInstances&Version=2016-11-15",
            None,
            Present::Sigv4("us-east-1", "ec2"),
            ok,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/?Action=DescribeInstanceAttribute&Version=2016-11-15&Attribute=userData",
            None,
            Present::Sigv4("us-east-1", "ec2"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/?Action=GetFlowLogsIntegrationTemplate&Version=2016-11-15",
            None,
            Present::Sigv4("us-east-1", "ec2"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#AmazonEC2ContainerServiceV20141113.DescribeTaskDefinition",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "ecs"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#GraniteServiceVersion20100801.StartQuery",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "monitoring"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/?Action=GetCallerIdentity&Version=2011-06-15",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "sts"),
            no,
            vec![],
        ),
        // Fifth review: only supported services, each read in its own protocol.
        (
            "aws",
            Method::GET,
            "/?Action=TerminateEnvironment&EnvironmentName=prod",
            None,
            Present::Sigv4("us-east-1", "elasticbeanstalk"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/v1/logs?Action=GetLogEvents",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "logs"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::GET,
            "/reports/2026.csv?Action=ListBuckets",
            None,
            Present::Sigv4("us-east-1", "s3"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::GET,
            "/usageplans/u1/keys",
            None,
            Present::Sigv4("us-east-1", "apigateway"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#AmazonSSM.GetParameter",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "ssm"),
            no,
            vec![],
        ),
        (
            "terraform",
            Method::GET,
            "/api/v2/workspaces/ws-1/current-state-version",
            None,
            Present::Bearer,
            no,
            vec![],
        ),
        // AWS: re-signed with the connection's keys for the service and region the client signed
        // for; reads only, granted services and regions only, nothing that returns credentials.
        (
            "aws",
            Method::POST,
            "/#Logs_20140328.FilterLogEvents",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "logs"),
            ok,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#Logs_20140328.PutLogEvents",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "logs"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#Logs_20140328.FilterLogEvents",
            Some("{}".into()),
            Present::Sigv4("eu-west-1", "logs"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/#AmazonEC2.DescribeInstances",
            Some("{}".into()),
            Present::Sigv4("us-east-1", "ec2"),
            no,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/?Action=GetCallerIdentity&Version=2011-06-15",
            Some(String::new()),
            Present::Sigv4("us-east-1", "sts"),
            ok,
            vec![],
        ),
        (
            "aws",
            Method::POST,
            "/?Action=GetSessionToken&Version=2011-06-15",
            Some(String::new()),
            Present::Sigv4("us-east-1", "sts"),
            no,
            vec![],
        ),
    ];
    for (slug, method, path, body, present, status, expected) in cases {
        let before = upstream.seen.lock().unwrap().len();
        let (got, text) = call(slug, method.clone(), path, body, present).await;
        assert_eq!(got, status, "{slug} {method} {path}: {text}");
        for secret in &secrets {
            assert!(!text.contains(secret), "{secret} reached the sandbox");
        }
        let seen = upstream.seen.lock().unwrap()[before..].to_vec();
        if status != ok {
            assert!(seen.is_empty(), "{slug} {method} {path} reached upstream");
            continue;
        }
        assert_eq!(seen.len(), 1, "{slug} {method} {path}");
        let seen = &seen[0];
        assert!(
            seen.headers.values().all(|v| !v.contains(&token)),
            "the egress token reached upstream: {seen:?}"
        );
        for (name, value) in &expected {
            assert_eq!(seen.headers.get(*name), Some(value), "{slug} {name}");
        }
        match present {
            Present::Sigv4(..) => {
                assert!(seen.route.starts_with(&format!("{method} /")), "{seen:?}");
                assert_eq!(seen.signed, Some(true), "{slug} {path}: {seen:?}");
                assert!(seen.headers["authorization"].contains(&format!("Credential={AWS_KEY}/")));
            }
            _ if expected.is_empty() => {
                assert!(!seen.headers.contains_key("authorization"), "{seen:?}")
            }
            _ => assert_eq!(seen.route, format!("{method} {path}")),
        }
    }
    // An AWS request read as a read in one place cannot carry another action elsewhere: every
    // action it names must agree, form bodies are read whatever the content type's case, and an
    // RPC request naming no action is refused.
    let form = "application/x-www-form-urlencoded; charset=utf-8";
    // (service, path and query, headers, body)
    let confused: [Confused; 8] = [
        (
            "sts",
            "/",
            vec![
                (
                    "x-amz-target",
                    "AWSSecurityTokenServiceV20110615.GetCallerIdentity",
                ),
                ("content-type", form),
            ],
            "Action=AssumeRole&Version=2011-06-15",
        ),
        (
            "logs",
            "/service/Logs_20140328/operation/DeleteLogGroup",
            vec![
                ("x-amz-target", "Logs_20140328.FilterLogEvents"),
                ("content-type", "application/x-amz-json-1.1"),
            ],
            "{}",
        ),
        (
            "sts",
            "/?Action=GetCallerIdentity&Version=2011-06-15",
            vec![("content-type", form)],
            "Action=AssumeRole&Version=2011-06-15",
        ),
        (
            "sts",
            "/",
            vec![("content-type", "Application/X-WWW-Form-Urlencoded")],
            "Action=GetSessionToken&Version=2011-06-15",
        ),
        ("sts", "/", vec![("content-type", "application/json")], "{}"),
        // A REST service ignores action names: an invoke is a write whatever its target says.
        (
            "lambda",
            "/2015-03-31/functions/f/invocations",
            vec![("x-amz-target", "AWSLambda.GetFunction")],
            "{}",
        ),
        // One target, one content type, an unencoded body: as AWS reads the request.
        (
            "logs",
            "/",
            vec![
                ("x-amz-target", "Logs_20140328.FilterLogEvents"),
                ("x-amz-target", "Logs_20140328.DeleteLogGroup"),
            ],
            "{}",
        ),
        (
            "logs",
            "/",
            vec![
                ("x-amz-target", "Logs_20140328.FilterLogEvents"),
                ("content-encoding", "gzip"),
            ],
            "{}",
        ),
    ];
    for (service, path, headers, body) in confused {
        let before = upstream.seen.lock().unwrap().len();
        let mut request = http
            .post(format!("{}{path}", route("aws")))
            .body(body.to_owned())
            .build()
            .unwrap();
        for (name, value) in &headers {
            request.headers_mut().append(*name, value.parse().unwrap());
        }
        let credentials = Credentials {
            access_key_id: token.clone(),
            secret_access_key: "tilde-egress".into(),
            session_token: None,
        };
        Signer::new(credentials, "us-east-1", service)
            .sign_request_at(&mut request, body.as_bytes(), std::time::SystemTime::now())
            .unwrap();
        let response = http.execute(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{service} {path} {headers:?} {body}"
        );
        assert_eq!(
            upstream.seen.lock().unwrap().len(),
            before,
            "{service} {path} reached upstream"
        );
    }

    // Dot segments, sent raw (HTTP clients resolve them, encoded or not), never reach AWS: the
    // checked path is the one AWS would receive.
    let host = fx.origin.trim_start_matches("http://").to_owned();
    for (method, service, path) in [
        (
            "POST",
            "monitoring",
            "/sandboxes/egress/aws/service/GraniteServiceVersion20100801/operation/GetMetricData/%2E%2E/DeleteAlarms",
        ),
        (
            "GET",
            "lambda",
            "/sandboxes/egress/aws/2015-03-31/functions/f/x/../configuration",
        ),
        ("GET", "s3", "/sandboxes/egress/aws/reports/./2026.csv"),
    ] {
        let before = upstream.seen.lock().unwrap().len();
        let status = raw(&host, method, path, &format!("AWS4-HMAC-SHA256 Credential={token}/20261126/us-east-1/{service}/aws4_request, SignedHeaders=host, Signature=00")).await;
        assert_eq!(status, 403, "{method} {path}");
        assert_eq!(
            upstream.seen.lock().unwrap().len(),
            before,
            "{path} reached upstream"
        );
    }

    // One refresh, with the stored refresh token; the new tokens are kept.
    let refreshes = upstream.refreshes.lock().unwrap().clone();
    assert_eq!(refreshes.len(), 1, "{refreshes:?}");
    assert!(
        refreshes[0].contains("refresh_token=lin_refresh_1"),
        "{refreshes:?}"
    );
    let stored = fx.within(fx.connections.resolve(oauth)).await.unwrap();
    assert_eq!(
        secrecy::ExposeSecret::expose_secret(&stored["refresh_token"]),
        "lin_refresh_2"
    );
    fx.close().await;
}
