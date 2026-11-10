//! The AWS credentials a connection's tool call runs with, through STS.
//!
//! Base credentials are the connection's access keys (`iam`), or for `oidc` a session from
//! `AssumeRoleWithWebIdentity` into `role_arn` with a token from Tilde's OIDC issuer
//! (`sub` = `connection:<id>`, `aud` = `sts.amazonaws.com`). A call naming another account uses
//! the connection's listed role in that account (`assume_role_arns`), assumed from the base
//! credentials with `sts:AssumeRole`, the connection's `external_id` and the session name
//! `tilde-<connection id>`. An account the connection lists no role for is refused.
//!
//! Temporary credentials are cached per connection, base identity and role until five minutes
//! before they expire. `Credentials` zeroize on drop, STS responses are read into zeroizing
//! buffers, and no credential is logged or returned in an error.
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::model;
use client_aws_sigv4::{Credentials, Signer};
use connectrpc::ConnectError;
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use zeroize::Zeroizing;

pub const AUDIENCE: &str = "sts.amazonaws.com";
const DEFAULT_REGION: &str = "us-east-1";
/// Refresh this long before STS says a session expires.
const MARGIN_SECONDS: i64 = 300;
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

struct Cached {
    credentials: Credentials,
    expires_at: chrono::DateTime<chrono::Utc>,
}
static SESSIONS: LazyLock<Mutex<HashMap<String, Cached>>> = LazyLock::new(Default::default);
/// Accounts of access keys, from `GetCallerIdentity`; account IDs are not secret.
static ACCOUNTS: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);

fn cached(key: &str) -> Option<Credentials> {
    let sessions = SESSIONS.lock().ok()?;
    let entry = sessions.get(key)?;
    (entry.expires_at - chrono::Duration::seconds(MARGIN_SECONDS) > chrono::Utc::now())
        .then(|| entry.credentials.clone())
}
fn remember(key: String, credentials: &Credentials, expires_at: chrono::DateTime<chrono::Utc>) {
    if let Ok(mut sessions) = SESSIONS.lock() {
        let now = chrono::Utc::now();
        sessions.retain(|_, s| s.expires_at > now);
        sessions.insert(
            key,
            Cached {
                credentials: credentials.clone(),
                expires_at,
            },
        );
    }
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

/// Credentials for a call in `account` (the connection's own when `None`).
pub async fn credentials(
    access: &Access,
    typ: &str,
    account: Option<&str>,
) -> ToolResult<Credentials> {
    let roles = roles(&access.values)?;
    let target = match account {
        None => None,
        Some(account) => match roles.iter().find(|r| r.account == account) {
            Some(role) => Some(role),
            None if own_account(access, typ).await? == account => None,
            None => {
                let reachable = roles
                    .iter()
                    .map(|r| r.account.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(ConnectError::permission_denied(format!(
                    "AWS account {account} is not reachable from this connection: it lists no role in it. Reachable accounts: its own{}{reachable}.",
                    if reachable.is_empty() { "" } else { ", " }
                )));
            }
        },
    };
    let (base, identity) = base(access, typ).await?;
    let Some(role) = target else {
        return Ok(base);
    };
    let external_id = model::optional(&access.values, "external_id");
    let key = format!(
        "{}|{identity}|{}|{}",
        access.connection_id,
        role.arn,
        digest(external_id.unwrap_or_default())
    );
    if let Some(credentials) = cached(&key) {
        return Ok(credentials);
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
    Ok(credentials)
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
