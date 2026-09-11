use crate::connections::categories::CATEGORY_DEVELOPER_TOOLS;
use crate::connections::model::*;

pub fn definition() -> Provider {
    Provider {
        account_name_label: Some("GitHub account".into()),
        icon_url: Some("https://cdn.jsdelivr.net/gh/glincker/thesvg@main/public/icons/github/default.svg".into()),
        instructions: Some("Create a GitHub App or connect an existing installation to receive repository conversations.".into()),
        id: "github".into(),
        name: "GitHub".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![ConnectionType {
            id: "github_app".into(),
            name: "GitHub App".into(),
            credential_source: CredentialSource::Custom,
            capabilities: vec![Capability::Channel],
        }],
    }
}

use super::{Endpoints, url};
use crate::{
    connections::oauth::{Http, Token},
    error::Error,
};
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};

pub(crate) async fn github_convert(
    http: &Http,
    endpoints: &Endpoints,
    code: &str,
) -> Result<Values, Error> {
    let response = http
        .json(
            http.client
                .post(url(
                    endpoints.get("github_api", "https://api.github.com"),
                    &["app-manifests", code, "conversions"],
                )?)
                .header("accept", "application/vnd.github+json")
                .header("x-github-api-version", "2022-11-28"),
        )
        .await?;
    let id = response
        .0
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| invalid("GitHub app ID missing"))?;
    let mut values = Values::new();
    values.insert("app_id".into(), SecretString::from(id.to_string()));
    for (key, path) in [
        ("slug", "/slug"),
        ("client_id", "/client_id"),
        ("client_secret", "/client_secret"),
        ("private_key", "/pem"),
        ("webhook_secret", "/webhook_secret"),
    ] {
        values.insert(key.into(), response.secret(path)?);
    }
    Ok(values)
}

pub(crate) fn github_install_url(
    endpoints: &Endpoints,
    values: &Values,
    state: &str,
) -> Result<String, Error> {
    let mut url = url(
        endpoints.get("github_web", "https://github.com"),
        &["apps", value(values, "slug")?, "installations", "new"],
    )?;
    url.query_pairs_mut().append_pair("state", state);
    Ok(url.to_string())
}

pub(crate) async fn github_token(
    http: &Http,
    endpoints: &Endpoints,
    values: &Values,
) -> Result<(Token, String), Error> {
    let installation = value(values, "installation_id")?;
    installation
        .parse::<u64>()
        .map_err(|_| invalid("Invalid GitHub installation ID"))?;
    let jwt = app_assertion(values)?;
    let installation_info = http
        .json(
            http.client
                .get(url(
                    endpoints.get("github_api", "https://api.github.com"),
                    &["app", "installations", installation],
                )?)
                .bearer_auth(jwt.expose_secret())
                .header("accept", "application/vnd.github+json"),
        )
        .await?;
    if installation_info
        .0
        .get("id")
        .and_then(|v| v.as_u64())
        .map(|n| n.to_string())
        .as_deref()
        != Some(installation)
    {
        return Err(invalid("GitHub installation does not match this app"));
    }
    let account = installation_info
        .0
        .pointer("/account/login")
        .and_then(|v| v.as_str())
        .unwrap_or("GitHub App")
        .to_string();
    let token = http
        .json(
            http.client
                .post(url(
                    endpoints.get("github_api", "https://api.github.com"),
                    &["app", "installations", installation, "access_tokens"],
                )?)
                .bearer_auth(jwt.expose_secret())
                .header("accept", "application/vnd.github+json"),
        )
        .await?;
    let expires_at = DateTime::parse_from_rfc3339(token.text("/expires_at")?)
        .map_err(|_| invalid("Invalid GitHub token expiry"))?
        .with_timezone(&Utc);
    Ok((
        Token {
            access: token.secret("/token")?,
            refresh: None,
            expires_at: Some(expires_at),
            scope: None,
            provider_values: Values::new(),
        },
        account,
    ))
}

pub(crate) async fn github_existing(
    http: &Http,
    endpoints: &Endpoints,
    values: &Values,
) -> Result<Values, Error> {
    let jwt = app_assertion(values)?;
    let app = http
        .json(
            http.client
                .get(url(
                    endpoints.get("github_api", "https://api.github.com"),
                    &["app"],
                )?)
                .bearer_auth(jwt.expose_secret())
                .header("accept", "application/vnd.github+json"),
        )
        .await?;
    if app
        .0
        .get("id")
        .and_then(|v| v.as_u64())
        .map(|id| id.to_string())
        .as_deref()
        != Some(value(values, "app_id")?)
    {
        return Err(invalid(
            "The private key does not match the supplied GitHub App ID",
        ));
    }
    let mut out = Values::new();
    out.insert("slug".into(), app.secret("/slug")?);
    if let Some(client_id) = app.0.get("client_id").and_then(|v| v.as_str()) {
        out.insert("client_id".into(), SecretString::from(client_id));
    }
    Ok(out)
}

use super::runtime::Runtime;
use crate::connections::service::Connections;

pub(crate) struct Github;
#[async_trait::async_trait]
impl Runtime for Github {
    fn instructions(&self, _typ: &ConnectionType) -> &'static [&'static str] {
        &[
            "Create a GitHub App or connect an existing installation using the form below.",
            "For an existing app, configure its webhook with the Webhook URL below and enter the matching signing secret.",
        ]
    }

    fn ui(&self) -> &'static str {
        "github"
    }
    fn validate_input(
        &self,
        _typ: &ConnectionType,
        step: &str,
        values: &Values,
    ) -> Result<(), Error> {
        match step {
            "fields" => {
                super::runtime::input(values, &["setup_path"], &[])?;
                if !matches!(value(values, "setup_path")?, "create" | "existing") {
                    return Err(invalid("Invalid setup path"));
                }
                Ok(())
            }
            "github_create" => {
                super::runtime::input(values, &["app_name", "owner_type"], &["account"])
            }
            "github_existing" => super::runtime::input(
                values,
                &["app_id", "private_key", "installation_id", "webhook_secret"],
                &[],
            ),
            _ => Err(invalid("This step requires a provider callback")),
        }
    }

    fn resume_step(&self) -> Option<&'static str> {
        Some("github_install")
    }
    fn accepts_callback(&self, step: &str, parameters: &Values, denied: bool) -> bool {
        let code = optional(parameters, "code");
        let installation = optional(parameters, "installation_id");
        match step {
            "github_manifest" => denied || (code.is_some() && installation.is_none()),
            "github_install" => denied || (installation.is_some() && code.is_none()),
            _ => false,
        }
    }
    async fn action(
        &self,
        service: &Connections,
        setup: &Setup,
        _typ: &ConnectionType,
    ) -> Result<Action, Error> {
        if matches!(
            setup.step.as_str(),
            "fields" | "github_create" | "github_existing"
        ) {
            return Ok(Action::Form);
        }
        let values = service.staged(setup).await?;
        let state = service.callback_state(setup)?;
        let action = match setup.step.as_str() {
            "github_manifest" => manifest::manifest(
                &service.endpoints,
                &values,
                &service.public_url,
                &service.callback_url(setup)?,
                state.expose_secret(),
                &format!(
                    "{}/connections/webhooks/{}",
                    service.event_ingress_url, setup.connection_id
                ),
            )?,
            "github_install" => Action::Redirect {
                url: github_install_url(&service.endpoints, &values, state.expose_secret())?,
            },
            _ => return Err(invalid("Unexpected GitHub setup phase")),
        };
        Ok(action)
    }
    async fn start(
        &self,
        service: &Connections,
        setup: &Setup,
        _typ: &ConnectionType,
        mut values: Values,
    ) -> Result<(), Error> {
        if setup.step == "fields" {
            return service
                .transition(setup, &format!("github_{}", value(&values, "setup_path")?))
                .await;
        }
        service.stage(setup, &values).await?;
        if setup.step == "github_existing" {
            let account = github_existing(&service.http, &service.endpoints, &values).await?;
            service.stage(setup, &account).await?;
            values.extend(account);
            let (token, label) = github_token(&service.http, &service.endpoints, &values).await?;
            return service.complete(setup, Some(token), Some(label)).await;
        }
        manifest::manifest(
            &service.endpoints,
            &values,
            &service.public_url,
            &service.callback_url(setup)?,
            "validate",
            &format!(
                "{}/connections/webhooks/{}",
                service.event_ingress_url, setup.connection_id
            ),
        )?;
        service.transition(setup, "github_manifest").await
    }
    async fn callback(
        &self,
        service: &Connections,
        setup: &Setup,
        _typ: &ConnectionType,
        parameters: &Values,
    ) -> Result<(), Error> {
        let code = optional(parameters, "code");
        let installation = optional(parameters, "installation_id");
        match setup.step.as_str() {
            "github_manifest" => {
                let generated = github_convert(
                    &service.http,
                    &service.endpoints,
                    code.ok_or_else(|| invalid("GitHub manifest code missing"))?,
                )
                .await?;
                service.stage(setup, &generated).await?;
                service.transition(setup, "github_install").await
            }
            "github_install" => {
                let installation =
                    installation.ok_or_else(|| invalid("GitHub installation missing"))?;
                installation
                    .parse::<u64>()
                    .map_err(|_| invalid("Invalid installation ID"))?;
                let mut values = service.staged(setup).await?;
                values.insert("installation_id".into(), SecretString::from(installation));
                service.stage(setup, &values).await?;
                let (token, label) =
                    github_token(&service.http, &service.endpoints, &values).await?;
                service.complete(setup, Some(token), Some(label)).await
            }
            _ => Err(invalid("Unexpected GitHub callback phase")),
        }
    }
    async fn refresh(
        &self,
        service: &Connections,
        _typ: &ConnectionType,
        values: &Values,
    ) -> Result<Token, Error> {
        github_token(&service.http, &service.endpoints, values)
            .await
            .map(|(token, _)| token)
    }
}

use zeroize::Zeroizing;
/// RS256 is the assertion algorithm used by the source Google and GitHub app implementations.
fn app_assertion(values: &Values) -> Result<SecretString, Error> {
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
    let issuer = optional(values, "client_id").unwrap_or(value(values, "app_id")?);
    let claims = Claims {
        iss: issuer,
        iat: now - 60,
        exp: now + 540,
        aud: None,
        scope: None,
        sub: None,
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
        &claims,
        &key,
    )
    .map(SecretString::from)
    .map_err(|_| invalid("Unable to sign provider assertion"))
}

mod manifest;
