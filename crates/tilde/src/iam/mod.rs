//! Agent runtime authorization: typed capability grants and signed, invocation-scoped agent
//! credentials. Management listeners are unauthenticated; operators put their own proxy in front.
pub mod capabilities;
pub mod db;
pub mod listeners;
pub mod tokens;
use uuid::Uuid;
/// The runtime caller: an agent acting under one invocation.
#[derive(Debug, Clone)]
pub enum Principal {
    Agent { id: Uuid, invocation_id: Uuid },
}

pub fn parse_id(value: &str) -> Result<Uuid, connectrpc::ConnectError> {
    Uuid::parse_str(value).map_err(|_| connectrpc::ConnectError::invalid_argument("Invalid UUID"))
}
