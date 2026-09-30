//! Complete API surfaces for each listener. Agent RPCs and trace ingestion authenticate their own
//! tokens; management routes are unauthenticated and belong behind the operator's proxy.
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
    tracing: &crate::telemetry::tracing::Tracing,
) -> Router {
    agent_rpc_router(agents, chat).merge(tracing.agent_ingestion_router())
}

/// Invocation RPCs validate signed claims and expiry; renewal checks current live state.
pub fn agent_rpc_router(agents: Agents, chat: Chat) -> Router {
    let mut router = Router::new()
        .merge(crate::agent::rpc::runtime::router(agents, chat.clone()))
        .merge(crate::chat::rpc::runtime::router(chat.clone()));
    // Prompts and skills live in Postgres; a sidecar relays these RPCs to the gateway instead.
    if let Ok(pool) = chat.pg() {
        router = router
            .merge(crate::prompts::rpc::runtime_router(
                crate::prompts::Prompts::new(pool.clone()),
                chat.clone(),
            ))
            .merge(crate::skills::rpc::runtime_router(
                chat.skills().expect("Postgres chat has skills"),
                chat.clone(),
            ));
    }
    // Model calls ride the same invocation token as every other runtime RPC.
    if let Some(inference) = chat.inference.clone() {
        router = router.merge(crate::inference::router(inference));
    }
    let mut router = router
        .layer(middleware::from_fn_with_state(chat.clone(), agent_guard))
        .merge(crate::chat::controls::router(chat.clone()));
    // Tool hosts dial in with their own tokens.
    if let Some(tools) = &chat.tools {
        router = router.merge(crate::tools::hosts::router(tools.hosts.clone()));
    }
    // Hosts that dial in: deployment-token Watch/Heartbeat and capability-scoped Report.
    if let Some(deployments) = chat.deployments.clone() {
        router = router.merge(crate::deployment::run::router(deployments, chat));
    }
    router
}

/// Management APIs and provider setup/callbacks. Agent trace ingestion is deliberately absent.
pub fn management_router(
    agents: Agents,
    chat: Chat,
    connections: crate::connections::service::Connections,
) -> Router {
    let deployments = crate::deployment::Deployments::new(
        chat.pg()
            .expect("management routes require Postgres")
            .clone(),
        chat.encryption.clone(),
        agents.clone(),
        connections.clone(),
    )
    .with_skills(chat.skills().expect("management routes require Postgres"));
    let access = crate::chat::access::AgentAccess::new(connections.clone(), chat.clone());
    Router::new()
        .merge(crate::identities::Identities::new(connections.clone()).router())
        .merge(crate::agent::rpc::management::router(agents))
        .merge(crate::deployment::rpc::management_router(deployments))
        .merge(crate::chat::access::rpc::management_router(access.clone()))
        .merge(crate::chat::providers::tilde::management_router(
            chat.clone(),
        ))
        .merge(crate::connections::rpc::management::router(
            connections.clone(),
        ))
        .merge(crate::tools::rpc::router(
            chat.tools
                .clone()
                .unwrap_or_else(|| crate::tools::Tools::new(connections.clone())),
        ))
        .merge(crate::inference::rpc::router(
            chat.pg()
                .expect("management routes require Postgres")
                .clone(),
        ))
        .merge(crate::prompts::rpc::management_router(
            crate::prompts::Prompts::new(
                chat.pg()
                    .expect("management routes require Postgres")
                    .clone(),
            ),
        ))
        .merge(crate::skills::rpc::management_router(
            chat.skills().expect("management routes require Postgres"),
        ))
        .merge(crate::chat::access::rpc::public_router(access))
        .merge(crate::connections::rpc::setup::router(connections.clone()))
}
/// Public provider events only. Each adapter authenticates its own webhook signature.
/// Management RPCs, browser setup and OAuth callbacks never mount on this listener.
pub fn public_event_ingress_router(
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
