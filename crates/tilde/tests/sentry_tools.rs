//! The managed Sentry provider across a connection, an agent's tools, the catalog and a fake
//! Sentry: hand-written, official-mapped and generated OpenAPI tools reach the right REST calls
//! with the connection's token, upstream errors surface, and untrusted regions never see it.
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

type Seen = Arc<Mutex<Vec<(String, String, String, Value)>>>;

#[tokio::test]
async fn sentry_tools_call_the_sentry_api_with_the_connection_token() {
    let seen: Seen = Default::default();
    let sink = seen.clone();
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(
            move |method: axum::http::Method,
                  uri: axum::http::Uri,
                  headers: axum::http::HeaderMap,
                  body: String| {
                let sink = sink.clone();
                async move {
                    let target = uri
                        .path_and_query()
                        .map(|p| p.to_string())
                        .unwrap_or_default();
                    let auth = headers
                        .get("authorization")
                        .map(|v| v.to_str().unwrap().to_owned())
                        .unwrap_or_default();
                    let body = serde_json::from_str(&body).unwrap_or(Value::Null);
                    sink.lock()
                        .unwrap()
                        .push((method.to_string(), target, auth, body));
                    match (method.as_str(), uri.path()) {
                        (_, "/api/0/organizations/acme/issues/MISSING/") => (
                            axum::http::StatusCode::NOT_FOUND,
                            axum::Json(json!({"detail": "Issue not found"})),
                        ),
                        ("GET", "/api/0/organizations/acme/issues/") => (
                            axum::http::StatusCode::OK,
                            axum::Json(json!([{"id": "42", "title": "Boom"}])),
                        ),
                        ("GET", "/api/0/projects/acme/api/keys/") => (
                            axum::http::StatusCode::OK,
                            axum::Json(json!([{"id": "key-1"}])),
                        ),
                        _ => (
                            axum::http::StatusCode::OK,
                            axum::Json(json!({"id": "ok", "path": uri.path()})),
                        ),
                    }
                }
            },
        );
        axum::serve(upstream, app).await.unwrap()
    });
    let fx = Fixture::new(
        73,
        Endpoints(BTreeMap::from([("sentry_api".into(), origin.clone())])),
        all_tools(),
    )
    .await;
    let tools = fx.chat.tools.clone().unwrap();
    let pg = fx.db.pool.get().await.unwrap();
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(73))
        .await
        .unwrap();
    let connect = async |typ: &str, key: &str, secret: &str| {
        let connection = Uuid::new_v4();
        pg.execute(
            "INSERT INTO connections(id,name,provider_id,type_id,status) VALUES($1,$2,'sentry',$3,'ready')",
            &[&connection, &typ, &typ],
        )
        .await
        .unwrap();
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: connection,
                    name: key,
                },
                &SecretString::from(secret),
            )
            .unwrap()
            .into_bytes();
        pg.execute(
            "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)",
            &[&connection, &key, &sealed],
        )
        .await
        .unwrap();
        connection
    };
    let token = connect("token", "auth_token", "sntrys_fixture").await;
    let oauth = connect("oauth", "access_token", "oauth-access").await;

    let offered = tools.provider_tools(token).await.unwrap();
    assert_eq!(
        offered.len(),
        237,
        "10 hand-written, 38 official, 1 Composio, 188 generated"
    );
    let hint = |name: &str| {
        let tool = offered.iter().find(|t| t.name == name).unwrap();
        let annotations = tool.annotations.clone().unwrap();
        (annotations.read_only, annotations.destructive)
    };
    assert_eq!(hint("sentry_search_issues"), (true, false));
    assert_eq!(hint("sentry_remove_team_from_project"), (false, true));
    assert_eq!(
        hint("sentry_create_organization_release_deploy"),
        (false, false)
    );

    assert_eq!(
        fx.use_tools(
            token,
            &[
                "sentry_search_issues",
                "sentry_update_issue",
                "sentry_get_issue_details",
                "sentry_find_dsns",
                "sentry_create_organization_release_deploy"
            ]
        )
        .await,
        "sentry_token"
    );
    assert_eq!(
        fx.use_tools(oauth, &["sentry_whoami"]).await,
        "sentry_oauth"
    );
    let take = || std::mem::take(&mut *seen.lock().unwrap());
    let bearer = "Bearer sntrys_fixture".to_owned();

    // Hand-written: defaults fill the issue search, and an issue URL names org and issue.
    assert_eq!(
        fx.output(
            "sentry_token.sentry_search_issues",
            json!({"organizationSlug": "acme", "sort": "freq"})
        )
        .await,
        json!([{"id": "42", "title": "Boom"}])
    );
    assert_eq!(
        take(),
        vec![(
            "GET".into(),
            "/api/0/organizations/acme/issues/?query=is%3Aunresolved&statsPeriod=30d&per_page=10&sort=freq".into(),
            bearer.clone(),
            Value::Null
        )]
    );
    let updated = fx
        .output(
            "sentry_token.sentry_update_issue",
            json!({"issueUrl": "https://acme.sentry.io/issues/API-42/", "status": "ignored", "ignoreDurationMinutes": 30, "reason": "Flaky"}),
        )
        .await;
    assert_eq!(updated["issue"]["id"], "ok");
    assert_eq!(updated["reasonNote"]["id"], "ok");
    assert_eq!(
        take(),
        vec![
            (
                "PUT".into(),
                "/api/0/organizations/acme/issues/API-42/".into(),
                bearer.clone(),
                json!({"status": "ignored", "substatus": "archived_until_condition_met", "ignoreDuration": 30})
            ),
            (
                "POST".into(),
                "/api/0/organizations/acme/issues/API-42/notes/".into(),
                bearer.clone(),
                json!({"text": "Flaky"})
            ),
        ]
    );

    // Official-mapped: camelCase inputs become the REST operation's parameters.
    assert_eq!(
        fx.output(
            "sentry_token.sentry_find_dsns",
            json!({"organizationSlug": "acme", "projectSlug": "api"})
        )
        .await,
        json!({"dsns": [{"id": "key-1"}]})
    );
    assert_eq!(
        take(),
        vec![(
            "GET".into(),
            "/api/0/projects/acme/api/keys/".into(),
            bearer.clone(),
            Value::Null
        )]
    );

    // Generated OpenAPI: path parameters are encoded and the rest is the JSON body.
    fx.output(
        "sentry_token.sentry_create_organization_release_deploy",
        json!({"organization_id_or_slug": "acme", "version": "1.0 beta", "environment": "production"}),
    )
    .await;
    assert_eq!(
        take(),
        vec![(
            "POST".into(),
            "/api/0/organizations/acme/releases/1.0%20beta/deploys/".into(),
            bearer.clone(),
            json!({"environment": "production"})
        )]
    );

    // OAuth connections authenticate with their access token.
    fx.output("sentry_oauth.sentry_whoami", json!({})).await;
    assert_eq!(take()[0].2, "Bearer oauth-access");

    // Upstream errors carry Sentry's detail.
    let failed = fx
        .invoke(
            "sentry_token.sentry_get_issue_details",
            json!({"organizationSlug": "acme", "issueId": "MISSING"}),
        )
        .await;
    assert!(
        failed["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("404") && m.contains("Issue not found")),
        "{failed}"
    );
    take();

    // Only HTTPS sentry.io origins may receive the token.
    for region in [
        "https://evil.example",
        "http://us.sentry.io",
        "https://sentry.io.evil.example",
        "https://us.sentry.io/api/0",
    ] {
        let rejected = fx
            .invoke(
                "sentry_token.sentry_search_issues",
                json!({"organizationSlug": "acme", "regionUrl": region}),
            )
            .await;
        assert!(rejected.get("error").is_some(), "{region}: {rejected}");
    }
    assert!(take().is_empty(), "rejected regions are never called");

    drop(pg);
    server.abort();
    fx.close().await;
}
