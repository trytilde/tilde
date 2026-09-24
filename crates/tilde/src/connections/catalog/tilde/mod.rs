//! The built-in Tilde chat provider as a catalog entry. Every agent owns exactly one
//! `tilde/application` connection, created by the system with the agent and deleted with
//! it; clients cannot start, disconnect or unassign one. Its single sealed value is the
//! application API key that gateway ingress checks before trusting an asserted identity.
use super::runtime::Runtime;
use crate::connections::categories::CATEGORY_CHAT;
use crate::connections::model::*;
use crate::{connections::service::Connections, encryption::Encryption, error::Error};
use secrecy::SecretString;
use uuid::Uuid;

pub const PROVIDER_ID: &str = "tilde";
pub const TYPE_ID: &str = "application";
pub const KEY_FIELD: &str = "api_key";

pub fn definition() -> Provider {
    Provider {
        account_name_label: None,
        icon_url: Some("/tilde-mark.svg".into()),
        instructions: None,
        id: PROVIDER_ID.into(),
        name: "Tilde".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_CHAT.into()],
        connection_types: vec![ConnectionType {
            id: TYPE_ID.into(),
            name: "Application".into(),
            credential_source: CredentialSource::Static {
                schema: serde_json::json!({"type": "object", "additionalProperties": false, "properties": {"api_key": {"type": "string", "title": "API key", "minLength": 1, "writeOnly": true}}, "required": ["api_key"]}),
            },
            capabilities: vec![Capability::Channel],
        }],
    }
}

/// No sending identity and nothing to validate: the connection never talks to a third party.
pub(crate) struct Tilde;
#[async_trait::async_trait]
impl Runtime for Tilde {}

/// The `tilde_chat_` prefix lets ingress tell an application key from a signed token.
pub fn generate_key() -> SecretString {
    SecretString::from(format!(
        "tilde_chat_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    ))
}

/// Create the agent's Tilde connection inside the caller's transaction, private by default.
pub(crate) async fn create(
    tx: &crate::database::Transaction<'_>,
    crypto: &Encryption,
    agent: Uuid,
    agent_name: &str,
) -> Result<Uuid, Error> {
    let id = Uuid::new_v4();
    let mut values = Values::new();
    values.insert(KEY_FIELD.into(), generate_key());
    Connections::create_ready(
        tx,
        crypto,
        id,
        agent_name,
        PROVIDER_ID,
        TYPE_ID,
        &values,
        agent,
    )
    .await?;
    Ok(id)
}
