//! Audience-specific contracts share the Chat application service and canonical entities.
pub mod ingress;
pub mod management;
pub mod runtime;
use crate::chat::{Chat, Scope};
use connectrpc::{ConnectError, RequestContext};
pub(crate) fn capability(ctx: &RequestContext) -> std::result::Result<&str, ConnectError> {
    ctx.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ConnectError::unauthenticated("Invocation capability required"))
}
pub(crate) async fn invocation_scope(
    chat: &Chat,
    ctx: &RequestContext,
) -> std::result::Result<Scope, ConnectError> {
    if let Some(scope) = ctx.extensions().get::<Scope>() {
        return Ok(scope.clone());
    }
    Ok(chat.scope(capability(ctx)?).await?)
}
