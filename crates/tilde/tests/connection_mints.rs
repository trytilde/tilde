//! Mints against a fake GitHub API: an App connection's tool call sends the installation token
//! the connection keeps, a scoped mint asks GitHub for a token limited to the scope's
//! repositories and permissions, repeats come from the cache, and a credential change mints anew.
mod common;
use axum::{Json, Router, extract::State, http::HeaderMap, routing::get, routing::post};
use common::live::Live;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tilde::connections::{
    catalog::Endpoints,
    mints::{self, MintScope},
};

const KEY: &str = include_str!("fixtures/connection-test-key.pem");

#[derive(Default)]
struct Upstream {
    /// (method and path, authorization, body) of every request.
    seen: Mutex<Vec<(String, String, Value)>>,
}
impl Upstream {
    fn record(&self, route: &str, headers: &HeaderMap, body: Value) -> usize {
        let auth = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let mut seen = self.seen.lock().unwrap();
        seen.push((route.to_owned(), auth, body));
        seen.iter().filter(|(r, ..)| r == route).count()
    }
    fn count(&self, route: &str) -> usize {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|(r, ..)| r == route)
            .count()
    }
    fn last(&self, route: &str) -> (String, Value) {
        let seen = self.seen.lock().unwrap();
        let (_, auth, body) = seen.iter().rev().find(|(r, ..)| r == route).unwrap();
        (auth.clone(), body.clone())
    }
}

async fn github() -> (String, Arc<Upstream>) {
    let upstream = Arc::new(Upstream::default());
    let app = Router::new()
        .route(
            "/app/installations/42",
            get(|State(u): State<Arc<Upstream>>, h: HeaderMap| async move {
                u.record("GET installation", &h, Value::Null);
                Json(json!({"id":42,"account":{"login":"Acme"}}))
            }),
        )
        .route(
            "/app/installations/42/access_tokens",
            post(
                |State(u): State<Arc<Upstream>>, h: HeaderMap, Json(body): Json<Value>| async move {
                    let n = u.record("POST access_tokens", &h, body);
                    let expires = chrono::Utc::now() + chrono::Duration::hours(1);
                    Json(json!({"token":format!("ghs_scoped_{n}"),"expires_at":expires.to_rfc3339()}))
                },
            ),
        )
        .route(
            "/repos/acme/widgets/issues/1",
            get(|State(u): State<Arc<Upstream>>, h: HeaderMap| async move {
                u.record("GET issue", &h, Value::Null);
                Json(json!({"number":1}))
            }),
        )
        .with_state(upstream.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (origin, upstream)
}

fn bearer(minted: &mints::Minted) -> String {
    let (name, value) = &minted.headers()[0];
    assert_eq!(name, "authorization");
    value.expose_secret().to_owned()
}

#[tokio::test]
async fn github_app_mints_scoped_installation_tokens_and_caches_them() {
    let (origin, upstream) = github().await;
    let live = Live::with_endpoints(
        71,
        "github",
        "github_app",
        &[
            ("app_id", "1001".into()),
            ("installation_id", "42".into()),
            ("private_key", KEY.into()),
            ("access_token", "ghs_stored".into()),
        ],
        Endpoints(BTreeMap::from([("github_api".into(), origin)])),
    )
    .await;

    // Tool calls send the installation token the connection keeps refreshed, unscoped.
    let issue = live
        .call(
            "github_get_issue",
            json!({"owner":"acme","repo":"widgets","issue_number":1}),
        )
        .await;
    assert_eq!(issue["data"]["number"], 1);
    assert_eq!(upstream.last("GET issue").0, "Bearer ghs_stored");
    assert_eq!(upstream.count("POST access_tokens"), 0);

    let access = live.fx.within(live.fx.connections.access(live.connection));
    let access = access.await.unwrap();
    let push = MintScope {
        resources: vec!["acme/widgets".into()],
        permissions: vec![("contents".into(), "write".into())],
    };
    let minted = mints::mint(&access, "github", "github_app", &push, "egress")
        .await
        .unwrap();
    assert_eq!(bearer(&minted), "Bearer ghs_scoped_1");
    assert!(minted.expires_at.is_some());
    // The token request names the repository within the installation's account, signed as the app.
    let (auth, body) = upstream.last("POST access_tokens");
    assert_eq!(
        body,
        json!({"repositories":["widgets"],"permissions":{"contents":"write"}})
    );
    assert_eq!(auth.trim_start_matches("Bearer ").split('.').count(), 3);

    // Repeats are cached, concurrent ones included; another scope mints its own token.
    let (a, b) = tokio::join!(
        mints::mint(&access, "github", "github_app", &push, "egress"),
        mints::mint(&access, "github", "github_app", &push, "egress"),
    );
    assert_eq!(bearer(&a.unwrap()), "Bearer ghs_scoped_1");
    assert_eq!(bearer(&b.unwrap()), "Bearer ghs_scoped_1");
    assert_eq!(upstream.count("POST access_tokens"), 1);
    let read = MintScope {
        permissions: vec![("contents".into(), "read".into())],
        ..push.clone()
    };
    let other = mints::mint(&access, "github", "github_app", &read, "egress")
        .await
        .unwrap();
    assert_eq!(bearer(&other), "Bearer ghs_scoped_2");

    // A repository of another account is never confused with the installation's own.
    let foreign = MintScope {
        resources: vec!["mallory/widgets".into()],
        permissions: vec![],
    };
    let refused = mints::mint(&access, "github", "github_app", &foreign, "egress").await;
    assert!(refused.is_err());
    assert_eq!(upstream.count("POST access_tokens"), 2);

    // A credential change misses the cache, as does the configuration notification's flush.
    live.fx
        .within(live.fx.connections.replace_value(
            live.connection,
            "access_token",
            &SecretString::from("ghs_rotated"),
        ))
        .await
        .unwrap();
    let access = live.fx.within(live.fx.connections.access(live.connection));
    let access = access.await.unwrap();
    let again = mints::mint(&access, "github", "github_app", &push, "egress")
        .await
        .unwrap();
    assert_eq!(bearer(&again), "Bearer ghs_scoped_3");
    live.fx.connections.forget_credentials();
    let flushed = mints::mint(&access, "github", "github_app", &push, "egress")
        .await
        .unwrap();
    assert_eq!(bearer(&flushed), "Bearer ghs_scoped_4");
    live.close().await;
}
