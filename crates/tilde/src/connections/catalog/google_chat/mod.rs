//! A Google Chat app: its Cloud project's service account signs the JWT-bearer grant for Chat API
//! tokens (`chat.bot`), and the project number is the audience of Google's signed requests to
//! the connection's webhook URL (`chat::providers::google_chat`).
use crate::connections::categories::CATEGORY_CHAT;
use crate::connections::model::*;

pub fn definition() -> Provider {
    let mut oauth = OAuth::standard("https://oauth2.googleapis.com/token");
    oauth.scopes = vec!["https://www.googleapis.com/auth/chat.bot".into()];
    Provider {
        account_name_label: Some("Chat app name".into()),
        icon_url: Some("/provider-icons/google_chat.svg".into()),
        instructions: Some(
            "Connect a Google Chat app so people can message this agent directly or @mention it in spaces. The app runs as a service account of your Google Cloud project and receives messages at the Webhook URL below."
                .into(),
        ),
        id: "google_chat".into(),
        name: "Google Chat".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_CHAT.into()],
        connection_types: vec![ConnectionType {
            mcp: None,
            id: "app".into(),
            name: "Google Chat app".into(),
            credential_source: CredentialSource::OAuth {
                grant: OAuthGrant::JwtBearer,
                configuration: Box::new(oauth),
                additional_schema: Some(serde_json::json!({"type":"object","properties":{
                    "project_number":{"type":"string","title":"Project number","pattern":"^[0-9]{6,20}$","minLength":6,"description":"The numeric project number of the Cloud project that owns the Chat app (IAM & Admin > Settings)."}
                },"required":["project_number"],"additionalProperties":false})),
            },
            capabilities: vec![Capability::Channel],
        }],
    }
}

use super::runtime::Runtime;
pub(crate) struct GoogleChat;
#[async_trait::async_trait]
impl Runtime for GoogleChat {
    fn instructions(&self, _typ: &ConnectionType) -> &'static [&'static str] {
        &[
            "In your Google Cloud project, enable the Google Chat API, create a service account and download a JSON key for it.",
            "On the Chat API's Configuration page, leave \"Build this Chat app as a Google Workspace add-on\" off, name the app, allow it to join spaces and group conversations, and choose HTTP endpoint URL with the Webhook URL below.",
            "Under Authentication Audience choose Project Number, and choose who the app is visible to.",
            "Enter the project number, then the key's client_email as the issuer and its private_key as the private key. Leave the subject empty.",
        ]
    }
}
