//! Native ingress for a specific agent. Gateway-deployed agents execute here;
//! sidecar-deployed agents have the call executed by the replica that owns the
//! conversation and the answer relayed back unchanged.
use super::Deployments;
use crate::{chat::Chat, error::Error, proto::tilde::agent_event_ingress::v1 as wire};
use axum::{
    extract::{Path, Request, State},
    response::{IntoResponse, Response},
};
use base64::Engine as _;
use secrecy::ExposeSecret;
use tower::ServiceExt;
use uuid::Uuid;
#[derive(Clone)]
struct Public {
    deployments: Deployments,
    chat: Chat,
}
pub fn router(deployments: Deployments, chat: Chat) -> axum::Router {
    axum::Router::new()
        .route("/agents/{agent}/{*path}", axum::routing::any(receive))
        .with_state(Public { deployments, chat })
}
async fn receive(
    State(state): State<Public>,
    Path((agent, path)): Path<(Uuid, String)>,
    request: Request,
) -> Response {
    match process(state, agent, path, request).await {
        Ok(response) => response,
        Err(e) => connectrpc::ConnectError::from(e).into_response(),
    }
}
async fn process(
    state: Public,
    agent: Uuid,
    path: String,
    request: Request,
) -> Result<Response, Error> {
    if !path.starts_with("tilde.provider.tilde.v1.ChatService/") {
        return Ok(http::StatusCode::NOT_FOUND.into_response());
    }
    let mut token = request
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(Error::Denied)?
        .to_owned();
    let claims = if token.starts_with("tilde_chat_") {
        let encoded = request
            .headers()
            .get("x-tilde-identity")
            .and_then(|v| v.to_str().ok())
            .ok_or(Error::Denied)?;
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| Error::Denied)?;
        let identity = std::str::from_utf8(&decoded).map_err(|_| Error::Denied)?;
        let (signed, claims) = state
            .deployments
            .authenticate_tilde(agent, &token, identity)
            .await?;
        token = signed.expose_secret().to_owned();
        claims
    } else {
        super::tokens::verify_ingress(&token, agent, &state.deployments.signing_key(agent).await?)?
    };
    let (mut parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, super::routing::MAX_BODY)
        .await
        .map_err(|_| Error::Invalid("Request exceeds limit".into()))?;
    let bytes = super::routing::plain_body(&mut parts.headers, bytes)?;
    let method = path.rsplit('/').next().ok_or(Error::Denied)?.to_owned();
    let content_type = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_owned();
    super::routing::check_creation_scope(agent, &method, &bytes, &content_type)?;
    let thread = match super::routing::thread(&state.chat, &method, &bytes, &content_type).await? {
        Some(thread) => {
            if claims.thread_id.is_some_and(|scope| scope != thread)
                || !crate::deployment::db::thread_agent_one(
                    &state.deployments.pool.get().await?,
                    thread,
                    agent,
                )
                .await?
                .allowed
            {
                return Err(Error::Denied);
            }
            crate::chat::providers::tilde::credentials::member(&state.chat, &claims, thread)
                .await?;
            Some(thread)
        }
        None if claims.thread_id.is_some() => return Err(Error::Denied),
        None => None,
    };
    // Watch the durable projection at the gateway. A directive carries one bounded
    // response and cannot relay an open stream. Origin-aware cursors also allow
    // replay to move between a replica and this projection without duplicates.
    let central = matches!(
        method.as_str(),
        "CreateThread"
            | "CreateUser"
            | "WatchThread"
            | "SearchMessages"
            | "ListSessions"
            | "ListThreads"
            | "GetSession"
            | "SetReadState"
            | "GetIdentity"
            | "ListAgents"
    );
    if !central
        && let Some(thread_id) = thread
        && state.deployments.is_sidecar(agent, thread_id).await?
    {
        let result = state
            .deployments
            .forward(
                agent,
                thread,
                wire::IngressCall {
                    method,
                    content_type,
                    body: bytes.to_vec(),
                    caller_token: token,
                    ..Default::default()
                },
            )
            .await?;
        return Ok(relay(result));
    }
    parts.uri = format!("/{path}").parse().map_err(|_| Error::Denied)?;
    parts.extensions.insert(claims);
    crate::chat::providers::tilde::rpc::router(state.chat)
        .oneshot(Request::from_parts(parts, axum::body::Body::from(bytes)))
        .await
        .map_err(|_| Error::Denied)
}
/// Reproduce the owner's HTTP answer for the original caller.
pub(crate) fn relay(result: wire::CallResult) -> Response {
    let status =
        http::StatusCode::from_u16(result.status as u16).unwrap_or(http::StatusCode::BAD_GATEWAY);
    let mut response = axum::body::Body::from(result.body).into_response();
    *response.status_mut() = status;
    if let Ok(value) = http::HeaderValue::from_str(&result.content_type) {
        response
            .headers_mut()
            .insert(http::header::CONTENT_TYPE, value);
    }
    response
}
