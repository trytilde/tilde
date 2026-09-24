//! Management authorization across the real router: guard, per-RPC rules, roles, SQL list
//! filters, API keys and the OIDC group sync at login.
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use tilde::{
    agent::Agents,
    chat::Chat,
    connections::service::Connections,
    encryption::Encryption,
    iam::oidc::{GroupMapping, Oidc},
};
use uuid::Uuid;

const KEY: &str = include_str!("fixtures/connection-test-key.pem");
const MODULUS: &str = "63eZzdHX1RByaXXHCNyi6RYqSxQP7QQtZqDE_OYO80sVhjGtqCkDH54yPQEBbW08EXxPdpvwtoc5rY4Ps6DO93uezxulEdS6sZljSutpysLMcgGp7t58kVJsL4JVK19sPXLRGpVcEnI65yjrPf9hVWz5TKQpo4J8EJyaD3w4K6PiYHoDVI9lTpDrT585B4HkaDRBxTqvDmu0vWQZo4MTRIxXWgTX2ZW6YoLD2O9u_v1CwsaEdkLzZ08xo4N4ktyg68vB2dbGZY6MMd5sq3xDI2yq5Ymgm0Rlw6OIT2Ebgio7EGwx-kuHs3SuP5H0D5eY0MYUKbmYOnXe8Tt36u2qhQ";

/// Signs whatever claims the test staged; the engine still runs discovery, JWKS and nonce checks.
#[derive(Clone)]
struct Provider {
    issuer: Arc<Mutex<String>>,
    claims: Arc<Mutex<Value>>,
}
async fn provider() -> (Provider, tokio::task::JoinHandle<()>) {
    use axum::{Json, extract::State, routing::get, routing::post};
    let state = Provider {
        issuer: Arc::default(),
        claims: Arc::new(Mutex::new(json!({}))),
    };
    let router = axum::Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(|State(p): State<Provider>| async move {
                let issuer = p.issuer.lock().unwrap().clone();
                Json(json!({"issuer":issuer,"authorization_endpoint":format!("{issuer}/authorize"),
                    "token_endpoint":format!("{issuer}/token"),"jwks_uri":format!("{issuer}/keys")}))
            }),
        )
        .route(
            "/keys",
            get(|| async {
                Json(json!({"keys":[{"kty":"RSA","kid":"fixture","alg":"RS256","use":"sig","n":MODULUS,"e":"AQAB"}]}))
            }),
        )
        .route(
            "/token",
            post(|State(p): State<Provider>| async move {
                let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
                header.kid = Some("fixture".into());
                let claims = p.claims.lock().unwrap().clone();
                let key = jsonwebtoken::EncodingKey::from_rsa_pem(KEY.as_bytes()).unwrap();
                Json(json!({"id_token":jsonwebtoken::encode(&header,&claims,&key).unwrap()}))
            }),
        )
        .with_state(state.clone());
    let (origin, server) = serve(router).await;
    *state.issuer.lock().unwrap() = origin;
    (state, server)
}
async fn serve(router: axum::Router) -> (String, tokio::task::JoinHandle<()>) {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    (
        origin,
        tokio::spawn(async move { axum::serve(socket, router).await.unwrap() }),
    )
}

struct Fixture {
    _db: Database,
    agents: Agents,
    provider: Provider,
    origin: String,
    servers: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for server in &self.servers {
            server.abort();
        }
    }
}
struct Session {
    token: String,
    user: String,
}
impl Fixture {
    async fn new(mapping: GroupMapping) -> Self {
        let db = Database::new().await;
        let encryption = Arc::new(Encryption::initialize(&db.pool, seed(61)).await.unwrap());
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
        let (provider, idp) = provider().await;
        let issuer = provider.issuer.lock().unwrap().clone();
        let oidc = Oidc::new(
            db.pool.clone(),
            encryption,
            issuer,
            "fixture".into(),
            SecretString::from("fixture-secret"),
            "http://127.0.0.1".into(),
            true,
        )
        .unwrap()
        .with_group_mapping(mapping);
        let viewers = tilde::telemetry::viewer::router(tilde::telemetry::viewer::Reader::new(
            db.pool.clone(),
            None,
        ))
        .merge(tilde::logs::viewer::router(
            tilde::logs::viewer::Reader::new(db.pool.clone(), None, false),
        ))
        .layer(axum::middleware::from_fn_with_state(
            oidc.clone(),
            tilde::iam::oidc::management_guard,
        ));
        let (origin, server) = serve(
            tilde::iam::listeners::management_router(agents.clone(), chat, connections, oidc)
                .merge(viewers),
        )
        .await;
        Self {
            _db: db,
            agents,
            provider,
            origin,
            servers: vec![idp, server],
        }
    }
    /// The engine's own login exchange against the staged provider claims.
    async fn login(&self, subject: &str, groups: Value) -> Session {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let http = reqwest::Client::new();
        let verifier = tilde::connections::model::random_secret();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.expose_secret().as_bytes()));
        let start: Value = http
            .post(format!("{}/auth/login", self.origin))
            .json(&json!({"challenge":challenge}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let url = reqwest::Url::parse(start["authorization_url"].as_str().unwrap()).unwrap();
        let nonce = url.query_pairs().find(|(k, _)| k == "nonce").unwrap().1;
        let now = chrono::Utc::now().timestamp();
        *self.provider.claims.lock().unwrap() = json!({
            "iss": *self.provider.issuer.lock().unwrap(), "aud": "fixture", "sub": subject,
            "nonce": nonce, "iat": now, "exp": now + 300,
            "email": format!("{subject}@example.test"), "groups": groups,
        });
        let response = http
            .post(format!("{}/auth/exchange", self.origin))
            .json(
                &json!({"code":"code","state":start["state"],"verifier":verifier.expose_secret()}),
            )
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "login {}",
            response.status()
        );
        let body: Value = response.json().await.unwrap();
        Session {
            token: body["access_token"].as_str().unwrap().into(),
            user: body["user_id"].as_str().unwrap().into(),
        }
    }
    async fn call(&self, token: &str, path: &str, body: Value) -> (u16, Value) {
        let response = reqwest::Client::new()
            .post(format!("{}/tilde.management.v1.{path}", self.origin))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap_or(Value::Null))
    }
    async fn ok(&self, token: &str, path: &str, body: Value) -> Value {
        let (status, value) = self.call(token, path, body).await;
        assert_eq!(status, 200, "{path}: {value}");
        value
    }
    async fn agent(&self, token: &str, name: &str) -> String {
        self.ok(token, "AgentService/CreateAgent", json!({"name":name}))
            .await["agent"]["id"]
            .as_str()
            .unwrap()
            .into()
    }
    /// Assign one of the agent's system roles; an empty agent id addresses every agent.
    async fn assign(&self, token: &str, agent: &str, role: &str, principal: Value) -> u16 {
        self.call(
            token,
            "IamService/AssignRole",
            json!({"roleId":role_id(agent, role),"principal":principal}),
        )
        .await
        .0
    }
    async fn revoke(&self, token: &str, agent: &str, role: &str, principal: Value) -> u16 {
        self.call(
            token,
            "IamService/RevokeRole",
            json!({"roleId":role_id(agent, role),"principal":principal}),
        )
        .await
        .0
    }
    async fn actions(&self, token: &str, agent: &str) -> Value {
        self.ok(
            token,
            "IamService/GetAccess",
            json!({"resource":{"kind":"RESOURCE_KIND_AGENT","id":agent}}),
        )
        .await["actions"]
            .clone()
    }
    async fn visible(&self, token: &str) -> Vec<String> {
        self.ok(token, "AgentService/ListAgents", json!({})).await["agents"]
            .as_array()
            .map(|agents| {
                agents
                    .iter()
                    .map(|a| a["id"].as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
}
fn user(id: &str) -> Value {
    json!({"type":"PRINCIPAL_TYPE_USER","id":id})
}
fn group(id: &str) -> Value {
    json!({"type":"PRINCIPAL_TYPE_GROUP","id":id})
}
fn role_id(agent: &str, role: &str) -> String {
    if agent.is_empty() {
        format!("agents/{role}")
    } else {
        format!("agent/{agent}/{role}")
    }
}

/// What each management RPC demands beyond authentication. `Admin` and `User` are checked
/// with an empty body, before any parsing; `Resource` RPCs name an agent in their request and
/// are driven with a real one below. `Open` RPCs need only a signed-in caller or key.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rule {
    Open,
    User,
    Admin,
    Resource,
}
const RULES: &[(&str, Rule)] = &[
    ("AgentAccessService/ListChannelAccess", Rule::Resource),
    ("AgentAccessService/ListChannelIdentities", Rule::Resource),
    (
        "AgentAccessService/RequestIdentityVerification",
        Rule::Resource,
    ),
    ("AgentAccessService/SetChannelAccess", Rule::Resource),
    ("AgentAccessService/SetIdentityAccess", Rule::Resource),
    ("AgentService/CreateAgent", Rule::Open),
    ("AgentService/DeleteAgent", Rule::Resource),
    ("AgentService/GetAgent", Rule::Resource),
    ("AgentService/ListAgents", Rule::Open),
    ("AgentService/PauseAgent", Rule::Resource),
    ("AgentService/ResumeAgent", Rule::Resource),
    ("AgentService/UpdateAgent", Rule::Resource),
    ("AgentService/UploadAgentAvatar", Rule::Resource),
    ("ApiKeysService/CreateApiKey", Rule::User),
    ("ApiKeysService/ListApiKeys", Rule::User),
    ("ApiKeysService/RevokeApiKey", Rule::User),
    ("ConnectionsService/AssignCapability", Rule::Resource),
    ("ConnectionsService/Disconnect", Rule::Open),
    ("ConnectionsService/GetConnection", Rule::Open),
    ("ConnectionsService/GetProvider", Rule::Open),
    ("ConnectionsService/ListConnections", Rule::Open),
    ("ConnectionsService/ListProviders", Rule::Open),
    ("ConnectionsService/Reconnect", Rule::Open),
    ("ConnectionsService/RegisterProvider", Rule::Admin),
    ("ConnectionsService/StartConnection", Rule::Open),
    ("ConnectionsService/UnassignCapability", Rule::Resource),
    ("DeploymentService/GetDeployment", Rule::Resource),
    ("DeploymentService/IssueDeploymentToken", Rule::Resource),
    ("DeploymentService/IssueIngressToken", Rule::Resource),
    ("DeploymentService/PromoteDeployment", Rule::Resource),
    ("DeploymentService/RegisterDeployment", Rule::Resource),
    ("DeploymentService/RetireDeployment", Rule::Resource),
    ("DeploymentService/SetDeployment", Rule::Resource),
    ("DeploymentService/SetDeploymentWeights", Rule::Resource),
    ("IamService/AddGroupMember", Rule::Admin),
    ("IamService/AssignRole", Rule::Resource),
    ("IamService/CreateGroup", Rule::Admin),
    ("IamService/DeleteGroup", Rule::Admin),
    ("IamService/GetAccess", Rule::Resource),
    ("IamService/GetCaller", Rule::Open),
    ("IamService/GetGroup", Rule::User),
    ("IamService/ListGroupMembers", Rule::User),
    ("IamService/ListGroups", Rule::User),
    ("IamService/ListRoleAssignments", Rule::Resource),
    ("IamService/ListRoles", Rule::Resource),
    ("IamService/ListUsers", Rule::User),
    ("IamService/RemoveGroupMember", Rule::Admin),
    ("IamService/RevokeRole", Rule::Resource),
    ("IamService/RevokeUserSessions", Rule::Admin),
    ("IdentitiesService/CreateIdentity", Rule::Open),
    ("IdentitiesService/CreateRootIdentity", Rule::Open),
    ("IdentitiesService/GetIdentity", Rule::Open),
    ("IdentitiesService/GetRootIdentity", Rule::Open),
    ("IdentitiesService/LinkIdentity", Rule::Open),
    ("IdentitiesService/ListIdentities", Rule::Open),
    ("IdentitiesService/ListRootIdentities", Rule::Open),
    ("IdentitiesService/UnlinkIdentity", Rule::Open),
    ("InferenceService/DeleteBudget", Rule::Resource),
    ("InferenceService/GetUsage", Rule::Resource),
    ("InferenceService/ListBudgets", Rule::Open),
    ("InferenceService/SetBudget", Rule::Resource),
    ("LogsService/GetLogMetrics", Rule::Resource),
    ("LogsService/GetLogsStatus", Rule::Resource),
    ("LogsService/ListLogs", Rule::Resource),
    ("TildeChatProviderService/GetCredentials", Rule::Resource),
    ("TildeChatProviderService/RotateCredentials", Rule::Resource),
    ("TracingService/GetObservationMetrics", Rule::Resource),
    ("TracingService/GetSession", Rule::Resource),
    ("TracingService/GetTrace", Rule::Resource),
    ("TracingService/GetTracingStatus", Rule::Resource),
    ("TracingService/ListObservations", Rule::Resource),
];
/// Every RPC the contracts declare, so a new one cannot ship without a rule above.
fn declared_rpcs() -> Vec<String> {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/tilde/management/v1"
    );
    let mut declared = Vec::new();
    for file in std::fs::read_dir(root).unwrap() {
        let text = std::fs::read_to_string(file.unwrap().path()).unwrap();
        let mut service = String::new();
        for line in text.lines().map(str::trim) {
            if let Some(rest) = line.strip_prefix("service ") {
                service = rest.split_whitespace().next().unwrap().into();
            } else if let Some(rest) = line.strip_prefix("rpc ") {
                declared.push(format!("{service}/{}", rest.split('(').next().unwrap()));
            }
        }
    }
    declared.sort();
    declared
}

/// The guard only authenticates; each handler applies its own rule. Drive every declared
/// RPC as nobody, as a plain user and as a key so a handler without a rule fails here.
#[tokio::test]
async fn every_management_rpc_applies_its_own_rule() {
    let mut listed: Vec<String> = RULES.iter().map(|(p, _)| p.to_string()).collect();
    listed.sort();
    assert_eq!(declared_rpcs(), listed);
    let f = Fixture::new(GroupMapping::default()).await;
    let admin = f.login("root", json!([])).await;
    let member = f.login("member", json!([])).await;
    let key = f
        .ok(
            &admin.token,
            "ApiKeysService/CreateApiKey",
            json!({"name":"probe","roleIds":["agents/editor"]}),
        )
        .await["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    for (path, rule) in RULES {
        assert_eq!(
            f.call("not-a-token", path, json!({})).await.0,
            401,
            "{path} without credentials"
        );
        if *rule == Rule::Admin {
            assert_eq!(
                f.call(&member.token, path, json!({})).await.0,
                403,
                "{path} by a plain user"
            );
        }
        if matches!(rule, Rule::Admin | Rule::User) {
            assert_eq!(
                f.call(&key, path, json!({})).await.0,
                403,
                "{path} by a key"
            );
        }
    }
}

#[tokio::test]
async fn roles_gate_agents_lists_sharing_capabilities_and_keys() {
    let f = Fixture::new(GroupMapping::default()).await;
    // The first user of an installation administers it; nobody after them does.
    let admin = f.login("root", json!([])).await;
    let member = f.login("member", json!([])).await;
    let session = reqwest::Client::new()
        .get(format!("{}/auth/session", f.origin))
        .bearer_auth(&member.token)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(session["admin"], false);
    let a = f.agent(&admin.token, "alpha").await;
    let b = f.agent(&admin.token, "beta").await;
    // Creation makes the agent's three roles; its creator is editor and deployer. Agents the
    // engine registers itself get the roles too, with nobody holding them.
    let registered = f
        .agents
        .create(tilde::agent::CreateAgent {
            concurrency_policy: tilde::agent::ConcurrencyPolicy::default(),
            id: Uuid::new_v4(),
            capabilities: Default::default(),
            name: "registered".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        f.ok(
            &admin.token,
            "IamService/ListRoles",
            json!({"resource":{"kind":"RESOURCE_KIND_AGENT","id":registered.id}}),
        )
        .await["roles"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let mut roles = f
        .ok(
            &admin.token,
            "IamService/ListRoles",
            json!({"resource":{"kind":"RESOURCE_KIND_AGENT","id":a}}),
        )
        .await["roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    roles.sort();
    let mut expected = ["reader", "editor", "deployer"].map(|r| role_id(&a, r));
    expected.sort();
    assert_eq!(roles, expected);

    // Invisible agents are indistinguishable from missing ones, in lists and point reads alike.
    assert!(f.visible(&member.token).await.is_empty());
    assert_eq!(
        f.call(&member.token, "AgentService/GetAgent", json!({"id":a}))
            .await
            .0,
        404
    );

    // Visibility through a local group: read the agent and what hangs off it, change nothing.
    f.ok(
        &admin.token,
        "IamService/CreateGroup",
        json!({"source":"GROUP_SOURCE_LOCAL","slug":"ops","name":"Ops"}),
    )
    .await;
    f.ok(
        &admin.token,
        "IamService/AddGroupMember",
        json!({"groupId":"local:ops","userId":member.user}),
    )
    .await;
    assert_eq!(
        f.assign(&admin.token, &a, "reader", group("local:ops"))
            .await,
        200
    );
    assert_eq!(f.visible(&member.token).await, vec![a.clone()]);
    f.ok(
        &member.token,
        "DeploymentService/GetDeployment",
        json!({"agentId":a}),
    )
    .await;
    assert_eq!(f.actions(&member.token, &a).await, json!(["view"]));
    for (path, body) in [
        ("AgentService/UpdateAgent", json!({"id":a,"name":"renamed"})),
        ("AgentService/PauseAgent", json!({"id":a})),
        ("DeploymentService/IssueIngressToken", json!({"agentId":a})),
        (
            "TildeChatProviderService/GetCredentials",
            json!({"agentId":a}),
        ),
    ] {
        assert_eq!(f.call(&member.token, path, body).await.0, 403, "{path}");
    }
    // Readers do not share.
    assert_eq!(
        f.assign(&member.token, &a, "reader", user(&member.user))
            .await,
        403
    );

    // An editor changes the agent and shares it, but only within reach: not deployer, which
    // gives an action the editor lacks, and never a role over every agent.
    assert_eq!(
        f.assign(&admin.token, &a, "editor", user(&member.user))
            .await,
        200
    );
    assert_eq!(
        f.actions(&member.token, &a).await,
        json!(["view", "edit", "share"])
    );
    f.ok(
        &member.token,
        "AgentService/UpdateAgent",
        json!({"id":a,"name":"renamed"}),
    )
    .await;
    assert_eq!(
        f.call(
            &member.token,
            "DeploymentService/IssueDeploymentToken",
            json!({"agentId":a})
        )
        .await
        .0,
        403
    );
    let other = f.login("other", json!([])).await;
    assert_eq!(
        f.assign(&member.token, &a, "reader", user(&other.user))
            .await,
        200
    );
    assert_eq!(
        f.assign(&member.token, &a, "deployer", user(&other.user))
            .await,
        403
    );
    assert_eq!(
        f.assign(&member.token, "", "reader", user(&other.user))
            .await,
        403
    );
    assert_eq!(
        f.assign(&member.token, &b, "reader", user(&other.user))
            .await,
        404
    );
    // A deployer deploys and hands out deployer, and holding both roles stacks their actions.
    assert_eq!(
        f.assign(&admin.token, &a, "deployer", user(&other.user))
            .await,
        200
    );
    assert_eq!(
        f.assign(&other.token, &a, "deployer", user(&member.user))
            .await,
        200
    );
    assert_eq!(
        f.actions(&member.token, &a).await,
        json!(["view", "edit", "deploy", "share"])
    );
    assert_ne!(
        f.call(
            &member.token,
            "DeploymentService/IssueDeploymentToken",
            json!({"agentId":a})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        f.assign(&member.token, &a, "deployer", user(&other.user))
            .await,
        200
    );
    // Assignments list every role a principal holds on the agent.
    let assignments = f
        .ok(
            &member.token,
            "IamService/ListRoleAssignments",
            json!({"resource":{"kind":"RESOURCE_KIND_AGENT","id":a}}),
        )
        .await["assignments"]
        .clone();
    let mine = assignments
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["principal"]["id"] == member.user)
        .unwrap();
    let mut names = mine["roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["Deployer", "Editor"]);
    assert_eq!(
        f.revoke(&other.token, &a, "deployer", user(&member.user))
            .await,
        200
    );
    assert_eq!(
        f.revoke(&other.token, &a, "editor", user(&member.user))
            .await,
        403
    );

    // Capabilities let an agent act on other agents, so a non-administrator only selects
    // agents they hold, and never all.
    let reach = |selection: Value| json!({"id":a,"capabilities":{"agentsInvoke":selection}});
    assert_eq!(
        f.call(
            &member.token,
            "AgentService/UpdateAgent",
            reach(json!({"mode":"TARGET_SELECTION_ALL"}))
        )
        .await
        .0,
        403
    );
    assert_eq!(
        f.call(
            &member.token,
            "AgentService/UpdateAgent",
            reach(json!({"mode":"TARGET_SELECTION_SELECTED","ids":[b]}))
        )
        .await
        .0,
        403
    );
    f.ok(
        &member.token,
        "AgentService/UpdateAgent",
        reach(json!({"mode":"TARGET_SELECTION_SELECTED","ids":[a]})),
    )
    .await;
    f.ok(
        &admin.token,
        "AgentService/UpdateAgent",
        reach(json!({"mode":"TARGET_SELECTION_ALL"})),
    )
    .await;
    // A reach an administrator set stays valid when the owner saves the form unchanged.
    f.ok(
        &member.token,
        "AgentService/UpdateAgent",
        reach(json!({"mode":"TARGET_SELECTION_ALL"})),
    )
    .await;

    // A key holds roles like a user, but never beyond what its creator could assign.
    for roles in [
        json!([role_id(&b, "reader")]),
        json!([role_id("", "reader")]),
        json!([role_id(&a, "deployer")]),
    ] {
        let (status, _) = f
            .call(
                &member.token,
                "ApiKeysService/CreateApiKey",
                json!({"name":"ci","roleIds":roles}),
            )
            .await;
        assert!(status == 403 || status == 404, "{status}");
    }
    let key = f
        .ok(
            &member.token,
            "ApiKeysService/CreateApiKey",
            json!({"name":"ci","roleIds":[role_id(&a, "reader")]}),
        )
        .await;
    let secret = key["secret"].as_str().unwrap();
    assert_eq!(f.visible(secret).await, vec![a.clone()]);
    assert_eq!(
        f.call(secret, "AgentService/PauseAgent", json!({"id":a}))
            .await
            .0,
        403
    );
    // A key that does not edit every agent cannot create one: nothing would tie it to the result.
    assert_eq!(
        f.call(secret, "AgentService/CreateAgent", json!({"name":"x"}))
            .await
            .0,
        403
    );
    let listed = f
        .ok(&member.token, "ApiKeysService/ListApiKeys", json!({}))
        .await;
    assert_eq!(listed["apiKeys"].as_array().unwrap().len(), 1);
    assert_eq!(
        listed["apiKeys"][0]["roles"][0]["id"],
        role_id(&a, "reader")
    );
    f.ok(
        &member.token,
        "ApiKeysService/RevokeApiKey",
        json!({"id":key["apiKey"]["id"]}),
    )
    .await;

    // Any user may create an agent and owns it; membership changes apply to live sessions at once.
    let c = f.agent(&member.token, "gamma").await;
    assert_eq!(
        f.actions(&member.token, &c).await,
        json!(["view", "edit", "deploy", "share"])
    );
    let mut seen = f.visible(&member.token).await;
    seen.sort();
    let mut expected = vec![a.clone(), c.clone()];
    expected.sort();
    assert_eq!(seen, expected);
    f.ok(
        &admin.token,
        "IamService/RemoveGroupMember",
        json!({"groupId":"local:ops","userId":member.user}),
    )
    .await;
    assert_eq!(
        f.revoke(&member.token, &a, "editor", user(&member.user))
            .await,
        200
    );
    assert_eq!(f.visible(&member.token).await, vec![c.clone()]);
    // Deleting an agent takes its roles with it.
    f.ok(&member.token, "AgentService/PauseAgent", json!({"id":c}))
        .await;
    f.ok(&member.token, "AgentService/DeleteAgent", json!({"id":c}))
        .await;
    assert!(
        f.ok(
            &admin.token,
            "IamService/ListRoles",
            json!({"resource":{"kind":"RESOURCE_KIND_AGENT","id":c}}),
        )
        .await["roles"]
            .as_array()
            .is_none_or(Vec::is_empty)
    );
    assert_eq!(
        f.call(
            &admin.token,
            "IamService/RemoveGroupMember",
            json!({"groupId":"tilde_system:admin","userId":admin.user})
        )
        .await
        .0,
        400
    );
}

#[tokio::test]
async fn every_resource_rpc_hides_an_agent_the_caller_does_not_hold() {
    let f = Fixture::new(GroupMapping::default()).await;
    let admin = f.login("root", json!([])).await;
    let outsider = f.login("outsider", json!([])).await;
    let a = f.agent(&admin.token, "alpha").await;
    let other = Uuid::new_v4().to_string();
    let budget = f
        .ok(
            &admin.token,
            "InferenceService/SetBudget",
            json!({"scope":"BUDGET_SCOPE_AGENT","scopeId":a,"period":"BUDGET_PERIOD_DAY","limitMicros":1000,"action":"BUDGET_ACTION_FLAG"}),
        )
        .await["budget"]["id"]
        .clone();
    let resource = json!({"kind":"RESOURCE_KIND_AGENT","id":a});
    for (path, rule) in RULES {
        if *rule != Rule::Resource {
            continue;
        }
        let (service, method) = path.split_once('/').unwrap();
        let body = match (service, method) {
            ("AgentService", _) => json!({"id":a}),
            ("ConnectionsService", _) => {
                json!({"connectionId":other,"assignment":{"capability":"CAPABILITY_CHANNEL","agentId":a}})
            }
            ("IamService", "AssignRole" | "RevokeRole") => {
                json!({"roleId":role_id(&a, "reader"),"principal":user(&outsider.user)})
            }
            ("IamService", _) => json!({"resource":resource}),
            ("InferenceService", "SetBudget") => {
                json!({"scope":"BUDGET_SCOPE_AGENT","scopeId":a,"period":"BUDGET_PERIOD_DAY","limitMicros":1,"action":"BUDGET_ACTION_FLAG"})
            }
            ("InferenceService", "DeleteBudget") => json!({"id":budget}),
            _ => json!({"agentId":a}),
        };
        let (status, value) = f.call(&outsider.token, path, body).await;
        assert_eq!(status, 404, "{path}: {value}");
    }
}

#[tokio::test]
async fn login_mirrors_provider_groups_and_mapped_administrators() {
    let f = Fixture::new(GroupMapping {
        admin_groups: vec!["Platform Admins".into()],
        ..GroupMapping::default()
    })
    .await;
    let admin = f.login("root", json!(["Platform Admins"])).await;
    let a = f.agent(&admin.token, "alpha").await;
    // A provider group can be granted access before any member has signed in.
    f.ok(
        &admin.token,
        "IamService/CreateGroup",
        json!({"source":"GROUP_SOURCE_EXTERNAL","slug":"data-team","name":"Data Team"}),
    )
    .await;
    assert_eq!(
        f.assign(&admin.token, &a, "reader", group("external:data-team"))
            .await,
        200
    );
    assert_eq!(
        f.call(
            &admin.token,
            "IamService/AddGroupMember",
            json!({"groupId":"external:data-team","userId":admin.user})
        )
        .await
        .0,
        400
    );
    f.ok(
        &admin.token,
        "IamService/CreateGroup",
        json!({"source":"GROUP_SOURCE_LOCAL","slug":"kept","name":"Kept"}),
    )
    .await;

    let analyst = f
        .login("analyst", json!(["Data Team", "tilde_system:admin"]))
        .await;
    f.ok(
        &admin.token,
        "IamService/AddGroupMember",
        json!({"groupId":"local:kept","userId":analyst.user}),
    )
    .await;
    let caller = f
        .ok(&analyst.token, "IamService/GetCaller", json!({}))
        .await;
    // A claimed name can only ever become an `external:` group, never a system one.
    assert!(!caller["admin"].as_bool().unwrap_or(false));
    assert_eq!(f.visible(&analyst.token).await, vec![a.clone()]);

    // The next login drops provider groups the user left and keeps locally managed ones.
    let analyst = f.login("analyst", json!([])).await;
    assert!(f.visible(&analyst.token).await.is_empty());
    let groups = f
        .ok(&analyst.token, "IamService/GetCaller", json!({}))
        .await["groupIds"]
        .clone();
    assert!(
        groups.as_array().unwrap().contains(&json!("local:kept")),
        "{groups}"
    );

    // Mapped administrators follow the provider in both directions.
    let promoted = f.login("analyst", json!(["platform-admins"])).await;
    assert_eq!(
        f.ok(&promoted.token, "IamService/GetCaller", json!({}))
            .await["admin"],
        true
    );
    let demoted = f.login("analyst", json!([])).await;
    assert_eq!(
        f.call(
            &demoted.token,
            "IamService/CreateGroup",
            json!({"source":"GROUP_SOURCE_LOCAL","slug":"x","name":"x"})
        )
        .await
        .0,
        403
    );
    // Sessions are revocable for removals that cannot wait for expiry.
    f.ok(
        &admin.token,
        "IamService/RevokeUserSessions",
        json!({"userId":demoted.user}),
    )
    .await;
    assert_eq!(
        f.call(&demoted.token, "IamService/GetCaller", json!({}))
            .await
            .0,
        401
    );
}

#[tokio::test]
async fn roles_over_every_agent_reach_new_ones_and_keys_never_manage_access() {
    let f = Fixture::new(GroupMapping::default()).await;
    let admin = f.login("root", json!([])).await;
    let member = f.login("member", json!([])).await;
    let a = f.agent(&admin.token, "alpha").await;

    // Installation-wide roles are an administrator's to give, to people and to keys, and
    // cover agents created later too.
    assert_eq!(
        f.assign(&member.token, "", "reader", user(&member.user))
            .await,
        403
    );
    assert_eq!(
        f.call(
            &member.token,
            "ApiKeysService/CreateApiKey",
            json!({"name":"watcher","roleIds":["agents/reader"]})
        )
        .await
        .0,
        403
    );
    let watcher = f
        .ok(
            &admin.token,
            "ApiKeysService/CreateApiKey",
            json!({"name":"watcher","roleIds":["agents/reader"]}),
        )
        .await["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let b = f.agent(&admin.token, "beta").await;
    let mut seen = f.visible(&watcher).await;
    seen.sort();
    let mut expected = vec![a.clone(), b.clone()];
    expected.sort();
    assert_eq!(seen, expected);
    assert_eq!(
        f.call(&watcher, "AgentService/PauseAgent", json!({"id":b}))
            .await
            .0,
        403
    );
    assert_eq!(
        f.assign(&admin.token, "", "reader", user(&member.user))
            .await,
        200
    );
    let mut members_view = f.visible(&member.token).await;
    members_view.sort();
    assert_eq!(members_view, seen);

    // A key editing every agent provisions and changes agents, and nothing beyond agents.
    let key = f
        .ok(
            &admin.token,
            "ApiKeysService/CreateApiKey",
            json!({"name":"provisioner","roleIds":["agents/editor"]}),
        )
        .await["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let c = f.agent(&key, "gamma").await;
    f.ok(
        &key,
        "AgentService/UpdateAgent",
        json!({"id":a,"name":"renamed"}),
    )
    .await;
    // Connections and identities carry no roles: anyone signed in reaches them, but attaching
    // one to an agent still needs the agent, and an unknown connection is not found.
    f.ok(
        &key,
        "IdentitiesService/CreateIdentity",
        json!({"identityType":"IDENTITY_TYPE_USERNAME","value":"someone"}),
    )
    .await;
    assert_eq!(
        f.call(
            &key,
            "ConnectionsService/AssignCapability",
            json!({"connectionId":Uuid::new_v4().to_string(),"assignment":{"capability":"CAPABILITY_CHANNEL","agentId":c}}),
        )
        .await
        .0,
        404
    );

    // However much a key holds, it never manages access, users, groups or keys.
    let resource = json!({"kind":"RESOURCE_KIND_AGENT","id":a});
    for (path, body) in [
        ("ApiKeysService/CreateApiKey", json!({"name":"child"})),
        ("ApiKeysService/ListApiKeys", json!({})),
        (
            "IamService/ListRoleAssignments",
            json!({"resource":resource}),
        ),
        (
            "IamService/AssignRole",
            json!({"roleId":role_id(&a, "editor"),"principal":user(&admin.user)}),
        ),
        ("IamService/ListUsers", json!({})),
        (
            "IamService/AddGroupMember",
            json!({"groupId":"tilde_system:admin","userId":admin.user}),
        ),
    ] {
        assert_eq!(f.call(&key, path, body).await.0, 403, "{path}");
    }
    let caller = f.ok(&key, "IamService/GetCaller", json!({})).await;
    assert!(!caller["admin"].as_bool().unwrap_or(false));
    assert!(caller["apiKeyId"].is_string());
    assert_eq!(caller["creatable"], json!(["RESOURCE_KIND_AGENT"]));
}
