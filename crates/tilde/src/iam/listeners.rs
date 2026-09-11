//! Complete API surfaces for each listener, with independent user, invocation and trace authentication.
use crate::{agent::Agents, chat::Chat};
use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};

/// Agent uploads use trace-specific authorization, including the terminal flush window.
/// Keep ingestion outside the RPC guard: terminal tokens must never regain RPC access.
pub fn agent_runtime_router(
    agents: Agents,
    chat: Chat,
    tracing: &crate::telemetry::Tracing,
) -> Router {
    agent_rpc_router(agents, chat).merge(tracing.agent_ingestion_router())
}

/// Invocation RPCs require live invocation state on every request.
pub fn agent_rpc_router(agents: Agents, chat: Chat) -> Router {
    Router::new()
        .merge(crate::agent::rpc::runtime::router(agents, chat.clone()))
        .merge(crate::chat::rpc::runtime::router(chat.clone()))
        .layer(middleware::from_fn_with_state(chat, agent_guard))
}

/// User-facing APIs and provider setup/callbacks. Agent trace ingestion is deliberately absent.
pub fn management_router(
    agents: Agents,
    chat: Chat,
    connections: crate::connections::service::Connections,
    oidc: super::oidc::Oidc,
) -> Router {
    let access = crate::chat::access::AgentAccess::new(connections.clone(), chat.clone());
    Router::new()
        .merge(crate::agent::rpc::management::router(agents))
        .merge(crate::chat::access::rpc::management_router(access.clone()))
        .merge(crate::chat::rpc::management::router(chat.clone()))
        .merge(crate::connections::rpc::management::router(
            connections.clone(),
        ))
        .layer(middleware::from_fn_with_state(
            oidc.clone(),
            super::oidc::management_guard,
        ))
        .merge(oidc.router())
        .merge(crate::chat::access::rpc::public_router(access))
        .merge(crate::connections::rpc::setup::router(connections.clone()))
}
/// Public provider events only. Each adapter authenticates its own webhook signature.
/// Management RPCs, browser setup and OAuth callbacks never mount on this listener.
pub fn event_ingress_router(
    chat: Chat,
    connections: crate::connections::service::Connections,
) -> Router {
    crate::chat::providers::ingress::router(chat, connections)
}
async fn agent_guard(State(chat): State<Chat>, mut request: Request, next: Next) -> Response {
    let token = request
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let Some(token) = token else {
        return connectrpc::ConnectError::unauthenticated("Agent connect token required")
            .into_response();
    };
    match chat.scope(token).await {
        Ok(scope) => {
            use opentelemetry::trace::TraceContextExt;
            let cx = opentelemetry::Context::current();
            for (key, value) in [
                ("tilde.invocation.id", scope.id),
                ("tilde.agent.id", scope.agent_id),
                ("tilde.run.id", scope.run_id),
                ("tilde.thread.id", scope.thread_id),
            ] {
                cx.span()
                    .set_attribute(opentelemetry::KeyValue::new(key, value.to_string()));
            }
            request.extensions_mut().insert(super::Principal::Agent {
                id: scope.agent_id,
                invocation_id: scope.id,
            });
            request.extensions_mut().insert(scope);
            next.run(request).await
        }
        Err(e) => connectrpc::ConnectError::from(e).into_response(),
    }
}
