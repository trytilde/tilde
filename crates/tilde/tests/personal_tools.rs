//! Personal tool federation across IAM, connection setup, encrypted credentials and the
//! invocation catalog: a user's own account is usable only by an agent granted that provider,
//! only while that user is the one being served, and never through tools an agent holds for everyone.
mod common;
use common::invocation::Fixture;
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::proto::tilde::types::v1 as types;
use tilde::{
    chat as application,
    connections::catalog::Endpoints,
    encryption::{Encryption, SecretBinding},
};
use uuid::Uuid;

#[tokio::test]
async fn a_users_own_connection_serves_only_that_users_conversations() {
    let seen: Arc<Mutex<Vec<String>>> = Default::default();
    let sink = seen.clone();
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_origin = format!("http://{}", upstream.local_addr().unwrap());
    let upstream_server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(move |headers: axum::http::HeaderMap| {
            let sink = sink.clone();
            async move {
                sink.lock()
                    .unwrap()
                    .push(headers["authorization"].to_str().unwrap().to_owned());
                axum::Json(json!({"results":[]}))
            }
        });
        axum::serve(upstream, app).await.unwrap()
    });
    let all = || types::TargetPermission {
        mode: types::TargetSelection::All.into(),
        ..Default::default()
    };
    let fx = Fixture::new(
        51,
        Endpoints(BTreeMap::from([("tavily_api".into(), upstream_origin)])),
        types::Capabilities {
            tools_invoke: all().into(),
            tools_personal: types::TargetPermission {
                mode: types::TargetSelection::Selected.into(),
                ids: vec!["tavily".into()],
                ..Default::default()
            }
            .into(),
            ..Default::default()
        },
    )
    .await;
    let names = async || -> Vec<String> {
        fx.tools()
            .await
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect()
    };
    let before = fx.tools().await;
    let connect = before
        .iter()
        .find(|t| t["name"] == "user.connect_tool")
        .expect("offered to a granted agent");
    let schema: Value = serde_json::from_str(connect["inputSchemaJson"].as_str().unwrap()).unwrap();
    assert_eq!(
        schema["properties"]["provider"]["enum"],
        json!(["tavily"]),
        "only granted providers"
    );
    assert!(
        before
            .iter()
            .all(|t| !t["name"].as_str().unwrap().starts_with("user.tavily"))
    );

    let link = fx
        .output("user.connect_tool", json!({"provider":"tavily"}))
        .await;
    assert!(
        link["setup_url"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:1/"),
        "{link}"
    );
    let pg = fx.db.pool.get().await.unwrap();
    let user = Uuid::parse_str(&fx.user_id).unwrap();
    let row = pg
        .query_one(
            "SELECT id,status FROM connections WHERE owner_user_id=$1",
            &[&user],
        )
        .await
        .unwrap();
    let (connection, status): (Uuid, String) = (row.get(0), row.get(1));
    assert_eq!(status, "requires_action");

    // The user completes the hosted setup page; seeded here as the broker would leave it.
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(51))
        .await
        .unwrap();
    let sealed = crypto
        .seal(
            SecretBinding {
                resource_kind: "connection",
                resource_id: connection,
                name: "api_key",
            },
            &SecretString::from("tvly-personal"),
        )
        .unwrap()
        .into_bytes();
    pg.execute("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,'api_key',$2)", &[&connection, &sealed]).await.unwrap();
    pg.execute(
        "UPDATE connections SET status='ready',name='sam@example.com' WHERE id=$1",
        &[&connection],
    )
    .await
    .unwrap();

    assert!(
        names()
            .await
            .contains(&"user.tavily.sam_example_com.search".to_owned()),
        "{:?}",
        names().await
    );
    fx.output(
        "user.tavily.sam_example_com.search",
        json!({"query":"mine"}),
    )
    .await;
    assert_eq!(*seen.lock().unwrap(), ["Bearer tvly-personal"]);

    // Management cannot give an agent a person's account for every user it serves.
    assert!(
        fx.chat
            .tools
            .as_ref()
            .unwrap()
            .add_source(
                fx.agent,
                tilde::tools::Target::Connection(connection),
                &["search".into()],
            )
            .await
            .is_err()
    );

    // With a second person present the invocation no longer acts for one known user.
    let other = fx.chat.create_user("Other").await.unwrap();
    fx.chat
        .add_participant(application::AddParticipant {
            thread_id: fx.thread.to_string(),
            participant: Some(types::ParticipantRef {
                user_id: Some(other.id),
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    assert!(
        names().await.iter().all(|n| !n.starts_with("user.")),
        "{:?}",
        names().await
    );
    assert!(
        fx.invoke(
            "user.tavily.sam_example_com.search",
            json!({"query":"mine"})
        )
        .await
        .get("error")
        .is_some()
    );
    assert_eq!(seen.lock().unwrap().len(), 1);
    drop(pg);
    upstream_server.abort();
    fx.close().await;
}
