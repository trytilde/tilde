//! Catalog participants own setup semantics; the broker owns durable claims and encrypted writes.
use crate::{
    connections::{
        model::*,
        oauth::{self, Token},
        service::Connections,
    },
    error::Error,
};
use secrecy::ExposeSecret;

/// Small adapter boundary shared by built-in catalog entries and runtime-defined OAuth/static entries.
#[async_trait::async_trait]
pub(crate) trait Runtime: Send + Sync {
    /// The compiled page is arbitrary provider-owned HTML/JS, not a server form descriptor.
    fn ui(&self) -> &'static str {
        "_standard"
    }
    /// Ordered setup steps for the selected connection type, below the provider overview.
    fn instructions(&self, _typ: &ConnectionType) -> &'static [&'static str] {
        &[]
    }
    /// Credential supplied from the saved account name during the fields step.
    /// Keep it in the canonical schema for validation; the broker omits it from the form.
    fn account_name_field(&self, _typ: &ConnectionType) -> Option<&'static str> {
        None
    }
    fn validate_input(
        &self,
        typ: &ConnectionType,
        step: &str,
        values: &Values,
    ) -> Result<(), Error> {
        if step != "fields" {
            return Err(invalid("This step requires a provider callback"));
        }
        typ.validate_values(values)
    }
    fn resume_step(&self) -> Option<&'static str> {
        None
    }
    fn accepts_callback(&self, step: &str, parameters: &Values, denied: bool) -> bool {
        step == "oauth_consent" && (denied || parameters.contains_key("code"))
    }
    fn oauth(&self, _service: &Connections, typ: &ConnectionType) -> Result<OAuth, Error> {
        typ.oauth()
            .cloned()
            .ok_or_else(|| invalid("OAuth configuration missing"))
    }
    /// Actual sending address/bot account; never the editable connection display name.
    fn channel_identity(
        &self,
        _values: &Values,
        _account: Option<&str>,
    ) -> Option<crate::chat::access::identity::Identity> {
        None
    }
    fn account(&self, _values: &Values) -> Option<String> {
        None
    }
    async fn action(
        &self,
        service: &Connections,
        setup: &Setup,
        typ: &ConnectionType,
    ) -> Result<Action, Error> {
        if setup.step == "fields" {
            return Ok(Action::Form);
        }
        if setup.step != "oauth_consent" {
            return Err(invalid("Unexpected setup phase"));
        }
        let values = service.staged(setup).await?;
        let state = service.callback_state(setup)?;
        Ok(Action::Redirect {
            url: oauth::authorization_url(
                &self.oauth(service, typ)?,
                &values,
                &service.callback_url(setup)?,
                state.expose_secret(),
            )?,
        })
    }
    async fn validate(
        &self,
        _service: &Connections,
        _values: &Values,
    ) -> Result<Option<String>, Error> {
        Ok(None)
    }
    async fn start(
        &self,
        service: &Connections,
        setup: &Setup,
        typ: &ConnectionType,
        mut values: Values,
    ) -> Result<(), Error> {
        service.stage(setup, &values).await?;
        match typ.driver() {
            Driver::Static => {
                service
                    .complete(setup, None, self.validate(service, &values).await?)
                    .await
            }
            Driver::OAuthCode => {
                values.insert("_pkce".into(), random_secret());
                service.stage(setup, &values).await?;
                service.transition(setup, "oauth_consent").await
            }
            Driver::OAuthClientCredentials | Driver::OAuthJwtBearer => {
                let token = oauth::exchange(
                    &service.http,
                    typ.driver(),
                    &self.oauth(service, typ)?,
                    &values,
                    None,
                    None,
                    false,
                )
                .await?;
                service.complete(setup, Some(token), None).await
            }
            _ => Err(invalid(
                "This connection type requires an installed catalog adapter",
            )),
        }
    }
    async fn callback(
        &self,
        service: &Connections,
        setup: &Setup,
        typ: &ConnectionType,
        parameters: &Values,
    ) -> Result<(), Error> {
        let values = service.staged(setup).await?;
        let token = oauth::exchange(
            &service.http,
            Driver::OAuthCode,
            &self.oauth(service, typ)?,
            &values,
            optional(parameters, "code"),
            Some(&service.callback_url(setup)?),
            false,
        )
        .await?;
        let account = self.account(&token.provider_values);
        service.complete(setup, Some(token), account).await
    }
    async fn refresh(
        &self,
        service: &Connections,
        typ: &ConnectionType,
        values: &Values,
    ) -> Result<Token, Error> {
        oauth::exchange(
            &service.http,
            typ.driver(),
            &self.oauth(service, typ)?,
            values,
            None,
            None,
            typ.driver() == Driver::OAuthCode,
        )
        .await
    }
}

pub(crate) struct Configured;
#[async_trait::async_trait]
impl Runtime for Configured {}

/// Validate an adapter's submitted secrets without defining any UI elements.
pub(super) fn input(
    values: &Values,
    required: &[&str],
    optional_keys: &[&str],
) -> Result<(), Error> {
    for key in required {
        if value(values, key)?.trim().is_empty() {
            return Err(invalid("Required credential is empty"));
        }
    }
    for (key, secret) in values {
        if !required.contains(&key.as_str()) && !optional_keys.contains(&key.as_str()) {
            return Err(invalid("Unexpected credential key"));
        }
        if secret.expose_secret().len() > 65536 {
            return Err(invalid("Credential value is too large"));
        }
    }
    Ok(())
}
