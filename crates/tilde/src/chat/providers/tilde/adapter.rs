//! Catalog adapter for the built-in provider. Applications assert who is talking, so
//! identities are usernames the application vouches for; nothing is verified, no message is
//! ever sent through the connection and the agent gets no channel tools from it.
use super::super::{Access, Adapter, ingress};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::Value;
pub struct Tilde;
impl Adapter for Tilde {
    fn identity_types(&self) -> &'static [types::IdentityType] {
        &[types::IdentityType::Username]
    }
    fn verification_recipient(
        &self,
        identity_type: types::IdentityType,
        value: &str,
    ) -> super::super::ToolResult<crate::chat::access::identity::Identity> {
        if identity_type != types::IdentityType::Username {
            return Err(ConnectError::invalid_argument(
                "Tilde identities are application usernames",
            ));
        }
        let identity = crate::chat::access::identity::Identity {
            identity_type,
            value: value.trim().to_owned(),
        };
        identity.validate()?;
        Ok(identity)
    }
    fn verification_supported(&self) -> bool {
        false
    }
    fn attests_identities(&self) -> bool {
        true
    }
    fn send_verification<'a>(
        &'a self,
        _: &'a Access,
        _: crate::chat::access::identity::VerificationMessage<'a>,
    ) -> BoxFuture<'a, super::super::ToolResult<()>> {
        Box::pin(async { Err(unsupported()) })
    }
    fn tools(&self) -> Vec<types::ToolDefinition> {
        vec![]
    }
    fn webhook<'a>(
        &'a self,
        _: &'a Access,
        _: &'a http::HeaderMap,
        _: &'a [u8],
    ) -> BoxFuture<'a, super::super::ToolResult<ingress::Webhook>> {
        Box::pin(async { Err(unsupported()) })
    }
    fn invoke<'a>(
        &'a self,
        _: &'a Access,
        _: &'a super::super::Context,
        _: &'a str,
        _: Value,
    ) -> BoxFuture<'a, super::super::ToolResult<Value>> {
        Box::pin(async { Err(unsupported()) })
    }
}
fn unsupported() -> ConnectError {
    ConnectError::unimplemented("The Tilde provider has no webhooks or channel tools")
}
