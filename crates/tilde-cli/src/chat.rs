//! `tilde dev`'s local chat page: a static page plus a reverse proxy to the agent's Tilde chat
//! ingress. The application key stays in this process, exactly as a real application's server
//! keeps it; the browser only ever talks to 127.0.0.1 and never sees a credential.
use anyhow::{Context, Result};
use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use std::{net::SocketAddr, sync::Arc, time::Duration};

const PAGE: &str = include_str!("assets/chat.html");
const SERVICE: &str = "tilde.provider.tilde.v1.ChatService";

struct Ingress {
    http: reqwest::Client,
    base: String,
    api_key: String,
    identity: String,
    agent_id: String,
}

/// Serve the page on `address` and return the bound address (port 0 picks a free one).
pub async fn serve(
    gateway: &str,
    agent_id: &str,
    api_key: String,
    identity: &str,
    address: SocketAddr,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let state = Arc::new(Ingress {
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(120))
            .build()?,
        base: format!("{gateway}/agents/{agent_id}"),
        api_key,
        identity: URL_SAFE_NO_PAD.encode(identity),
        agent_id: agent_id.to_string(),
    });
    let app = Router::new()
        .route("/", get(page))
        .route("/agent", get(agent))
        .route("/rpc/{method}", post(rpc))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("Could not listen on {address}"))?;
    let bound = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok((bound, handle))
}

async fn page() -> Html<&'static str> {
    Html(PAGE)
}

/// The page needs the agent id to open a thread; it is not a secret.
async fn agent(State(state): State<Arc<Ingress>>) -> Response {
    axum::Json(serde_json::json!({ "agentId": state.agent_id })).into_response()
}

async fn rpc(
    State(state): State<Arc<Ingress>>,
    Path(method): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Only forward this service's own unary methods, and only from the local page.
    if !method.chars().all(|c| c.is_ascii_alphanumeric()) {
        return (StatusCode::BAD_REQUEST, "unknown method").into_response();
    }
    let mut request = state
        .http
        .post(format!("{}/{SERVICE}/{method}", state.base))
        .header(header::AUTHORIZATION, format!("Bearer {}", state.api_key))
        .header("x-tilde-identity", state.identity.clone())
        .header(header::CONTENT_TYPE, "application/json")
        .header("connect-protocol-version", "1")
        .body(body);
    if let Some(timeout) = headers.get("connect-timeout-ms") {
        request = request.header("connect-timeout-ms", timeout.clone());
    }
    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let payload = response.bytes().await.unwrap_or_default();
            let mut out = Response::new(axum::body::Body::from(payload));
            *out.status_mut() = status;
            out.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            out.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            out
        }
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            format!("Tilde chat ingress is unreachable: {error}"),
        )
            .into_response(),
    }
}

/// One call to the chat ingress whose only purpose is to make the channel record the identity.
/// A refusal is the expected outcome, so the result is deliberately ignored.
pub async fn probe_identity(gateway: &str, agent_id: &str, api_key: &str, identity: &str) {
    let Ok(http) = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
    else {
        return;
    };
    let _ = http
        .post(format!("{gateway}/agents/{agent_id}/{SERVICE}/GetIdentity"))
        .header(header::AUTHORIZATION, format!("Bearer {api_key}"))
        .header("x-tilde-identity", URL_SAFE_NO_PAD.encode(identity))
        .header(header::CONTENT_TYPE, "application/json")
        .header("connect-protocol-version", "1")
        .body("{}")
        .send()
        .await;
}
