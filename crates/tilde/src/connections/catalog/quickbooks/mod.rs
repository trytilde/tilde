//! QuickBooks Online: standard OAuth 2.0 (authorization code, rotating refresh tokens) through the
//! shared flow, plus the one thing Intuit returns outside the token response: the company's
//! `realmId`, a query parameter of the authorization callback, stored as the connection's
//! `realm_id`. API requests name it in their path (`/v3/company/<realm_id>/...`), so request
//! credentials carry it beside the bearer token. `environment` picks Intuit's sandbox or
//! production API host (an Intuit app's development keys reach sandbox companies only).
//!
//! Webhooks: an Intuit app has one webhook URL. With your own app, set it to this connection's
//! webhook URL and store the app's verifier token; each delivery's `intuit-signature` (base64
//! HMAC-SHA256 of the body) is checked, and only deliveries naming this connection's realm reach
//! ontology environments (other companies on the same app rely on polling). Tilde's own app
//! (CLOUD ONLY) has no per-connection webhook; its connections are polled.
use super::runtime::Runtime;
use crate::chat::providers::Access;
use crate::chat::tools::ToolResult;
use crate::connections::{
    categories::CATEGORY_ACCOUNTING,
    model::*,
    oauth::{self},
    service::Connections,
};
use crate::error::Error;
use connectrpc::ConnectError;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};

pub const PROVIDER_ID: &str = "quickbooks";
pub const PRODUCTION: &str = "https://quickbooks.api.intuit.com";
pub const SANDBOX: &str = "https://sandbox-quickbooks.api.intuit.com";
const AUTHORIZE: &str = "https://appcenter.intuit.com/connect/oauth2";
const TOKEN: &str = "https://oauth.platform.intuit.com/oauth2/v1/tokens/bearer";

fn environment() -> Value {
    json!({"type":"string","title":"Environment","enum":["production","sandbox"],"default":"production",
        "description":"Production companies, or sandbox companies (an Intuit app's development keys)."})
}

pub fn definition() -> Provider {
    let mut oauth = OAuth::standard(TOKEN);
    oauth.authorization_url = Some(AUTHORIZE.into());
    oauth.client_auth = ClientAuth::Basic;
    oauth.pkce = false;
    oauth.scopes = vec!["com.intuit.quickbooks.accounting".into()];
    let own = ConnectionType {
        mcp: None,
        id: "oauth".into(),
        name: "Your Intuit app".into(),
        credential_source: CredentialSource::OAuth {
            grant: OAuthGrant::AuthorizationCode,
            configuration: Box::new(oauth),
            additional_schema: Some(json!({"type":"object","properties":{
                "environment": environment(),
                "webhook_verifier_token":{"type":"string","title":"Webhook verifier token","writeOnly":true,
                    "description":"Optional. Your Intuit app's webhook verifier token (Webhooks page), so this connection's webhook URL accepts change notifications."}
            },"required":["environment"],"additionalProperties":false})),
        },
        capabilities: vec![],
    };
    Provider {
        api_hosts: vec![
            "quickbooks.api.intuit.com".into(),
            "sandbox-quickbooks.api.intuit.com".into(),
        ],
        account_name_label: Some("Company name".into()),
        icon_url: Some("https://quickbooks.intuit.com/favicon.ico".into()),
        instructions: Some(
            "Connect a QuickBooks Online company so an ontology can ingest its chart of accounts, journal entries, invoices, bills, payments, customers, vendors and items (read-only accounting scope)."
                .into(),
        ),
        id: PROVIDER_ID.into(),
        name: "QuickBooks Online".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_ACCOUNTING.into()],
        connection_types: {
            vec![
                own,
            ]
        },
    }
}

/// The API origin of a connection's environment (`quickbooks_api` endpoint override in tests).
pub(crate) fn base<'a>(endpoints: &'a super::Endpoints, values: &Values) -> &'a str {
    let default = match optional(values, "environment") {
        Some("sandbox") => SANDBOX,
        _ => PRODUCTION,
    };
    endpoints.get("quickbooks_api", default)
}

pub(crate) struct Quickbooks;

#[async_trait::async_trait]
impl Runtime for Quickbooks {
    fn instructions(&self, typ: &ConnectionType) -> &'static [&'static str] {
        if typ.id == "oauth" {
            &[
                "In the Intuit Developer portal, create an app with the com.intuit.quickbooks.accounting scope and add the redirect URI below. Use its development keys for sandbox companies and its production keys for real ones.",
                "Optional: under Webhooks, set the endpoint below, subscribe to the accounting entities, and paste the verifier token here.",
            ]
        } else {
            &["Continue to sign in to QuickBooks and choose the company to connect."]
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
        oauth.token_url = service.endpoints.get("quickbooks_token", TOKEN).into();
        oauth.authorization_url = Some(
            service
                .endpoints
                .get("quickbooks_authorize", AUTHORIZE)
                .into(),
        );
        Ok(oauth)
    }
    async fn callback(
        &self,
        service: &Connections,
        setup: &Setup,
        typ: &ConnectionType,
        parameters: &Values,
    ) -> Result<(), Error> {
        let realm = value(parameters, "realmId")?.to_owned();
        if realm.is_empty() || !realm.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid("QuickBooks returned no company"));
        }
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
        // The company's name labels the account; the connection works without it.
        let company = service
            .http
            .json(
                service
                    .http
                    .client
                    .get(format!(
                        "{}/v3/company/{realm}/companyinfo/{realm}",
                        base(&service.endpoints, &values)
                    ))
                    .bearer_auth(token.access.expose_secret())
                    .header("accept", "application/json"),
            )
            .await
            .ok()
            .and_then(|info| {
                info.text("/CompanyInfo/CompanyName")
                    .ok()
                    .map(str::to_owned)
            });
        token
            .provider_values
            .insert("realm_id".into(), SecretString::from(realm.clone()));
        service
            .complete(setup, Some(token), company.or(Some(realm)))
            .await
    }
}

/// Checks `intuit-signature`; whether the delivery names this connection's company.
pub(crate) fn verify(access: &Access, headers: &http::HeaderMap, body: &[u8]) -> ToolResult<bool> {
    use base64::Engine;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(crate::chat::providers::ingress::header(
            headers,
            "intuit-signature",
        )?)
        .map_err(|_| ConnectError::unauthenticated("Invalid signature"))?;
    let key = access
        .secret("webhook_verifier_token")
        .map_err(|_| ConnectError::unauthenticated("Webhooks are not configured"))?;
    crate::chat::providers::ingress::hmac_verify(key.as_bytes(), body, &signature)?;
    let realm = access.secret("realm_id")?;
    let payload: Value = serde_json::from_slice(body)
        .map_err(|_| ConnectError::invalid_argument("Invalid QuickBooks webhook"))?;
    // The original format (`eventNotifications[].realmId`), or CloudEvents (an array of events
    // with `intuitaccountid`).
    let legacy = payload["eventNotifications"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|event| event["realmId"].as_str() == Some(realm));
    let cloud_events = payload
        .as_array()
        .into_iter()
        .flatten()
        .any(|event| event["intuitaccountid"].as_str() == Some(realm));
    Ok(legacy || cloud_events)
}
