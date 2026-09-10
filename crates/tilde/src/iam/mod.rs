//! Installation IAM: human OIDC sessions and signed, invocation-scoped agent credentials.
//! Human users have unrestricted authority; there is deliberately no role model.
pub mod capabilities;
pub mod oidc;
pub mod tokens;
use uuid::Uuid;
#[derive(Debug, Clone)]
pub enum Principal {
    User { id: Uuid },
    Agent { id: Uuid, invocation_id: Uuid },
}

pub mod listeners;
