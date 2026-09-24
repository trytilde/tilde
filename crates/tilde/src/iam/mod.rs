//! Installation IAM: human OIDC sessions and signed, invocation-scoped agent credentials.
//! Human users and management API keys are authorized per resource through groups and grants;
//! see `authz`.
pub mod api_keys;
pub mod authz;
pub mod capabilities;
pub mod db;
pub mod oidc;
pub mod service;
pub mod tokens;
use uuid::Uuid;
/// The runtime caller: an agent acting under one invocation. Management callers are
/// `authz::Access`, and the two never share a listener.
#[derive(Debug, Clone)]
pub enum Principal {
    Agent { id: Uuid, invocation_id: Uuid },
}

pub mod listeners;
pub fn parse_id(value: &str) -> Result<Uuid, connectrpc::ConnectError> {
    Uuid::parse_str(value).map_err(|_| connectrpc::ConnectError::invalid_argument("Invalid UUID"))
}
