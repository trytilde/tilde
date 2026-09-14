//! The sidecar runs the cached runtime for one or more agents beside their agent
//! processes. Two listeners: a loopback runtime listener for the agent process,
//! and one network listener for provider webhooks, native ingress and hand-offs
//! from peer replicas. Everything else is an outbound connection to the gateway.
use super::{
    gateway,
    runtime::{Runtime, RuntimeOptions},
};
use crate::{
    chat::{Chat, ChatError, Result},
    proto::tilde::{agent_event_ingress::v1 as wire, types::v1 as types},
};
use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use secrecy::{ExposeSecret, SecretString};
use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;
#[derive(Clone)]
pub struct Node {
    pub runtime: Arc<Runtime>,
    pub gateway: gateway::Client,
    /// For handing a request to the peer replica that holds its thread.
    peers: reqwest::Client,
}
pub struct Options {
    pub gateway_url: String,
    pub tokens: Vec<SecretString>,
    pub endpoints: BTreeMap<Uuid, String>,
    pub runtime_listen: SocketAddr,
    pub listen: SocketAddr,
    pub public_url: Option<String>,
    /// Idle time after which a cached thread is dropped and its lease released.
    pub idle_after: Duration,
}
impl Options {
    pub fn from_env() -> Result<Self> {
        let env = |name: &str| {
            std::env::var(name).map_err(|_| ChatError::Invalid(format!("{name} is required")))
        };
        let tokens = SecretString::from(env("ENGINE_SIDECAR_AGENT_TOKENS")?);
        let tokens = tokens
            .expose_secret()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(SecretString::from)
            .collect::<Vec<_>>();
        if tokens.is_empty() {
            return Err(ChatError::Invalid(
                "At least one deployment token is required".into(),
            ));
        }
        let mut endpoints = BTreeMap::new();
        for entry in env("ENGINE_SIDECAR_AGENT_ENDPOINTS")?.split(',') {
            let (agent, endpoint) = entry.split_once('=').ok_or_else(|| {
                ChatError::Invalid("Agent endpoints must be agent_id=http(s)://host pairs".into())
            })?;
            endpoints.insert(
                crate::chat::id(agent.trim())?,
                crate::agent::validate_endpoint(endpoint.trim().into())
                    .map_err(|_| ChatError::Invalid("Invalid local agent endpoint".into()))?,
            );
        }
        let socket = |name: &str, default: &str| {
            std::env::var(name)
                .unwrap_or_else(|_| default.into())
                .parse()
                .map_err(|_| ChatError::Invalid(format!("Invalid {name}")))
        };
        let result = Self {
            gateway_url: env("ENGINE_SIDECAR_GATEWAY_URL")?,
            tokens,
            endpoints,
            runtime_listen: socket("ENGINE_SIDECAR_RUNTIME_LISTEN", "127.0.0.1:8081")?,
            listen: socket("ENGINE_SIDECAR_LISTEN", "0.0.0.0:8082")?,
            public_url: std::env::var("ENGINE_SIDECAR_PUBLIC_URL")
                .ok()
                .filter(|s| !s.is_empty()),
            idle_after: Duration::from_secs(
                std::env::var("ENGINE_SIDECAR_THREAD_IDLE_SECONDS")
                    .ok()
                    .map(|v| {
                        v.parse().map_err(|_| {
                            ChatError::Invalid("Invalid ENGINE_SIDECAR_THREAD_IDLE_SECONDS".into())
                        })
                    })
                    .transpose()?
                    .unwrap_or(24 * 60 * 60),
            ),
        };
        if !result.runtime_listen.ip().is_loopback() {
            return Err(ChatError::Invalid("ENGINE_SIDECAR_RUNTIME_LISTEN must be a loopback address; only the agent process next door may use it".into()));
        }
        if result.runtime_listen == result.listen {
            return Err(ChatError::Invalid(
                "Sidecar listeners must have distinct addresses".into(),
            ));
        }
        Ok(result)
    }
}
/// A running sidecar: its nodes, bound addresses, and the handle that stops it.
pub struct Sidecar {
    pub instance: Uuid,
    pub nodes: Vec<Node>,
    pub runtime_address: SocketAddr,
    pub address: SocketAddr,
    stop: tokio::sync::watch::Sender<bool>,
    workers: tokio::task::JoinSet<()>,
    servers: tokio::task::JoinHandle<Result<()>>,
    platform: Option<(
        opentelemetry_sdk::trace::SdkTracerProvider,
        tokio::task::JoinHandle<()>,
    )>,
}
impl Sidecar {
    /// Stop serving, drop live executions, and let the gateway see this incarnation disappear.
    pub async fn stop(mut self) {
        let _ = self.stop.send(true);
        self.servers.abort();
        self.workers.abort_all();
        while self.workers.join_next().await.is_some() {}
        if let Some((provider, worker)) = self.platform.take() {
            let _ = tokio::task::spawn_blocking(move || provider.shutdown()).await;
            worker.abort();
        }
    }
}
pub async fn run(options: Options) -> Result<()> {
    let mut sidecar = start(options).await?;
    let result = tokio::select! {
        _ = tokio::signal::ctrl_c() => Ok(()),
        result = &mut sidecar.servers => result.unwrap_or(Err(ChatError::Transport)),
        _ = sidecar.workers.join_next() => Err(ChatError::Transport),
    };
    sidecar.stop().await;
    result
}
pub async fn start(options: Options) -> Result<Sidecar> {
    // A restart loses live executions. A fresh incarnation lets the gateway see
    // the previous owner disappear and apply the agent's failure policy.
    let instance = Uuid::new_v4();
    let runtime_listener = tokio::net::TcpListener::bind(options.runtime_listen)
        .await
        .map_err(|_| ChatError::Transport)?;
    let public_listener = tokio::net::TcpListener::bind(options.listen)
        .await
        .map_err(|_| ChatError::Transport)?;
    let runtime_port = runtime_listener
        .local_addr()
        .map_err(|_| ChatError::Transport)?
        .port();
    let public_address = public_listener
        .local_addr()
        .map_err(|_| ChatError::Transport)?;
    let public_url = options
        .public_url
        .clone()
        .unwrap_or_else(|| format!("http://{public_address}"));
    let platform_routes = super::telemetry::PlatformRoutes::default();
    let (platform_provider, platform_worker) = super::telemetry::platform(platform_routes.clone());
    opentelemetry::global::set_tracer_provider(platform_provider.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let mut workers = tokio::task::JoinSet::new();
    let peers = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| ChatError::Transport)?;
    let mut runtime_router = Router::new();
    let mut public_router = Router::new();
    let mut nodes: Vec<Node> = vec![];
    for token in options.tokens {
        let gateway = gateway::Client::new(&options.gateway_url, token)?;
        let request = wire::WatchRequest {
            instance_id: instance.to_string(),
            public_url: public_url.clone(),
            runtime_url: format!("http://127.0.0.1:{runtime_port}"),
            ..Default::default()
        };
        let mut stream = gateway.watch(request.clone()).await?;
        let first = stream
            .message()
            .await
            .map_err(|_| ChatError::Transport)?
            .ok_or(ChatError::Transport)?;
        let first: wire::WatchResponse = first.to_owned_message();
        let Some(wire::watch_response::Frame::Snapshot(snapshot)) = first.frame else {
            return Err(ChatError::Invalid(
                "Gateway did not begin with a snapshot".into(),
            ));
        };
        let mut snapshot = *snapshot;
        let configuration = snapshot
            .configuration
            .into_option()
            .ok_or(ChatError::Transport)?;
        let agent = crate::chat::id(&configuration.agent.id)?;
        if nodes.iter().any(|n| n.runtime.agent_id == agent) {
            return Err(ChatError::Invalid(
                "An agent appears more than once in the deployment token list".into(),
            ));
        }
        let endpoint = options
            .endpoints
            .get(&agent)
            .ok_or_else(|| ChatError::Invalid(format!("Local endpoint missing for agent {agent}")))?
            .clone();
        let prefix = format!("/agents/{agent}");
        let runtime = Arc::new(Runtime::new(
            gateway.clone(),
            RuntimeOptions {
                agent_id: agent,
                instance_id: instance,
                signing_key: SecretString::from(std::mem::take(&mut snapshot.token_signing_key)),
                host_key: SecretString::from(configuration.webhook_signing_key.clone()),
                local_endpoint: endpoint,
                callback_url: format!("http://127.0.0.1:{runtime_port}{prefix}"),
                idle_after: options.idle_after,
            },
        ));
        runtime.configure(configuration);
        for lease in std::mem::take(&mut snapshot.leases) {
            runtime.apply_lease(lease).await?;
        }
        let node = Node {
            runtime: runtime.clone(),
            gateway: gateway.clone(),
            peers: peers.clone(),
        };
        platform_routes
            .write()
            .map_err(|_| ChatError::Transport)?
            .insert(agent, runtime.clone());
        runtime_router = runtime_router.nest(&prefix, node.runtime_router());
        public_router = public_router.nest(
            &prefix,
            node.public_router().layer(middleware::from_fn_with_state(
                node.clone(),
                super::telemetry::capture,
            )),
        );
        // Listeners are bound; the first heartbeat may already report readiness.
        runtime
            .state
            .serving
            .store(true, std::sync::atomic::Ordering::Release);
        workers.spawn(runtime.as_ref().clone().worker(rx.clone()));
        workers.spawn(runtime.as_ref().clone().shipper(rx.clone()));
        workers.spawn(runtime.as_ref().clone().heartbeat_worker(rx.clone()));
        workers.spawn(runtime.as_ref().clone().probe_worker(rx.clone()));
        workers.spawn(runtime.as_ref().clone().housekeeping(rx.clone()));
        workers.spawn(runtime.as_ref().clone().attachment_worker(rx.clone()));
        let watcher = node.clone();
        let mut shutdown = rx.clone();
        workers.spawn(async move {
            let mut current = Some(stream);
            loop {
                if let Some(mut stream) = current.take() {
                    tokio::select! {
                        _ = shutdown.changed() => return,
                        result = watcher.runtime.consume(&watcher, &mut stream) => {
                            if result.is_err() { tracing::warn!(agent_id=%watcher.runtime.agent_id, "Gateway watch stream failed"); }
                        }
                    }
                }
                tokio::select! { _ = shutdown.changed() => return, _ = tokio::time::sleep(Duration::from_secs(1)) => {} }
                match watcher.gateway.watch(request.clone()).await {
                    Ok(stream) => current = Some(stream),
                    Err(_) => tracing::warn!(agent_id=%watcher.runtime.agent_id, "Gateway watch reconnect failed"),
                }
            }
        });
        nodes.push(node);
    }
    let readiness = nodes.iter().map(|n| n.runtime.clone()).collect::<Vec<_>>();
    let health = Router::new().route(
        "/healthz",
        get(move || {
            let readiness = readiness.clone();
            async move {
                if readiness.iter().all(|r| r.healthy()) {
                    http::StatusCode::OK
                } else {
                    http::StatusCode::SERVICE_UNAVAILABLE
                }
            }
        }),
    );
    runtime_router = runtime_router.merge(health.clone());
    public_router = public_router.merge(health);
    let runtime_address = runtime_listener
        .local_addr()
        .map_err(|_| ChatError::Transport)?;
    tracing::info!(runtime_address=%runtime_address, address=%public_address, agents=nodes.len(), "tilde-sidecar listening");
    let servers = tokio::spawn(async move {
        tokio::try_join!(
            axum::serve(runtime_listener, runtime_router),
            axum::serve(public_listener, public_router)
        )
        .map(|_| ())
        .map_err(|_| ChatError::Transport)
    });
    Ok(Sidecar {
        instance,
        nodes,
        runtime_address,
        address: public_address,
        stop,
        workers,
        servers,
        platform: Some((platform_provider, platform_worker)),
    })
}
impl Node {
    pub fn runtime_router(&self) -> Router {
        crate::chat::rpc::runtime::router(Chat::from_sidecar(self.runtime.clone()))
            .route(
                "/tilde.runtime.v1.AgentService/{*method}",
                axum::routing::any(registry_relay).with_state(self.clone()),
            )
            .layer(middleware::from_fn_with_state(self.clone(), runtime_guard))
            .layer(middleware::from_fn_with_state(
                self.clone(),
                super::telemetry::capture,
            ))
            .merge(super::telemetry::router(self.clone()))
            .merge(
                crate::chat::controls::router(Chat::from_sidecar(self.runtime.clone()))
                    .layer(middleware::from_fn_with_state(self.clone(), control_guard)),
            )
    }
    pub fn public_router(&self) -> Router {
        crate::chat::rpc::ingress::router(Chat::from_sidecar(self.runtime.clone()))
            .layer(middleware::from_fn_with_state(self.clone(), public_guard))
            .merge(
                Router::new()
                    .route(
                        "/connections/webhooks/{id}",
                        axum::routing::post(provider_receive).get(provider_challenge),
                    )
                    .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024))
                    .with_state(self.clone()),
            )
            .merge(
                Router::new()
                    .route(
                        "/peers/provider-events/{id}",
                        axum::routing::post(peer_provider_event),
                    )
                    .layer(middleware::from_fn_with_state(self.clone(), peer_guard))
                    .layer(axum::extract::DefaultBodyLimit::max(4 * 1024 * 1024))
                    .with_state(self.clone()),
            )
    }
    /// Re-send an ingress request to the replica holding its thread.
    async fn hand_off(
        &self,
        url: &str,
        path: &str,
        parts: &http::request::Parts,
        body: Vec<u8>,
    ) -> Response {
        let target = format!(
            "{}/agents/{}/{}",
            url.trim_end_matches('/'),
            self.runtime.agent_id,
            path
        );
        let mut request = self.peers.post(&target).body(body);
        for name in [
            http::header::AUTHORIZATION,
            http::header::CONTENT_TYPE,
            http::header::ACCEPT,
            http::HeaderName::from_static("connect-protocol-version"),
            http::HeaderName::from_static("connect-timeout-ms"),
        ] {
            if let Some(value) = parts.headers.get(&name) {
                request = request.header(name, value);
            }
        }
        match request.send().await {
            Ok(response) => {
                let status = response.status();
                let content_type = response
                    .headers()
                    .get(http::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_owned();
                let body = response
                    .bytes()
                    .await
                    .map(|b| b.to_vec())
                    .unwrap_or_default();
                super::public::relay(wire::CallResult {
                    status: status.as_u16() as i32,
                    content_type,
                    body,
                    ..Default::default()
                })
            }
            Err(_) => {
                tracing::warn!(agent_id=%self.runtime.agent_id, "Peer replica holding the thread is unreachable");
                connectrpc::ConnectError::unavailable(
                    "The replica holding this conversation is unreachable",
                )
                .into_response()
            }
        }
    }
    /// A short-lived credential peers accept for handed-off provider events.
    fn peer_token(&self) -> Result<String> {
        #[derive(serde::Serialize)]
        struct Claims<'a> {
            iss: &'a str,
            aud: &'a str,
            sub: Uuid,
            iat: i64,
            exp: i64,
        }
        let now = chrono::Utc::now().timestamp();
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &Claims {
                iss: "tilde:sidecar",
                aud: "tilde:sidecar-peer",
                sub: self.runtime.agent_id,
                iat: now,
                exp: now + 60,
            },
            &jsonwebtoken::EncodingKey::from_secret(
                self.runtime.signing_key.expose_secret().as_bytes(),
            ),
        )
        .map_err(|_| ChatError::Transport)
    }
    fn verify_peer_token(&self, token: &str) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct Claims {
            sub: Uuid,
        }
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_issuer(&["tilde:sidecar"]);
        validation.set_audience(&["tilde:sidecar-peer"]);
        let claims = jsonwebtoken::decode::<Claims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(
                self.runtime.signing_key.expose_secret().as_bytes(),
            ),
            &validation,
        )
        .map_err(|_| ChatError::Denied)?
        .claims;
        if claims.sub != self.runtime.agent_id {
            return Err(ChatError::Denied);
        }
        Ok(())
    }
    /// Ingress calls executed here on the gateway's behalf: verified, never forwarded again.
    fn local_router(&self) -> Router {
        crate::chat::rpc::ingress::router(Chat::from_sidecar(self.runtime.clone()))
            .layer(middleware::from_fn_with_state(self.clone(), local_guard))
    }
    pub(crate) async fn directive(self, directive: wire::Directive) {
        tracing::debug!(agent_id=%self.runtime.agent_id, instance_id=%self.runtime.instance_id, directive_id=%directive.id, thread_id=%directive.thread_id, action=?directive.action.as_ref().map(std::mem::discriminant), "Directive received");
        let result = match directive.action {
            Some(wire::directive::Action::IngressCall(call)) => self.execute_call(*call).await,
            Some(wire::directive::Action::ProviderEvent(event)) => {
                let outcome = async {
                    let connection = crate::chat::id(&event.connection_id)?;
                    let message = crate::chat::providers::ingress::IncomingMessage::try_from(
                        event.event.into_option().ok_or(ChatError::Transport)?,
                    )
                    .map_err(|_| ChatError::Invalid("Invalid provider event".into()))?;
                    self.runtime.ingest_provider(connection, message).await
                }
                .await;
                status_result(outcome)
            }
            Some(wire::directive::Action::Relay(relay)) => {
                let outcome = async {
                    let roster = relay.thread.into_option().ok_or(ChatError::Transport)?;
                    let message = relay.message.into_option().ok_or(ChatError::Transport)?;
                    self.runtime.relay(roster, message).await
                }
                .await;
                status_result(outcome)
            }
            // Wakes are for hosts that dial the gateway directly; a sidecar drives its
            // agent from its own leases. Phase 3 routes wakes to the local agent.
            Some(wire::directive::Action::Wake(_)) | None => wire::CallResult {
                status: 400,
                ..Default::default()
            },
        };
        if result.status >= 400 {
            tracing::warn!(agent_id=%self.runtime.agent_id, directive_id=%directive.id, thread_id=%directive.thread_id, status=result.status, "Directive failed");
        }
        self.runtime.push(
            wire::DirectiveResult {
                id: directive.id,
                result: result.into(),
                ..Default::default()
            }
            .into(),
        );
    }
    async fn execute_call(&self, call: wire::IngressCall) -> wire::CallResult {
        let mut request = Request::builder()
            .method(http::Method::POST)
            .uri(format!("/tilde.ingress.v1.ChatService/{}", call.method));
        if let Ok(value) = http::HeaderValue::from_str(&call.content_type) {
            request = request.header(http::header::CONTENT_TYPE, value);
        }
        if let Ok(value) = http::HeaderValue::from_str(&format!("Bearer {}", call.caller_token)) {
            request = request.header(http::header::AUTHORIZATION, value);
        }
        let Ok(request) = request.body(axum::body::Body::from(call.body)) else {
            return wire::CallResult {
                status: 400,
                ..Default::default()
            };
        };
        let response = match self.local_router().oneshot(request).await {
            Ok(response) => response,
            Err(never) => match never {},
        };
        let status = response.status().as_u16() as i32;
        let content_type = response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let body = axum::body::to_bytes(response.into_body(), 192 * 1024 * 1024)
            .await
            .map(|b| b.to_vec())
            .unwrap_or_default();
        wire::CallResult {
            status,
            content_type,
            body,
            ..Default::default()
        }
    }
}
impl Node {
    async fn hand_off_provider_event(
        &self,
        url: &str,
        connection: Uuid,
        event: types::ProviderEvent,
    ) -> Result<()> {
        use buffa::Message;
        let target = format!(
            "{}/agents/{}/peers/provider-events/{connection}",
            url.trim_end_matches('/'),
            self.runtime.agent_id
        );
        let response = self
            .peers
            .post(&target)
            .bearer_auth(self.peer_token()?)
            .header(http::header::CONTENT_TYPE, "application/proto")
            .body(event.encode_to_vec())
            .send()
            .await
            .map_err(|_| ChatError::Transport)?;
        match response.status().as_u16() {
            200..=299 => Ok(()),
            409 => Err(ChatError::Denied),
            404 => Err(ChatError::NotFound),
            _ => Err(ChatError::Transport),
        }
    }
}
async fn peer_guard(State(node): State<Node>, request: Request, next: Next) -> Response {
    let Some(token) = bearer(&request) else {
        return http::StatusCode::UNAUTHORIZED.into_response();
    };
    if node.verify_peer_token(token).is_err() {
        return http::StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}
/// A provider event another replica received for a thread this one holds.
async fn peer_provider_event(
    State(node): State<Node>,
    axum::extract::Path(connection): axum::extract::Path<Uuid>,
    body: axum::body::Bytes,
) -> Response {
    use buffa::Message;
    let outcome = async {
        let event = types::ProviderEvent::decode_from_slice(&body)
            .map_err(|_| ChatError::Invalid("Invalid provider event".into()))?;
        let message = crate::chat::providers::ingress::IncomingMessage::try_from(event)
            .map_err(|_| ChatError::Invalid("Invalid provider event".into()))?;
        node.runtime.ingest_provider(connection, message).await
    }
    .await;
    let result = status_result(outcome);
    let status =
        http::StatusCode::from_u16(result.status as u16).unwrap_or(http::StatusCode::BAD_GATEWAY);
    status.into_response()
}
fn status_result(outcome: Result<()>) -> wire::CallResult {
    match outcome {
        Ok(()) => wire::CallResult {
            status: 204,
            ..Default::default()
        },
        Err(ChatError::Denied) => wire::CallResult {
            status: 409,
            ..Default::default()
        },
        Err(ChatError::NotFound) => wire::CallResult {
            status: 404,
            ..Default::default()
        },
        Err(_) => wire::CallResult {
            status: 503,
            ..Default::default()
        },
    }
}
fn bearer(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}
async fn runtime_guard(State(node): State<Node>, mut request: Request, next: Next) -> Response {
    let Some(token) = bearer(&request) else {
        return connectrpc::ConnectError::unauthenticated("Invocation token required")
            .into_response();
    };
    let Ok(thread) = node.runtime.token_thread(token) else {
        return connectrpc::ConnectError::unauthenticated("Invalid invocation token")
            .into_response();
    };
    if node.runtime.load(thread).await.is_err() {
        return connectrpc::ConnectError::unauthenticated("Invalid invocation token")
            .into_response();
    }
    match node.runtime.scope(token).await {
        Ok(scope) => {
            use opentelemetry::trace::TraceContextExt;
            let cx = opentelemetry::Context::current();
            for (key, value) in [
                ("tilde.thread.id", scope.thread_id),
                ("tilde.run.id", scope.run_id),
                ("tilde.invocation.id", scope.id),
            ] {
                cx.span()
                    .set_attribute(opentelemetry::KeyValue::new(key, value.to_string()));
            }
            request.extensions_mut().insert(scope);
            next.run(request).await
        }
        Err(e) => connectrpc::ConnectError::from(e).into_response(),
    }
}
/// Registry calls run at the gateway; the live invocation was verified by `runtime_guard`.
async fn registry_relay(State(node): State<Node>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let Some(token) = parts
        .headers
        .get(http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::to_owned)
    else {
        return connectrpc::ConnectError::unauthenticated("Invocation token required")
            .into_response();
    };
    let content_type = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let body = match axum::body::to_bytes(body, 4 * 1024 * 1024).await {
        Ok(body) => body.to_vec(),
        Err(_) => return http::StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    let path = parts.uri.path().trim_start_matches('/').to_owned();
    match node
        .gateway
        .relay(node.runtime.instance_id, &path, &content_type, body, &token)
        .await
    {
        Ok(result) => super::public::relay(result),
        Err(e) => connectrpc::ConnectError::from(e).into_response(),
    }
}
async fn control_guard(State(node): State<Node>, request: Request, next: Next) -> Response {
    let Some(token) = bearer(&request) else {
        return http::StatusCode::UNAUTHORIZED.into_response();
    };
    if node.runtime.verify_token_signature(token).is_err() {
        return http::StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}
/// Verify the ingress credential and decide between local execution and the owner.
async fn public_guard(State(node): State<Node>, request: Request, next: Next) -> Response {
    let Some(token) = bearer(&request).map(str::to_owned) else {
        return connectrpc::ConnectError::unauthenticated("Ingress token required").into_response();
    };
    let claims = match super::tokens::verify_ingress(
        &token,
        node.runtime.agent_id,
        &node.runtime.signing_key,
    ) {
        Ok(claims) => claims,
        Err(_) => {
            return connectrpc::ConnectError::unauthenticated("Invalid ingress token")
                .into_response();
        }
    };
    let (mut parts, body) = request.into_parts();
    let body = match axum::body::to_bytes(body, 192 * 1024 * 1024).await {
        Ok(body) => body,
        Err(_) => return http::StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    let method = parts.uri.path().rsplit('/').next().unwrap_or("").to_owned();
    let content_type = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_owned();
    if let Err(e) =
        super::routing::check_creation_scope(node.runtime.agent_id, &method, &body, &content_type)
    {
        return connectrpc::ConnectError::from(e).into_response();
    }
    let chat = Chat::from_sidecar(node.runtime.clone());
    // Agent-wide listings and history pages come from the projection, which is
    // the only place that sees every replica's conversations.
    let listing = method == "ListThreads"
        || (method == "ListMessages"
            && super::routing::decode::<crate::proto::tilde::ingress::v1::ListMessagesRequest>(
                &body,
                &content_type,
            )
            .is_ok_and(|r| !r.before_message_id.is_empty()));
    let thread = match super::routing::thread(&chat, &method, &body, &content_type).await {
        Ok(_) if listing => Err(ChatError::NotFound),
        Ok(thread) => Ok(thread),
        Err(e) => Err(e),
    };
    let thread = match thread {
        Ok(thread) => thread,
        Err(ChatError::NotFound) => {
            // Known only elsewhere: the gateway resolves the conversation from the call.
            let result = node
                .gateway
                .forward(
                    None,
                    wire::IngressCall {
                        method,
                        content_type,
                        body: body.to_vec(),
                        caller_token: token,
                        ..Default::default()
                    },
                )
                .await;
            return match result {
                Ok(result) => super::public::relay(result),
                Err(e) => connectrpc::ConnectError::from(e).into_response(),
            };
        }
        Err(e) => return connectrpc::ConnectError::from(e).into_response(),
    };
    if let Some(thread) = thread {
        if claims.thread_id.is_some_and(|allowed| allowed != thread) {
            return connectrpc::ConnectError::permission_denied(
                "Credential is scoped to another conversation",
            )
            .into_response();
        }
        // One gateway call takes the lease for a thread first seen here; a thread
        // another live replica holds is handed to it directly.
        let held = match node.runtime.load(thread).await {
            Ok(shared) => node.runtime.owns(&*shared.lock().await),
            Err(ChatError::Denied) => false,
            Err(ChatError::NotFound) => false,
            Err(e) => return connectrpc::ConnectError::from(e).into_response(),
        };
        if !held {
            let path = parts
                .uri
                .path()
                .rsplit_once('/')
                .map(|(_, m)| format!("tilde.ingress.v1.ChatService/{m}"))
                .unwrap_or_default();
            if let Some(url) = node.runtime.held_elsewhere(thread) {
                return node.hand_off(&url, &path, &parts, body.to_vec()).await;
            }
            // Nobody live holds it and this replica could not take it: the gateway
            // finds a replica that can.
            let result = node
                .gateway
                .forward(
                    Some(thread),
                    wire::IngressCall {
                        method,
                        content_type,
                        body: body.to_vec(),
                        caller_token: token,
                        ..Default::default()
                    },
                )
                .await;
            return match result {
                Ok(result) => super::public::relay(result),
                Err(e) => connectrpc::ConnectError::from(e).into_response(),
            };
        }
    } else if claims.thread_id.is_some() {
        return connectrpc::ConnectError::permission_denied(
            "Credential requires a conversation-scoped operation",
        )
        .into_response();
    }
    parts.extensions.insert(claims);
    next.run(Request::from_parts(parts, axum::body::Body::from(body)))
        .await
}
async fn local_guard(State(node): State<Node>, mut request: Request, next: Next) -> Response {
    let Some(token) = bearer(&request) else {
        return connectrpc::ConnectError::unauthenticated("Ingress token required").into_response();
    };
    match super::tokens::verify_ingress(token, node.runtime.agent_id, &node.runtime.signing_key) {
        Ok(claims) => {
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(_) => {
            connectrpc::ConnectError::unauthenticated("Invalid ingress token").into_response()
        }
    }
}
async fn provider_receive(
    State(node): State<Node>,
    axum::extract::Path(connection): axum::extract::Path<Uuid>,
    headers: http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let result: Result<Option<String>> = async {
        let (provider, typ, access) = node
            .runtime
            .connection_access(connection)
            .await
            .map_err(|_| ChatError::Denied)?;
        let adapter =
            crate::chat::providers::adapter(&provider, &typ).ok_or(ChatError::NotFound)?;
        match adapter
            .webhook(&access, &headers, &body)
            .await
            .map_err(|_| ChatError::Denied)?
        {
            crate::chat::providers::ingress::Webhook::Challenge(value) => Ok(Some(value)),
            crate::chat::providers::ingress::Webhook::Messages(messages) => {
                for message in messages {
                    let thread = crate::chat::providers::ingress::stable(
                        connection,
                        "thread",
                        &format!("{}:{}", node.runtime.agent_id, message.thread_id),
                    );
                    let event = crate::proto::tilde::types::v1::ProviderEvent::from(message);
                    let local =
                        crate::chat::providers::ingress::IncomingMessage::try_from(event.clone())
                            .map_err(|_| ChatError::Invalid("Invalid provider event".into()))?;
                    match node.runtime.ingest_provider(connection, local).await {
                        Ok(()) => {}
                        // Another live replica holds this conversation: hand the event to it.
                        Err(ChatError::Denied) => match node.runtime.held_elsewhere(thread) {
                            Some(url) => {
                                node.hand_off_provider_event(&url, connection, event)
                                    .await?
                            }
                            None => return Err(ChatError::Denied),
                        },
                        Err(e) => return Err(e),
                    }
                }
                Ok(None)
            }
        }
    }
    .await;
    match result {
        Ok(Some(challenge)) => {
            axum::Json(serde_json::json!({"challenge":challenge})).into_response()
        }
        Ok(None) => http::StatusCode::NO_CONTENT.into_response(),
        Err(_) => (
            http::StatusCode::SERVICE_UNAVAILABLE,
            "Webhook could not be accepted",
        )
            .into_response(),
    }
}
#[derive(serde::Deserialize)]
struct ProviderChallenge {
    #[serde(rename = "hub.mode")]
    mode: String,
    #[serde(rename = "hub.verify_token")]
    token: String,
    #[serde(rename = "hub.challenge")]
    challenge: String,
}
async fn provider_challenge(
    State(node): State<Node>,
    axum::extract::Path(connection): axum::extract::Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<ProviderChallenge>,
) -> Response {
    use subtle::ConstantTimeEq;
    match node.runtime.connection_access(connection).await {
        Ok((provider, _, access))
            if provider == "whatsapp"
                && query.mode == "subscribe"
                && access.secret("verify_token").is_ok_and(|expected| {
                    bool::from(expected.as_bytes().ct_eq(query.token.as_bytes()))
                }) =>
        {
            query.challenge.into_response()
        }
        _ => http::StatusCode::UNAUTHORIZED.into_response(),
    }
}
