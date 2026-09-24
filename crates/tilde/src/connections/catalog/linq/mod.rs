use crate::connections::categories::CATEGORY_CHAT;
use crate::connections::model::*;

pub fn definition() -> Provider {
    Provider {
        account_name_label: Some("Linq phone number".into()),
        icon_url: Some("https://skywalker-next.linqapp.com/favicon.ico?v=4".into()),
        instructions: Some(
            "Enter your Linq account credentials to connect messaging to this agent.".into(),
        ),
        id: "linq".into(),
        name: "Linq".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_CHAT.into()],
        connection_types: vec![ConnectionType {
            id: "account".into(),
            name: "Linq account".into(),
            credential_source: CredentialSource::Static {
                schema: serde_json::json!({"type": "object", "additionalProperties": false, "properties": {"api_token": {"type": "string", "title": "API token", "minLength": 1, "writeOnly": true}, "phone_number": {"type": "string", "title": "Sending phone number", "minLength": 1, "pattern": "^\\+[0-9]{7,15}$"}, "webhook_signing_secret": {"type": "string", "title": "Webhook signing secret", "minLength": 1, "writeOnly": true}}, "required": ["api_token", "phone_number", "webhook_signing_secret"]}),
            },
            capabilities: vec![Capability::Channel],
        }],
    }
}

use super::runtime::Runtime;
use crate::{connections::service::Connections, error::Error};
pub(crate) struct Linq;
#[async_trait::async_trait]
impl Runtime for Linq {
    fn channel_identity(
        &self,
        values: &Values,
        _account: Option<&str>,
    ) -> Option<crate::chat::access::identity::Identity> {
        Some(crate::chat::access::identity::Identity {
            identity_type: crate::proto::tilde::types::v1::IdentityType::PhoneNumber,
            value: optional(values, "phone_number")?.to_owned(),
        })
    }

    fn account_name_field(&self, _typ: &ConnectionType) -> Option<&'static str> {
        Some("phone_number")
    }

    fn instructions(&self, _typ: &ConnectionType) -> &'static [&'static str] {
        &[
            "Enter your Linq API token and sending phone number below.",
            "Configure your Linq webhook with the Webhook URL below and enter its signing secret.",
        ]
    }

    async fn validate(
        &self,
        service: &Connections,
        values: &Values,
    ) -> Result<Option<String>, Error> {
        let url = super::url(
            service
                .endpoints
                .get("linq_api", "https://api.linqapp.com/api/partner/v3"),
            &["phone-numbers"],
        )?;
        let token = value(values, "api_token")?;
        service
            .http
            .json(service.http.client.get(url).bearer_auth(token))
            .await?;
        Ok(optional(values, "phone_number").map(str::to_owned))
    }
}
