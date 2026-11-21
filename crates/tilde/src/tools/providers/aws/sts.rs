//! The AWS credentials a connection's tool call runs with, through STS.
//!
//! Base credentials are the connection's access keys (`iam`), or for `oidc` a session from
//! `AssumeRoleWithWebIdentity` into `role_arn` with a token from Tilde's OIDC issuer
//! (`sub` = `connection:<id>`, `aud` = `sts.amazonaws.com`). A call naming another account uses
//! the connection's listed role in that account (`assume_role_arns`), assumed from the base
//! credentials with `sts:AssumeRole`, the connection's `external_id` and the session name
//! `tilde-<connection id>`. An account the connection lists no role for is refused.
//!
//! Clients outside the engine (`session`, for `connections::credentials`) get the same: the base
//! session, or with an account the listed role's session, refused for any other account.
//!
//! Temporary credentials are cached per connection, base identity and role in the shared mint
//! cache (`connections::mints`) until five minutes before they expire. `Credentials` zeroize on drop, STS responses are read into zeroizing
//! buffers, and no credential is logged or returned in an error.
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{mints, model};
use client_aws_sigv4::{Credentials, Signer};
use connectrpc::ConnectError;
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use zeroize::Zeroizing;

pub const AUDIENCE: &str = "sts.amazonaws.com";
const DEFAULT_REGION: &str = "us-east-1";
const SESSION_SECONDS: u32 = 3600;

/// A role the connection may assume, and the account it is in.
pub struct Role {
    pub arn: String,
    pub account: String,
}
/// `arn:<partition>:iam::<account>:role/<path/name>`.
fn role(arn: &str) -> Option<Role> {
    let mut parts = arn.splitn(6, ':');
    let (Some("arn"), Some(partition), Some("iam"), Some(""), Some(account), Some(resource)) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return None;
    };
    (partition.starts_with("aws")
        && account.len() == 12
        && account.bytes().all(|b| b.is_ascii_digit())
        && resource.starts_with("role/")
        && resource.len() > 5)
        .then(|| Role {
            arn: arn.to_owned(),
            account: account.to_owned(),
        })
}
/// The roles listed in `assume_role_arns` (commas, spaces or newlines between them).
pub fn roles(values: &model::Values) -> ToolResult<Vec<Role>> {
    model::optional(values, "assume_role_arns")
        .unwrap_or_default()
        .split([',', ' ', '\n'])
        .map(str::trim)
        .filter(|arn| !arn.is_empty())
        .map(|arn| {
            role(arn).ok_or_else(|| {
                ConnectError::failed_precondition(format!(
                    "The connection lists an invalid role ARN: {arn}"
                ))
            })
        })
        .collect()
}
fn region(values: &model::Values) -> &str {
    model::optional(values, "region").unwrap_or(DEFAULT_REGION)
}

/// Accounts of access keys, from `GetCallerIdentity`; account IDs are not secret.
static ACCOUNTS: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);

// Sessions live in the shared mint cache (`connections::mints`), which empties on credential
// changes and zeroizes what it drops.
fn cached(key: &str) -> Option<Credentials> {
    mints::cached(&format!("aws|{key}"))?.aws().ok()
}
fn expiry(key: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    mints::cached(&format!("aws|{key}"))?.expires_at
}
fn remember(key: String, credentials: &Credentials, expires_at: chrono::DateTime<chrono::Utc>) {
    mints::remember(
        format!("aws|{key}"),
        mints::Minted {
            credential: mints::Credential::Aws(credentials.clone()),
            expires_at: Some(expires_at),
        },
    );
}
fn digest(value: &str) -> String {
    hex::encode(&Sha256::digest(value.as_bytes())[..12])
}

/// The connection's own account: the web identity role's, or the access keys' (asked of STS
/// once and remembered).
pub async fn own_account(access: &Access, typ: &str) -> ToolResult<String> {
    if typ == "oidc" {
        return role(access.secret("role_arn")?)
            .map(|r| r.account)
            .ok_or_else(|| {
                ConnectError::failed_precondition("The connection's role ARN is invalid")
            });
    }
    let key = format!(
        "{}|{}",
        access.connection_id,
        access.secret("access_key_id")?
    );
    if let Some(account) = ACCOUNTS.lock().ok().and_then(|a| a.get(&key).cloned()) {
        return Ok(account);
    }
    let (base, _) = base(access, typ).await?;
    let response = call(access, Some(base), &[("Action", "GetCallerIdentity")]).await?;
    let account = tag(&response, "Account")
        .ok_or_else(|| ConnectError::unknown("AWS STS returned no account"))?;
    if let Ok(mut accounts) = ACCOUNTS.lock() {
        accounts.insert(key, account.clone());
    }
    Ok(account)
}

/// The listed role a call in `account` assumes; `None` for the connection's own account. An
/// account the connection lists no role in is refused: an unlisted ARN is never assumed.
async fn target<'a>(
    access: &Access,
    typ: &str,
    roles: &'a [Role],
    account: Option<&str>,
) -> ToolResult<Option<&'a Role>> {
    let Some(account) = account else {
        return Ok(None);
    };
    if let Some(role) = roles.iter().find(|r| r.account == account) {
        return Ok(Some(role));
    }
    if own_account(access, typ).await? == account {
        return Ok(None);
    }
    let reachable = roles
        .iter()
        .map(|r| r.account.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(ConnectError::permission_denied(format!(
        "AWS account {account} is not reachable from this connection: it lists no role in it. Reachable accounts: its own{}{reachable}.",
        if reachable.is_empty() { "" } else { ", " }
    )))
}

/// `sts:AssumeRole` into `role` from the base credentials, with the connection's `external_id`
/// and the session name `tilde-<connection id>`; cached like every session.
async fn assume(
    access: &Access,
    base: Credentials,
    identity: &str,
    role: &Role,
) -> ToolResult<(Credentials, chrono::DateTime<chrono::Utc>)> {
    let external_id = model::optional(&access.values, "external_id");
    let key = format!(
        "{}|{identity}|{}|{}",
        access.connection_id,
        role.arn,
        digest(external_id.unwrap_or_default())
    );
    if let (Some(credentials), Some(at)) = (cached(&key), expiry(&key)) {
        return Ok((credentials, at));
    }
    let session = format!("tilde-{}", access.connection_id);
    let duration = SESSION_SECONDS.to_string();
    let mut parameters = vec![
        ("Action", "AssumeRole"),
        ("RoleArn", role.arn.as_str()),
        ("RoleSessionName", session.as_str()),
        ("DurationSeconds", duration.as_str()),
    ];
    if let Some(external_id) = external_id {
        parameters.push(("ExternalId", external_id));
    }
    let response = call(access, Some(base), &parameters).await?;
    let (credentials, expires_at) = session_credentials(&response)?;
    remember(key, &credentials, expires_at);
    Ok((credentials, expires_at))
}

/// Credentials for a tool call in `account` (the connection's own when `None`).
pub async fn credentials(
    access: &Access,
    typ: &str,
    account: Option<&str>,
) -> ToolResult<Credentials> {
    let roles = roles(&access.values)?;
    let target = target(access, typ, &roles, account).await?;
    let (base, identity) = base(access, typ).await?;
    match target {
        None => Ok(base),
        Some(role) => Ok(assume(access, base, &identity, role).await?.0),
    }
}

/// Short-lived credentials for clients outside the engine (`connections::credentials`) and
/// when they expire (`None`: unknown). In the connection's own account (`account` `None` or
/// its own): the web identity session (`oidc`), else a `GetSessionToken` session from the
/// access keys; keys that already carry a session token are temporary and go as they are. In
/// another account: the session of the role the connection lists there, chained from those.
pub async fn session(
    access: &Access,
    typ: &str,
    account: Option<&str>,
) -> ToolResult<(Credentials, Option<chrono::DateTime<chrono::Utc>>)> {
    let roles = roles(&access.values)?;
    let target = target(access, typ, &roles, account).await?;
    let (base, identity) = base(access, typ).await?;
    if let Some(role) = target {
        let (credentials, expires_at) = assume(access, base, &identity, role).await?;
        return Ok((credentials, Some(expires_at)));
    }
    if typ == "oidc" || base.session_token.is_some() {
        let key = format!("{}|{identity}", access.connection_id);
        return Ok((base, expiry(&key)));
    }
    let key = format!("{}|{identity}|session", access.connection_id);
    if let (Some(credentials), Some(at)) = (cached(&key), expiry(&key)) {
        return Ok((credentials, Some(at)));
    }
    let duration = SESSION_SECONDS.to_string();
    let response = call(
        access,
        Some(base),
        &[
            ("Action", "GetSessionToken"),
            ("DurationSeconds", duration.as_str()),
        ],
    )
    .await?;
    let (credentials, expires_at) = session_credentials(&response)?;
    remember(key, &credentials, expires_at);
    Ok((credentials, Some(expires_at)))
}

/// The connection's base credentials and a non-secret name for them (the cache key part).
async fn base(access: &Access, typ: &str) -> ToolResult<(Credentials, String)> {
    if typ != "oidc" {
        let access_key_id = access.secret("access_key_id")?.to_owned();
        return Ok((
            Credentials {
                access_key_id: access_key_id.clone(),
                secret_access_key: access.secret("secret_access_key")?.to_owned(),
                session_token: model::optional(&access.values, "session_token").map(str::to_owned),
            },
            access_key_id,
        ));
    }
    let role_arn = access.secret("role_arn")?;
    let identity = format!("oidc:{role_arn}");
    let key = format!("{}|{identity}", access.connection_id);
    if let Some(credentials) = cached(&key) {
        return Ok((credentials, identity));
    }
    let issuer = access.oidc.as_ref().ok_or_else(|| {
        ConnectError::failed_precondition("Web identity is only available on the gateway")
    })?;
    let token = issuer
        .mint(&format!("connection:{}", access.connection_id), AUDIENCE)
        .await
        .map_err(|_| ConnectError::unavailable("Tilde could not sign a web identity token"))?;
    let session = format!("tilde-{}", access.connection_id);
    let duration = SESSION_SECONDS.to_string();
    let response = call(
        access,
        None,
        &[
            ("Action", "AssumeRoleWithWebIdentity"),
            ("RoleArn", role_arn),
            ("RoleSessionName", session.as_str()),
            ("DurationSeconds", duration.as_str()),
            ("WebIdentityToken", token.expose_secret()),
        ],
    )
    .await?;
    let (credentials, expires_at) = session_credentials(&response)?;
    remember(key, &credentials, expires_at);
    Ok((credentials, identity))
}

/// One STS Query API call, SigV4-signed with `signer` unless it is a web identity exchange.
async fn call(
    access: &Access,
    signer: Option<Credentials>,
    parameters: &[(&str, &str)],
) -> ToolResult<Zeroizing<String>> {
    let region = region(&access.values);
    let url = match access.endpoints.0.get("aws_sts") {
        Some(url) => url.clone(),
        None if region.starts_with("cn-") => format!("https://sts.{region}.amazonaws.com.cn/"),
        None => format!("https://sts.{region}.amazonaws.com/"),
    };
    let body = {
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("Version", "2011-06-15");
        for (key, value) in parameters {
            form.append_pair(key, value);
        }
        Zeroizing::new(form.finish())
    };
    let transport = || ConnectError::unavailable("AWS STS could not be reached");
    let mut request = access
        .http
        .client
        .post(&url)
        .header(
            "content-type",
            "application/x-www-form-urlencoded; charset=utf-8",
        )
        .body(body.as_bytes().to_vec())
        .build()
        .map_err(|_| transport())?;
    if let Some(credentials) = signer {
        Signer::new(credentials, region, "sts")
            .sign_request_at(&mut request, body.as_bytes(), std::time::SystemTime::now())
            .map_err(|_| ConnectError::internal("AWS request signing failed"))?;
    }
    let response = access
        .http
        .client
        .execute(request)
        .await
        .map_err(|_| transport())?;
    let status = response.status();
    let text = Zeroizing::new(response.text().await.map_err(|_| transport())?);
    if !status.is_success() {
        let action = parameters
            .iter()
            .find(|(k, _)| *k == "Action")
            .map_or("call", |(_, v)| *v);
        return Err(ConnectError::permission_denied(format!(
            "AWS STS refused {action}: {}: {}",
            tag(&text, "Code").unwrap_or_else(|| status.as_str().into()),
            tag(&text, "Message").unwrap_or_default()
        )));
    }
    Ok(text)
}
fn session_credentials(response: &str) -> ToolResult<(Credentials, chrono::DateTime<chrono::Utc>)> {
    let missing = || ConnectError::unknown("AWS STS returned no credentials");
    let expires_at = tag(response, "Expiration")
        .and_then(|e| chrono::DateTime::parse_from_rfc3339(&e).ok())
        .ok_or_else(missing)?
        .with_timezone(&chrono::Utc);
    Ok((
        Credentials {
            access_key_id: tag(response, "AccessKeyId").ok_or_else(missing)?,
            secret_access_key: tag(response, "SecretAccessKey").ok_or_else(missing)?,
            session_token: Some(tag(response, "SessionToken").ok_or_else(missing)?),
        },
        expires_at,
    ))
}
/// The text of the first `<name>` element, unescaped. STS answers are flat enough for this.
fn tag(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&format!("</{name}>"))?;
    Some(
        xml[start..end]
            .trim()
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}
