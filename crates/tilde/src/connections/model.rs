use crate::error::Error;
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Channel,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Driver {
    Static,
    OAuthCode,
    OAuthClientCredentials,
    OAuthJwtBearer,
    Custom,
}
impl Driver {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::OAuthCode => "oauth_code",
            Self::OAuthClientCredentials => "oauth_client_credentials",
            Self::OAuthJwtBearer => "oauth_jwt_bearer",
            Self::Custom => "custom",
        }
    }
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "static" => Ok(Self::Static),
            "oauth_code" => Ok(Self::OAuthCode),
            "oauth_client_credentials" => Ok(Self::OAuthClientCredentials),
            "oauth_jwt_bearer" => Ok(Self::OAuthJwtBearer),
            "custom" => Ok(Self::Custom),
            _ => Err(invalid("Unknown authentication driver")),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientAuth {
    Body,
    Basic,
    None,
}
impl ClientAuth {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Basic => "basic",
            Self::None => "none",
        }
    }
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "body" => Ok(Self::Body),
            "basic" => Ok(Self::Basic),
            "none" => Ok(Self::None),
            _ => Err(invalid("Invalid OAuth client authentication")),
        }
    }
}
/// A provider-owned scalar returned with a token (workspace IDs, bot IDs, etc.). Values stay private.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultField {
    pub key: String,
    pub path: String,
    pub required: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuth {
    pub authorization_url: Option<String>,
    pub token_url: String,
    pub client_auth: ClientAuth,
    pub pkce: bool,
    pub scopes: Vec<String>,
    pub scope_separator: String,
    pub authorization_parameters: BTreeMap<String, String>,
    pub token_parameters: BTreeMap<String, String>,
    pub access_token_path: String,
    pub refresh_token_path: String,
    pub expires_in_path: String,
    pub scope_path: String,
    pub success_path: Option<String>,
    pub result_fields: Vec<ResultField>,
}
impl OAuth {
    pub fn standard(token_url: &str) -> Self {
        Self {
            authorization_url: None,
            token_url: token_url.into(),
            client_auth: ClientAuth::Body,
            pkce: true,
            scopes: vec![],
            scope_separator: " ".into(),
            authorization_parameters: BTreeMap::new(),
            token_parameters: BTreeMap::new(),
            access_token_path: "/access_token".into(),
            refresh_token_path: "/refresh_token".into(),
            expires_in_path: "/expires_in".into(),
            scope_path: "/scope".into(),
            success_path: None,
            result_fields: vec![],
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthGrant {
    AuthorizationCode,
    ClientCredentials,
    JwtBearer,
}
impl OAuthGrant {
    pub fn driver(self) -> Driver {
        match self {
            Self::AuthorizationCode => Driver::OAuthCode,
            Self::ClientCredentials => Driver::OAuthClientCredentials,
            Self::JwtBearer => Driver::OAuthJwtBearer,
        }
    }
}
/// Standard methods are configuration. Custom methods use provider-owned UI and handlers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CredentialSource {
    Static {
        schema: serde_json::Value,
    },
    OAuth {
        grant: OAuthGrant,
        configuration: Box<OAuth>,
        additional_schema: Option<serde_json::Value>,
    },
    Custom,
}
impl CredentialSource {
    pub fn driver(&self) -> Driver {
        match self {
            Self::Static { .. } => Driver::Static,
            Self::OAuth { grant, .. } => grant.driver(),
            Self::Custom => Driver::Custom,
        }
    }
    pub fn oauth(&self) -> Option<&OAuth> {
        match self {
            Self::OAuth { configuration, .. } => Some(configuration),
            _ => None,
        }
    }
    pub fn input_schema(&self) -> Option<serde_json::Value> {
        match self {
            Self::Static { schema } => Some(schema.clone()),
            Self::OAuth {
                grant,
                configuration,
                additional_schema,
            } => Some(super::schema::oauth_inputs(
                *grant,
                configuration,
                additional_schema.as_ref(),
            )),
            Self::Custom => None,
        }
    }
    pub(crate) fn from_storage(
        driver: Driver,
        schema: Option<serde_json::Value>,
        oauth: Option<OAuth>,
    ) -> Result<Self, Error> {
        let configuration = || invalid("OAuth configuration missing");
        Ok(match driver {
            Driver::Static => Self::Static {
                schema: schema.ok_or_else(|| invalid("Credential schema missing"))?,
            },
            Driver::OAuthCode => Self::OAuth {
                grant: OAuthGrant::AuthorizationCode,
                configuration: oauth.ok_or_else(configuration)?.into(),
                additional_schema: schema,
            },
            Driver::OAuthClientCredentials => Self::OAuth {
                grant: OAuthGrant::ClientCredentials,
                configuration: oauth.ok_or_else(configuration)?.into(),
                additional_schema: schema,
            },
            Driver::OAuthJwtBearer => Self::OAuth {
                grant: OAuthGrant::JwtBearer,
                configuration: oauth.ok_or_else(configuration)?.into(),
                additional_schema: schema,
            },
            Driver::Custom => Self::Custom,
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionType {
    pub id: String,
    pub name: String,
    pub capabilities: Vec<Capability>,
    pub credential_source: CredentialSource,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteProvider {
    pub endpoint: String,
    pub ui_url: Option<String>,
}
/// Provider origin and execution location are mutually exclusive, not independent flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderKind {
    BuiltIn,
    Configured,
    Remote(RemoteProvider),
}
impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BuiltIn => "built_in",
            Self::Configured => "configured",
            Self::Remote(_) => "remote",
        }
    }
    pub fn remote(&self) -> Option<&RemoteProvider> {
        match self {
            Self::Remote(remote) => Some(remote),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub categories: Vec<String>,
    pub connection_types: Vec<ConnectionType>,
}
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Connection {
    pub id: Uuid,
    pub name: String,
    pub provider_id: String,
    pub type_id: String,
    pub status: String,
    pub account_label: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub credential_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub channel_capable: bool,
    pub associated_agents: sqlx::types::Json<Vec<AssociatedAgent>>,
}
#[derive(sqlx::FromRow)]
pub(crate) struct Setup {
    pub provider_redirect_url: Option<String>,
    pub id: Uuid,
    pub connection_id: Uuid,
    pub step: String,
    pub action_id: Uuid,
    pub connection_setup_token: Vec<u8>,
    pub connection_setup_token_hash: Vec<u8>,
    pub callback_token: Vec<u8>,
    pub callback_hash: Vec<u8>,
    pub error_code: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
}
/// SecretString redacts Debug, prevents plaintext serialization, and clears owned strings on drop.
pub type Values = BTreeMap<String, SecretString>;
pub struct Started {
    pub connection: Connection,
    pub brokering_url: String,
}
pub struct BrokerView {
    pub webhook_url: Option<String>,
    pub setup_id: Uuid,
    pub connection_id: Uuid,
    pub connection_name: String,
    pub type_name: String,
    pub type_id: String,
    pub action_id: Uuid,
    pub step: String,
    pub error_code: Option<String>,
    pub ui_path: String,
    pub draft: Values,
    pub auth_driver: Driver,
    pub input_schema: Option<serde_json::Value>,
    pub action: Action,
}
pub enum Action {
    Form,
    Redirect {
        url: String,
    },
    FormPost {
        url: String,
        fields: Vec<(String, String)>,
    },
    Complete,
    Working,
    Failed,
    Cancelled,
}

pub fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}
pub fn value<'a>(values: &'a Values, key: &str) -> Result<&'a str, Error> {
    values
        .get(key)
        .map(|v| v.expose_secret())
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| invalid(&format!("Missing required field: {key}")))
}
pub fn optional<'a>(values: &'a Values, key: &str) -> Option<&'a str> {
    values
        .get(key)
        .map(|v| v.expose_secret())
        .filter(|v| !v.is_empty())
}
pub fn random_secret() -> SecretString {
    use base64::Engine;
    use rand::RngCore;
    use zeroize::Zeroizing;
    let mut bytes = Zeroizing::new([0u8; 32]);
    rand::rngs::OsRng.fill_bytes(&mut *bytes);
    SecretString::from(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes.as_slice()))
}
pub fn endpoint(raw: &str) -> Result<url::Url, Error> {
    let url = url::Url::parse(raw).map_err(|_| invalid("Invalid provider endpoint"))?;
    let local = url.host_str().is_some_and(|v| {
        v == "localhost"
            || v.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if !(url.scheme() == "https" || (url.scheme() == "http" && local))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(
            "Provider endpoints require HTTPS (HTTP is allowed on loopback), without user info or fragments",
        ));
    }
    Ok(url)
}
pub fn identifier(id: &str) -> Result<(), Error> {
    if id.is_empty()
        || id.len() > 160
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./-".contains(&b))
    {
        return Err(invalid("Invalid provider, type or field identifier"));
    }
    Ok(())
}
impl Provider {
    pub fn validate(&self) -> Result<(), Error> {
        if self.categories.is_empty() || self.categories.len() > 32 {
            return Err(invalid("Provider categories are required"));
        }
        let mut categories = BTreeSet::new();
        for category in &self.categories {
            if category.is_empty()
                || category.len() > 80
                || !category
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                || !categories.insert(category)
            {
                return Err(invalid("Invalid or duplicate provider category"));
            }
        }

        if let Some(remote) = self.kind.remote() {
            for address in std::iter::once(&remote.endpoint).chain(remote.ui_url.as_ref()) {
                let url = endpoint(address)?;
                if url.query().is_some() || url.fragment().is_some() {
                    return Err(invalid("Remote URLs cannot contain query or fragment"));
                }
            }
            if self
                .connection_types
                .iter()
                .any(|typ| typ.driver() == Driver::Custom)
                && remote.ui_url.is_none()
            {
                return Err(invalid("Custom sources require a provider UI"));
            }
        } else if self.connection_types.iter().any(|typ| {
            typ.driver() == Driver::Custom && matches!(self.kind, ProviderKind::Configured)
        }) {
            return Err(invalid(
                "Remote connection types require a registered backend",
            ));
        }

        identifier(&self.id)?;
        if self.name.trim().is_empty()
            || self.name.len() > 200
            || self.connection_types.is_empty()
            || self.connection_types.len() > 32
        {
            return Err(invalid("Provider needs a name and 1-32 connection types"));
        }
        let mut ids = BTreeSet::new();
        for typ in &self.connection_types {
            identifier(&typ.id)?;
            if !ids.insert(&typ.id) {
                return Err(invalid("Duplicate connection type"));
            }
            typ.validate()?;
        }
        Ok(())
    }
}
impl ConnectionType {
    pub fn driver(&self) -> Driver {
        self.credential_source.driver()
    }
    pub fn oauth(&self) -> Option<&OAuth> {
        self.credential_source.oauth()
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.name.trim().is_empty() || self.capabilities.len() > 1 {
            return Err(invalid("Invalid connection type"));
        }
        if let CredentialSource::Static { schema } = &self.credential_source {
            super::schema::validate_schema(schema)?;
        }
        if let CredentialSource::OAuth {
            additional_schema: Some(extra),
            ..
        } = &self.credential_source
        {
            super::schema::validate_schema(extra)?;
            if extra["properties"].as_object().is_some_and(|properties| {
                properties.keys().any(|key| {
                    [
                        "client_id",
                        "client_secret",
                        "issuer",
                        "private_key",
                        "subject",
                        "access_token",
                        "refresh_token",
                        "scope",
                    ]
                    .contains(&key.as_str())
                })
            }) {
                return Err(invalid(
                    "Additional OAuth fields cannot override protocol credentials",
                ));
            }
        }
        if let Some(config) = self.oauth() {
            endpoint(&config.token_url)?;
            if self.driver() == Driver::OAuthCode
                && config.client_auth == ClientAuth::None
                && !config.pkce
            {
                return Err(invalid("Public OAuth clients require PKCE"));
            }

            if self.driver() == Driver::OAuthCode {
                endpoint(
                    config
                        .authorization_url
                        .as_deref()
                        .ok_or_else(|| invalid("Authorization URL required"))?,
                )?;
            }
            if config.scope_separator.is_empty() || config.scope_separator.len() > 4 {
                return Err(invalid("Invalid OAuth scope separator"));
            }
            for path in [
                &config.access_token_path,
                &config.refresh_token_path,
                &config.expires_in_path,
                &config.scope_path,
            ] {
                if !path.is_empty() && !path.starts_with('/') {
                    return Err(invalid("OAuth response paths must be JSON pointers"));
                }
            }
            for params in [&config.authorization_parameters, &config.token_parameters] {
                if params.len() > 30
                    || params.keys().any(|key| {
                        [
                            "state",
                            "redirect_uri",
                            "client_id",
                            "client_secret",
                            "code",
                            "code_verifier",
                            "code_challenge",
                            "code_challenge_method",
                            "grant_type",
                            "refresh_token",
                            "assertion",
                            "response_type",
                            "scope",
                        ]
                        .contains(&key.as_str())
                    })
                {
                    return Err(invalid(
                        "OAuth parameters cannot override protocol-owned fields",
                    ));
                }
            }
            let mut result_keys = BTreeSet::new();
            if config.result_fields.len() > 32 {
                return Err(invalid("Too many OAuth result mappings"));
            }
            for field in &config.result_fields {
                identifier(&field.key)?;
                if !result_keys.insert(&field.key)
                    || !field.path.starts_with('/')
                    || field.key.starts_with('_')
                    || [
                        "client_id",
                        "client_secret",
                        "private_key",
                        "access_token",
                        "refresh_token",
                        "scope",
                        "configuration_refresh_token",
                    ]
                    .contains(&field.key.as_str())
                {
                    return Err(invalid(
                        "Invalid OAuth result mapping or reserved credential field",
                    ));
                }
            }
        }
        Ok(())
    }
    pub fn validate_values(&self, values: &Values) -> Result<(), Error> {
        let schema = self
            .credential_source
            .input_schema()
            .ok_or_else(|| invalid("Custom sources validate their own inputs"))?;
        super::schema::validate_values(&schema, values)
    }
}

/// Public relationship projection. No credential or IAM grant is exposed here.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AssociatedAgent {
    pub capability: Capability,
    pub id: Uuid,
    pub name: String,
    pub endpoint_url: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub capability: Capability,
    pub agent_id: Uuid,
}
