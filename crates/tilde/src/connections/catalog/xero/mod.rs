//! Xero: standard OAuth 2.0 (authorization code, rotating refresh tokens, 30-minute access
//! tokens) through the shared flow. Xero names the organisation outside the token response: after
//! the exchange, `GET /connections` lists the tenants the token reaches, and the one authorized in
//! this sign-in (its `authEventId` equals the access token's `authentication_event_id`; the first
//! organisation of the event if it authorized several) becomes the connection's `tenant_id`. One
//! connection is one organisation; API requests send it as `xero-tenant-id`, which request
//! credentials include.
//!
//! Webhooks: a Xero app has one webhook URL. With your own app, set it to this connection's
//! webhook URL and store the webhook key; `x-xero-signature` (base64 HMAC-SHA256 of the body) is
//! checked, an invalid one answered 401 and a valid one 200 as Xero's intent-to-receive check
//! requires. Deliveries with events for this connection's tenant reach ontology environments; the
//! intent-to-receive probe (no events) and other tenants' events do not. Tilde's own app (CLOUD
//! ONLY) has no per-connection webhook; its connections are polled.
use super::runtime::Runtime;
use crate::chat::providers::Access;
use crate::chat::tools::ToolResult;
use crate::connections::{categories::CATEGORY_ACCOUNTING, model::*, oauth, service::Connections};
use crate::error::Error;
use connectrpc::ConnectError;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};

pub const PROVIDER_ID: &str = "xero";
pub const API: &str = "https://api.xero.com";
const AUTHORIZE: &str = "https://login.xero.com/identity/connect/authorize";
const TOKEN: &str = "https://identity.xero.com/connect/token";
/// Read-only granular scopes (required of Xero apps created since March 2026).
pub const SCOPES: &[&str] = &[
    "openid",
    "profile",
    "email",
    "offline_access",
    "accounting.settings.read",
    "accounting.contacts.read",
    "accounting.journals.read",
    "accounting.invoices.read",
    "accounting.payments.read",
    "accounting.banktransactions.read",
    "accounting.manualjournals.read",
];

pub fn definition() -> Provider {
    let mut oauth = OAuth::standard(TOKEN);
    oauth.authorization_url = Some(AUTHORIZE.into());
    oauth.client_auth = ClientAuth::Basic;
    oauth.scopes = SCOPES.iter().map(|s| (*s).to_owned()).collect();
    let own = ConnectionType {
        mcp: None,
        id: "oauth".into(),
        name: "Your Xero app".into(),
        credential_source: CredentialSource::OAuth {
            grant: OAuthGrant::AuthorizationCode,
            configuration: Box::new(oauth),
            additional_schema: Some(json!({"type":"object","properties":{
                "webhook_key":{"type":"string","title":"Webhook key","writeOnly":true,
                    "description":"Optional. Your Xero app's webhook key (Webhooks page), so this connection's webhook URL accepts change notifications."}
            },"required":[],"additionalProperties":false})),
        },
        capabilities: vec![],
    };
    Provider {
        api_hosts: vec!["api.xero.com".into()],
        account_name_label: Some("Organisation name".into()),
        icon_url: Some("https://www.xero.com/favicon.ico".into()),
        instructions: Some(
            "Connect a Xero organisation so an ontology can ingest its chart of accounts, posted journals, manual journals, invoices, bills, bank transactions, payments and contacts (read-only scopes)."
                .into(),
        ),
        id: PROVIDER_ID.into(),
        name: "Xero".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_ACCOUNTING.into()],
        connection_types: {
            vec![
                own,
            ]
        },
    }
}

pub(crate) struct Xero;

#[async_trait::async_trait]
impl Runtime for Xero {
    fn instructions(&self, typ: &ConnectionType) -> &'static [&'static str] {
        if typ.id == "oauth" {
            &[
                "In the Xero Developer portal (My Apps), create a Web app and add the redirect URI below.",
                "Optional: under Webhooks, add the endpoint below with the events to deliver, copy its webhook key here, finish connecting, then send Xero's Intent to receive.",
            ]
        } else {
            &["Continue to sign in to Xero and choose the organisation to connect."]
        }
    }
    fn connection_instructions(
        &self,
        service: &Connections,
        connection: uuid::Uuid,
        typ: &ConnectionType,
    ) -> Vec<String> {
        if typ.id != "oauth" {
            return vec![];
        }
        vec![
            format!(
                "Redirect URI: {}{}",
                service.public_url,
                crate::connections::CALLBACK_PATH
            ),
            format!(
                "Webhook endpoint: {}/connections/webhooks/{connection}",
                service.public_event_ingress_url
            ),
        ]
    }
    fn oauth(&self, service: &Connections, typ: &ConnectionType) -> Result<OAuth, Error> {
        let mut oauth = typ
            .oauth()
            .cloned()
            .ok_or_else(|| invalid("OAuth configuration missing"))?;
        oauth.token_url = service.endpoints.get("xero_token", TOKEN).into();
        oauth.authorization_url = Some(service.endpoints.get("xero_authorize", AUTHORIZE).into());
        Ok(oauth)
    }
    async fn callback(
        &self,
        service: &Connections,
        setup: &Setup,
        typ: &ConnectionType,
        parameters: &Values,
    ) -> Result<(), Error> {
        let values = service.staged(setup).await?;
        let mut token = oauth::exchange(
            &service.http,
            Driver::OAuthCode,
            &self.oauth(service, typ)?,
            &values,
            optional(parameters, "code"),
            Some(&service.callback_url(setup)?),
            false,
        )
        .await?;
        let tenants = service
            .http
            .json(
                service
                    .http
                    .client
                    .get(format!(
                        "{}/connections",
                        service.endpoints.get("xero_api", API)
                    ))
                    .bearer_auth(token.access.expose_secret())
                    .header("accept", "application/json"),
            )
            .await?;
        let event = auth_event(token.access.expose_secret());
        let tenants = tenants.0.as_array().cloned().unwrap_or_default();
        let organisations = || tenants.iter().filter(|t| t["tenantType"] == "ORGANISATION");
        let tenant = organisations()
            .find(|t| event.is_some() && t["authEventId"].as_str() == event.as_deref())
            .or_else(|| organisations().next())
            .ok_or_else(|| invalid("Xero authorized no organisation"))?;
        let id = tenant["tenantId"]
            .as_str()
            .ok_or_else(|| invalid("Xero returned no tenant"))?;
        let name = tenant["tenantName"].as_str().map(str::to_owned);
        token
            .provider_values
            .insert("tenant_id".into(), SecretString::from(id));
        service.complete(setup, Some(token), name).await
    }
}

/// The access token's `authentication_event_id` claim (the token is Xero's own JWT, just
/// received from its token endpoint, so it is read without verification).
fn auth_event(access_token: &str) -> Option<String> {
    use base64::Engine;
    let claims = access_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(claims.trim_end_matches('='))
        .ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims["authentication_event_id"]
        .as_str()
        .map(str::to_owned)
}

/// Checks `x-xero-signature`; whether the delivery has events for this connection's tenant.
pub(crate) fn verify(access: &Access, headers: &http::HeaderMap, body: &[u8]) -> ToolResult<bool> {
    use base64::Engine;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(crate::chat::providers::ingress::header(
            headers,
            "x-xero-signature",
        )?)
        .map_err(|_| ConnectError::unauthenticated("Invalid signature"))?;
    let key = access
        .secret("webhook_key")
        .map_err(|_| ConnectError::unauthenticated("Webhooks are not configured"))?;
    crate::chat::providers::ingress::hmac_verify(key.as_bytes(), body, &signature)?;
    let tenant = access.secret("tenant_id")?;
    let payload: Value = serde_json::from_slice(body)
        .map_err(|_| ConnectError::invalid_argument("Invalid Xero webhook"))?;
    Ok(payload["events"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|event| event["tenantId"].as_str() == Some(tenant)))
}
