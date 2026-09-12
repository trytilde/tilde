//! The sidecar owns Corrosion child processes and local agent execution. Its
//! listeners have independent audiences; management routes are never mounted.
use super::{
    corrosion::process::Process,
    gateway,
    runtime::{Runtime, RuntimeOptions},
};
use crate::{
    chat::{Chat, ChatError, Result},
    proto::tilde::agent_event_ingress::v1 as wire,
};
use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
use zeroize::Zeroize;
#[derive(Clone)]
pub struct Node {
    pub runtime: Arc<Runtime>,
    pub gateway: gateway::Client,
    api_token: SecretString,
    corrosion_url: String,
}
pub struct Options {
    pub gateway_url: String,
    pub tokens: Vec<SecretString>,
    pub endpoints: BTreeMap<Uuid, String>,
    pub advertise: IpAddr,
    pub runtime_listen: SocketAddr,
    pub public_ingress_listen: SocketAddr,
    pub agent_ingress_listen: SocketAddr,
    pub gossip_port: u16,
    pub state_directory: PathBuf,
    pub corrosion_binary: PathBuf,
    pub public_url: Option<String>,
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
            advertise: env("ENGINE_SIDECAR_ADVERTISE_ADDRESS")?
                .parse()
                .map_err(|_| ChatError::Invalid("Advertised address must be an IP".into()))?,
            runtime_listen: socket("ENGINE_AGENT_RUNTIME_LISTEN", "0.0.0.0:8081")?,
            public_ingress_listen: socket("ENGINE_PUBLIC_EVENT_INGRESS_LISTEN", "0.0.0.0:8082")?,
            agent_ingress_listen: socket("ENGINE_AGENT_EVENT_INGRESS_LISTEN", "0.0.0.0:8083")?,
            gossip_port: std::env::var("ENGINE_SIDECAR_GOSSIP_PORT")
                .unwrap_or_else(|_| "8787".into())
                .parse()
                .map_err(|_| ChatError::Invalid("Invalid gossip port".into()))?,
            state_directory: std::env::var("ENGINE_SIDECAR_STATE_DIRECTORY")
                .unwrap_or_else(|_| "/var/lib/tilde-sidecar".into())
                .into(),
            corrosion_binary: std::env::var("ENGINE_CORROSION_BINARY")
                .unwrap_or_else(|_| "corrosion".into())
                .into(),
            public_url: std::env::var("ENGINE_PUBLIC_EVENT_INGRESS_PUBLIC_URL").ok(),
        };
        if result.advertise.is_unspecified() {
            return Err(ChatError::Invalid(
                "Advertised address must be reachable".into(),
            ));
        }
        let listeners = [
            result.runtime_listen,
            result.public_ingress_listen,
            result.agent_ingress_listen,
        ];
        for (i, l) in listeners.iter().enumerate() {
            if l.port() != 0 && listeners[..i].contains(l) {
                return Err(ChatError::Invalid(
                    "Sidecar listeners must have distinct addresses".into(),
                ));
            }
        }
        Ok(result)
    }
}
pub async fn run(options: Options) -> Result<()> {
    tokio::fs::create_dir_all(&options.state_directory)
        .await
        .map_err(|_| ChatError::Transport)?;
    // A process restart loses its live executions. A fresh incarnation lets
    // the gateway detect the old owner as unavailable and apply its policy.
    let instance = Uuid::new_v4();
    let runtime_listener = tokio::net::TcpListener::bind(options.runtime_listen)
        .await
        .map_err(|_| ChatError::Transport)?;
    let public_listener = tokio::net::TcpListener::bind(options.public_ingress_listen)
        .await
        .map_err(|_| ChatError::Transport)?;
    let agent_listener = tokio::net::TcpListener::bind(options.agent_ingress_listen)
        .await
        .map_err(|_| ChatError::Transport)?;
    let runtime_port = runtime_listener
        .local_addr()
        .map_err(|_| ChatError::Transport)?
        .port();
    let public_port = public_listener
        .local_addr()
        .map_err(|_| ChatError::Transport)?
        .port();
    let agent_port = agent_listener
        .local_addr()
        .map_err(|_| ChatError::Transport)?
        .port();
    let platform_routes = super::telemetry::PlatformRoutes::default();
    let (platform_provider, platform_worker) = super::telemetry::platform(platform_routes.clone());
    opentelemetry::global::set_tracer_provider(platform_provider.clone());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let mut workers = tokio::task::JoinSet::new();
    let mut runtime_router = Router::new();
    let mut public_router = Router::new();
    let mut agent_router = Router::new();
    let serving = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut readiness = Vec::new();
    let mut registered = std::collections::BTreeSet::new();
    for (index, token) in options.tokens.into_iter().enumerate() {
        let gateway = gateway::Client::new(&options.gateway_url, token)?;
        let mut configuration = gateway.configuration().await?;
        let agent = crate::chat::id(&configuration.agent.id)?;
        if !registered.insert(agent) {
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
        let gossip = SocketAddr::new(
            options.advertise,
            options
                .gossip_port
                .checked_add(
                    u16::try_from(index)
                        .map_err(|_| ChatError::Invalid("Too many agent registrations".into()))?,
                )
                .ok_or_else(|| ChatError::Invalid("Too many gossip ports".into()))?,
        );
        let public_url = format!(
            "{}{prefix}",
            options.public_url.clone().unwrap_or_else(|| format!(
                "http://{}",
                SocketAddr::new(options.advertise, public_port)
            ))
        );
        let registration = wire::RegisterSidecarRequest {
            instance_id: instance.to_string(),
            public_ingress_url: public_url,
            runtime_url: format!(
                "http://{}{prefix}",
                SocketAddr::new(options.advertise, runtime_port)
            ),
            agent_ingress_url: format!(
                "http://{}{prefix}",
                SocketAddr::new(options.advertise, agent_port)
            ),
            gossip_address: gossip.to_string(),
            local_agent_endpoint: endpoint.clone(),
            ..Default::default()
        };
        let mut registration = gateway.register(&registration).await?;
        if registration.schema_revision != super::schema_revision() {
            return Err(ChatError::Invalid(
                "Gateway and sidecar schemas differ; deploy matching binaries".into(),
            ));
        }
        let reserved =
            std::net::TcpListener::bind("127.0.0.1:0").map_err(|_| ChatError::Transport)?;
        let api = reserved.local_addr().map_err(|_| ChatError::Transport)?;
        drop(reserved);
        let api_token = SecretString::from(std::mem::take(&mut registration.corrosion_token));
        let mut process = Process::start(
            options.corrosion_binary.clone(),
            &options.state_directory.join(agent.to_string()),
            api,
            gossip,
            &api_token,
            &registration,
        )
        .await?;
        if let Some(peer) = registration.peers.iter().find(|p| p.ready) {
            let client = super::corrosion::Client::new(
                &format!("{}/corrosion", peer.agent_ingress_url.trim_end_matches('/')),
                api_token.clone(),
            )?;
            process.client.catch_up(&client).await?;
        }
        let encryption = Arc::new(
            crate::encryption::Encryption::from_agent_key(
                agent,
                SecretString::from(std::mem::take(&mut registration.encryption_key)),
            )
            .map_err(|_| ChatError::Transport)?,
        );
        let runtime = Arc::new(
            Runtime::new(
                process.client.clone(),
                RuntimeOptions {
                    agent_id: agent,
                    instance_id: instance,
                    encryption,
                    signing_key: SecretString::from(std::mem::take(
                        &mut registration.token_signing_key,
                    )),
                    host_key: SecretString::from(std::mem::take(
                        &mut configuration.webhook_signing_key,
                    )),
                    local_endpoint: endpoint,
                    callback_url: format!("http://127.0.0.1:{runtime_port}{prefix}"),
                },
            )
            .with_archive(gateway.clone()),
        );
        registration.tls_private_key.zeroize();
        runtime.configure(&configuration).await?;
        super::runtime::providers::clear_secrets(&mut configuration);
        let node = Node {
            runtime: runtime.clone(),
            gateway,
            api_token,
            corrosion_url: format!("http://{api}"),
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
        agent_router = agent_router.nest(&prefix, node.agent_router());
        workers.spawn(node.clone().gateway_commands(rx.clone()));
        let mut shutdown = rx.clone();
        let local = node.clone();
        let worker_runtime = runtime.as_ref().clone();
        workers.spawn(worker_runtime.worker(rx.clone()));
        workers.spawn(runtime.as_ref().clone().retirement_worker(rx.clone()));
        workers.spawn(runtime.as_ref().clone().attachment_worker(rx.clone()));
        workers.spawn(runtime.as_ref().clone().configuration_worker(rx.clone()));
        let serving = serving.clone();
        let healthy = Arc::new(std::sync::atomic::AtomicBool::new(false));
        readiness.push(healthy.clone());
        workers.spawn(async move {
            let mut tick=tokio::time::interval(Duration::from_secs(5));
            loop{tokio::select!{_=shutdown.changed()=>break,_=tick.tick()=>{
                let process_ready=process.supervise().await.is_ok() && serving.load(std::sync::atomic::Ordering::Acquire);
                let began=std::time::Instant::now();
                let health=crate::proto::tilde::agent_host::v1::HealthzRequest::default();
                let agent_ready=match crate::chat::runtime::client(&local.runtime.local_endpoint,local.runtime.host_key.expose_secret(),"Healthz",&health){Ok(client)=>matches!(tokio::time::timeout(Duration::from_secs(3),client.healthz(health)).await,Ok(Ok(value)) if value.view().ready),Err(_)=>false};
                let _=local.runtime.client.transaction(vec![super::corrosion::client::statement("INSERT INTO health(id,agent_id,instance_id,checked_at,healthy,latency_ms) VALUES(?,?,?,?,?,?)",vec![serde_json::json!(Uuid::new_v4()),serde_json::json!(agent),serde_json::json!(instance),serde_json::json!(chrono::Utc::now().timestamp_millis()),serde_json::json!(i32::from(process_ready&&agent_ready)),serde_json::json!(began.elapsed().as_millis().min(i32::MAX as u128) as i32)])]).await;
                healthy.store(process_ready&&agent_ready, std::sync::atomic::Ordering::Release);
                let request=wire::HeartbeatRequest{instance_id:instance.to_string(),ready:process_ready,agent_ready,..Default::default()};
                if local.gateway.heartbeat(&request).await.is_err(){tracing::warn!(agent_id=%agent,"Gateway heartbeat unavailable");}
            }}}
            process.stop().await;
        });
    }
    let health = Router::new().route(
        "/healthz",
        get(move || {
            let readiness = readiness.clone();
            async move {
                if readiness
                    .iter()
                    .all(|r| r.load(std::sync::atomic::Ordering::Acquire))
                {
                    http::StatusCode::OK
                } else {
                    http::StatusCode::SERVICE_UNAVAILABLE
                }
            }
        }),
    );
    runtime_router = runtime_router.merge(health.clone());
    public_router = public_router.merge(health.clone());
    agent_router = agent_router.merge(health);
    serving.store(true, std::sync::atomic::Ordering::Release);
    let server = async {
        tokio::try_join!(
            axum::serve(runtime_listener, runtime_router),
            axum::serve(public_listener, public_router),
            axum::serve(agent_listener, agent_router)
        )
        .map_err(|_| ChatError::Transport)
    };
    tokio::select! {_=tokio::signal::ctrl_c()=>{},result=server=>{result?;},_=workers.join_next()=>return Err(ChatError::Transport)};
    let _ = tokio::task::spawn_blocking(move || platform_provider.shutdown()).await;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), platform_worker).await;
    let _ = stop.send(true);
    while workers.join_next().await.is_some() {}
    Ok(())
}
impl Node {
    pub fn new(
        runtime: Arc<Runtime>,
        gateway: gateway::Client,
        api_token: SecretString,
        corrosion_url: String,
    ) -> Self {
        Self {
            runtime,
            gateway,
            api_token,
            corrosion_url,
        }
    }
    pub fn runtime_router(&self) -> Router {
        crate::chat::rpc::runtime::router(Chat::from_sidecar(self.runtime.clone()))
            .route(
                "/tilde.runtime.v1.AgentService/{*method}",
                any(registry_proxy).with_state(self.clone()),
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
        crate::chat::rpc::ingress::router_with_gateway(
            Chat::from_sidecar(self.runtime.clone()),
            Some(self.gateway.clone()),
        )
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
    }
    pub fn agent_router(&self) -> Router {
        Router::new()
            .route("/bridge", axum::routing::post(bridge_receive))
            .route(
                "/provider-events/{id}",
                axum::routing::post(provider_internal),
            )
            .route(
                "/attachments/{id}",
                get(attachment_get).delete(attachment_delete),
            )
            .route("/corrosion/{*path}", any(corrosion_proxy))
            .with_state(self.clone())
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
fn token_thread(token: &str) -> Option<Uuid> {
    let bytes = URL_SAFE_NO_PAD.decode(token.split('.').nth(1)?).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("thread_id")?.as_str()?.parse().ok()
}
async fn runtime_guard(State(node): State<Node>, mut request: Request, next: Next) -> Response {
    let mut cross_run = false;
    if request.uri().path().ends_with(".ChatService/StartRun") {
        let (parts, body) = request.into_parts();
        let bytes = match axum::body::to_bytes(body, 256 * 1024).await {
            Ok(bytes) => bytes,
            Err(_) => return http::StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        };
        let content_type = parts
            .headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let run: crate::proto::tilde::runtime::v1::StartRunRequest =
            match super::routing::decode(&bytes, content_type) {
                Ok(run) => run,
                Err(e) => return connectrpc::ConnectError::from(e).into_response(),
            };
        cross_run = run.agent_id != node.runtime.agent_id.to_string();
        request = Request::from_parts(parts, axum::body::Body::from(bytes));
    }
    let Some(token) = bearer(&request) else {
        return connectrpc::ConnectError::unauthenticated("Invocation token required")
            .into_response();
    };
    let Some(thread) = token_thread(token) else {
        return connectrpc::ConnectError::unauthenticated("Invalid invocation token")
            .into_response();
    };
    // Signature validation must also precede the absence fallback; the gateway
    // then validates the live invocation against its authoritative storage.
    if node.runtime.verify_token_signature(token).is_err() {
        return connectrpc::ConnectError::unauthenticated("Invalid invocation token")
            .into_response();
    }
    if cross_run {
        return node.gateway.proxy(node.runtime.agent_id, request).await;
    }
    match node.runtime.exists(thread).await {
        Ok(false) => node.gateway.proxy(node.runtime.agent_id, request).await,
        Ok(true) if node.runtime.retiring(thread).await.unwrap_or(false) => {
            node.gateway.proxy(node.runtime.agent_id, request).await
        }
        Ok(true) => match node.runtime.scope(token).await {
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
        },
        Err(e) => connectrpc::ConnectError::from(e).into_response(),
    }
}
async fn public_guard(State(node): State<Node>, request: Request, next: Next) -> Response {
    let Some(token) = bearer(&request) else {
        return connectrpc::ConnectError::unauthenticated("Ingress token required").into_response();
    };
    let claims = match super::tokens::verify_ingress(
        token,
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
    let method = parts.uri.path().rsplit('/').next().unwrap_or("");
    let content_type = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if let Err(e) =
        super::routing::check_creation_scope(node.runtime.agent_id, method, &body, content_type)
    {
        return connectrpc::ConnectError::from(e).into_response();
    }
    let chat = Chat::from_sidecar(node.runtime.clone());
    let thread = super::routing::thread(&chat, method, &body, content_type).await;
    let fallback = match thread {
        Ok(Some(thread)) => {
            if claims.thread_id.is_some_and(|allowed| allowed != thread) {
                return connectrpc::ConnectError::permission_denied(
                    "Credential is scoped to another conversation",
                )
                .into_response();
            }
            match node.runtime.exists(thread).await {
                Ok(exists) => !exists || node.runtime.retiring(thread).await.unwrap_or(false),
                Err(e) => return connectrpc::ConnectError::from(e).into_response(),
            }
        }
        Ok(None) => {
            if claims.thread_id.is_some() {
                return connectrpc::ConnectError::permission_denied(
                    "Credential requires a conversation-scoped operation",
                )
                .into_response();
            }
            false
        }
        Err(ChatError::NotFound) => true,
        Err(e) => return connectrpc::ConnectError::from(e).into_response(),
    };
    parts.extensions.insert(claims);
    let request = Request::from_parts(parts, axum::body::Body::from(body));
    if fallback {
        node.gateway.proxy(node.runtime.agent_id, request).await
    } else {
        next.run(request).await
    }
}

async fn corrosion_proxy(State(node): State<Node>, mut request: Request) -> Response {
    use subtle::ConstantTimeEq;
    if !bearer(&request).is_some_and(|v| {
        bool::from(
            v.as_bytes()
                .ct_eq(node.api_token.expose_secret().as_bytes()),
        )
    }) {
        return http::StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(path) = request.uri().path().strip_prefix("/corrosion/") else {
        return http::StatusCode::NOT_FOUND.into_response();
    };
    if !["v1/queries", "v1/transactions", "v1/subscriptions"]
        .iter()
        .any(|p| path == *p || path.starts_with(&format!("{p}/")))
    {
        return http::StatusCode::NOT_FOUND.into_response();
    }
    let url = format!("{}/{}", node.corrosion_url, path);
    request.headers_mut().remove(http::header::HOST);
    let (parts, body) = request.into_parts();
    match reqwest::Client::new()
        .request(parts.method, url)
        .headers(parts.headers)
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let mut out = axum::body::Body::from_stream(response.bytes_stream()).into_response();
            *out.status_mut() = status;
            for (name, value) in headers.iter() {
                if name != http::header::TRANSFER_ENCODING && name != http::header::CONTENT_LENGTH {
                    out.headers_mut().insert(name, value.clone());
                }
            }
            out
        }
        Err(_) => http::StatusCode::BAD_GATEWAY.into_response(),
    }
}

async fn registry_proxy(State(node): State<Node>, request: Request) -> Response {
    node.gateway.proxy(node.runtime.agent_id, request).await
}

#[derive(serde::Deserialize)]
struct AttachmentQuery {
    thread_id: Uuid,
}
fn control_authorized(node: &Node, headers: &http::HeaderMap) -> bool {
    use subtle::ConstantTimeEq;
    headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|token| {
            bool::from(
                token
                    .as_bytes()
                    .ct_eq(node.api_token.expose_secret().as_bytes()),
            )
        })
}
async fn attachment_get(
    State(node): State<Node>,
    axum::extract::Path(key): axum::extract::Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<AttachmentQuery>,
    headers: http::HeaderMap,
) -> Response {
    if !control_authorized(&node, &headers) {
        return http::StatusCode::UNAUTHORIZED.into_response();
    }
    match node.runtime.download_attachment(query.thread_id, key).await {
        Ok(value) => value.content.into_response(),
        Err(_) => http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
async fn attachment_delete(
    State(node): State<Node>,
    axum::extract::Path(key): axum::extract::Path<Uuid>,
    headers: http::HeaderMap,
) -> Response {
    if !control_authorized(&node, &headers) {
        return http::StatusCode::UNAUTHORIZED.into_response();
    }
    node.runtime.release_attachment(key).await;
    http::StatusCode::NO_CONTENT.into_response()
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
                    if node.runtime.exists(thread).await? && !node.runtime.retiring(thread).await? {
                        node.runtime.ingest_provider(connection, message).await?;
                    } else {
                        let location: wire::LocateConversationResponse = node
                            .gateway
                            .rpc(
                                "LocateConversation",
                                &wire::LocateConversationRequest {
                                    thread_id: thread.to_string(),
                                    ..Default::default()
                                },
                            )
                            .await?;
                        if location.storage == "not_found" {
                            node.runtime.ingest_provider(connection, message).await?;
                        } else {
                            let _: wire::IngestProviderEventResponse = node
                                .gateway
                                .rpc(
                                    "IngestProviderEvent",
                                    &wire::IngestProviderEventRequest {
                                        connection_id: connection.to_string(),
                                        event: crate::proto::tilde::types::v1::ProviderEvent::from(
                                            message,
                                        )
                                        .into(),
                                        ..Default::default()
                                    },
                                )
                                .await?;
                        }
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
async fn provider_internal(
    State(node): State<Node>,
    axum::extract::Path(connection): axum::extract::Path<Uuid>,
    headers: http::HeaderMap,
    axum::Json(event): axum::Json<crate::proto::tilde::types::v1::ProviderEvent>,
) -> Response {
    if !control_authorized(&node, &headers) {
        return http::StatusCode::UNAUTHORIZED.into_response();
    }
    let thread = crate::chat::providers::ingress::stable(
        connection,
        "thread",
        &format!("{}:{}", node.runtime.agent_id, event.thread_id),
    );
    if !node.runtime.exists(thread).await.unwrap_or(false) {
        return http::StatusCode::NOT_FOUND.into_response();
    }
    let message = match crate::chat::providers::ingress::IncomingMessage::try_from(event) {
        Ok(value) => value,
        Err(_) => return http::StatusCode::BAD_REQUEST.into_response(),
    };
    match node.runtime.ingest_provider(connection, message).await {
        Ok(()) => http::StatusCode::NO_CONTENT.into_response(),
        Err(_) => http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

async fn bridge_receive(
    State(node): State<Node>,
    headers: http::HeaderMap,
    axum::Json(snapshot): axum::Json<crate::proto::tilde::types::v1::BridgeSnapshot>,
) -> Response {
    if !control_authorized(&node, &headers) {
        return http::StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(thread) = crate::chat::id(&snapshot.thread.id) else {
        return http::StatusCode::BAD_REQUEST.into_response();
    };
    if !node.runtime.exists(thread).await.unwrap_or(false) {
        let location: Result<wire::LocateConversationResponse> = node
            .gateway
            .rpc(
                "LocateConversation",
                &wire::LocateConversationRequest {
                    thread_id: thread.to_string(),
                    ..Default::default()
                },
            )
            .await;
        match location {
            Ok(value) if value.storage == "postgres" => {
                return http::StatusCode::NO_CONTENT.into_response();
            }
            Ok(value) if value.storage == "corrosion" => {}
            _ => return http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
    }
    match node.runtime.apply_bridge(snapshot).await {
        Ok(()) => http::StatusCode::NO_CONTENT.into_response(),
        Err(_) => http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

async fn control_guard(State(node): State<Node>, request: Request, next: Next) -> Response {
    let Some(token) = bearer(&request) else {
        return http::StatusCode::UNAUTHORIZED.into_response();
    };
    if node.runtime.verify_token_signature(token).is_err() {
        return http::StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(thread) = token_thread(token) else {
        return http::StatusCode::UNAUTHORIZED.into_response();
    };
    match node.runtime.exists(thread).await {
        Ok(true) => next.run(request).await,
        Ok(false) => node.gateway.proxy(node.runtime.agent_id, request).await,
        Err(e) => connectrpc::ConnectError::from(e).into_response(),
    }
}
