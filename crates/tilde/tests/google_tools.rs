//! Google Workspace managed tools across their boundaries: OAuth connections seeded with an
//! encrypted access token, the agent's tools on each, invocation over the agent runtime and
//! the upstream Google APIs (a fake recording each request).
mod common;
use axum::response::IntoResponse;
use base64::Engine;
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

/// Method, path with query, authorization header and body of one upstream request.
type Seen = (String, String, String, Vec<u8>);

fn respond(method: &str, path: &str) -> (u16, Value) {
    match (method, path) {
        ("GET", "/gmail/v1/users/me/messages") => (
            200,
            json!({"messages":[{"id":"m1"}],"resultSizeEstimate":1}),
        ),
        ("GET", "/gmail/v1/users/me/messages/m1") => (
            200,
            json!({"id":"m1","threadId":"t1","snippet":"Hi","labelIds":["INBOX"],"payload":{
                "headers":[{"name":"From","value":"ann@example.com"},{"name":"subject","value":"Hello"}]
            }}),
        ),
        ("POST", "/gmail/v1/users/me/messages/send") => (200, json!({"id":"sent1"})),
        ("GET", "/sheets/v4/spreadsheets/s1/values/My%20Sheet!A1:B2") => (
            200,
            json!({"range":"'My Sheet'!A1:B2","majorDimension":"ROWS","values":[["a","b"]]}),
        ),
        ("POST", "/data/v1beta/properties/123:runReport") => (
            200,
            json!({"rowCount":1,"dimensionHeaders":[{"name":"country"}]}),
        ),
        ("POST", "/measurement/mp/collect") => (204, Value::Null),
        _ => (404, json!({"error":{"message":"File not found"}})),
    }
}

#[tokio::test]
async fn google_tools_call_google_apis_with_the_connection_token() {
    let seen: Arc<Mutex<Vec<Seen>>> = Default::default();
    let sink = seen.clone();
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let upstream_server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(
            move |method: axum::http::Method,
                  uri: axum::http::Uri,
                  headers: axum::http::HeaderMap,
                  body: axum::body::Bytes| {
                let sink = sink.clone();
                async move {
                    let auth = headers
                        .get("authorization")
                        .map(|value| value.to_str().unwrap().to_owned())
                        .unwrap_or_default();
                    let (status, reply) = respond(method.as_str(), uri.path());
                    sink.lock().unwrap().push((
                        method.to_string(),
                        uri.to_string(),
                        auth,
                        body.to_vec(),
                    ));
                    let status = axum::http::StatusCode::from_u16(status).unwrap();
                    match reply {
                        Value::Null => status.into_response(),
                        reply => (status, axum::Json(reply)).into_response(),
                    }
                }
            },
        );
        axum::serve(upstream, app).await.unwrap()
    });
    let endpoints = [
        ("google_gmail_api", "/gmail/v1"),
        ("google_sheets_api", "/sheets/v4"),
        ("google_drive_api", "/drive/v3"),
        ("google_analytics_data", "/data"),
        ("google_analytics_measurement", "/measurement"),
    ]
    .map(|(key, path)| (key.to_owned(), format!("{origin}{path}")));
    let fx = Fixture::new(61, Endpoints(BTreeMap::from(endpoints)), all_tools()).await;
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(61))
        .await
        .unwrap();
    let pg = fx.db.pool.get().await.unwrap();

    // One ready OAuth connection per product, as the OAuth broker would leave it.
    let mut connections = BTreeMap::new();
    for provider in [
        "google_mail",
        "google_sheets",
        "google_drive",
        "google_analytics",
    ] {
        let id = Uuid::new_v4();
        fx.connections
            .start(id, provider, provider, "oauth", &[])
            .await
            .unwrap();
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: id,
                    name: "access_token",
                },
                &SecretString::from(format!("ya29.{provider}")),
            )
            .unwrap()
            .into_bytes();
        pg.execute("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,'access_token',$2)", &[&id, &sealed]).await.unwrap();
        pg.execute("UPDATE connections SET status='ready' WHERE id=$1", &[&id])
            .await
            .unwrap();
        connections.insert(provider, id);
    }
    // Each connection's slug is its provider's, since the account is named after it.
    for (provider, tools) in [
        (
            "google_mail",
            &["google_mail_fetch_emails", "google_mail_send_email"][..],
        ),
        ("google_sheets", &["google_sheets_read_range"]),
        ("google_drive", &["google_drive_get_file_metadata"]),
        (
            "google_analytics",
            &[
                "google_analytics_run_report",
                "google_analytics_send_events",
            ],
        ),
    ] {
        assert_eq!(fx.use_tools(connections[provider], tools).await, provider);
    }
    let tools = fx.tools().await;
    let fetch = tools
        .iter()
        .find(|t| t["name"] == "google_mail.google_mail_fetch_emails")
        .unwrap();
    assert_eq!(fetch["providerId"], "google_mail");
    assert_eq!(fetch["annotations"]["readOnly"], true);
    let take = || std::mem::take(&mut *seen.lock().unwrap());

    // A read spanning two calls, flattened into Gmail summaries.
    let fetched = fx
        .output(
            "google_mail.google_mail_fetch_emails",
            json!({"query":"is:unread","max_results":5}),
        )
        .await;
    assert_eq!(
        fetched,
        json!({"messages":[{"id":"m1","thread_id":"t1","snippet":"Hi","from":"ann@example.com","to":null,
            "subject":"Hello","date":null,"label_ids":["INBOX"],"body":null}],
            "next_page_token":null,"result_size_estimate":1})
    );
    let requests = take();
    assert_eq!(
        requests
            .iter()
            .map(|(method, uri, auth, _)| (method.as_str(), uri.as_str(), auth.as_str()))
            .collect::<Vec<_>>(),
        [
            (
                "GET",
                "/gmail/v1/users/me/messages?maxResults=5&q=is%3Aunread",
                "Bearer ya29.google_mail"
            ),
            (
                "GET",
                "/gmail/v1/users/me/messages/m1?format=metadata",
                "Bearer ya29.google_mail"
            ),
        ]
    );

    // A write: the message is sent as a base64url RFC 2822 document.
    let sent = fx
        .output(
            "google_mail.google_mail_send_email",
            json!({"to":"bob@example.com","subject":"Plan","body":"<b>Go</b>","is_html":true}),
        )
        .await;
    assert_eq!(sent, json!({"message_id":"sent1"}));
    let (_, uri, _, body) = take().remove(0);
    assert_eq!(uri, "/gmail/v1/users/me/messages/send");
    let body: Value = serde_json::from_slice(&body).unwrap();
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(body["raw"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        String::from_utf8(raw).unwrap(),
        "To: bob@example.com\r\nSubject: Plan\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<b>Go</b>"
    );

    // Path parameters are encoded as single segments.
    let read = fx
        .output(
            "google_sheets.google_sheets_read_range",
            json!({"spreadsheet_id":"s1","range":"My Sheet!A1:B2"}),
        )
        .await;
    assert_eq!(read["values"], json!([["a", "b"]]));
    assert_eq!(take()[0].2, "Bearer ya29.google_sheets");

    // Table-driven analytics: resource names are reduced to IDs and keys are camelCased out
    // and snake_cased back.
    let report = fx
        .output(
            "google_analytics.google_analytics_run_report",
            json!({"property":"properties/123","date_ranges":[{"start_date":"7daysAgo","end_date":"today"}],"keep_empty_rows":null}),
        )
        .await;
    assert_eq!(
        report,
        json!({"row_count":1,"dimension_headers":[{"name":"country"}]})
    );
    let (method, uri, auth, body) = take().remove(0);
    assert_eq!(
        (method.as_str(), uri.as_str(), auth.as_str()),
        (
            "POST",
            "/data/v1beta/properties/123:runReport",
            "Bearer ya29.google_analytics"
        )
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"dateRanges":[{"startDate":"7daysAgo","endDate":"today"}]})
    );
    // Measurement Protocol is authenticated by its query, never the Google token.
    let events = fx
        .output(
            "google_analytics.google_analytics_send_events",
            json!({"measurement_id":"G-1","api_secret":"s3cret","client_id":"c1","events":[{"name":"signup"}]}),
        )
        .await;
    assert_eq!(events, json!({}));
    let (_, uri, auth, body) = take().remove(0);
    assert_eq!(
        (uri.as_str(), auth.as_str()),
        (
            "/measurement/mp/collect?measurementId=G-1&apiSecret=s3cret",
            ""
        )
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"clientId":"c1","events":[{"name":"signup"}]})
    );

    // Google's error reaches the agent so it can correct the call.
    let failed = fx
        .invoke(
            "google_drive.google_drive_get_file_metadata",
            json!({"file_id":"missing"}),
        )
        .await;
    let message = failed["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("404") && message.contains("File not found"),
        "{failed}"
    );
    assert_eq!(take()[0].1, "/drive/v3/files/missing?fields=*");

    drop(pg);
    upstream_server.abort();
    fx.close().await;
}
