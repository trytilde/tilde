//! An owner chooses which agents may use their personal connection: every agent whose
//! `tools.personal` covers the provider (the default, which every earlier connection keeps), or
//! only the agents they list. The invocation catalog and each call check both the grant and the
//! owner's choice, a change shows from the agent's next invocation, and every call records the
//! personal connection it ran on.
mod common;
use common::invocation::Fixture;
use secrecy::SecretString;
use serde_json::json;
use std::collections::BTreeMap;
use tilde::connections::catalog::Endpoints;
use tilde::encryption::{Encryption, SecretBinding};
use tilde::proto::tilde::types::v1 as types;
use uuid::Uuid;

const TOOL: &str = "user.tavily.sam_example_com.search";

async fn sql(fx: &Fixture, query: &str, params: &[&(dyn tokio_postgres::types::ToSql + Sync)]) {
    fx.within(async {
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .execute(query, params)
            .await
            .unwrap()
    })
    .await;
}

#[tokio::test]
async fn an_owner_decides_which_permitted_agents_use_their_connection() {
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_origin = format!("http://{}", upstream.local_addr().unwrap());
    let upstream_server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(async || axum::Json(json!({"results":[]})));
        axum::serve(upstream, app).await.unwrap()
    });
    let mut fx = Fixture::new(
        54,
        Endpoints(BTreeMap::from([("tavily_api".into(), upstream_origin)])),
        types::Capabilities {
            tools_invoke: types::TargetPermission {
                mode: types::TargetSelection::All.into(),
                ..Default::default()
            }
            .into(),
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
    fx.output("user.connect_tool", json!({"provider":"tavily"}))
        .await;
    let user = Uuid::parse_str(&fx.user_id).unwrap();
    let connection: Uuid = fx
        .within(async {
            fx.db
                .pool
                .get()
                .await
                .unwrap()
                .query_one(
                    "SELECT id FROM connections WHERE owner_user_id=$1",
                    &[&user],
                )
                .await
                .unwrap()
                .get(0)
        })
        .await;
    // The user completes the hosted setup page; seeded here as the broker would leave it.
    let sealed = Encryption::initialize(&fx.db.pool, common::seed(54))
        .await
        .unwrap()
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
    sql(&fx,
        "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,'api_key',$2)",
        &[&connection, &sealed],
    )
    .await;
    sql(
        &fx,
        "UPDATE connections SET status='ready',name='sam@example.com' WHERE id=$1",
        &[&connection],
    )
    .await;
    let listed = async |fx: &Fixture| fx.tools().await.iter().any(|t| t["name"] == TOOL);

    // A new connection lets every permitted agent use it, and the call records whose it was.
    assert!(listed(&fx).await);
    fx.output(TOOL, json!({"query":"mine"})).await;
    let recorded: Option<Uuid> = fx
        .within(async {
            fx.db
                .pool
                .get()
                .await
                .unwrap()
                .query_one(
                    "SELECT personal_connection_id FROM chat_tool_calls WHERE name=$1",
                    &[&TOOL],
                )
                .await
                .unwrap()
                .get(0)
        })
        .await;
    assert_eq!(recorded, Some(connection));

    // Listing no agent withdraws it: refused at once, gone from the next invocation's catalog.
    sql(
        &fx,
        "UPDATE connections SET all_permitted_agents=false WHERE id=$1",
        &[&connection],
    )
    .await;
    assert!(
        fx.invoke(TOOL, json!({"query":"mine"}))
            .await
            .get("error")
            .is_some()
    );
    fx.next_invocation().await;
    assert!(!listed(&fx).await);

    // Listing this agent gives it back.
    sql(
        &fx,
        "INSERT INTO personal_connection_agents(connection_id,agent_id) VALUES($1,$2)",
        &[&connection, &fx.agent],
    )
    .await;
    fx.next_invocation().await;
    assert!(listed(&fx).await);

    // The owner's choice never widens the agent's grant.
    let narrowed = json!({"tools.invoke":{"mode":"all"}});
    sql(
        &fx,
        "UPDATE agents SET capabilities=$2 WHERE id=$1",
        &[&fx.agent, &narrowed],
    )
    .await;
    fx.next_invocation().await;
    assert!(!listed(&fx).await);
    upstream_server.abort();
    fx.close().await;
}
