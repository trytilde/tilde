//! A gateway invocation reachable over the agent runtime listener: an agent with chosen
//! capabilities, a thread with one user, a running invocation and its connect token.
#![allow(dead_code, reason = "Each test binary uses part of this fixture.")]
use super::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::sync::Arc;
use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::{catalog::Endpoints, service::Connections},
    encryption::Encryption,
};
use uuid::Uuid;

pub fn all_tools() -> types::Capabilities {
    types::Capabilities {
        tools_invoke: types::TargetPermission {
            mode: types::TargetSelection::All.into(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}
pub fn framed(value: Value) -> Vec<u8> {
    let bytes = serde_json::to_vec(&value).unwrap();
    let mut out = vec![0];
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend(bytes);
    out
}
/// Complete Connect envelopes at the front of `buffer`, leaving any partial frame behind.
pub fn drain(buffer: &mut Vec<u8>) -> Vec<Value> {
    let mut out = vec![];
    while buffer.len() >= 5 {
        let len = u32::from_be_bytes(buffer[1..5].try_into().unwrap()) as usize;
        if buffer.len() < 5 + len {
            break;
        }
        out.push(serde_json::from_slice(&buffer[5..5 + len]).unwrap());
        buffer.drain(..5 + len);
    }
    out
}
pub async fn create_agent(agents: &Agents, name: &str, capabilities: types::Capabilities) -> Uuid {
    let id = Uuid::new_v4();
    agents
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id,
            name: name.into(),
            capabilities: tilde::iam::capabilities::Capabilities::from_wire(capabilities).unwrap(),
        })
        .await
        .unwrap();
    id
}
pub struct Fixture {
    pub db: Database,
    pub chat: Chat,
    pub agents: Agents,
    pub connections: Connections,
    pub agent: Uuid,
    pub thread: Uuid,
    pub user_id: String,
    pub origin: String,
    pub token: SecretString,
    server: tokio::task::JoinHandle<()>,
}
impl Fixture {
    pub async fn new(
        seed_byte: u8,
        endpoints: Endpoints,
        capabilities: types::Capabilities,
    ) -> Self {
        let db = Database::new().await;
        let crypto = Arc::new(
            Encryption::initialize(&db.pool, seed(seed_byte))
                .await
                .unwrap(),
        );
        let connections = Connections::with_endpoints(
            db.pool.clone(),
            crypto.clone(),
            "http://127.0.0.1:1".into(),
            "http://127.0.0.1:2".into(),
            endpoints,
        )
        .unwrap();
        connections.seed().await.unwrap();
        let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1:1".into())
            .with_connections(connections.clone());
        let agents = Agents::new(db.pool.clone(), crypto);
        let agent = create_agent(&agents, "Operator", capabilities).await;
        let user = chat.create_user("User").await.unwrap();
        let (thread, token) = invocation(&chat, &db, agent, &user.id).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let router = tilde::iam::listeners::agent_rpc_router(agents.clone(), chat.clone());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            db,
            chat,
            agents,
            connections,
            agent,
            thread,
            user_id: user.id,
            origin,
            token,
            server,
        }
    }
    pub async fn tools(&self) -> Vec<Value> {
        let catalog: Value = reqwest::Client::new()
            .post(format!(
                "{}/tilde.runtime.v1.ChatService/ListTools",
                self.origin
            ))
            .bearer_auth(self.token.expose_secret())
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        catalog["tools"].as_array().cloned().unwrap_or_default()
    }
    /// Continue as the agent's next invocation (in a thread of its own): an agent's catalog is
    /// read once per invocation, so changes to its tools show there.
    pub async fn next_invocation(&mut self) {
        (self.thread, self.token) =
            invocation(&self.chat, &self.db, self.agent, &self.user_id).await;
    }
    /// The single response envelope: `outputJson` on success, `error` otherwise.
    pub async fn invoke(&self, name: &str, input: Value) -> Value {
        let response = reqwest::Client::new()
            .post(format!("{}/tilde.runtime.v1.ChatService/InvokeTool", self.origin))
            .bearer_auth(self.token.expose_secret())
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(framed(json!({"name":name,"callId":Uuid::new_v4().to_string(),"sequence":0,"inputJson":input.to_string(),"finish":true})))
            .send()
            .await
            .unwrap();
        let mut bytes = response.bytes().await.unwrap().to_vec();
        drain(&mut bytes).into_iter().next().unwrap()
    }
    /// A successful call's decoded output.
    pub async fn output(&self, name: &str, input: Value) -> Value {
        let result = self.invoke(name, input).await;
        serde_json::from_str(
            result["outputJson"]
                .as_str()
                .unwrap_or_else(|| panic!("{result}")),
        )
        .unwrap()
    }
    /// Gives the fixture's agent `tools` of a connection; returns the prefix of their names.
    pub async fn use_tools(&self, connection: Uuid, tools: &[&str]) -> String {
        self.chat
            .tools
            .as_ref()
            .unwrap()
            .add_source(
                self.agent,
                tilde::tools::Target::Connection(connection),
                &tools.iter().map(|t| t.to_string()).collect::<Vec<_>>(),
            )
            .await
            .unwrap()
            .slug
    }
    pub async fn close(self) {
        self.server.abort();
        self.db.close().await;
    }
}

/// A thread with the agent and `user`, a running invocation in it, and that invocation's token.
async fn invocation(chat: &Chat, db: &Database, agent: Uuid, user: &str) -> (Uuid, SecretString) {
    let thread = chat
        .create_thread(application::CreateThread {
            title: "Ops".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![
                types::ParticipantRef {
                    agent_id: Some(agent.to_string()),
                    ..Default::default()
                },
                types::ParticipantRef {
                    user_id: Some(user.to_owned()),
                    ..Default::default()
                },
            ],
        })
        .await
        .unwrap();
    let run = chat
        .start_run(application::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "operate".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    db.pool.get().await.unwrap().execute("UPDATE chat_invocations SET status='running',started_at=NOW(),lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1", &[&invocation]).await.unwrap();
    let thread = Uuid::parse_str(&thread.id).unwrap();
    let token = chat
        .tokens
        .issue(agent, invocation, thread, Uuid::parse_str(&run.id).unwrap())
        .await
        .unwrap();
    (thread, token)
}
