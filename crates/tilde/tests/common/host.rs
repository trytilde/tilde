//! A gateway with a fake agent: the public event ingress, the agent runtime listener and the
//! chat worker, plus a host dialled in over `RunService.Watch` with a deployment token as the
//! SDKs do, so work for the agent reaches it as wake frames and it calls back with each wake's
//! capability.
#![allow(dead_code, reason = "Each test binary uses part of this fixture.")]
use connectrpc::client::{ClientConfig, HttpClient};
use secrecy::{ExposeSecret, SecretString};
use std::{sync::Arc, time::Duration};
use tilde::proto::tilde::{agent_host::v1::InvokeRequest, run::v1 as run, types::v1 as types};
use tilde::services::tilde::run::v1::RunServiceClient;
use tilde::{
    agent::Agents,
    chat::Chat,
    connections::{catalog::Endpoints, model::Assignment, service::Connections},
    deployment::Deployments,
    encryption::{Encryption, SecretBinding},
    routines::Routines,
};
use uuid::Uuid;

pub struct Gateway {
    pub db: super::Database,
    pub crypto: Arc<Encryption>,
    pub agent: Uuid,
    pub connections: Connections,
    pub chat: Chat,
    pub routines: Routines,
    /// Origin of the public event ingress (`/connections/webhooks/{id}`).
    pub ingress: String,
    /// Origin of the agent runtime listener.
    pub runtime: String,
    wakes: tokio::sync::mpsc::UnboundedReceiver<InvokeRequest>,
    stop: tokio::sync::watch::Sender<bool>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
fn client(base: &str, bearer: &str) -> RunServiceClient<HttpClient> {
    let mut headers = http::HeaderMap::new();
    headers.insert(
        http::header::AUTHORIZATION,
        format!("Bearer {bearer}").parse().unwrap(),
    );
    RunServiceClient::new(
        HttpClient::plaintext_http2_only(),
        ClientConfig::new(base.parse().unwrap()).with_default_headers(headers),
    )
}
impl Gateway {
    /// An agent with `capabilities` whose dialled-in host is ready for work.
    pub async fn start(seed: u8, endpoints: Endpoints, capabilities: types::Capabilities) -> Self {
        let db = super::Database::new().await;
        let crypto = Arc::new(
            Encryption::initialize(&db.pool, super::seed(seed))
                .await
                .unwrap(),
        );
        let agents = Agents::new(db.pool.clone(), crypto.clone());
        let agent = super::invocation::create_agent(&agents, "Bug fixer", capabilities).await;
        let connections = Connections::with_endpoints(
            db.pool.clone(),
            crypto.clone(),
            "http://127.0.0.1".into(),
            "https://ingress.example".into(),
            endpoints,
        )
        .unwrap();
        connections.seed().await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let runtime = format!("http://{}", listener.local_addr().unwrap());
        let deployments = Deployments::new(
            db.pool.clone(),
            crypto.clone(),
            agents.clone(),
            connections.clone(),
        );
        let chat = Chat::new(db.pool.clone(), crypto.clone(), runtime.clone())
            .with_connections(connections.clone())
            .with_deployments(deployments.clone());
        let routines = Routines::new(db.pool.clone(), chat.clone());
        let router = tilde::iam::listeners::agent_rpc_router(agents, chat.clone());
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        let ingress_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ingress = format!("http://{}", ingress_listener.local_addr().unwrap());
        let ingress_router =
            tilde::iam::listeners::public_event_ingress_router(chat.clone(), connections.clone());
        let ingress_server = tokio::spawn(async move {
            let _ = axum::serve(ingress_listener, ingress_router).await;
        });
        let (stop, stop_rx) = tokio::sync::watch::channel(false);
        let worker = tokio::spawn(chat.clone().worker(stop_rx));
        let (_, token, _) = deployments
            .register_deployment(
                agent,
                tilde::deployment::RegisterDeployment {
                    source: types::DeploymentSource::Manual,
                    target: types::DeploymentTarget::Gateway,
                    target_reference: None,
                    repository: None,
                    commit_sha: None,
                    external_id: None,
                    label: None,
                    commit_message: None,
                    branch: None,
                    commit_author: None,
                    declarations: Default::default(),
                },
            )
            .await
            .unwrap();
        let host = client(&runtime, token.unwrap().expose_secret());
        let instance = Uuid::new_v4().to_string();
        let mut watch = host
            .watch(run::WatchRequest {
                instance_id: instance.clone(),
                ..Default::default()
            })
            .await
            .unwrap();
        watch.message().await.unwrap().expect("registration frame");
        host.heartbeat(run::HeartbeatRequest {
            instance_id: instance,
            ready: true,
            ..Default::default()
        })
        .await
        .unwrap();
        let (wakes, received) = tokio::sync::mpsc::unbounded_channel();
        let reader = tokio::spawn(async move {
            while let Ok(Some(frame)) = watch.message().await {
                let frame: run::WatchResponse = frame.to_owned_message();
                if let Some(run::watch_response::Frame::Wake(wake)) = frame.frame
                    && wakes.send(*wake).is_err()
                {
                    break;
                }
            }
        });
        Self {
            db,
            crypto,
            agent,
            connections,
            chat,
            routines,
            ingress,
            runtime,
            wakes: received,
            stop,
            tasks: vec![server, ingress_server, worker, reader],
        }
    }
    /// A ready installation connection with these credentials, as if its setup had completed,
    /// optionally with the agent's channel assignment.
    pub async fn connection(
        &self,
        provider: &str,
        typ: &str,
        values: &[(&str, &str)],
        assignments: &[Assignment],
    ) -> Uuid {
        let connection = self
            .connections
            .start(Uuid::new_v4(), "acme", provider, typ, assignments)
            .await
            .unwrap()
            .connection
            .id;
        let client = self.db.pool.get().await.unwrap();
        for (key, value) in values {
            let sealed = self
                .crypto
                .seal(
                    SecretBinding {
                        resource_kind: "connection",
                        resource_id: connection,
                        name: key,
                    },
                    &SecretString::from(*value),
                )
                .unwrap()
                .into_bytes();
            client
                .execute(
                    "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)",
                    &[&connection, key, &sealed],
                )
                .await
                .unwrap();
        }
        client
            .execute(
                "UPDATE connections SET status='ready' WHERE id=$1",
                &[&connection],
            )
            .await
            .unwrap();
        connection
    }
    /// Posts a webhook delivery; returns the status and body.
    pub async fn deliver(
        &self,
        connection: Uuid,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> (u16, String) {
        let mut request = reqwest::Client::new()
            .post(format!(
                "{}/connections/webhooks/{connection}",
                self.ingress
            ))
            .body(body.to_vec());
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        (status, response.text().await.unwrap())
    }
    /// Runs the agent has, across all its threads.
    pub async fn runs(&self) -> i64 {
        self.db
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT COUNT(*) FROM chat_runs WHERE agent_id=$1",
                &[&self.agent],
            )
            .await
            .unwrap()
            .get(0)
    }
    /// The next wake the host receives.
    pub async fn wake(&mut self) -> InvokeRequest {
        tokio::time::timeout(Duration::from_secs(20), self.wakes.recv())
            .await
            .expect("a wake within 20 seconds")
            .expect("stream stays open")
    }
    /// No wake arrives within `wait`.
    pub async fn idle(&mut self, wait: Duration) {
        if let Ok(Some(wake)) = tokio::time::timeout(wait, self.wakes.recv()).await {
            panic!("unexpected wake: {}", wake.objective);
        }
    }
    /// Calls a runtime tool as the woken invocation, with its capability; returns the response
    /// envelope (`outputJson` on success, `error` otherwise).
    pub async fn invoke_tool(
        &self,
        wake: &InvokeRequest,
        name: &str,
        input: serde_json::Value,
    ) -> serde_json::Value {
        let response = reqwest::Client::new()
            .post(format!(
                "{}/tilde.runtime.v1.ChatService/InvokeTool",
                self.runtime
            ))
            .bearer_auth(&wake.capability)
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(super::invocation::framed(serde_json::json!({
                "name": name,
                "callId": Uuid::new_v4().to_string(),
                "sequence": 0,
                "inputJson": input.to_string(),
                "finish": true,
            })))
            .send()
            .await
            .unwrap();
        let mut bytes = response.bytes().await.unwrap().to_vec();
        super::invocation::drain(&mut bytes)
            .into_iter()
            .next()
            .unwrap()
    }
    pub async fn close(self) {
        let _ = self.stop.send(true);
        for task in self.tasks {
            task.abort();
        }
        self.db.close().await;
    }
}
