//! Shared OAuth execution. Provider definitions configure protocol details, not executable scripts.
use super::model::*;
use crate::error::Error;
use base64::Engine;
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

/// Provider JSON can contain tokens at arbitrary mapped paths; clear owned string values on drop.
pub(crate) struct PrivateJson(pub Value);
impl Drop for PrivateJson {
    fn drop(&mut self) {
        fn clear(v: &mut Value) {
            match v {
                Value::String(s) => s.zeroize(),
                Value::Array(a) => a.iter_mut().for_each(clear),
                Value::Object(o) => o.values_mut().for_each(clear),
                _ => {}
            }
        }
        clear(&mut self.0);
    }
}
impl PrivateJson {
    pub fn text(&self, path: &str) -> Result<&str, Error> {
        self.0
            .pointer(path)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("Provider response omitted a required value"))
    }
    pub fn secret(&self, path: &str) -> Result<SecretString, Error> {
        Ok(SecretString::from(self.text(path)?))
    }
}
#[derive(Clone)]
pub struct Http {
    pub(crate) client: reqwest::Client,
}
impl Http {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(25))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("tilde-connections")
                .build()
                .map_err(|_| invalid("Unable to create provider HTTP client"))?,
        })
    }
    pub(crate) async fn json(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<PrivateJson, Error> {
        let mut response = request
            .send()
            .await
            .map_err(|_| invalid("Provider request failed; external outcome may be uncertain"))?;
        let status = response.status();
        if matches!(status.as_u16(), 401 | 403) {
            return Err(Error::ConnectionAuthorizationRequired);
        }
        if response
            .content_length()
            .is_some_and(|length| length > 1024 * 1024)
        {
            return Err(invalid("Provider response is too large"));
        }
        let mut bytes = Zeroizing::new(Vec::new());
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| invalid("Unable to read provider response"))?
        {
            if bytes.len() + chunk.len() > 1024 * 1024 {
                return Err(invalid("Provider response is too large"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let parsed: Value = serde_json::from_slice(&bytes)
            .map_err(|_| invalid("Invalid provider JSON response"))?;
        let value = PrivateJson(parsed);
        if value
            .0
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|code| {
                [
                    "invalid_grant",
                    "invalid_client",
                    "unauthorized_client",
                    "access_denied",
                    "invalid_token",
                    "invalid_auth",
                    "token_revoked",
                    "account_inactive",
                ]
                .contains(&code)
            })
        {
            return Err(Error::ConnectionAuthorizationRequired);
        }
        if !status.is_success() {
            return Err(invalid("Provider rejected the request"));
        }

        if value.0.get("ok").and_then(Value::as_bool) == Some(false) {
            return Err(invalid("Provider rejected the request"));
        }
        Ok(value)
    }
}

pub(crate) struct Token {
    pub access: SecretString,
    pub refresh: Option<SecretString>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scope: Option<String>,
    pub provider_values: Values,
}
/// No verifier is put into an authorization URL; only its S256 challenge leaves the server.
pub(crate) fn authorization_url(
    config: &OAuth,
    values: &Values,
    callback: &str,
    state: &str,
) -> Result<String, Error> {
    let mut url = endpoint(
        config
            .authorization_url
            .as_deref()
            .ok_or_else(|| invalid("Authorization URL missing"))?,
    )?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("response_type", "code")
            .append_pair("client_id", value(values, "client_id")?)
            .append_pair("redirect_uri", callback)
            .append_pair("state", state);
        if !config.scopes.is_empty() {
            q.append_pair("scope", &config.scopes.join(&config.scope_separator));
        }
        if config.pkce {
            let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(Sha256::digest(value(values, "_pkce")?.as_bytes()));
            q.append_pair("code_challenge_method", "S256")
                .append_pair("code_challenge", &challenge);
        }
        for (k, v) in &config.authorization_parameters {
            q.append_pair(k, v);
        }
    }
    Ok(url.to_string())
}

/// Explicit secret wrappers cover form values and encoded bodies; neither is logged.
pub(crate) async fn exchange(
    http: &Http,
    driver: Driver,
    config: &OAuth,
    values: &Values,
    code: Option<&str>,
    callback: Option<&str>,
    refresh: bool,
) -> Result<Token, Error> {
    let mut form: Vec<(String, SecretString)> = config
        .token_parameters
        .iter()
        .map(|(k, v)| (k.clone(), SecretString::from(v.as_str())))
        .collect();
    let mut add = |key: &str, v: &str| form.push((key.into(), SecretString::from(v)));
    if refresh {
        add("grant_type", "refresh_token");
        add("refresh_token", value(values, "refresh_token")?);
    } else {
        match driver {
            Driver::OAuthCode => {
                add("grant_type", "authorization_code");
                add("code", code.ok_or_else(|| invalid("OAuth code missing"))?);
                add(
                    "redirect_uri",
                    callback.ok_or_else(|| invalid("OAuth callback missing"))?,
                );
                if config.pkce {
                    add("code_verifier", value(values, "_pkce")?);
                }
            }
            Driver::OAuthClientCredentials => add("grant_type", "client_credentials"),
            Driver::OAuthJwtBearer => {
                let assertion = jwt_assertion(
                    values,
                    &config.token_url,
                    &config.scopes.join(&config.scope_separator),
                )?;
                add("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer");
                add("assertion", assertion.expose_secret());
            }
            _ => return Err(invalid("Unsupported OAuth grant")),
        }
    }
    if driver != Driver::OAuthJwtBearer && config.client_auth != ClientAuth::Basic {
        add("client_id", value(values, "client_id")?);
        if config.client_auth == ClientAuth::Body {
            add("client_secret", value(values, "client_secret")?);
        }
    }
    if !config.scopes.is_empty() {
        add("scope", &config.scopes.join(&config.scope_separator));
    }
    let encoded = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in &form {
            serializer.append_pair(key, value.expose_secret());
        }
        Zeroizing::new(serializer.finish())
    };
    let mut request = http
        .client
        .post(endpoint(&config.token_url)?)
        .header("accept", "application/json")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(encoded.as_bytes().to_vec());
    if driver != Driver::OAuthJwtBearer && config.client_auth == ClientAuth::Basic {
        let client_id = Zeroizing::new(
            url::form_urlencoded::byte_serialize(value(values, "client_id")?.as_bytes())
                .collect::<String>(),
        );
        let client_secret = Zeroizing::new(
            url::form_urlencoded::byte_serialize(value(values, "client_secret")?.as_bytes())
                .collect::<String>(),
        );
        request = request.basic_auth(client_id.as_str(), Some(client_secret.as_str()));
    }
    let json = http.json(request).await?;
    parse_token(&json, config, refresh)
}
pub(crate) fn parse_token(
    json: &PrivateJson,
    config: &OAuth,
    is_refresh: bool,
) -> Result<Token, Error> {
    if let Some(path) = &config.success_path
        && json.0.pointer(path).and_then(Value::as_bool) != Some(true)
    {
        return Err(invalid("OAuth response reported failure"));
    }
    let access = json.secret(&config.access_token_path)?;
    let refresh = json
        .0
        .pointer(&config.refresh_token_path)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(SecretString::from);
    let expires = json
        .0
        .pointer(&config.expires_in_path)
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|v| v.parse().ok()))
                .filter(|v| *v > 0)
                .ok_or_else(|| invalid("Invalid OAuth token expiry"))
        })
        .transpose()?;
    let expires_at = expires
        .map(|seconds| {
            chrono::Duration::try_seconds(seconds)
                .and_then(|duration| Utc::now().checked_add_signed(duration))
                .ok_or_else(|| invalid("OAuth token expiry overflow"))
        })
        .transpose()?;
    let scope = json
        .0
        .pointer(&config.scope_path)
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut provider_values = Values::new();
    for field in &config.result_fields {
        let scalar = json.0.pointer(&field.path).and_then(|value| match value {
            Value::String(value) if !value.is_empty() => Some(SecretString::from(value.as_str())),
            Value::Number(_) | Value::Bool(_) => Some(SecretString::from(value.to_string())),
            _ => None,
        });
        if let Some(value) = scalar {
            provider_values.insert(field.key.clone(), value);
        } else if field.required && !is_refresh {
            return Err(invalid(
                "Provider response omitted a required account field",
            ));
        }
    }
    Ok(Token {
        access,
        refresh,
        expires_at,
        scope,
        provider_values,
    })
}
/// Sign a standard RS256 OAuth JWT bearer assertion.
pub(crate) fn jwt_assertion(
    values: &Values,
    audience: &str,
    scope: &str,
) -> Result<SecretString, Error> {
    #[derive(serde::Serialize)]
    struct Claims<'a> {
        iss: &'a str,
        iat: i64,
        exp: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        aud: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sub: Option<&'a str>,
    }
    let pem = Zeroizing::new(value(values, "private_key")?.replace("\\n", "\n"));
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes())
        .map_err(|_| invalid("Invalid RSA private key"))?;
    let now = Utc::now().timestamp();
    let issuer = value(values, "issuer")?;
    let claims = Claims {
        iss: issuer,
        iat: now - 60,
        exp: now + 3600,
        aud: Some(audience),
        scope: (!scope.is_empty()).then_some(scope),
        sub: optional(values, "subject"),
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
        &claims,
        &key,
    )
    .map(SecretString::from)
    .map_err(|_| invalid("Unable to sign provider assertion"))
}
