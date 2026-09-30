//! GitHub and Slack managed tools across their boundaries: encrypted connection credentials,
//! the agent's tools on each, the invocation catalog over ConnectRPC and the upstream REST
//! call, checked against in-process stand-ins for api.github.com and slack.com/api.
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

/// Method, path with query, authorization, content type and body of one upstream request.
type Seen = (String, String, String, String, String);

async fn upstream(
    method: axum::http::Method,
    uri: axum::http::Uri,
) -> (
    axum::http::StatusCode,
    [(String, String); 1],
    axum::Json<Value>,
) {
    use axum::http::StatusCode;
    let rate = [("x-ratelimit-remaining".to_owned(), "4999".to_owned())];
    let (status, reply) = match (method.as_str(), uri.path()) {
        ("GET", "/github/repos/octo/app/issues/7") => (StatusCode::OK, json!({"number":7})),
        ("GET", "/github/search/issues") => (StatusCode::OK, json!({"total_count":0})),
        ("POST", "/github/repos/octo/app/issues/7/comments") => {
            (StatusCode::CREATED, json!({"id":99}))
        }
        ("GET", "/github/repos/octo/app/contents/src/lib.rs") => {
            (StatusCode::OK, json!({"type":"file"}))
        }
        ("DELETE", "/github/repos/octo/app/contents/docs/old.md") => {
            (StatusCode::OK, json!({"commit":{"sha":"c2"}}))
        }
        ("GET", "/github/repos/octo/gone") => {
            (StatusCode::NOT_FOUND, json!({"message":"Not Found"}))
        }
        ("POST", "/slack/chat.postMessage") => (StatusCode::OK, json!({"ok":true,"ts":"1.2"})),
        ("GET", "/slack/conversations.history") => {
            (StatusCode::OK, json!({"ok":true,"messages":[]}))
        }
        ("POST", "/slack/files.getUploadURLExternal") => {
            (StatusCode::OK, json!({"ok":true,"file_id":"F1"}))
        }
        ("POST", "/slack/pins.add") => (
            StatusCode::OK,
            json!({"ok":false,"error":"missing_scope","needed":"pins:write"}),
        ),
        _ => (StatusCode::IM_A_TEAPOT, json!({})),
    };
    (status, rate, axum::Json(reply))
}

async fn seed(fx: &Fixture, provider: &str, typ: &str, values: &[(&str, &str)]) -> Uuid {
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(83))
        .await
        .unwrap();
    let connection = Uuid::new_v4();
    fx.connections
        .start(connection, provider, provider, typ, &[])
        .await
        .unwrap();
    let pg = fx.db.pool.get().await.unwrap();
    for (key, secret) in values {
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: connection,
                    name: key,
                },
                &SecretString::from(*secret),
            )
            .unwrap()
            .into_bytes();
        pg.execute("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)", &[&connection, key, &sealed]).await.unwrap();
    }
    pg.execute(
        "UPDATE connections SET status='ready' WHERE id=$1",
        &[&connection],
    )
    .await
    .unwrap();
    connection
}

#[tokio::test]
async fn github_and_slack_tools_call_their_apis_with_connection_credentials() {
    let seen: Arc<Mutex<Vec<Seen>>> = Default::default();
    let sink = seen.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(
            move |method: axum::http::Method,
                  uri: axum::http::Uri,
                  headers: axum::http::HeaderMap,
                  body: String| {
                let sink = sink.clone();
                async move {
                    let header = |name: &str| {
                        headers
                            .get(name)
                            .map(|v| v.to_str().unwrap().to_owned())
                            .unwrap_or_default()
                    };
                    sink.lock().unwrap().push((
                        method.to_string(),
                        uri.to_string(),
                        header("authorization"),
                        header("content-type"),
                        body,
                    ));
                    if uri.path().starts_with("/github") {
                        assert_eq!(header("accept"), "application/vnd.github+json");
                        assert_eq!(header("x-github-api-version"), "2022-11-28");
                    }
                    upstream(method, uri).await
                }
            },
        );
        axum::serve(listener, app).await.unwrap()
    });
    let fx = Fixture::new(
        83,
        Endpoints(BTreeMap::from([
            ("github_api".into(), format!("{origin}/github")),
            ("slack_api".into(), format!("{origin}/slack")),
        ])),
        all_tools(),
    )
    .await;
    let github = seed(&fx, "github", "pat", &[("token", "github_pat_fixture")]).await;
    // A chat Slack app's bot token also serves its tools.
    let slack = seed(
        &fx,
        "slack",
        "slack_app",
        &[("access_token", "xoxb-fixture"), ("slack_team_id", "T1")],
    )
    .await;

    let tools = fx.chat.tools.clone().unwrap();
    assert_eq!(tools.provider_tools(github).await.unwrap().len(), 78);
    assert_eq!(tools.provider_tools(slack).await.unwrap().len(), 53);
    // Each connection is named after its provider, which is then its tools' prefix.
    assert_eq!(
        fx.use_tools(
            github,
            &[
                "github_get_issue",
                "github_search_issues",
                "github_add_issue_comment",
                "github_get_file_content",
                "github_delete_file",
                "github_get_repo"
            ]
        )
        .await,
        "github"
    );
    assert_eq!(
        fx.use_tools(
            slack,
            &[
                "slack_post_message",
                "slack_get_conversation_history",
                "slack_get_upload_url",
                "slack_pin_message",
                "slack_search_messages"
            ]
        )
        .await,
        "slack"
    );
    let listed = fx.tools().await;
    let issue = listed
        .iter()
        .find(|t| t["name"] == "github.github_get_issue")
        .unwrap();
    assert_eq!(issue["annotations"]["readOnly"], true);
    let schema: Value = serde_json::from_str(issue["inputSchemaJson"].as_str().unwrap()).unwrap();
    assert_eq!(schema["required"], json!(["owner", "repo", "issue_number"]));

    let gh = "Bearer github_pat_fixture".to_owned();
    let json_type = "application/json".to_owned();
    let last = || seen.lock().unwrap().last().cloned().unwrap();

    let read = fx
        .output(
            "github.github_get_issue",
            json!({"owner":"octo","repo":"app","issue_number":7}),
        )
        .await;
    assert_eq!(read["data"]["number"], 7);
    assert_eq!(read["rate_limit"]["remaining"], "4999");
    assert_eq!(
        last(),
        (
            "GET".into(),
            "/github/repos/octo/app/issues/7".into(),
            gh.clone(),
            String::new(),
            String::new()
        )
    );

    fx.output(
        "github.github_search_issues",
        json!({"query":"is:open repo:octo/app","max_results":50,"sort":"updated"}),
    )
    .await;
    assert_eq!(
        last().1,
        "/github/search/issues?per_page=50&q=is%3Aopen+repo%3Aocto%2Fapp&sort=updated",
        "query becomes q, max_results per_page and extra fields pass through"
    );

    let comment = fx
        .output(
            "github.github_add_issue_comment",
            json!({"owner":"octo","repo":"app","issue_number":7,"body":"On it"}),
        )
        .await;
    assert_eq!(
        (comment["status"].clone(), comment["data"]["id"].clone()),
        (json!(201), json!(99))
    );
    let (method, path, auth, content, body) = last();
    assert_eq!(
        (method.as_str(), path.as_str(), auth, content),
        (
            "POST",
            "/github/repos/octo/app/issues/7/comments",
            gh.clone(),
            json_type.clone()
        )
    );
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({"body":"On it"})
    );

    fx.output(
        "github.github_get_file_content",
        json!({"owner":"octo","repo":"app","path":"src/lib.rs","ref":"main"}),
    )
    .await;
    assert_eq!(
        last().1,
        "/github/repos/octo/app/contents/src/lib.rs?ref=main"
    );
    let escape = fx
        .invoke(
            "github.github_get_file_content",
            json!({"owner":"octo","repo":"app","path":"../../../user"}),
        )
        .await;
    assert!(escape.get("error").is_some(), "{escape}");

    fx.output(
        "github.github_delete_file",
        json!({"owner":"octo","repo":"app","path":"docs/old.md","message":"Remove","sha":"b1"}),
    )
    .await;
    let (method, path, _, _, body) = last();
    assert_eq!(
        (method.as_str(), path.as_str()),
        ("DELETE", "/github/repos/octo/app/contents/docs/old.md")
    );
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({"message":"Remove","sha":"b1"})
    );

    let missing = fx
        .invoke(
            "github.github_get_repo",
            json!({"owner":"octo","repo":"gone"}),
        )
        .await;
    let error = missing["error"].to_string();
    assert!(
        error.contains("404") && error.contains("Not Found"),
        "{missing}"
    );

    let xoxb = "Bearer xoxb-fixture".to_owned();
    let posted = fx
        .output(
            "slack.slack_post_message",
            json!({"channel_id":"C1","text":"Deployed","thread_ts":"1.1"}),
        )
        .await;
    assert_eq!(posted["ts"], "1.2");
    let (method, path, auth, content, body) = last();
    assert_eq!(
        (method.as_str(), path.as_str(), auth, content),
        ("POST", "/slack/chat.postMessage", xoxb.clone(), json_type)
    );
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({"channel":"C1","text":"Deployed","thread_ts":"1.1"})
    );

    fx.output(
        "slack.slack_get_conversation_history",
        json!({"channel_id":"C1","limit":5}),
    )
    .await;
    let (method, path, auth, ..) = last();
    assert_eq!(
        (method.as_str(), path.as_str(), auth),
        (
            "GET",
            "/slack/conversations.history?channel=C1&limit=5",
            xoxb.clone()
        )
    );

    fx.output(
        "slack.slack_get_upload_url",
        json!({"filename":"a.txt","length":3}),
    )
    .await;
    let (_, _, _, content, body) = last();
    assert_eq!(
        (content.as_str(), body.as_str()),
        (
            "application/x-www-form-urlencoded",
            "filename=a.txt&length=3"
        )
    );

    let refused = fx
        .invoke(
            "slack.slack_pin_message",
            json!({"channel_id":"C1","timestamp":"1.2"}),
        )
        .await;
    let error = refused["error"].to_string();
    assert!(
        error.contains("missing_scope") && error.contains("pins:write"),
        "{refused}"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&last().4).unwrap(),
        json!({"channel":"C1","timestamp":"1.2"})
    );

    let calls = seen.lock().unwrap().len();
    let user_only = fx
        .invoke("slack.slack_search_messages", json!({"query":"deploy"}))
        .await;
    assert!(
        user_only["error"].to_string().contains("user token"),
        "{user_only}"
    );
    assert_eq!(
        seen.lock().unwrap().len(),
        calls,
        "user-token methods never reach Slack with the bot token"
    );

    server.abort();
    fx.close().await;
}
