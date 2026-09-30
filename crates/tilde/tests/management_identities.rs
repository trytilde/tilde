mod common;
use common::{Database, seed};
use serde_json::{Value, json};
use std::sync::Arc;
use tilde::{agent::Agents, chat::Chat, connections::service::Connections, encryption::Encryption};
use uuid::Uuid;

struct Fixture {
    db: Database,
    connections: Connections,
    origin: String,
    server: tokio::task::JoinHandle<()>,
}
async fn serve(router: axum::Router) -> (String, tokio::task::JoinHandle<()>) {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    (
        origin,
        tokio::spawn(async move { axum::serve(socket, router).await.unwrap() }),
    )
}
impl Fixture {
    async fn new() -> Self {
        let db = Database::new().await;
        let encryption = Arc::new(Encryption::initialize(&db.pool, seed(35)).await.unwrap());
        let agents = Agents::new(db.pool.clone(), encryption.clone());
        let connections = Connections::new(
            db.pool.clone(),
            encryption.clone(),
            "http://127.0.0.1".into(),
            "http://127.0.0.1".into(),
        )
        .unwrap();
        connections.seed().await.unwrap();
        let chat = Chat::new(
            db.pool.clone(),
            encryption.clone(),
            "http://127.0.0.1".into(),
        )
        .with_connections(connections.clone());
        let deployments = tilde::deployment::Deployments::new(
            db.pool.clone(),
            encryption.clone(),
            agents.clone(),
            connections.clone(),
        );
        let (origin, server) = serve(
            tilde::iam::listeners::management_router(agents, chat.clone(), connections.clone())
                .merge(tilde::deployment::public::router(deployments, chat)),
        )
        .await;
        Self {
            db,
            connections,
            origin,
            server,
        }
    }
    async fn call(&self, service: &str, method: &str, body: Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!(
                "{}/tilde.management.v1.{service}/{method}",
                self.origin
            ))
            .json(&body)
            .send()
            .await
            .unwrap()
    }
    async fn ok(&self, service: &str, method: &str, body: Value) -> Value {
        let r = self.call(service, method, body).await;
        assert!(
            r.status().is_success(),
            "RPC {service}/{method}: {}",
            r.status()
        );
        r.json().await.unwrap()
    }
    async fn chat(&self, agent: Uuid, method: &str, body: Value) -> Value {
        let issued = self
            .ok(
                "DeploymentService",
                "IssueIngressToken",
                json!({"agentId":agent}),
            )
            .await;
        let response = reqwest::Client::new()
            .post(format!(
                "{}/agents/{agent}/tilde.provider.tilde.v1.ChatService/{method}",
                self.origin
            ))
            .bearer_auth(issued["token"].as_str().unwrap())
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "Provider {method}: {}",
            response.status()
        );
        response.json().await.unwrap()
    }
    async fn close(self) {
        self.server.abort();
        self.db.close().await;
    }
}

#[tokio::test]
async fn roots_are_optional_atomic_and_cannot_steal_another_association() {
    let f = Fixture::new().await;
    let identity = f
        .ok(
            "IdentitiesService",
            "CreateIdentity",
            json!({"identityType":"IDENTITY_TYPE_USERNAME","value":"app-user-42"}),
        )
        .await["identity"]
        .clone();
    let id = identity["id"].as_str().unwrap();
    assert!(identity.get("rootIdentityId").is_none());
    assert!(identity.get("attestedAt").is_none());
    assert_eq!(identity["providerId"], "tilde");
    let root_a = Uuid::new_v4();
    let root_b = Uuid::new_v4();
    f.ok(
        "IdentitiesService",
        "CreateRootIdentity",
        json!({"id":root_a}),
    )
    .await;
    f.ok(
        "IdentitiesService",
        "CreateRootIdentity",
        json!({"id":root_b}),
    )
    .await;
    let a = f.call(
        "IdentitiesService",
        "LinkIdentity",
        json!({"identityId":id,"rootIdentityId":root_a}),
    );
    let b = f.call(
        "IdentitiesService",
        "LinkIdentity",
        json!({"identityId":id,"rootIdentityId":root_b}),
    );
    let (a, b) = tokio::join!(a, b);
    assert_ne!(a.status().is_success(), b.status().is_success());
    let result = f
        .ok("IdentitiesService", "GetIdentity", json!({"id":id}))
        .await;
    let linked = result["identity"]["rootIdentityId"].as_str().unwrap();
    let wrong = if linked == root_a.to_string() {
        root_b
    } else {
        root_a
    };
    assert_eq!(
        f.call(
            "IdentitiesService",
            "UnlinkIdentity",
            json!({"identityId":id,"rootIdentityId":wrong})
        )
        .await
        .status(),
        400
    );
    let batch = Uuid::new_v4();
    assert!(
        !f.call(
            "IdentitiesService",
            "CreateRootIdentity",
            json!({"id":batch,"identityIds":[id]})
        )
        .await
        .status()
        .is_success()
    );
    assert_eq!(
        f.call("IdentitiesService", "GetRootIdentity", json!({"id":batch}))
            .await
            .status(),
        404
    );
    let unlinked = f
        .ok(
            "IdentitiesService",
            "UnlinkIdentity",
            json!({"identityId":id,"rootIdentityId":linked}),
        )
        .await;
    assert!(unlinked["identity"].get("rootIdentityId").is_none());
    assert_eq!(unlinked["identity"]["id"], id);
    let created=f.ok("IdentitiesService","CreateIdentity",json!({"identityType":"IDENTITY_TYPE_USERNAME","value":"app-user-42","createRoot":true,"skipVerification":true})).await;
    let root = created["identity"]["rootIdentityId"].clone();
    assert!(root.is_string());
    assert!(created["identity"]["attestedAt"].is_string());
    assert!(created["identity"].get("verifiedAt").is_none());
    let retry=f.ok("IdentitiesService","CreateIdentity",json!({"identityType":"IDENTITY_TYPE_USERNAME","value":"app-user-42","createRoot":true})).await;
    assert_eq!(retry["identity"]["rootIdentityId"], root);
    let invalid=f.call("IdentitiesService","CreateIdentity",json!({"identityType":"IDENTITY_TYPE_USERNAME","value":"invalid","createRoot":true,"rootIdentityId":root})).await;
    assert_eq!(invalid.status(), 400);
    let members = f
        .ok(
            "IdentitiesService",
            "ListIdentities",
            json!({"rootIdentityId":root}),
        )
        .await;
    assert_eq!(members["identities"].as_array().unwrap().len(), 1);
    f.close().await;
}

#[tokio::test]
async fn channel_identity_resolution_is_agent_scoped_and_linking_preserves_participants() {
    use tilde::connections::model::{Assignment, Capability};
    let f = Fixture::new().await;
    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();
    for agent in [agent_a, agent_b] {
        f.ok(
            "AgentService",
            "CreateAgent",
            json!({"id":agent,"name":"Identity fixture"}),
        )
        .await;
    }
    let mut channels = Vec::new();
    for agent in [agent_a, agent_b] {
        let id = Uuid::new_v4();
        f.connections
            .start(
                id,
                "work",
                "slack",
                "slack_app",
                &[Assignment {
                    capability: Capability::Channel,
                    agent_id: agent,
                    alias: None,
                }],
            )
            .await
            .unwrap();
        // The fixture exercises identity/address routing, without contacting Slack for credentials.
        tilde::connections::db::connection_ready_execute(
            &f.db.pool.get().await.unwrap(),
            id,
            Some("Workspace"),
            None,
        )
        .await
        .unwrap();
        channels.push(id);
    }
    let mut identities = Vec::new();
    for (agent, connection) in [agent_a, agent_b].into_iter().zip(channels.iter()) {
        let identity=f.ok("IdentitiesService","CreateIdentity",json!({"agentId":agent,"providerId":"slack/work","identityType":"IDENTITY_TYPE_USERNAME","value":"U12345","skipVerification":true})).await["identity"].clone();
        assert_eq!(identity["connectionId"], connection.to_string());
        assert!(identity.get("verifiedAt").is_none());
        assert!(identity["attestedAt"].is_string());
        identities.push(identity["id"].as_str().unwrap().to_owned());
    }
    assert_ne!(identities[0], identities[1]);
    // Attestation and root grouping don't grant channel access.
    let list = f
        .ok(
            "AgentAccessService",
            "ListChannelIdentities",
            json!({"agentId":agent_a}),
        )
        .await;
    assert!(
        list["identities"][0]["allowed"]
            .as_bool()
            .is_none_or(|allowed| !allowed)
    );
    f.ok("AgentAccessService","SetIdentityAccess",json!({"agentId":agent_a,"connectionId":channels[0],"identityId":identities[0],"allowed":true})).await;
    let allowed = tilde::chat::access::db::allowed_one(
        &f.db.pool.get().await.unwrap(),
        agent_a,
        Uuid::parse_str(&identities[0]).unwrap(),
    )
    .await
    .unwrap();
    assert!(allowed.allowed);
    let thread=f.chat(agent_a,"CreateThread",json!({"title":"Specific address","primaryAgentId":agent_a,"participants":[{"agentId":agent_a},{"userId":identities[0]},{"userId":identities[1]}]})).await["thread"].clone();
    let root = Uuid::new_v4();
    f.ok(
        "IdentitiesService",
        "CreateRootIdentity",
        json!({"id":root,"identityIds":identities}),
    )
    .await;
    let after = f
        .chat(agent_a, "GetThread", json!({"id":thread["id"]}))
        .await["thread"]
        .clone();
    assert_eq!(after["participants"], thread["participants"]);
    // A second ready channel with the same name cannot be assigned to the same agent.
    let duplicate = Uuid::new_v4();
    f.connections
        .start(duplicate, "work", "slack", "slack_app", &[])
        .await
        .unwrap();
    tilde::connections::db::connection_ready_execute(
        &f.db.pool.get().await.unwrap(),
        duplicate,
        Some("Other workspace"),
        None,
    )
    .await
    .unwrap();
    assert!(
        f.connections
            .assign(
                duplicate,
                &Assignment {
                    capability: Capability::Channel,
                    agent_id: agent_a,
                    alias: None
                }
            )
            .await
            .is_err()
    );
    let denied=f.call("IdentitiesService","CreateIdentity",json!({"agentId":agent_a,"providerId":"slack/missing","identityType":"IDENTITY_TYPE_USERNAME","value":"U12345","createRoot":true})).await;
    assert_eq!(denied.status(), 404);
    let pending_a = Uuid::new_v4();
    let pending_b = Uuid::new_v4();
    for pending in [pending_a, pending_b] {
        f.connections
            .start(
                pending,
                "racing",
                "slack",
                "slack_app",
                &[Assignment {
                    capability: Capability::Channel,
                    agent_id: agent_a,
                    alias: None,
                }],
            )
            .await
            .unwrap();
    }
    let client_a = f.db.pool.get().await.unwrap();
    let client_b = f.db.pool.get().await.unwrap();
    let (ready_a, ready_b) = tokio::join!(
        tilde::connections::db::connection_ready_execute(&client_a, pending_a, None, None),
        tilde::connections::db::connection_ready_execute(&client_b, pending_b, None, None)
    );
    assert_ne!(
        ready_a.is_ok(),
        ready_b.is_ok(),
        "Only one conflicting route may become ready"
    );
    let error = ready_a.err().or_else(|| ready_b.err()).unwrap();
    assert!(
        error.is_unique_violation(),
        "Route races must fail on uniqueness, not deadlock"
    );
    drop(client_a);
    drop(client_b);
    f.close().await;
}

/// Application-key ingress through the agent's own Tilde connection, exercised end to end:
/// AgentService -> ConnectionsService/AgentAccessService -> TildeChatProviderService ->
/// provider ingress -> the shared channel access rule.
#[tokio::test]
async fn tilde_connection_is_created_with_the_agent_and_gates_application_keys() {
    use base64::Engine;
    let f = Fixture::new().await;
    async fn native(f: &Fixture, agent: Uuid, key: &str, identity: &str) -> u16 {
        reqwest::Client::new()
            .post(format!(
                "{}/agents/{agent}/tilde.provider.tilde.v1.ChatService/GetIdentity",
                f.origin
            ))
            .bearer_auth(key)
            .header(
                "x-tilde-identity",
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(identity),
            )
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    }
    let agent = Uuid::new_v4();
    f.ok(
        "AgentService",
        "CreateAgent",
        json!({"id":agent,"name":"Embedded"}),
    )
    .await;
    // Created with the agent, private, and visible to whoever holds the agent.
    let routes = f
        .ok(
            "AgentAccessService",
            "ListChannelAccess",
            json!({"agentId":agent}),
        )
        .await["routes"]
        .clone();
    assert_eq!(routes.as_array().unwrap().len(), 1, "{routes}");
    let route = &routes[0];
    assert_eq!(route["providerId"], "tilde");
    assert_eq!(route["accountName"], "Embedded");
    assert_eq!(route["mode"], "CHANNEL_ACCESS_MODE_PRIVATE");
    assert_eq!(route["iconUrl"], "/tilde-mark.svg");
    assert_eq!(route["identitiesAttested"], true);
    // proto3 JSON omits false.
    assert_ne!(route["verificationSupported"], true);
    let connection = route["connectionId"].as_str().unwrap().to_owned();
    let listed = f
        .ok(
            "ConnectionsService",
            "ListConnections",
            json!({"agentId":agent,"capability":"CAPABILITY_CHANNEL"}),
        )
        .await;
    assert_eq!(listed["connections"][0]["id"], connection, "{listed}");
    assert_eq!(listed["connections"][0]["status"], "ready");
    let key = f
        .ok(
            "TildeChatProviderService",
            "GetCredentials",
            json!({"agentId":agent}),
        )
        .await["apiKey"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(key.starts_with("tilde_chat_"));
    // Unseen identities are refused but recorded and attested, so an editor can allow them.
    assert_eq!(native(&f, agent, &key, "alice").await, 403);
    assert_eq!(native(&f, agent, "tilde_chat_wrong", "alice").await, 403);
    let identities = f
        .ok(
            "AgentAccessService",
            "ListChannelIdentities",
            json!({"agentId":agent,"connectionId":connection}),
        )
        .await["identities"]
        .clone();
    let alice = identities
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["value"] == "alice")
        .unwrap_or_else(|| panic!("{identities}"));
    assert_eq!(alice["identityType"], "IDENTITY_TYPE_USERNAME");
    assert!(alice["attestedAt"].is_string() && alice.get("verifiedAt").is_none());
    assert_ne!(alice["allowed"], true);
    f.ok(
        "AgentAccessService",
        "SetIdentityAccess",
        json!({"agentId":agent,"connectionId":connection,"identityId":alice["id"],"allowed":true}),
    )
    .await;
    assert_eq!(native(&f, agent, &key, "alice").await, 200);
    assert_eq!(native(&f, agent, &key, "bob").await, 403);
    // Added ahead of first contact: nothing to deliver, allowed immediately.
    let added = f.ok("AgentAccessService","RequestIdentityVerification",json!({"id":Uuid::new_v4(),"agentId":agent,"connectionId":connection,"identityType":"IDENTITY_TYPE_USERNAME","value":"carol"})).await;
    assert_eq!(added["status"], "approved");
    assert_eq!(native(&f, agent, &key, "carol").await, 200);
    f.ok(
        "AgentAccessService",
        "SetChannelAccess",
        json!({"agentId":agent,"connectionId":connection,"mode":"CHANNEL_ACCESS_MODE_DISABLED"}),
    )
    .await;
    assert_eq!(native(&f, agent, &key, "alice").await, 403);
    f.ok(
        "AgentAccessService",
        "SetChannelAccess",
        json!({"agentId":agent,"connectionId":connection,"mode":"CHANNEL_ACCESS_MODE_PUBLIC"}),
    )
    .await;
    assert_eq!(native(&f, agent, &key, "bob").await, 200);
    // Rotation replaces the connection value; the old key dies with every cached copy.
    let rotated = f
        .ok(
            "TildeChatProviderService",
            "RotateCredentials",
            json!({"agentId":agent}),
        )
        .await["apiKey"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(rotated, key);
    assert_eq!(native(&f, agent, &key, "alice").await, 403);
    assert_eq!(native(&f, agent, &rotated, "alice").await, 200);
    assert_eq!(
        f.ok(
            "TildeChatProviderService",
            "GetCredentials",
            json!({"agentId":agent})
        )
        .await["apiKey"],
        rotated
    );
    // The system owns the connection: nobody starts, disconnects or unassigns one.
    for (service, method, body) in [
        (
            "ConnectionsService",
            "StartConnection",
            json!({"name":"mine","providerId":"tilde","typeId":"application"}),
        ),
        ("ConnectionsService", "Disconnect", json!({"id":connection})),
        (
            "ConnectionsService",
            "UnassignCapability",
            json!({"connectionId":connection,"assignment":{"capability":"CAPABILITY_CHANNEL","agentId":agent}}),
        ),
    ] {
        let response = f.call(service, method, body).await;
        assert_eq!(response.status(), 400, "{service}/{method}");
    }
    assert!(
        f.connections
            .get(Uuid::parse_str(&connection).unwrap())
            .await
            .is_ok()
    );
    // Deleting the agent removes its Tilde connection and asserted identities.
    f.ok("AgentService", "PauseAgent", json!({"id":agent}))
        .await;
    f.ok("AgentService", "DeleteAgent", json!({"id":agent}))
        .await;
    assert!(
        f.connections
            .get(Uuid::parse_str(&connection).unwrap())
            .await
            .is_err()
    );
    assert_eq!(native(&f, agent, &rotated, "alice").await, 403);
    f.close().await;
}
