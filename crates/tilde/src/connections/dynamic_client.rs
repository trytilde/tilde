//! OAuth for MCP servers that register their clients dynamically, as the MCP authorization spec
//! describes: the server names its authorization server (RFC 9728 protected resource metadata,
//! found through `WWW-Authenticate` or the well-known path), whose metadata (RFC 8414 or OpenID
//! discovery) gives the endpoints; Tilde registers a public client there (RFC 7591) with PKCE and
//! sends the server as the `resource` (RFC 8707). Discovery runs once per setup; the result is
//! kept with the connection so refresh needs no rediscovery. Every URL here comes from the
//! server, so each request goes through `Http::discovered`: HTTPS to public addresses only.
use super::model::{OAuth, Values, invalid};
use crate::error::Error;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};

/// Connection values the discovered client lives in. They are credentials like any other, kept
/// encrypted with the connection.
pub const AUTHORIZATION_URL: &str = "oauth_authorization_url";
pub const TOKEN_URL: &str = "oauth_token_url";
pub const RESOURCE: &str = "oauth_resource";
pub const SCOPE: &str = "oauth_scope";

/// Discover the server's authorization server and register Tilde as a client for `redirect_uri`.
pub async fn register(
    http: &super::oauth::Http,
    server: &str,
    redirect_uri: &str,
) -> Result<Values, Error> {
    let server_url = url::Url::parse(server).map_err(|_| invalid("Invalid MCP server URL"))?;
    let failed = |what: &str| invalid(&format!("The MCP server's OAuth discovery failed: {what}"));
    // An unauthenticated request is answered 401 with the metadata location; servers that omit
    // it serve the metadata at the well-known path for the server's own path.
    let probe = http
        .discovered(server)
        .await?
        .post(server_url.clone())
        .header("accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"tilde","version":env!("CARGO_PKG_VERSION")}}}))
        .send()
        .await
        .map_err(|_| failed("the server is unreachable"))?;
    let advertised = probe
        .headers()
        .get_all(reqwest::header::WWW_AUTHENTICATE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|header| parameter(header, "resource_metadata"));
    let metadata_url = match advertised {
        Some(url) => url,
        None => well_known(&server_url, "oauth-protected-resource")?,
    };
    let resource = get(http, &metadata_url)
        .await
        .map_err(|_| failed("no protected resource metadata"))?;
    let issuer = resource["authorization_servers"][0]
        .as_str()
        .ok_or_else(|| failed("no authorization server"))?;
    let issuer_url = url::Url::parse(issuer).map_err(|_| failed("invalid authorization server"))?;
    let mut authorization = Value::Null;
    for candidate in [
        well_known(&issuer_url, "oauth-authorization-server")?,
        format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        ),
    ] {
        if let Ok(found) = get(http, &candidate).await {
            authorization = found;
            break;
        }
    }
    let text = |key: &str| {
        authorization[key]
            .as_str()
            .filter(|v| v.starts_with("https://") || v.starts_with("http://"))
            .map(str::to_owned)
            .ok_or_else(|| failed(&format!("no {key}")))
    };
    let (authorize, token, registration) = (
        text("authorization_endpoint")?,
        text("token_endpoint")?,
        text("registration_endpoint")?,
    );
    // The token endpoint is dialed again at every exchange; refuse a private one up front.
    http.discovered(&token).await?;
    if !authorization["code_challenge_methods_supported"]
        .as_array()
        .is_some_and(|methods| methods.iter().any(|m| m == "S256"))
    {
        return Err(failed("PKCE (S256) is not supported"));
    }
    let registered = http
        .discovered(&registration)
        .await?
        .post(&registration)
        .json(&json!({
            "client_name": "Tilde",
            "redirect_uris": [redirect_uri],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        }))
        .send()
        .await
        .map_err(|_| failed("client registration was unreachable"))?;
    if !registered.status().is_success() {
        return Err(failed("client registration was refused"));
    }
    let client: Value = registered
        .json()
        .await
        .map_err(|_| failed("invalid client registration"))?;
    let client_id = client["client_id"]
        .as_str()
        .ok_or_else(|| failed("no client ID"))?;
    let scopes = resource["scopes_supported"]
        .as_array()
        .or(authorization["scopes_supported"].as_array())
        .map(|scopes| {
            scopes
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let mut values = Values::new();
    values.insert("client_id".into(), SecretString::from(client_id));
    if let Some(secret) = client["client_secret"].as_str() {
        values.insert("client_secret".into(), SecretString::from(secret));
    }
    values.insert(AUTHORIZATION_URL.into(), SecretString::from(authorize));
    values.insert(TOKEN_URL.into(), SecretString::from(token));
    values.insert(
        RESOURCE.into(),
        SecretString::from(resource["resource"].as_str().unwrap_or(server)),
    );
    values.insert(SCOPE.into(), SecretString::from(scopes));
    Ok(values)
}

/// A dynamic type's OAuth configuration for one connection, from what its setup discovered.
pub fn resolve(mut config: OAuth, values: &Values) -> Result<OAuth, Error> {
    let value = |key: &str| {
        values
            .get(key)
            .map(|v| v.expose_secret().to_owned())
            .ok_or_else(|| invalid("The connection's OAuth client was not registered"))
    };
    config.authorization_url = Some(value(AUTHORIZATION_URL)?);
    config.token_url = value(TOKEN_URL)?;
    let scope = value(SCOPE)?;
    config.scopes = scope.split_whitespace().map(str::to_owned).collect();
    config.scope_separator = " ".into();
    config.pkce = true;
    config.client_auth = if values.contains_key("client_secret") {
        super::model::ClientAuth::Body
    } else {
        super::model::ClientAuth::None
    };
    let resource = value(RESOURCE)?;
    config
        .authorization_parameters
        .insert("resource".into(), resource.clone());
    config.token_parameters.insert("resource".into(), resource);
    Ok(config)
}

async fn get(http: &super::oauth::Http, url: &str) -> Result<Value, ()> {
    let response = http
        .discovered(url)
        .await
        .map_err(|_| ())?
        .get(url)
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|_| ())?;
    if !response.status().is_success() {
        return Err(());
    }
    response.json().await.map_err(|_| ())
}
/// `https://host/.well-known/<name>/<path>`: the RFC 8414/9728 location for a URL with a path.
fn well_known(url: &url::Url, name: &str) -> Result<String, Error> {
    let path = url.path().trim_end_matches('/');
    Ok(format!(
        "{}/.well-known/{name}{path}",
        url.origin().ascii_serialization()
    ))
}
/// One `key="value"` parameter of a `WWW-Authenticate` challenge.
fn parameter(header: &str, key: &str) -> Option<String> {
    header.split(',').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        let name = name.rsplit(' ').next()?;
        (name.eq_ignore_ascii_case(key)).then(|| value.trim_matches('"').to_owned())
    })
}
