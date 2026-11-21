//! An application's user connects their own accounts through the native provider: the agent's
//! chat key and the asserted identity reach the ChatService connection methods at the agent
//! ingress, which start Tilde's hosted setup for a provider the agent may use personally, owned
//! by that identity's chat user, and let only that user list, reconnect and remove it.
mod common;
use base64::Engine;
use common::invocation::Fixture;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::sync::Arc;
use tilde::proto::tilde::types::v1 as types;
use uuid::Uuid;

#[tokio::test]
async fn an_identity_connects_lists_reconnects_and_removes_only_its_own_accounts() {
    let fx = Fixture::new(
        53,
        Default::default(),
        types::Capabilities {
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
    let encryption = Arc::new(
        tilde::encryption::Encryption::initialize(&fx.db.pool, common::seed(53))
            .await
            .unwrap(),
    );
    let deployments = tilde::deployment::Deployments::new(
        fx.db.pool.clone(),
        encryption,
        fx.agents.clone(),
        fx.connections.clone(),
    );
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    let router = tilde::deployment::public::router(deployments, fx.chat.clone());
    let server = tokio::spawn(async move { axum::serve(socket, router).await.unwrap() });
    let agent = fx.agent;
    let key = fx
        .within(async { fx.chat.tilde_chat_key(agent, false).await.unwrap() })
        .await;
    // A public agent admits every identity the application vouches for.
    fx.within(async {
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE connection_agents SET access_mode='public' WHERE agent_id=$1",
                &[&agent],
            )
            .await
            .unwrap()
    })
    .await;
    let call = async |identity: &str, method: &str, body: Value| -> (u16, Value) {
        let response = reqwest::Client::new()
            .post(format!(
                "{origin}/agents/{agent}/tilde.provider.tilde.v1.ChatService/{method}"
            ))
            .bearer_auth(key.expose_secret())
            .header(
                "x-tilde-identity",
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(identity),
            )
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap_or(Value::Null))
    };

    let (status, providers) = call("ann", "ListConnectableProviders", json!({})).await;
    assert_eq!(status, 200, "{providers}");
    let ids: Vec<&str> = providers["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["tavily"],
        "only providers the agent may use personally"
    );
    assert_eq!(
        call("ann", "StartConnection", json!({"provider": "github"}))
            .await
            .0,
        400
    );

    let (status, started) = call("ann", "StartConnection", json!({"provider": "tavily"})).await;
    assert_eq!(status, 200, "{started}");
    let setup = &started["setup"];
    let connection = setup["connectionId"].as_str().unwrap().to_owned();
    assert!(
        setup["setupUrl"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:1/"),
        "Tilde's hosted setup page: {setup}"
    );
    // Owned by ann's chat user: the identity the agent's chats know her by.
    let owner: Uuid = fx
        .within(async {
            fx.db
                .pool
                .get()
                .await
                .unwrap()
                .query_one(
                    "SELECT c.owner_user_id FROM connections c WHERE c.id=$1",
                    &[&Uuid::parse_str(&connection).unwrap()],
                )
                .await
                .unwrap()
                .get(0)
        })
        .await;
    let ann: Uuid = Uuid::parse_str(
        call("ann", "GetIdentity", json!({})).await.1["user"]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(owner, ann);

    let listed = call("ann", "ListConnections", json!({})).await.1;
    assert_eq!(listed["connections"][0]["id"], connection);
    assert_eq!(listed["connections"][0]["status"], "requires_action");
    // Another identity sees none of it and cannot touch it.
    assert_eq!(call("bob", "ListConnections", json!({})).await.1, json!({}));
    let theirs = json!({"connectionId": connection});
    assert_eq!(
        call("bob", "ReconnectConnection", theirs.clone()).await.0,
        404
    );
    assert_eq!(call("bob", "DeleteConnection", theirs.clone()).await.0, 404);

    let (status, again) = call("ann", "ReconnectConnection", theirs.clone()).await;
    assert_eq!(status, 200, "{again}");
    assert_eq!(again["setup"]["connectionId"], connection);
    assert_eq!(call("ann", "DeleteConnection", theirs).await.0, 200);
    assert_eq!(call("ann", "ListConnections", json!({})).await.1, json!({}));
    server.abort();
}
