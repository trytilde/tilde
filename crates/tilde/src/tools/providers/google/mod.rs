//! Google Workspace tools. Every Google provider is its own connection (one OAuth grant per
//! product, with that product's scopes) and calls its REST API with the connection's access
//! token. Base URLs are overridable per API through the endpoint keys so tests can use a fake.
use super::rest::{self, Hints};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::model;
use crate::proto::tilde::types::v1 as types;
use reqwest::Method;
use serde_json::{Value, json};

pub mod analytics;
pub mod calendar;
pub mod docs;
pub mod drive;
pub mod mail;
pub mod search_console;
pub mod sheets;

/// A Google product's provider: one OAuth connection type with offline access, so the refresh
/// token keeps the connection usable.
fn definition(
    id: &str,
    name: &str,
    description: &str,
    category: &str,
    scopes: &[&str],
) -> model::Provider {
    let mut oauth = model::OAuth::standard("https://oauth2.googleapis.com/token");
    oauth.authorization_url = Some("https://accounts.google.com/o/oauth2/v2/auth".into());
    oauth.scopes = scopes
        .iter()
        .map(|scope| format!("https://www.googleapis.com/auth/{scope}"))
        .chain(
            ["userinfo.email", "userinfo.profile"]
                .map(|scope| format!("https://www.googleapis.com/auth/{scope}")),
        )
        .chain(["openid".to_owned()])
        .collect();
    oauth.authorization_parameters = [("access_type", "offline"), ("prompt", "consent")]
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .into();
    model::Provider {
        account_name_label: Some("Account name".into()),
        icon_url: Some(format!("/provider-icons/{id}.svg")),
        instructions: Some(format!(
            "{description} Create an OAuth client in Google Cloud Console with this provider's API enabled."
        )),
        id: id.into(),
        name: name.into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![category.into()],
        connection_types: {
            let own = model::ConnectionType {
                mcp: None,
                id: "oauth".into(),
                name: "Google account".into(),
                credential_source: model::CredentialSource::OAuth {
                    grant: model::OAuthGrant::AuthorizationCode,
                    configuration: oauth.into(),
                    additional_schema: None,
                },
                capabilities: vec![model::Capability::Tool],
            };
            vec![own]
        },
    }
}

const READ: Hints = Hints {
    read_only: true,
    destructive: false,
};
const WRITE: Hints = Hints {
    read_only: false,
    destructive: false,
};
const DELETE: Hints = Hints {
    read_only: false,
    destructive: true,
};

/// A tool whose input is a closed object of `properties`.
fn tool(
    provider: &str,
    name: &str,
    summary: &str,
    description: &str,
    hints: Hints,
    properties: Value,
    required: &[&str],
) -> types::ToolDefinition {
    rest::definition(
        provider,
        name,
        summary,
        description,
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
        hints,
    )
}

/// One Google API base, authenticated with the connection's OAuth access token.
struct Api<'a> {
    access: &'a Access,
    endpoint: &'static str,
    base: &'static str,
}
impl<'a> Api<'a> {
    fn new(access: &'a Access, endpoint: &'static str, base: &'static str) -> Self {
        Self {
            access,
            endpoint,
            base,
        }
    }
    /// `segments` are appended to the base path, each percent-encoded as one path segment.
    fn request(&self, method: Method, segments: &[&str]) -> ToolResult<reqwest::RequestBuilder> {
        let url = self.access.url(self.endpoint, self.base, segments)?;
        Ok(self
            .access
            .http
            .client
            .request(method, url)
            .bearer_auth(self.access.secret("access_token")?))
    }
    fn get(&self, segments: &[&str]) -> ToolResult<reqwest::RequestBuilder> {
        self.request(Method::GET, segments)
    }
    fn post(&self, segments: &[&str]) -> ToolResult<reqwest::RequestBuilder> {
        self.request(Method::POST, segments)
    }
}
/// The upstream JSON body (null when empty); a non-2xx status is the tool's error.
async fn call(request: reqwest::RequestBuilder) -> ToolResult<Value> {
    Ok(rest::send(request).await?["data"].take())
}

fn text<'a>(input: &'a Value, key: &str) -> &'a str {
    input[key].as_str().unwrap_or_default()
}
fn optional<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input[key].as_str()
}
fn string_list(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}
