use crate::connections::categories::CATEGORY_CHAT;
use crate::connections::model::*;

pub fn definition() -> Provider {
    Provider {
        account_name_label: Some("Telnyx WhatsApp phone number".into()),
        icon_url: Some("https://telnyx.com/favicon.ico".into()),
        instructions: Some(
            "Enter your Telnyx API key and WhatsApp account details to connect this agent.".into(),
        ),
        id: "telnyx".into(),
        name: "Telnyx WhatsApp".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_CHAT.into()],
        connection_types: vec![ConnectionType {
            id: "whatsapp".into(),
            name: "Telnyx WhatsApp".into(),
            credential_source: CredentialSource::Static {
                schema: serde_json::json!({"type": "object", "additionalProperties": false, "properties": {"api_key": {"type": "string", "title": "API key", "minLength": 1, "writeOnly": true}, "phone_number": {"type": "string", "title": "Sending phone number", "minLength": 1, "pattern": "^\\+[0-9]{7,15}$"}, "messaging_profile_id": {"type": "string", "title": "Messaging profile ID", "minLength": 1}, "public_key": {"type": "string", "title": "Ed25519 public key", "minLength": 1}}, "required": ["api_key", "phone_number", "messaging_profile_id", "public_key"]}),
            },
            capabilities: vec![Capability::Channel],
        }],
    }
}

use super::runtime::Runtime;
use crate::{connections::service::Connections, error::Error};
pub(crate) struct Telnyx;
#[async_trait::async_trait]
impl Runtime for Telnyx {
    fn account_name_field(&self, _typ: &ConnectionType) -> Option<&'static str> {
        Some("phone_number")
    }

    fn instructions(&self, _typ: &ConnectionType) -> &'static [&'static str] {
        &[
            "Enter your Telnyx API key, sending phone number, messaging profile ID, and public key below.",
            "Configure your Telnyx webhook with the Webhook URL below.",
        ]
    }

    async fn validate(
        &self,
        service: &Connections,
        values: &Values,
    ) -> Result<Option<String>, Error> {
        use base64::Engine;
        if base64::engine::general_purpose::STANDARD
            .decode(value(values, "public_key")?)
            .ok()
            .is_none_or(|v| v.len() != 32)
        {
            return Err(invalid("Telnyx public key must encode 32 Ed25519 bytes"));
        }
        let url = super::url(
            service
                .endpoints
                .get("telnyx_api", "https://api.telnyx.com/v2"),
            &["messaging_profiles", value(values, "messaging_profile_id")?],
        )?;
        let token = value(values, "api_key")?;
        service
            .http
            .json(service.http.client.get(url).bearer_auth(token))
            .await?;
        Ok(optional(values, "phone_number").map(str::to_owned))
    }
}
