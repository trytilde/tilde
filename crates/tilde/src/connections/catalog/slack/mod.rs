use crate::connections::categories::CATEGORY_CHAT;
use crate::connections::model::*;

pub fn definition() -> Provider {
    Provider {
        id: "slack".into(),
        name: "Slack".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_CHAT.into()],
        connection_types: vec![ConnectionType {
            id: "slack_app".into(),
            name: "Slack App".into(),
            credential_source: CredentialSource::Custom,
            capabilities: vec![Capability::Channel],
        }],
    }
}

use super::{Endpoints, url};
use crate::{
    connections::oauth::{self, Http, Token},
    error::Error,
};
use secrecy::ExposeSecret;
use serde_json::json;

pub(crate) async fn slack_create(
    http: &Http,
    endpoints: &Endpoints,
    values: &Values,
    callback: &str,
    webhook_url: &str,
) -> Result<Values, Error> {
    let rotated = http
        .json(
            http.client
                .post(url(
                    endpoints.get("slack_api", "https://slack.com/api"),
                    &["tooling.tokens.rotate"],
                )?)
                .form(&[(
                    "refresh_token",
                    value(values, "configuration_refresh_token")?,
                )]),
        )
        .await?;
    let token = rotated.secret("/token")?;
    let manifest = json!({"display_information":{"name":value(values,"app_name")?,"description":"A user-owned Tilde channel app"},"features":{"bot_user":{"display_name":value(values,"app_name")?,"always_online":true},"app_home":{"messages_tab_enabled":true,"messages_tab_read_only_enabled":false}},"oauth_config":{"redirect_urls":[callback],"scopes":{"bot":slack_scopes()}},"settings":{"socket_mode_enabled":false,"event_subscriptions":{"request_url":webhook_url,"bot_events":["message.channels","message.groups","message.im","message.mpim","app_mention","member_joined_channel","member_left_channel"]}}});
    let app = http
        .json(
            http.client
                .post(url(
                    endpoints.get("slack_api", "https://slack.com/api"),
                    &["apps.manifest.create"],
                )?)
                .bearer_auth(token.expose_secret())
                .json(&json!({"manifest":manifest.to_string()})),
        )
        .await?;
    let mut out = Values::new();
    for (key, path) in [
        ("app_id", "/app_id"),
        ("client_id", "/credentials/client_id"),
        ("client_secret", "/credentials/client_secret"),
        ("signing_secret", "/credentials/signing_secret"),
    ] {
        out.insert(key.into(), app.secret(path)?);
    }
    out.insert(
        "configuration_refresh_token".into(),
        rotated.secret("/refresh_token")?,
    );
    Ok(out)
}

pub(crate) fn slack_oauth(endpoints: &Endpoints) -> OAuth {
    let mut config = OAuth::standard(&format!(
        "{}/oauth.v2.access",
        endpoints
            .get("slack_api", "https://slack.com/api")
            .trim_end_matches('/')
    ));
    config.authorization_url = Some(
        endpoints
            .get("slack_authorize", "https://slack.com/oauth/v2/authorize")
            .to_owned(),
    );
    config.pkce = false;
    config.scopes = slack_scopes().into_iter().map(str::to_owned).collect();
    config.scope_separator = ",".into();
    config.success_path = Some("/ok".into());
    config.result_fields = [
        ("slack_team_id", "/team/id", true),
        ("slack_team_name", "/team/name", false),
        ("slack_enterprise_id", "/enterprise/id", false),
        ("slack_api_app_id", "/app_id", true),
        ("slack_bot_user_id", "/bot_user_id", true),
    ]
    .into_iter()
    .map(|(key, path, required)| ResultField {
        key: key.into(),
        path: path.into(),
        required,
    })
    .collect();
    config
}

pub(crate) fn slack_account_label(values: &Values) -> Option<String> {
    optional(values, "slack_team_name")
        .or_else(|| optional(values, "slack_team_id"))
        .map(str::to_owned)
}

fn slack_scopes() -> Vec<&'static str> {
    vec![
        "app_mentions:read",
        "channels:history",
        "channels:join",
        "channels:read",
        "chat:write",
        "chat:write.public",
        "groups:history",
        "groups:read",
        "im:history",
        "im:read",
        "im:write",
        "mpim:history",
        "mpim:read",
        "files:read",
        "reactions:read",
        "reactions:write",
        "users:read",
        "users:read.email",
    ]
}

use super::runtime::Runtime;
use crate::connections::service::Connections;
pub(crate) struct Slack;
#[async_trait::async_trait]
impl Runtime for Slack {
    fn ui(&self) -> &'static str {
        "slack"
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
            "slack_create" => {
                super::runtime::input(values, &["app_name", "configuration_refresh_token"], &[])
            }
            "slack_existing" => super::runtime::input(
                values,
                &["client_id", "client_secret", "signing_secret"],
                &[],
            ),
            _ => Err(invalid("This step requires a provider callback")),
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
            "fields" | "slack_create" | "slack_existing"
        ) {
            return Ok(Action::Form);
        }
        if setup.step != "oauth_consent" {
            return Err(invalid("Unexpected setup phase"));
        }
        let values = service.staged(setup).await?;
        let state = service.callback_state(setup)?;
        Ok(Action::Redirect {
            url: oauth::authorization_url(
                &slack_oauth(&service.endpoints),
                &values,
                &service.callback_url(setup)?,
                state.expose_secret(),
            )?,
        })
    }

    fn resume_step(&self) -> Option<&'static str> {
        Some("oauth_consent")
    }
    fn oauth(&self, service: &Connections, _typ: &ConnectionType) -> Result<OAuth, Error> {
        Ok(slack_oauth(&service.endpoints))
    }
    fn account(&self, values: &Values) -> Option<String> {
        slack_account_label(values)
    }
    async fn start(
        &self,
        service: &Connections,
        setup: &Setup,
        _typ: &ConnectionType,
        values: Values,
    ) -> Result<(), Error> {
        if setup.step == "fields" {
            return service
                .transition(setup, &format!("slack_{}", value(&values, "setup_path")?))
                .await;
        }
        service.stage(setup, &values).await?;
        if setup.step == "slack_create" {
            let generated = slack_create(
                &service.http,
                &service.endpoints,
                &values,
                &service.callback_url(setup)?,
                &format!(
                    "{}/connections/webhooks/{}",
                    service.public_url, setup.connection_id
                ),
            )
            .await?;
            service.stage(setup, &generated).await?;
        }
        service.transition(setup, "oauth_consent").await
    }
    async fn refresh(
        &self,
        service: &Connections,
        typ: &ConnectionType,
        values: &Values,
    ) -> Result<Token, Error> {
        oauth::exchange(
            &service.http,
            Driver::OAuthCode,
            &self.oauth(service, typ)?,
            values,
            None,
            None,
            true,
        )
        .await
    }
}
