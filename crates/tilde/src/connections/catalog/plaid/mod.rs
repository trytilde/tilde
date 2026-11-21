//! Plaid: one connection is one Plaid Item (a person's login at one bank), linked through Plaid
//! Hosted Link so the setup stays a plain redirect and callback: the setup creates a
//! `link_token` with a `hosted_link` completion URI pointing at the shared OAuth callback (with
//! the setup's callback state), the person links their bank on Plaid's page, and the callback
//! reads the session's `public_token` (`/link/token/get`) and exchanges it for the Item's
//! `access_token`. No Plaid script runs in the setup iframe.
//!
//! Credential model: Plaid issues API keys per company, not per end user, and production access
//! is a company-level agreement, so the long-term model for hosted Tilde is Tilde's own Plaid
//! app (`link`, CLOUD ONLY: `TOOL_MANAGED_PROVIDER_PLAID_OAUTH_CLIENT_ID`/`_SECRET` for
//! production, or the `PLAID_SANDBOX` pair) that every tenant's end users link through, with
//! `own_keys` for a tenant that holds its own Plaid agreement (its branding in Link, its billing).
//! Either way the connection stores the client ID, secret and environment beside the Item's
//! access token (as managed OAuth stores Tilde's client), so request credentials, webhook
//! verification and refresh-free polling read one place. Values: `client_id`, `secret`,
//! `environment` (`sandbox` | `production`), `access_token`, `item_id`.
//!
//! Re-linking: an Item that needs the person again (`ITEM_LOGIN_REQUIRED`,
//! `PENDING_EXPIRATION`, `PENDING_DISCONNECT`, reported by the Item webhook) puts the connection
//! in `requires_action`; Reconnect resumes at `plaid_update` with the stored values and opens
//! Link in update mode for the same Item, so its history and sync cursors stay valid.
//!
//! Webhooks: the Item's webhook is this connection's webhook URL (set on the link token). Each
//! delivery's `Plaid-Verification` JWT (ES256) is checked against Plaid's key for its `kid`
//! (`/webhook_verification_key/get`), its `iat` must be under five minutes old and its
//! `request_body_sha256` must match the body; verified deliveries reach ontology environments
//! (ducktail webhook pipelines, which can ask the polled pipeline to sync now).
use super::runtime::Runtime;
use crate::chat::providers::Access;
use crate::chat::tools::ToolResult;
use crate::connections::{model::*, oauth::PrivateJson, service::Connections};
use crate::error::Error;
use connectrpc::ConnectError;
use secrecy::ExposeSecret;
use serde_json::{Value, json};

pub const PROVIDER_ID: &str = "plaid";
pub const PRODUCTION: &str = "https://production.plaid.com";
pub const SANDBOX: &str = "https://sandbox.plaid.com";

pub fn definition() -> Provider {
    let typ = |id: &str, name: &str| ConnectionType {
        mcp: None,
        id: id.into(),
        name: name.into(),
        credential_source: CredentialSource::Custom,
        capabilities: vec![],
    };
    Provider {
        api_hosts: vec!["production.plaid.com".into(), "sandbox.plaid.com".into()],
        account_name_label: Some("Account name".into()),
        icon_url: Some("https://plaid.com/favicon.ico".into()),
        instructions: Some(
            "Link a bank, card, loan or brokerage login through Plaid so an ontology can ingest its accounts, balances, transactions, investments and liabilities. Each connection is one login at one institution."
                .into(),
        ),
        id: PROVIDER_ID.into(),
        name: "Plaid".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![crate::connections::categories::CATEGORY_FINANCE.into()],
        connection_types: vec![
            typ("own_keys", "Your Plaid keys"),
        ],
    }
}

/// `https://production.plaid.com` or the sandbox; the `plaid_api` endpoint override in tests.
pub(crate) fn base<'a>(endpoints: &'a super::Endpoints, values: &Values) -> &'a str {
    let default = match optional(values, "environment") {
        Some("sandbox") => SANDBOX,
        _ => PRODUCTION,
    };
    endpoints.get("plaid_api", default)
}

/// One Plaid API call, authenticated with the connection's client in the body. Plaid answers
/// errors with HTTP 4xx and `{error_type, error_code}`; only the code is kept.
async fn call(
    http: &crate::connections::oauth::Http,
    base: &str,
    path: &str,
    values: &Values,
    mut body: Value,
) -> Result<PrivateJson, Error> {
    body["client_id"] = value(values, "client_id")?.into();
    body["secret"] = value(values, "secret")?.into();
    let request = http
        .client
        .post(endpoint(&format!("{}{path}", base.trim_end_matches('/')))?)
        .json(&body);
    // The body holds the secret: overwrite it once sent.
    drop(PrivateJson(body));
    let response = request
        .send()
        .await
        .map_err(|_| invalid("Plaid request failed"))?;
    let status = response.status();
    let json = PrivateJson(
        response
            .json::<Value>()
            .await
            .map_err(|_| invalid("Invalid Plaid response"))?,
    );
    if !status.is_success() {
        let code = json.0["error_code"].as_str().unwrap_or("UNKNOWN");
        tracing::warn!(%status, error_code = code, path, "Plaid request refused");
        return Err(Error::Invalid(format!("Plaid refused {path}: {code}")));
    }
    Ok(json)
}

pub(crate) struct Plaid;

impl Plaid {
    /// Creates the step's link token (update mode when the values hold an Item) and stages its
    /// Hosted Link URL.
    async fn link(
        &self,
        service: &Connections,
        setup: &Setup,
        values: &Values,
    ) -> Result<(), Error> {
        let state = service.callback_state(setup)?;
        let mut completion = endpoint(&service.callback_url(setup)?)?;
        completion
            .query_pairs_mut()
            .append_pair("state", state.expose_secret());
        let mut request = json!({
            "client_name": "Tilde",
            "language": "en",
            "country_codes": ["US"],
            "user": {"client_user_id": setup.connection_id.to_string()},
            "webhook": format!(
                "{}/connections/webhooks/{}",
                service.public_event_ingress_url, setup.connection_id
            ),
            "hosted_link": {"completion_redirect_uri": completion.as_str()},
        });
        match optional(values, "access_token") {
            Some(access_token) => request["access_token"] = access_token.into(),
            None => {
                request["products"] = json!(["transactions"]);
                request["optional_products"] = json!(["investments", "liabilities"]);
                request["transactions"] = json!({"days_requested": 730});
            }
        }
        let created = call(
            &service.http,
            base(&service.endpoints, values),
            "/link/token/create",
            values,
            request,
        )
        .await?;
        let mut staged = Values::new();
        staged.insert("_link_token".into(), created.secret("/link_token")?);
        staged.insert(
            "_hosted_link_url".into(),
            created.secret("/hosted_link_url")?,
        );
        service.stage(setup, &staged).await
    }
}

#[async_trait::async_trait]
impl Runtime for Plaid {
    fn ui(&self) -> &'static str {
        "plaid"
    }
    fn instructions(&self, typ: &ConnectionType) -> &'static [&'static str] {
        if typ.id == "own_keys" {
            &[
                "In the Plaid Dashboard (Developers > Keys), copy your client ID and the secret of the environment to link in.",
                "Plaid sends Item webhooks to this connection automatically; no webhook settings are needed.",
                "Continue to choose the institution and sign in on Plaid's page.",
            ]
        } else {
            &["Continue to choose your institution and sign in on Plaid's page."]
        }
    }
    fn validate_input(
        &self,
        typ: &ConnectionType,
        step: &str,
        values: &Values,
    ) -> Result<(), Error> {
        match (step, typ.id.as_str()) {
            ("fields", "own_keys") => {
                super::runtime::input(values, &["client_id", "secret", "environment"], &[])?;
                if !matches!(value(values, "environment")?, "sandbox" | "production") {
                    return Err(invalid("Environment is sandbox or production"));
                }
                Ok(())
            }
            ("fields" | "plaid_update", _) => super::runtime::input(values, &[], &[]),
            _ => Err(invalid("This step requires a provider callback")),
        }
    }
    fn resume_step(&self) -> Option<&'static str> {
        Some("plaid_update")
    }
    fn accepts_callback(&self, step: &str, _parameters: &Values, _denied: bool) -> bool {
        step == "plaid_link"
    }
    async fn action(
        &self,
        service: &Connections,
        setup: &Setup,
        _typ: &ConnectionType,
    ) -> Result<Action, Error> {
        match setup.step.as_str() {
            "fields" | "plaid_update" => Ok(Action::Form),
            "plaid_link" => Ok(Action::Redirect {
                url: value(&service.staged(setup).await?, "_hosted_link_url")?.to_owned(),
            }),
            _ => Err(invalid("Unexpected Plaid setup phase")),
        }
    }
    async fn start(
        &self,
        service: &Connections,
        setup: &Setup,
        _typ: &ConnectionType,
        values: Values,
    ) -> Result<(), Error> {
        let values = match setup.step.as_str() {
            // Re-linking: the stored values (copied at Reconnect) name the Item.
            "plaid_update" => service.staged(setup).await?,
            _ => values,
        };
        service.stage(setup, &values).await?;
        self.link(service, setup, &values).await?;
        service.transition(setup, "plaid_link").await
    }
    async fn callback(
        &self,
        service: &Connections,
        setup: &Setup,
        _typ: &ConnectionType,
        _parameters: &Values,
    ) -> Result<(), Error> {
        let values = service.staged(setup).await?;
        let base = base(&service.endpoints, &values);
        if optional(&values, "access_token").is_some() {
            // Update mode keeps the Item and its token; it is usable again once Plaid reports no error.
            let item = call(
                &service.http,
                base,
                "/item/get",
                &values,
                json!({
                    "access_token": value(&values, "access_token")?,
                }),
            )
            .await?;
            if !item.0.pointer("/item/error").is_none_or(Value::is_null) {
                return Err(invalid("The Plaid Item still needs the account holder"));
            }
            return service.complete(setup, None, None).await;
        }
        let session = call(
            &service.http,
            base,
            "/link/token/get",
            &values,
            json!({
                "link_token": value(&values, "_link_token")?,
            }),
        )
        .await?;
        // The newest session that added an Item; the person may have retried within one token.
        let added = session.0["link_sessions"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find_map(|s| s.pointer("/results/item_add_results/0"))
            .ok_or_else(|| invalid("Plaid Link finished without linking an Item"))?;
        let public_token = added["public_token"]
            .as_str()
            .ok_or_else(|| invalid("Plaid Link returned no public token"))?;
        let institution = added
            .pointer("/institution/name")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let exchanged = call(
            &service.http,
            base,
            "/item/public_token/exchange",
            &values,
            json!({
                "public_token": public_token,
            }),
        )
        .await?;
        let mut item = Values::new();
        item.insert("access_token".into(), exchanged.secret("/access_token")?);
        item.insert("item_id".into(), exchanged.secret("/item_id")?);
        service.stage(setup, &item).await?;
        service.complete(setup, None, institution).await
    }
}

/// Item webhook codes after which the Item needs its holder to re-link (update mode).
const NEEDS_HOLDER: &[&str] = &[
    "PENDING_EXPIRATION",
    "PENDING_DISCONNECT",
    "USER_PERMISSION_REVOKED",
];

/// Whether a verified webhook says the Item needs its holder again (Reconnect).
pub(crate) fn needs_holder(payload: &Value) -> bool {
    payload["webhook_type"] == "ITEM"
        && (payload["webhook_code"] == "ERROR"
            && payload.pointer("/error/error_code") == Some(&json!("ITEM_LOGIN_REQUIRED"))
            || payload["webhook_code"]
                .as_str()
                .is_some_and(|code| NEEDS_HOLDER.contains(&code)))
}

/// Verifies a delivery's `Plaid-Verification` JWT (module docs).
pub(crate) async fn verify(
    access: &Access,
    headers: &http::HeaderMap,
    body: &[u8],
) -> ToolResult<()> {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    use subtle::ConstantTimeEq;
    let unauthenticated = || ConnectError::unauthenticated("Invalid Plaid webhook");
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let jwt = crate::chat::providers::ingress::header(headers, "plaid-verification")?;
    let mut parts = jwt.split('.');
    let (Some(head), Some(claims), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(unauthenticated());
    };
    let decode = |part: &str| -> ToolResult<Value> {
        b64.decode(part)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or_else(unauthenticated)
    };
    let head = decode(head)?;
    if head["alg"] != "ES256" {
        return Err(unauthenticated());
    }
    let kid = head["kid"].as_str().ok_or_else(unauthenticated)?;
    let key = call(
        &access.http,
        base(&access.endpoints, &access.values),
        "/webhook_verification_key/get",
        &access.values,
        json!({"key_id": kid}),
    )
    .await
    .map_err(|_| unauthenticated())?;
    if !key.0.pointer("/key/expired_at").is_none_or(Value::is_null) {
        return Err(unauthenticated());
    }
    let coordinate = |name: &str| {
        key.0
            .pointer(&format!("/key/{name}"))
            .and_then(Value::as_str)
            .and_then(|v| b64.decode(v).ok())
            .filter(|v| v.len() == 32)
            .ok_or_else(unauthenticated)
    };
    let point = [vec![4], coordinate("x")?, coordinate("y")?].concat();
    let signature = b64.decode(signature).map_err(|_| unauthenticated())?;
    let signed = &jwt[..jwt.len() - jwt.rsplit('.').next().map_or(0, str::len) - 1];
    aws_lc_rs::signature::UnparsedPublicKey::new(
        &aws_lc_rs::signature::ECDSA_P256_SHA256_FIXED,
        point,
    )
    .verify(signed.as_bytes(), &signature)
    .map_err(|_| unauthenticated())?;
    let claims = decode(claims)?;
    let issued = claims["iat"].as_i64().ok_or_else(unauthenticated)?;
    if chrono::Utc::now().timestamp().abs_diff(issued) > 300 {
        return Err(ConnectError::unauthenticated("Expired Plaid webhook"));
    }
    let digest = hex::encode(Sha256::digest(body));
    let claimed = claims["request_body_sha256"]
        .as_str()
        .ok_or_else(unauthenticated)?;
    if digest.as_bytes().ct_eq(claimed.as_bytes()).unwrap_u8() != 1 {
        return Err(unauthenticated());
    }
    Ok(())
}
