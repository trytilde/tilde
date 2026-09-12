//! The agent-event-ingress listener accepts authenticated sidecar fallback.
//! Retired conversations stay in Postgres; live misses route to a Corrosion peer.
use super::Deployments;
use crate::iam::capabilities::Capabilities;
use crate::{chat::Chat, error::Error};
use axum::{
    Router,
    extract::{Path, Request, State},
    response::{IntoResponse, Response},
    routing::any,
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sqlx::types::Json;
use tower::ServiceExt;
use uuid::Uuid;
#[derive(Clone)]
pub struct Proxy {
    pub service: Deployments,
    pub chat: Chat,
}
pub fn router(service: Deployments, chat: Chat) -> Router {
    Router::new()
        .route("/agents/{agent}/proxy/{*path}", any(receive))
        .with_state(Proxy { service, chat })
}
#[derive(serde::Serialize, Deserialize)]
pub(crate) struct RuntimeClaims {
    pub iss: String,
    pub aud: String,
    pub sub: Uuid,
    pub invocation_id: Uuid,
    pub run_id: Uuid,
    pub thread_id: Uuid,
    pub capabilities: crate::iam::capabilities::Capabilities,
    pub iat: i64,
    pub exp: i64,
    pub assignment_generation: u64,
    pub agent_generation: i64,
}
pub(crate) fn verify_runtime(
    token: &str,
    agent: Uuid,
    key: &SecretString,
) -> Result<RuntimeClaims, Error> {
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_issuer(&["tilde:invocation"]);
    validation.set_audience(&["tilde:agent-api"]);
    let claims = jsonwebtoken::decode::<RuntimeClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(key.expose_secret().as_bytes()),
        &validation,
    )
    .map_err(|_| Error::Denied)?
    .claims;
    if claims.sub != agent {
        return Err(Error::Denied);
    }
    Ok(claims)
}
async fn receive(
    State(state): State<Proxy>,
    Path((agent, path)): Path<(Uuid, String)>,
    request: Request,
) -> Response {
    match process(state, agent, path, request).await {
        Ok(response) => response,
        Err(error) => connectrpc::ConnectError::from(error).into_response(),
    }
}
async fn process(
    state: Proxy,
    agent: Uuid,
    path: String,
    request: Request,
) -> Result<Response, Error> {
    let credential = request
        .headers()
        .get("x-tilde-sidecar-token")
        .and_then(|h| h.to_str().ok())
        .ok_or(Error::Denied)?;
    if state.service.authenticate(credential).await? != agent {
        return Err(Error::Denied);
    }
    if matches!(path.as_str(), "v1/traces" | "v1/logs") {
        return state.service.trace_proxy(agent, request).await;
    }
    let token = request
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(Error::Denied)?;
    let deployment = sqlx::query_file!("../../queries/deployment/get.sql", agent)
        .fetch_one(&state.service.pool)
        .await?;
    let secret = state.service.open_secrets(
        agent,
        deployment
            .encrypted_secrets
            .as_deref()
            .ok_or(Error::Denied)?,
    )?;
    let is_runtime = path.starts_with("tilde.runtime.v1.");
    if !is_runtime && !path.starts_with("tilde.ingress.v1.") {
        return Err(Error::Denied);
    }
    let runtime_claims = if is_runtime {
        Some(verify_runtime(token, agent, &secret.signing_key)?)
    } else {
        None
    };
    // Revoked executions must still observe and acknowledge their own stop.
    // The control service performs its terminal-only authorization separately.
    if !path.starts_with("tilde.runtime.v1.InvocationControlService/")
        && runtime_claims
            .as_ref()
            .is_some_and(|c| c.agent_generation != deployment.generation)
    {
        return Err(Error::Denied);
    }
    let ingress_claims = if !is_runtime {
        Some(super::tokens::verify_ingress(
            token,
            agent,
            &secret.signing_key,
        )?)
    } else {
        None
    };
    let registry = path.starts_with("tilde.runtime.v1.AgentService/");
    let retained_authority = if registry || path == "tilde.runtime.v1.ChatService/StartRun" {
        let thread = runtime_claims.as_ref().ok_or(Error::Denied)?.thread_id;
        sqlx::query_file!(
            "../../queries/deployment/project/location.sql",
            thread,
            agent
        )
        .fetch_optional(&state.service.pool)
        .await?
        .is_some_and(|r| r.storage == "corrosion")
    } else {
        false
    };
    if retained_authority {
        let peer = state
            .service
            .peer(agent)
            .await?
            .ok_or_else(|| Error::Invalid("Agent sidecar is unavailable".into()))?;
        let mut scope = peer.scope(token).await?;
        scope.capabilities = scope
            .capabilities
            .intersect(&state.service.agents.get(agent).await?.capabilities.0);
        let (mut parts, body) = request.into_parts();
        parts.uri = format!("/{path}").parse().map_err(|_| Error::Denied)?;
        parts.headers.remove("x-tilde-sidecar-token");
        parts.extensions.insert(scope);
        let router = if registry {
            crate::agent::rpc::runtime::router(state.service.agents, state.chat)
        } else {
            crate::chat::rpc::runtime::router(state.chat.with_deployments(state.service))
        };
        return router
            .oneshot(Request::from_parts(parts, body))
            .await
            .map_err(|_| Error::Denied);
    }
    let (mut parts, body) = request.into_parts();
    parts.headers.remove("x-tilde-sidecar-token");
    parts.headers.remove(http::header::HOST);
    parts.uri = format!("/{path}").parse().map_err(|_| Error::Denied)?;
    let mut body = if is_runtime {
        body
    } else {
        let bytes = axum::body::to_bytes(body, 192 * 1024 * 1024)
            .await
            .map_err(|_| Error::Invalid("Request exceeds limit".into()))?;
        axum::body::Body::from(bytes)
    };
    let thread = if let Some(claims) = &runtime_claims {
        claims.thread_id
    } else {
        let bytes = axum::body::to_bytes(body, 192 * 1024 * 1024)
            .await
            .map_err(|_| Error::Invalid("Invalid request".into()))?;
        let method = path.rsplit('/').next().unwrap_or("");
        let content_type = parts
            .headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        super::routing::check_creation_scope(agent, method, &bytes, content_type)?;
        let target = super::routing::thread(&state.chat, method, &bytes, content_type)
            .await?
            .ok_or_else(|| Error::Invalid("Fallback requires an existing conversation".into()))?;
        if ingress_claims
            .as_ref()
            .and_then(|c| c.thread_id)
            .is_some_and(|scope| scope != target)
        {
            return Err(Error::Denied);
        }
        body = axum::body::Body::from(bytes);
        target
    };
    if !sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
        .fetch_one(&state.service.pool)
        .await?
        .allowed
    {
        return Err(Error::NotFound);
    }
    let location = sqlx::query_file!(
        "../../queries/deployment/project/location.sql",
        thread,
        agent
    )
    .fetch_optional(&state.service.pool)
    .await?;
    if location.as_ref().is_some_and(|r| r.storage == "retiring") {
        return Ok((
            http::StatusCode::SERVICE_UNAVAILABLE,
            "Conversation retirement is completing; retry shortly",
        )
            .into_response());
    }
    if location.is_some_and(|r| r.storage == "corrosion") {
        return forward_live(
            &state.service,
            agent,
            thread,
            is_runtime,
            Request::from_parts(parts, body),
        )
        .await;
    }
    if path.starts_with("tilde.runtime.v1.InvocationControlService/") {
        let claims = runtime_claims.ok_or(Error::Denied)?;
        let token = state
            .chat
            .tokens
            .sign_for_proxy(crate::iam::tokens::Claims {
                iss: claims.iss,
                aud: claims.aud,
                sub: agent,
                invocation_id: claims.invocation_id,
                thread_id: claims.thread_id,
                run_id: claims.run_id,
                capabilities: claims.capabilities,
                iat: claims.iat,
                exp: claims.exp,
            })
            .await?;
        let mut header = http::HeaderValue::from_str(&format!("Bearer {}", token.expose_secret()))
            .map_err(|_| Error::Denied)?;
        header.set_sensitive(true);
        parts.headers.insert(http::header::AUTHORIZATION, header);
        return crate::chat::controls::router(state.chat)
            .oneshot(Request::from_parts(parts, body))
            .await
            .map_err(|_| Error::Denied);
    }
    let router = if let Some(claims) = runtime_claims {
        let row = sqlx::query_file!(
            "../../queries/deployment/proxy_scope.sql",
            claims.invocation_id,
            agent
        )
        .fetch_optional(&state.service.pool)
        .await?
        .ok_or(Error::Denied)?;
        if row.thread_id != claims.thread_id
            || row.run_id != claims.run_id
            || row.status != "running"
            || row
                .lease_expires_at
                .is_none_or(|until| until < chrono::Utc::now())
        {
            return Err(Error::Denied);
        }
        let owner = sqlx::query_file!("../../queries/deployment/current_owner.sql", thread, agent)
            .fetch_optional(&state.service.pool)
            .await?
            .ok_or(Error::Denied)?;
        if owner.generation != claims.assignment_generation as i64 || owner.stopped {
            return Err(Error::Denied);
        }
        let caps = claims.capabilities.intersect(&row.capabilities.0);
        let token = state
            .chat
            .tokens
            .sign_for_proxy(crate::iam::tokens::Claims {
                iss: claims.iss,
                aud: claims.aud,
                sub: agent,
                invocation_id: row.id,
                run_id: row.run_id,
                thread_id: thread,
                capabilities: caps,
                iat: claims.iat,
                exp: claims.exp,
            })
            .await?;
        let mut value = http::HeaderValue::from_str(&format!("Bearer {}", token.expose_secret()))
            .map_err(|_| Error::Denied)?;
        value.set_sensitive(true);
        parts.headers.insert(http::header::AUTHORIZATION, value);
        if registry {
            crate::agent::rpc::runtime::router(state.service.agents, state.chat)
        } else {
            crate::chat::rpc::runtime::router(state.chat)
        }
    } else {
        parts
            .extensions
            .insert(ingress_claims.ok_or(Error::Denied)?);
        crate::chat::rpc::ingress::router(state.chat)
    };
    router
        .oneshot(Request::from_parts(parts, body))
        .await
        .map_err(|_| Error::Invalid("Proxy dispatch failed".into()))
}
async fn forward_live(
    service: &Deployments,
    agent: Uuid,
    thread: Uuid,
    runtime: bool,
    request: Request,
) -> Result<Response, Error> {
    let row = sqlx::query_file!("../../queries/deployment/get.sql", agent)
        .fetch_one(&service.pool)
        .await?;
    let secrets = service.open_secrets(
        agent,
        row.encrypted_secrets.as_deref().ok_or(Error::Denied)?,
    )?;
    let nodes = sqlx::query_file!("../../queries/deployment/project/node_available.sql", agent)
        .fetch_all(&service.pool)
        .await?;
    for node in nodes {
        let peer = super::corrosion::Client::new(
            &format!("{}/corrosion", node.agent_ingress_url.trim_end_matches('/')),
            secrets.api_token.clone(),
        )?;
        if peer
            .query::<super::runtime::IdRow>(
                "SELECT id FROM threads WHERE id=?",
                vec![serde_json::json!(thread)],
            )
            .await
            .is_ok_and(|r| !r.is_empty())
        {
            let (parts, body) = request.into_parts();
            let endpoint = if runtime {
                node.runtime_url
            } else {
                node.public_ingress_url
            };
            let url = format!("{}{}", endpoint.trim_end_matches('/'), parts.uri);
            let response = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| Error::Denied)?
                .request(parts.method, url)
                .headers(parts.headers)
                .body(reqwest::Body::wrap_stream(body.into_data_stream()))
                .send()
                .await
                .map_err(|_| Error::Invalid("Live conversation peer is unavailable".into()))?;
            let status = response.status();
            let headers = response.headers().clone();
            let mut out = axum::body::Body::from_stream(response.bytes_stream()).into_response();
            *out.status_mut() = status;
            for (name, value) in headers.iter() {
                if name != http::header::TRANSFER_ENCODING && name != http::header::CONTENT_LENGTH {
                    out.headers_mut().insert(name, value.clone());
                }
            }
            return Ok(out);
        }
    }
    Ok((
        http::StatusCode::SERVICE_UNAVAILABLE,
        "Live conversation has not reached an available peer",
    )
        .into_response())
}
