use crate::connections::categories::CATEGORY_CHAT;
use crate::connections::model::*;

pub fn definition() -> Provider {
    Provider {
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
