//! Inference gateway. Agents reach model providers through one route on the runtime listener,
//! `/inference/{provider}/{account}/{*rest}`, speaking each provider's own wire format: an
//! agent points its OpenAI, Anthropic or Vercel AI SDK client at the slug and keeps its code.
//!
//! The hot path does four things and decodes nothing: verify the invocation token (done by the
//! listener guard), check the connection is in the token's `inference` claim, swap the agent's
//! placeholder credential for the real one, and relay bytes both ways. The request body is
//! buffered (SigV4 needs its hash; prompts are small); the response streams through, with each
//! frame's refcount shared into a bounded capture that [`audit`] parses off the request path.
//!
//! Credentials come from the connection cache: on a sidecar the replicated configuration, on the
//! gateway [`gateway::Loader`], both refreshed by the same Postgres notifications. No database
//! or lock is touched per request beyond one read of that map.
//!
//! ponytail: this is a hand-rolled reverse proxy on hyper via axum and reqwest. If proxy
//! overhead ever shows in p99 traces, the filter steps below map one to one onto pingora's
//! `ProxyHttp` phases.
pub mod audit;
pub mod budgets;
pub mod db;
pub mod gateway;
pub mod prices;
mod providers;
pub mod rpc;
pub use providers::Provider;

use crate::chat::Scope;
use crate::connections::model::Values;
use crate::error::Error;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Extension, Path, Request, State},
    response::{IntoResponse, Response},
};
use futures::Stream;
use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, RwLock},
    task::{Context, Poll},
    time::{Duration, Instant},
};
use url::Url;
use uuid::Uuid;

/// Multi-modal requests carry inline media; audio uploads are the largest at about 25 MiB.
pub const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;
/// Response bytes kept for usage parsing. Past this, usage is recorded as truncated.
const CAPTURE_BYTES: usize = 4 * 1024 * 1024;

/// What an allowlisted path does, derived from the path alone so the body is never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Chat,
    Embeddings,
    Rerank,
    CountTokens,
    Image,
    Transcription,
    Speech,
}
impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Embeddings => "embeddings",
            Self::Rerank => "rerank",
            Self::CountTokens => "count_tokens",
            Self::Image => "image",
            Self::Transcription => "transcription",
            Self::Speech => "speech",
        }
    }
    /// OpenAI, Anthropic, Google, Bedrock, Cohere and Voyage paths, by their final segment.
    /// ponytail: Bedrock `invoke` is recorded as chat whatever model it addresses.
    pub fn of(path: &str) -> Option<Self> {
        let mut segments = path.rsplit('/');
        let last = segments.next()?;
        let previous = segments.next();
        let action = last.rsplit_once(':').map(|(_, a)| a).unwrap_or(last);
        Some(match action {
            "completions"
            | "responses"
            | "messages"
            | "chat"
            | "generateContent"
            | "streamGenerateContent"
            | "converse"
            | "converse-stream"
            | "invoke"
            | "invoke-with-response-stream" => Self::Chat,
            "embeddings" | "embed" | "embedContent" | "batchEmbedContents" => Self::Embeddings,
            "rerank" => Self::Rerank,
            "count_tokens" | "countTokens" | "count-tokens" => Self::CountTokens,
            "generations" | "edits" if previous == Some("images") => Self::Image,
            "predict" => Self::Image,
            "transcriptions" | "translations" if previous == Some("audio") => Self::Transcription,
            "speech" if previous == Some("audio") => Self::Speech,
            _ => return None,
        })
    }
}

/// How one connection authenticates upstream.
pub(crate) enum Auth {
    Header(HeaderName, SecretString),
    SigV4 {
        region: String,
        access_key_id: SecretString,
        secret_access_key: SecretString,
        session_token: Option<SecretString>,
    },
}
impl Auth {
    fn header(name: &'static str, value: impl Into<SecretString>) -> Self {
        Self::Header(HeaderName::from_static(name), value.into())
    }
}

/// One ready inference connection with a warm connection pool to its provider.
pub struct Upstream {
    pub connection: Uuid,
    pub provider: Provider,
    pub version: i64,
    pub base: Url,
    auth: Auth,
    client: reqwest::Client,
}
impl Upstream {
    pub fn new(
        connection: Uuid,
        version: i64,
        provider: Provider,
        values: &Values,
    ) -> Result<Self, Error> {
        let (base, auth) = provider.upstream(values)?;
        // Tuned against LiteLLM's defaults (5 s connect, 120 s keep-alive, 600 s completion
        // deadline, no cookies). Differences are deliberate: redirects are never followed because
        // this client injects credentials, a stream is bounded by silence between bytes rather
        // than by total duration, and TCP keep-alive is on so NATs never drop a pooled socket.
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(300))
            .pool_idle_timeout(Duration::from_secs(120))
            .pool_max_idle_per_host(64)
            .tcp_keepalive(Duration::from_secs(30))
            .tcp_nodelay(true)
            .http2_keep_alive_interval(Duration::from_secs(30))
            .http2_keep_alive_while_idle(true)
            .http2_adaptive_window(true)
            .user_agent("tilde-inference")
            .build()
            .map_err(|_| Error::Invalid("Unable to build the provider HTTP client".into()))?;
        Ok(Self {
            connection,
            provider,
            version,
            base,
            auth,
            client,
        })
    }
    fn authorize(&self, request: &mut reqwest::Request, body: &[u8]) -> Result<(), Error> {
        match &self.auth {
            Auth::Header(name, value) => {
                let mut value = HeaderValue::from_str(value.expose_secret())
                    .map_err(|_| Error::Invalid("Invalid credential header".into()))?;
                value.set_sensitive(true);
                request.headers_mut().insert(name.clone(), value);
            }
            Auth::SigV4 {
                region,
                access_key_id,
                secret_access_key,
                session_token,
            } => {
                client_aws_sigv4::Signer::new(
                    client_aws_sigv4::Credentials {
                        access_key_id: access_key_id.expose_secret().to_owned(),
                        secret_access_key: secret_access_key.expose_secret().to_owned(),
                        session_token: session_token.as_ref().map(|t| t.expose_secret().to_owned()),
                    },
                    region.clone(),
                    "bedrock",
                )
                .sign_request_at(request, body, std::time::SystemTime::now())
                .map_err(|_| Error::Invalid("Unable to sign the Bedrock request".into()))?;
            }
        }
        Ok(())
    }
}

/// A connection that should be routable, before it is turned into an [`Upstream`].
pub struct Candidate {
    pub slug: String,
    pub connection: Uuid,
    pub version: i64,
    pub provider: Provider,
    pub values: Values,
    /// Agents assigned this inference connection; slugs are resolved inside their scope.
    pub agents: Vec<Uuid>,
    /// `(agent, alias)` pairs that name this connection.
    pub aliases: Vec<(Uuid, String)>,
}
/// One atomic routing snapshot. Upstreams are reused by connection ID; names and aliases
/// are scoped to an agent, so identical provider/account names cannot cross agent boundaries.
#[derive(Default)]
pub struct Upstreams {
    state: RwLock<Routing>,
}
#[derive(Default)]
struct Routing {
    connections: HashMap<Uuid, Arc<Upstream>>,
    names: HashMap<(Uuid, String), Arc<Upstream>>,
}
impl Upstreams {
    pub fn get(&self, agent: Uuid, provider: &str, account: &str) -> Option<Arc<Upstream>> {
        self.alias(agent, &format!("{provider}/{account}"))
    }
    pub fn alias(&self, agent: Uuid, name: &str) -> Option<Arc<Upstream>> {
        self.state
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .names
            .get(&(agent, name.to_owned()))
            .cloned()
    }
    /// Diagnostic union only; request routing always requires an authenticated agent.
    pub fn slugs(&self) -> Vec<String> {
        let mut slugs: Vec<_> = self
            .state
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .names
            .keys()
            .filter(|(_, name)| name.contains('/'))
            .map(|(_, name)| name.clone())
            .collect();
        slugs.sort();
        slugs.dedup();
        slugs
    }
    pub fn replace(&self, candidates: Vec<Candidate>) {
        let current = self
            .state
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .connections
            .clone();
        let mut next = Routing::default();
        for candidate in candidates {
            let kept = current
                .get(&candidate.connection)
                .filter(|u| u.version == candidate.version)
                .cloned();
            let upstream = match kept {
                Some(upstream) => upstream,
                None => match Upstream::new(
                    candidate.connection,
                    candidate.version,
                    candidate.provider,
                    &candidate.values,
                ) {
                    Ok(upstream) => Arc::new(upstream),
                    Err(error) => {
                        tracing::warn!(slug=%candidate.slug,%error,"Inference connection is not routable");
                        continue;
                    }
                },
            };
            for agent in candidate.agents {
                next.names
                    .insert((agent, candidate.slug.clone()), upstream.clone());
            }
            for (agent, alias) in candidate.aliases {
                next.names.insert((agent, alias), upstream.clone());
            }
            next.connections.insert(candidate.connection, upstream);
        }
        *self.state.write().unwrap_or_else(|e| e.into_inner()) = next;
    }
    /// The sidecar's replicated configuration already carries decrypted credentials.
    pub fn from_configuration(
        config: &crate::proto::tilde::agent_event_ingress::v1::GetConfigurationResponse,
    ) -> Vec<Candidate> {
        config
            .connections
            .iter()
            .filter(|c| {
                c.capability == crate::proto::tilde::types::v1::Capability::Inference
                    && c.status == "ready"
            })
            .filter_map(|c| {
                let agent = Uuid::parse_str(&config.agent.as_option()?.id).ok()?;
                Some(Candidate {
                    slug: format!("{}/{}", c.provider_id, c.name),
                    connection: Uuid::parse_str(&c.id).ok()?,
                    version: c.credential_version,
                    provider: Provider::parse(&c.provider_id)?,
                    agents: vec![agent],
                    aliases: if c.alias.is_empty() {
                        vec![]
                    } else {
                        vec![(agent, c.alias.clone())]
                    },
                    values: c
                        .credentials
                        .iter()
                        .map(|f| (f.name.clone(), SecretString::from(f.value.clone())))
                        .collect(),
                })
            })
            .collect()
    }
}

/// Everything the route needs; cheap to clone into the router state.
#[derive(Clone)]
pub struct Gateway {
    pub upstreams: Arc<Upstreams>,
    pub audit: audit::Audit,
}
pub fn router(gateway: Gateway) -> Router {
    Router::new()
        .route("/inference/{*path}", axum::routing::any(forward))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(gateway)
}

/// Provider SDKs surface a JSON error body; Tilde's own refusals use the same shape.
fn refuse(status: StatusCode, message: &str) -> Response {
    (
        status,
        [(http::header::CONTENT_TYPE, "application/json")],
        serde_json::json!({"error": {"message": message, "type": "tilde_inference_gateway"}})
            .to_string(),
    )
        .into_response()
}
/// Hop-by-hop headers, the agent's placeholder credential in any header a provider SDK might
/// use, encoding negotiation (so usage stays parseable) and Tilde's own trace context.
fn forwardable(name: &HeaderName) -> bool {
    let name = name.as_str();
    !matches!(
        name,
        "host"
            | "authorization"
            | "content-length"
            | "transfer-encoding"
            | "connection"
            | "keep-alive"
            | "te"
            | "trailer"
            | "upgrade"
            | "proxy-authorization"
            | "proxy-connection"
            | "accept-encoding"
            | "x-api-key"
            | "api-key"
            | "x-goog-api-key"
            | "traceparent"
            | "tracestate"
            | "baggage"
            | "forwarded"
            | "x-forwarded-for"
            | "x-forwarded-host"
            | "x-forwarded-proto"
            | "x-real-ip"
    ) && !name.starts_with("x-amz-")
        && !name.starts_with("x-tilde-")
}
fn forwardable_response(name: &HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "transfer-encoding" | "connection" | "keep-alive" | "trailer" | "upgrade" | "set-cookie"
    )
}

async fn forward(
    State(gateway): State<Gateway>,
    Extension(scope): Extension<Scope>,
    Path(path): Path<String>,
    request: Request,
) -> Response {
    // `{provider}/{account}/{provider path}` or `{alias}/{provider path}`: a provider id is
    // never a valid alias, so the first segment decides.
    let mut segments = path.splitn(2, '/');
    let first = segments.next().unwrap_or_default().to_owned();
    let (upstream, rest) = if Provider::parse(&first).is_some() {
        let mut tail = segments.next().unwrap_or_default().splitn(2, '/');
        let account = tail.next().unwrap_or_default();
        let rest = tail.next().unwrap_or_default().to_owned();
        (gateway.upstreams.get(scope.agent_id, &first, account), rest)
    } else {
        (
            gateway.upstreams.alias(scope.agent_id, &first),
            segments.next().unwrap_or_default().to_owned(),
        )
    };
    let Some(kind) = Kind::of(&rest) else {
        return refuse(
            StatusCode::NOT_FOUND,
            "This path is not an inference endpoint",
        );
    };
    if rest
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return refuse(StatusCode::NOT_FOUND, "Invalid inference path");
    }
    let Some(upstream) = upstream else {
        return refuse(
            StatusCode::NOT_FOUND,
            "No ready inference connection with this slug or alias",
        );
    };
    let provider = upstream.provider.id();
    if !scope.inference.contains(&upstream.connection) {
        return refuse(
            StatusCode::FORBIDDEN,
            "This agent is not assigned that inference connection",
        );
    }
    let (parts, body) = request.into_parts();
    let mut url = upstream.base.clone();
    {
        let Ok(mut segments) = url.path_segments_mut() else {
            return refuse(
                StatusCode::BAD_GATEWAY,
                "Provider base URL cannot be extended",
            );
        };
        segments.pop_if_empty();
        segments.extend(rest.split('/'));
    }
    url.set_query(parts.uri.query());
    let Ok(body) = axum::body::to_bytes(body, MAX_REQUEST_BYTES).await else {
        return refuse(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Inference request body is too large",
        );
    };
    let mut headers = HeaderMap::with_capacity(parts.headers.len());
    for (name, value) in &parts.headers {
        if forwardable(name) {
            headers.append(name.clone(), value.clone());
        }
    }
    let started = Instant::now();
    let mut raw = audit::Raw {
        scope: scope.clone(),
        connection: upstream.connection,
        provider: upstream.provider,
        kind,
        path: rest,
        prompts: parts
            .headers
            .get("x-tilde-prompt")
            .and_then(|v| v.to_str().ok())
            .map(audit::prompt_stamps)
            .unwrap_or_default(),
        status: 0,
        started,
        first_byte: None,
        request: body.clone(),
        content_type: None,
        encoded: false,
        response: Vec::new(),
        response_bytes: 0,
        truncated: false,
        complete: false,
    };
    let built = upstream
        .client
        .request(parts.method.clone(), url)
        .headers(headers)
        .body(body.clone())
        .build()
        .map_err(|_| Error::Invalid("Invalid upstream request".into()))
        .and_then(|mut request| upstream.authorize(&mut request, &body).map(|_| request));
    let request = match built {
        Ok(request) => request,
        Err(_) => {
            return refuse(
                StatusCode::BAD_GATEWAY,
                "Unable to prepare the provider request",
            );
        }
    };
    drop(body);
    let response = match upstream.client.execute(request).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(provider=%provider, %error, "Inference provider unreachable");
            raw.status = StatusCode::BAD_GATEWAY.as_u16();
            gateway.audit.record(raw);
            return refuse(StatusCode::BAD_GATEWAY, "Inference provider unreachable");
        }
    };
    raw.status = response.status().as_u16();
    let mut out = Response::builder().status(response.status());
    for (name, value) in response.headers() {
        if forwardable_response(name) {
            out = out.header(name, value);
        }
    }
    raw.content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    raw.encoded = response
        .headers()
        .contains_key(http::header::CONTENT_ENCODING);
    let tee = Tee {
        inner: Box::pin(response.bytes_stream()),
        raw: Some(raw),
        audit: gateway.audit.clone(),
    };
    out.body(Body::from_stream(tee))
        .unwrap_or_else(|_| refuse(StatusCode::BAD_GATEWAY, "Invalid provider response"))
}

/// Relays upstream frames untouched while sharing each `Bytes` (a refcount, not a copy) into
/// the capture. Finishing or dropping, including on client disconnect, hands the capture to
/// the audit queue exactly once.
struct Tee {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    raw: Option<audit::Raw>,
    audit: audit::Audit,
}
impl Tee {
    fn finish(&mut self) {
        if let Some(raw) = self.raw.take() {
            self.audit.record(raw);
        }
    }
}
impl Stream for Tee {
    type Item = Result<Bytes, reqwest::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(raw) = self.raw.as_mut() {
                    if raw.first_byte.is_none() {
                        raw.first_byte = Some(raw.started.elapsed());
                    }
                    raw.response_bytes += frame.len() as u64;
                    if raw.response_bytes as usize <= CAPTURE_BYTES {
                        raw.response.push(frame.clone());
                    } else {
                        raw.truncated = true;
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(error))) => {
                self.finish();
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                if let Some(raw) = self.raw.as_mut() {
                    raw.complete = true;
                }
                self.finish();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
impl Drop for Tee {
    fn drop(&mut self) {
        self.finish();
    }
}
