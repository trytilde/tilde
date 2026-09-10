use crate::connections::categories::CATEGORY_EMAIL;
use crate::connections::model::*;

pub fn definition() -> Provider {
    Provider {
        id: "agentmail".into(),
        name: "AgentMail".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_EMAIL.into()],
        connection_types: vec![ConnectionType {
            id: "inbox".into(),
            name: "AgentMail inbox".into(),
            credential_source: CredentialSource::Static {
                schema: serde_json::json!({"type": "object", "additionalProperties": false, "properties": {"inbox_id": {"type": "string", "title": "Inbox ID", "minLength": 1}, "api_key": {"type": "string", "title": "API key", "minLength": 1, "writeOnly": true}, "webhook_secret": {"type": "string", "title": "Webhook signing secret", "minLength": 1, "writeOnly": true}}, "required": ["inbox_id", "api_key", "webhook_secret"]}),
            },
            capabilities: vec![Capability::Channel],
        }],
    }
}

use super::runtime::Runtime;
use crate::{connections::service::Connections, error::Error};
pub(crate) struct Agentmail;
#[async_trait::async_trait]
impl Runtime for Agentmail {
    async fn validate(
        &self,
        service: &Connections,
        values: &Values,
    ) -> Result<Option<String>, Error> {
        let url = super::url(
            service
                .endpoints
                .get("agentmail_api", "https://api.agentmail.to/v0"),
            &["inboxes", value(values, "inbox_id")?],
        )?;
        let token = value(values, "api_key")?;
        service
            .http
            .json(service.http.client.get(url).bearer_auth(token))
            .await?;
        Ok(optional(values, "inbox_id").map(str::to_owned))
    }
}
