//! Managed tool providers across their boundaries: an installation connection with encrypted
//! credentials, an agent using a provider tool, the agent's catalog call, and the
//! upstream request each provider builds (method, path, query, authentication and body) against
//! an in-process stand-in for PostHog, Firecrawl, Stripe, Payload and Tavily.
mod common;
use common::invocation::{Fixture, all_tools};
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::{
    connections::catalog::Endpoints,
    encryption::{Encryption, SecretBinding},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
struct Seen {
    method: String,
    target: String,
    auth: String,
    headers: axum::http::HeaderMap,
    body: String,
}
impl Seen {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

async fn upstream(seen: Arc<Mutex<Vec<Seen>>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let polls = Arc::new(Mutex::new(0));
    let app = axum::Router::new().fallback(
        move |method: axum::http::Method,
              uri: axum::http::Uri,
              headers: axum::http::HeaderMap,
              body: axum::body::Bytes| {
            let seen = seen.clone();
            let polls = polls.clone();
            async move {
                let auth = headers
                    .get("authorization")
                    .map(|v| v.to_str().unwrap().to_owned())
                    .unwrap_or_default();
                seen.lock().unwrap().push(Seen {
                    method: method.to_string(),
                    target: uri.to_string(),
                    auth,
                    headers,
                    body: String::from_utf8(body.to_vec()).unwrap(),
                });
                let (status, reply) = match (method.as_str(), uri.path()) {
                    ("GET", "/api/projects/7/") => (200, json!({"id":7,"api_token":"phc_project"})),
                    ("POST", "/i/v0/e/") => (200, json!({"status":1})),
                    ("GET", "/api/projects/7/persons/") => {
                        (200, json!({"results":[{"distinct_ids":["user-1"]}]}))
                    }
                    ("PATCH", "/api/projects/7/insights/99/") => {
                        (404, json!({"detail":"Not found."}))
                    }
                    ("POST", "/v2/crawl") => (200, json!({"success":true,"id":"crawl-1"})),
                    ("GET", "/v2/crawl/crawl-1") => {
                        let mut polls = polls.lock().unwrap();
                        *polls += 1;
                        if *polls < 2 {
                            (200, json!({"status":"scraping"}))
                        } else {
                            (
                                200,
                                json!({"status":"completed","data":[{"markdown":"# Docs"}]}),
                            )
                        }
                    }
                    ("POST", "/v1/refunds") => (200, json!({"id":"re_1","status":"succeeded"})),
                    _ => (200, json!({"ok":true})),
                };
                (
                    axum::http::StatusCode::from_u16(status).unwrap(),
                    axum::Json(reply),
                )
            }
        },
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    origin
}

/// A ready installation connection, its values sealed as the setup broker would leave them.
async fn connection(fx: &Fixture, provider: &str, values: &[(&str, String)]) -> Uuid {
    let id = Uuid::new_v4();
    let pg = fx.db.pool.get().await.unwrap();
    pg.execute(
        "INSERT INTO connections(id,name,provider_id,type_id,status) VALUES($1,$2,$2,'api','ready')",
        &[&id, &provider],
    )
    .await
    .unwrap();
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(91))
        .await
        .unwrap();
    for (key, value) in values {
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: id,
                    name: key,
                },
                &SecretString::from(value.clone()),
            )
            .unwrap()
            .into_bytes();
        pg.execute("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)", &[&id, key, &sealed]).await.unwrap();
    }
    id
}

#[tokio::test]
async fn managed_provider_tools_call_their_upstream_apis_with_connection_credentials() {
    let seen: Arc<Mutex<Vec<Seen>>> = Default::default();
    let origin = upstream(seen.clone()).await;
    let endpoints = [
        "posthog_api",
        "firecrawl_api",
        "stripe_api",
        "tavily_api",
        "payload_private_origin",
    ]
    .map(|key| (key.to_owned(), origin.clone()));
    let fx = Fixture::new(91, Endpoints(BTreeMap::from(endpoints)), all_tools()).await;
    let tools = fx.chat.tools.clone().unwrap();
    let posthog = connection(
        &fx,
        "posthog",
        &[
            ("subdomain", "eu".into()),
            ("api_key", "phx_personal".into()),
        ],
    )
    .await;
    let firecrawl = connection(&fx, "firecrawl", &[("api_key", "fc-key".into())]).await;
    let stripe = connection(&fx, "stripe", &[("api_key", "rk_test".into())]).await;
    let payload = connection(
        &fx,
        "payload",
        &[
            ("api_base_url", format!("{origin}/api")),
            ("auth_collection_slug", "editors".into()),
            ("api_key", "pl-key".into()),
        ],
    )
    .await;
    let tavily = connection(&fx, "tavily", &[("api_key", "tvly-key".into())]).await;

    assert_eq!(tools.provider_tools(posthog).await.unwrap().len(), 66);
    assert_eq!(tools.provider_tools(firecrawl).await.unwrap().len(), 26);
    assert_eq!(tools.provider_tools(payload).await.unwrap().len(), 7);

    assert_eq!(
        fx.use_tools(
            posthog,
            &[
                "list_feature_flags",
                "update_feature_flag",
                "delete_insight",
                "create_person"
            ]
        )
        .await,
        "posthog"
    );
    assert_eq!(
        fx.use_tools(firecrawl, &["scrape", "crawl", "monitor_checks"])
            .await,
        "firecrawl"
    );
    assert_eq!(fx.use_tools(stripe, &["process_refund"]).await, "stripe");
    assert_eq!(
        fx.use_tools(payload, &["find_documents", "update_document"])
            .await,
        "payload"
    );
    assert_eq!(fx.use_tools(tavily, &["usage"]).await, "tavily");
    let take = || std::mem::take(&mut *seen.lock().unwrap());

    // PostHog: list filters become the query, update fields the body, the personal key the auth.
    let flags = fx
        .output(
            "posthog.list_feature_flags",
            json!({"project_id":7,"query":{"limit":5,"search":"beta"}}),
        )
        .await;
    assert_eq!(flags["status"], 200);
    let flag = fx
        .output(
            "posthog.update_feature_flag",
            json!({"project_id":"7","id":3,"data":{"active":false}}),
        )
        .await;
    assert_eq!(flag["data"]["ok"], true);
    let calls = take();
    assert_eq!(
        (
            calls[0].method.as_str(),
            calls[0].target.as_str(),
            calls[0].auth.as_str()
        ),
        (
            "GET",
            "/api/projects/7/feature_flags/?limit=5&search=beta",
            "Bearer phx_personal"
        )
    );
    assert_eq!(
        (calls[1].method.as_str(), calls[1].target.as_str()),
        ("PATCH", "/api/projects/7/feature_flags/3/")
    );
    assert_eq!(calls[1].json(), json!({"active":false}));

    // An upstream failure is the tool's error with PostHog's message. PostHog refuses to hard
    // delete insights, so deleting one archives it.
    let failed = fx
        .invoke("posthog.delete_insight", json!({"project_id":7,"id":99}))
        .await;
    let message = failed["error"].to_string();
    assert!(
        message.contains("404") && message.contains("Not found."),
        "{failed}"
    );
    assert_eq!(take()[0].json(), json!({"deleted":true}));

    // Creating a person identifies it with the project's token, then reads it back.
    let person = fx
        .output(
            "posthog.create_person",
            json!({"project_id":7,"distinct_id":"user-1","properties":{"plan":"pro"}}),
        )
        .await;
    assert_eq!(person["data"]["results"][0]["distinct_ids"][0], "user-1");
    let calls = take();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1].target, "/i/v0/e/");
    assert_eq!(
        calls[1].auth, "",
        "capture authenticates by the project token"
    );
    assert_eq!(
        calls[1].json(),
        json!({"api_key":"phc_project","event":"$identify","distinct_id":"user-1","properties":{"$set":{"plan":"pro"}}})
    );
    assert_eq!(
        calls[2].target,
        "/api/projects/7/persons/?distinct_id=user-1"
    );

    // Firecrawl: typed formats, the origin marker in header and body.
    fx.output(
        "firecrawl.scrape",
        json!({"url":"https://docs.example","formats":["markdown","json"],"jsonOptions":{"prompt":"title"},"excludeTags":[]}),
    )
    .await;
    let calls = take();
    assert_eq!(
        (calls[0].method.as_str(), calls[0].target.as_str()),
        ("POST", "/v2/scrape")
    );
    assert_eq!(calls[0].auth, "Bearer fc-key");
    assert_eq!(calls[0].headers["x-origin"], "tilde-managed-provider");
    assert_eq!(
        calls[0].json(),
        json!({"url":"https://docs.example","formats":["markdown",{"type":"json","prompt":"title"}],"origin":"tilde-managed-provider"})
    );

    // A crawl starts a job and polls it until it completes.
    let crawled = fx
        .output(
            "firecrawl.crawl",
            json!({"url":"https://docs.example","limit":3,"pollInterval":1}),
        )
        .await;
    assert_eq!(crawled["data"]["status"], "completed");
    let calls = take();
    assert_eq!(
        calls
            .iter()
            .map(|c| format!("{} {}", c.method, c.target))
            .collect::<Vec<_>>(),
        [
            "POST /v2/crawl",
            "GET /v2/crawl/crawl-1",
            "GET /v2/crawl/crawl-1"
        ]
    );
    assert_eq!(
        calls[0].json(),
        json!({"url":"https://docs.example","limit":3,"origin":"tilde-managed-provider"})
    );
    fx.output(
        "firecrawl.monitor_checks",
        json!({"id":"m1","limit":10,"status":"changed"}),
    )
    .await;
    assert_eq!(
        take()[0].target,
        "/v2/monitor/m1/checks?limit=10&status=changed"
    );

    // Stripe: a form-encoded refund, keyed by the call for idempotency.
    assert!(
        fx.invoke(
            "stripe.process_refund",
            json!({"charge":"ch_1","payment_intent":"pi_1"})
        )
        .await
        .get("error")
        .is_some()
    );
    let refund = fx
        .output(
            "stripe.process_refund",
            json!({"payment_intent":"pi_123","amount":500,"reason":"requested_by_customer"}),
        )
        .await;
    assert_eq!(refund["data"]["id"], "re_1");
    let calls = take();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (calls[0].method.as_str(), calls[0].target.as_str()),
        ("POST", "/v1/refunds")
    );
    assert_eq!(calls[0].auth, "Bearer rk_test");
    assert_eq!(
        calls[0].headers["content-type"],
        "application/x-www-form-urlencoded"
    );
    assert!(Uuid::parse_str(calls[0].headers["idempotency-key"].to_str().unwrap()).is_ok());
    assert_eq!(
        calls[0].body,
        "payment_intent=pi_123&amount=500&reason=requested_by_customer"
    );

    // Payload: the collection API key header, qs-encoded filters and the document body.
    fx.output(
        "payload.find_documents",
        json!({"collection_slug":"posts","where":{"title":{"equals":"Hello"}},"limit":5,"fallback_locale":"en"}),
    )
    .await;
    assert!(
        fx.invoke(
            "payload.update_document",
            json!({"collection_slug":"posts","data":{"title":"x"}})
        )
        .await
        .get("error")
        .is_some(),
        "an update names a document or a filter"
    );
    fx.output(
        "payload.update_document",
        json!({"collection_slug":"posts","id":42,"data":{"title":"Hi"}}),
    )
    .await;
    let calls = take();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].auth, "editors API-Key pl-key");
    assert_eq!(
        calls[0].target,
        "/api/posts?fallbackLocale=en&limit=5&where%5Btitle%5D%5Bequals%5D=Hello"
    );
    assert_eq!(
        (calls[1].method.as_str(), calls[1].target.as_str()),
        ("PATCH", "/api/posts/42")
    );
    assert_eq!(calls[1].json(), json!({"title":"Hi"}));

    // Tavily usage reads the key's credits.
    fx.output("tavily.usage", json!({})).await;
    let calls = take();
    assert_eq!(
        (
            calls[0].method.as_str(),
            calls[0].target.as_str(),
            calls[0].auth.as_str()
        ),
        ("GET", "/usage", "Bearer tvly-key")
    );

    fx.close().await;
}
