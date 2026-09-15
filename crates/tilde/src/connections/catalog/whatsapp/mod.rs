use crate::connections::categories::CATEGORY_CHAT;
use crate::connections::model::*;

pub fn definition() -> Provider {
    Provider {
        account_name_label: Some("WhatsApp phone number".into()),
        icon_url: Some(
            "https://cdn.jsdelivr.net/gh/glincker/thesvg@main/public/icons/whatsapp/default.svg"
                .into(),
        ),
        instructions: Some("Connect your Meta WhatsApp Business account to this agent.".into()),
        id: "whatsapp".into(),
        name: "WhatsApp".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_CHAT.into()],
        connection_types: vec![ConnectionType {
            id: "meta".into(),
            name: "Meta WhatsApp Business".into(),
            credential_source: CredentialSource::Static {
                schema: serde_json::json!({"type": "object", "additionalProperties": false, "properties": {"access_token": {"type": "string", "title": "Access token", "minLength": 1, "writeOnly": true}, "app_secret": {"type": "string", "title": "Meta app secret", "minLength": 1, "writeOnly": true}, "verify_token": {"type": "string", "title": "Webhook verify token", "minLength": 1, "writeOnly": true}, "phone_number_id": {"type": "string", "title": "Phone number ID", "minLength": 1}, "waba_id": {"type": "string", "title": "Business account ID", "minLength": 1}}, "required": ["access_token", "app_secret", "verify_token", "phone_number_id", "waba_id"]}),
            },
            capabilities: vec![Capability::Channel],
        }],
    }
}

use super::runtime::Runtime;
use crate::{connections::service::Connections, error::Error};
pub(crate) struct Whatsapp;
#[async_trait::async_trait]
impl Runtime for Whatsapp {
    fn channel_identity(
        &self,
        _values: &Values,
        account: Option<&str>,
    ) -> Option<crate::chat::access::identity::Identity> {
        let number = account?.trim();
        // Old account labels held a Graph ID, not a dialable address. Reconnect those
        // channels to fetch the real number rather than misrepresenting the Graph ID.
        if !number.starts_with('+') {
            return None;
        }
        Some(crate::chat::access::identity::Identity {
            identity_type: crate::proto::tilde::types::v1::IdentityType::PhoneNumber,
            value: number.to_owned(),
        })
    }

    fn instructions(&self, _typ: &ConnectionType) -> &'static [&'static str] {
        &[
            "Enter the credentials and IDs for your Meta WhatsApp Business account below.",
            "Configure your Meta app webhook with the Webhook URL below and the same verify token you enter here.",
        ]
    }

    async fn validate(
        &self,
        service: &Connections,
        values: &Values,
    ) -> Result<Option<String>, Error> {
        let mut url = super::url(
            service
                .endpoints
                .get("meta_api", "https://graph.facebook.com/v21.0"),
            &[value(values, "phone_number_id")?],
        )?;
        url.query_pairs_mut()
            .append_pair("fields", "display_phone_number");
        let token = value(values, "access_token")?;
        let response = service
            .http
            .json(service.http.client.get(url).bearer_auth(token))
            .await?;
        let digits: String = response
            .text("/display_phone_number")?
            .chars()
            .filter(char::is_ascii_digit)
            .collect();
        if !(7..=15).contains(&digits.len()) {
            return Err(invalid("Invalid WhatsApp sending phone number"));
        }
        Ok(Some(format!("+{digits}")))
    }
}
