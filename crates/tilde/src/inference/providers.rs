//! Every supported model provider, and how a connection's credential fields become an upstream:
//! a base URL that the agent's request path is appended to, plus the header or signature that
//! authenticates there. Agents keep using each provider's own SDK; only the base URL changes.
use super::Auth;
use crate::connections::model::{Values, endpoint, invalid};
use crate::error::Error;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    OpenAi,
    Anthropic,
    AzureOpenAi,
    AzureAi,
    GoogleAi,
    Bedrock,
    Baseten,
    Cerebras,
    VercelAiGateway,
    Cohere,
    Voyage,
}
impl Provider {
    pub const ALL: [Provider; 11] = [
        Self::OpenAi,
        Self::Anthropic,
        Self::AzureOpenAi,
        Self::AzureAi,
        Self::GoogleAi,
        Self::Bedrock,
        Self::Baseten,
        Self::Cerebras,
        Self::VercelAiGateway,
        Self::Cohere,
        Self::Voyage,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::AzureOpenAi => "azure_openai",
            Self::AzureAi => "azure_ai",
            Self::GoogleAi => "google_ai",
            Self::Bedrock => "bedrock",
            Self::Baseten => "baseten",
            Self::Cerebras => "cerebras",
            Self::VercelAiGateway => "vercel_ai_gateway",
            Self::Cohere => "cohere",
            Self::Voyage => "voyage",
        }
    }
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::AzureOpenAi => "Azure OpenAI",
            Self::AzureAi => "Azure AI Foundry",
            Self::GoogleAi => "Google AI Studio",
            Self::Bedrock => "Amazon Bedrock",
            Self::Baseten => "Baseten",
            Self::Cerebras => "Cerebras",
            Self::VercelAiGateway => "Vercel AI Gateway",
            Self::Cohere => "Cohere",
            Self::Voyage => "Voyage AI",
        }
    }
    /// theSVG slug served through jsDelivr, like the chat providers.
    pub fn icon_url(self) -> String {
        let slug = match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::AzureOpenAi => "azure-azure-openai",
            Self::AzureAi => "azure-ai-studio",
            Self::GoogleAi => "ai-studio-google",
            Self::Bedrock => "aws-amazon-bedrock",
            Self::Baseten => "baseten",
            Self::Cerebras => "cerebras",
            Self::VercelAiGateway => "vercel",
            Self::Cohere => "cohere",
            Self::Voyage => "voyage",
        };
        format!("https://cdn.jsdelivr.net/gh/glincker/thesvg@main/public/icons/{slug}/default.svg")
    }
    /// What the account name means here; it becomes the slug agents route by.
    pub fn account_name_label(self) -> &'static str {
        match self {
            Self::AzureOpenAi | Self::AzureAi => "Resource name",
            Self::GoogleAi | Self::VercelAiGateway => "Project name",
            Self::Bedrock => "Account name",
            _ => "API key name",
        }
    }
    pub fn instructions(self) -> &'static [&'static str] {
        match self {
            Self::OpenAi => &["Create an API key at platform.openai.com and paste it below."],
            Self::Anthropic => &["Create an API key at console.anthropic.com and paste it below."],
            Self::AzureOpenAi => &[
                "Copy the resource endpoint (https://<resource>.openai.azure.com) and one of its keys from the Azure portal.",
                "Agents address deployments exactly as they would against Azure directly.",
            ],
            Self::AzureAi => {
                &["Copy the model inference endpoint and key from your Azure AI Foundry project."]
            }
            Self::GoogleAi => &["Create an API key in Google AI Studio and paste it below."],
            Self::Bedrock => &[
                "Choose the AWS region that hosts your Bedrock models.",
                "Paste either a Bedrock API key, or an IAM access key pair with bedrock:InvokeModel* permissions.",
            ],
            Self::Baseten => &["Create an API key in your Baseten workspace and paste it below."],
            Self::Cerebras => &["Create an API key at cloud.cerebras.ai and paste it below."],
            Self::VercelAiGateway => {
                &["Create an AI Gateway API key in your Vercel project and paste it below."]
            }
            Self::Cohere => &["Create an API key at dashboard.cohere.com and paste it below."],
            Self::Voyage => &["Create an API key at dash.voyageai.com and paste it below."],
        }
    }
    pub fn schema(self) -> Value {
        let key = |title: &str| json!({"type": "string", "title": title, "minLength": 1, "writeOnly": true});
        let base_url = |default: &str| json!({"type": "string", "title": "Base URL", "description": format!("Optional; defaults to {default}")});
        let mut properties = serde_json::Map::new();
        let mut required = vec![];
        match self {
            Self::AzureOpenAi | Self::AzureAi => {
                properties.insert(
                    "endpoint".into(),
                    json!({"type": "string", "title": "Endpoint", "minLength": 1}),
                );
                properties.insert("api_key".into(), key("API key"));
                required.extend(["endpoint", "api_key"]);
            }
            Self::Bedrock => {
                properties.insert(
                    "region".into(),
                    json!({"type": "string", "title": "AWS region", "minLength": 1}),
                );
                properties.insert("api_key".into(), key("Bedrock API key"));
                properties.insert(
                    "access_key_id".into(),
                    json!({"type": "string", "title": "IAM access key ID"}),
                );
                properties.insert("secret_access_key".into(), key("IAM secret access key"));
                properties.insert("session_token".into(), key("IAM session token"));
                required.push("region");
            }
            _ => {
                properties.insert("api_key".into(), key("API key"));
                properties.insert("base_url".into(), base_url(self.default_base()));
                required.push("api_key");
            }
        }
        json!({"type": "object", "additionalProperties": false, "properties": properties, "required": required})
    }
    fn default_base(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::GoogleAi => "https://generativelanguage.googleapis.com/v1beta",
            Self::Baseten => "https://inference.baseten.co/v1",
            Self::Cerebras => "https://api.cerebras.ai/v1",
            Self::VercelAiGateway => "https://ai-gateway.vercel.sh/v1",
            Self::Cohere => "https://api.cohere.com/v2",
            Self::Voyage => "https://api.voyageai.com/v1",
            Self::AzureOpenAi | Self::AzureAi | Self::Bedrock => "",
        }
    }
    /// Turn credential fields into the upstream base and its authentication. Also the only
    /// validation a connection setup performs: a key that parses here is accepted.
    pub(crate) fn upstream(self, values: &Values) -> Result<(Url, Auth), Error> {
        let field = |name: &str| {
            values
                .get(name)
                .map(|v| v.expose_secret())
                .filter(|v| !v.is_empty())
        };
        let required =
            |name: &str| field(name).ok_or_else(|| invalid(&format!("{name} is required")));
        let secret = |name: &str| required(name).map(SecretString::from);
        let bearer = |name: &str| {
            Ok::<_, Error>(Auth::header(
                "authorization",
                format!("Bearer {}", required(name)?),
            ))
        };
        let base = |default: &str| endpoint(field("base_url").unwrap_or(default));
        Ok(match self {
            Self::OpenAi | Self::Cerebras | Self::VercelAiGateway | Self::Cohere | Self::Voyage => {
                (base(self.default_base())?, bearer("api_key")?)
            }
            Self::Baseten => (
                base(self.default_base())?,
                Auth::header("authorization", format!("Api-Key {}", required("api_key")?)),
            ),
            Self::Anthropic => (
                base(self.default_base())?,
                Auth::header("x-api-key", secret("api_key")?),
            ),
            Self::GoogleAi => (
                base(self.default_base())?,
                Auth::header("x-goog-api-key", secret("api_key")?),
            ),
            Self::AzureOpenAi => {
                let mut url = endpoint(required("endpoint")?)?;
                url.path_segments_mut()
                    .map_err(|_| invalid("Invalid endpoint"))?
                    .pop_if_empty()
                    .push("openai");
                (url, Auth::header("api-key", secret("api_key")?))
            }
            Self::AzureAi => (
                endpoint(required("endpoint")?)?,
                Auth::header("api-key", secret("api_key")?),
            ),
            Self::Bedrock => {
                let region = required("region")?;
                if !region
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                {
                    return Err(invalid("Invalid AWS region"));
                }
                let url = endpoint(&format!("https://bedrock-runtime.{region}.amazonaws.com"))?;
                let auth = if field("api_key").is_some() {
                    bearer("api_key")?
                } else {
                    Auth::SigV4 {
                        region: region.to_owned(),
                        access_key_id: secret("access_key_id")?,
                        secret_access_key: secret("secret_access_key")?,
                        session_token: field("session_token").map(SecretString::from),
                    }
                };
                (url, auth)
            }
        })
    }
}
