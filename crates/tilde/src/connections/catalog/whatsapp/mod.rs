use crate::connections::categories::CATEGORY_CHAT;
use crate::connections::model::*;

pub fn definition() -> Provider {
    Provider {
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
    async fn validate(
        &self,
        service: &Connections,
        values: &Values,
    ) -> Result<Option<String>, Error> {
        let url = super::url(
            service
                .endpoints
                .get("meta_api", "https://graph.facebook.com/v21.0"),
            &[value(values, "phone_number_id")?],
        )?;
        let token = value(values, "access_token")?;
        service
            .http
            .json(service.http.client.get(url).bearer_auth(token))
            .await?;
        Ok(optional(values, "phone_number_id").map(str::to_owned))
    }
}
