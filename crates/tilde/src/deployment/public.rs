//! Native ingress for gateway deployments shares the sidecar's operation API.
use super::Deployments;
use crate::{chat::Chat, error::Error};
use axum::{
    extract::{Path, Request, State},
    response::{IntoResponse, Response},
};
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
    if !path.starts_with("tilde.ingress.v1.ChatService/") {
        return Ok(http::StatusCode::NOT_FOUND.into_response());
    }
    let token = request
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(Error::Denied)?;
    let deployment = sqlx::query_file!("../../queries/deployment/get.sql", agent)
        .fetch_optional(&state.deployments.pool)
        .await?
        .ok_or(Error::NotFound)?;
    let secret = state.deployments.open_secrets(
        agent,
        deployment
            .encrypted_secrets
            .as_deref()
            .ok_or(Error::Denied)?,
    )?;
    let claims = super::tokens::verify_ingress(token, agent, &secret.signing_key)?;
    if deployment.deployment_mode != "gateway" {
        return Err(Error::Invalid(
            "Send this agent's requests to its sidecar".into(),
        ));
    }
    let (mut parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 192 * 1024 * 1024)
        .await
        .map_err(|_| Error::Invalid("Request exceeds limit".into()))?;
    let method = path.rsplit('/').next().ok_or(Error::Denied)?;
    let content_type = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    super::routing::check_creation_scope(agent, method, &bytes, content_type)?;
    match super::routing::thread(&state.chat, method, &bytes, content_type).await? {
        Some(thread) => {
            if claims.thread_id.is_some_and(|scope| scope != thread)
                || !sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
                    .fetch_one(&state.deployments.pool)
                    .await?
                    .allowed
            {
                return Err(Error::Denied);
            }
        }
        None if claims.thread_id.is_some() => return Err(Error::Denied),
        None => {}
    }
    parts.uri = format!("/{path}").parse().map_err(|_| Error::Denied)?;
    parts.extensions.insert(claims);
    crate::chat::rpc::ingress::router(state.chat)
        .oneshot(Request::from_parts(parts, axum::body::Body::from(bytes)))
        .await
        .map_err(|_| Error::Denied)
}
