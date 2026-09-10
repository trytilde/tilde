//! Remote setup hooks share the generated Connect contract and never receive a management token.
use super::{model::*, service::Connections, setup::bounded};
use crate::proto::tilde::provider::v1 as provider_pb;
use crate::proto::tilde::types::v1 as types;
use crate::{
    encryption::{SealedSecret, SecretBinding},
    error::Error,
    services::tilde::provider::v1::ConnectionProviderServiceClient,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::client::{ClientConfig, HttpClient};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub(super) fn binding(id: Uuid) -> SecretBinding<'static> {
    SecretBinding {
        resource_kind: "connection_provider",
        resource_id: id,
        name: "backend_token",
    }
}
pub(super) fn ui_path(provider: &Provider, local: &str) -> String {
    if matches!(provider.kind, ProviderKind::Remote(_)) {
        format!(
            "/connections/ui/remote/{}/index.html",
            URL_SAFE_NO_PAD.encode(&provider.id)
        )
    } else {
        format!("/catalog/{local}/ui")
    }
}
fn fields(values: &Values) -> Vec<types::InputField> {
    values
        .iter()
        .map(|(key, value)| types::InputField {
            key: key.clone(),
            value: value.expose_secret().into(),
            ..Default::default()
        })
        .collect()
}
fn values(fields: Vec<types::InputField>) -> Result<Values, Error> {
    let mut out = Values::new();
    for mut field in fields {
        use zeroize::Zeroize;
        let value = SecretString::from(field.value.as_str());
        field.value.zeroize();
        if out.insert(field.key, value).is_some() {
            return Err(invalid("Duplicate provider output key"));
        }
    }
    bounded(&out)?;
    Ok(out)
}
pub(super) async fn execute(
    service: &Connections,
    setup: &Setup,
    connection: &Connection,
    action: &str,
    input: &Values,
    is_callback: bool,
) -> Result<(), Error> {
    let row = sqlx::query_file!(
        "../../queries/connections/provider_backend.sql",
        connection.provider_id
    )
    .fetch_one(&service.pool)
    .await?;
    if row.kind != "remote" {
        return Err(invalid("Provider is no longer remote"));
    }
    let backend_endpoint = row
        .remote_endpoint
        .ok_or_else(|| invalid("Remote endpoint missing"))?;
    let key = service.crypto.open(
        binding(
            row.remote_authorization_id
                .ok_or_else(|| invalid("Provider authorization identity missing"))?,
        ),
        SealedSecret::from_bytes(
            &row.remote_authorization
                .ok_or_else(|| invalid("Remote provider authorization missing"))?,
        )?,
    )?;
    let callback = random_secret();
    let ciphertext = service.seal(setup.id, "callback", &callback)?;
    let hash = Sha256::digest(callback.expose_secret()).to_vec();
    if sqlx::query_file!(
        "../../queries/connections/setup_callback_rotate.sql",
        setup.id,
        setup.action_id,
        ciphertext,
        hash
    )
    .fetch_optional(&service.pool)
    .await?
    .is_none()
    {
        return Err(invalid("Setup was cancelled"));
    }
    let request = provider_pb::HandleSetupRequest {
        setup_id: setup.id.to_string(),
        connection_id: connection.id.to_string(),
        action: action.into(),
        fields: fields(input),
        draft: fields(&service.draft(setup).await?),
        callback_url: service.callback_url(setup)?,
        callback_state: format!("{}.{}", setup.id, callback.expose_secret()),
        is_callback,
        action_id: setup.action_id.to_string(),
        provider_id: connection.provider_id.clone(),
        type_id: connection.type_id.clone(),
        ..Default::default()
    };
    let transport = if backend_endpoint.starts_with("https://") {
        let roots = connectrpc::rustls::RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
        );
        let tls = connectrpc::rustls::ClientConfig::builder_with_provider(Arc::new(
            connectrpc::rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| invalid("Unable to initialize provider TLS"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
        HttpClient::with_tls(Arc::new(tls))
    } else {
        HttpClient::plaintext()
    };
    let mut authorization = http::HeaderValue::from_str(&format!("Bearer {}", key.expose_secret()))
        .map_err(|_| invalid("Invalid provider authorization"))?;
    authorization.set_sensitive(true);
    let mut headers = http::HeaderMap::new();
    headers.insert(http::header::AUTHORIZATION, authorization);
    let client = ConnectionProviderServiceClient::new(
        transport,
        ClientConfig::new(
            backend_endpoint
                .parse()
                .map_err(|_| invalid("Invalid provider endpoint"))?,
        )
        .with_default_headers(headers),
    );
    let response = tokio::time::timeout(Duration::from_secs(25), client.handle_setup(request))
        .await
        .map_err(|_| invalid("Remote provider timed out; outcome may be uncertain"))?
        .map_err(|_| invalid("Remote provider setup failed"))?;
    let (_, response, _) = response.into_owned_parts();
    service.write_draft(setup, &values(response.draft)?).await?;
    match response.result {
        Some(provider_pb::handle_setup_response::Result::AwaitInput(next)) => {
            if let Some(url) = &next.redirect_url {
                endpoint(url)?;
            }
            sqlx::query_file!(
                "../../queries/connections/setup_redirect.sql",
                setup.id,
                setup.action_id,
                next.redirect_url
            )
            .execute(&service.pool)
            .await?;
            service
                .transition(
                    setup,
                    if next.redirect_url.is_some() {
                        "provider_redirect"
                    } else {
                        "fields"
                    },
                )
                .await
        }
        Some(provider_pb::handle_setup_response::Result::Complete(result)) => {
            let credentials = values(result.fields)?;
            if credentials.is_empty() {
                return Err(invalid("Provider returned no credentials"));
            }
            service.stage(setup, &credentials).await?;
            service.complete(setup, None, result.account_label).await
        }
        None => Err(invalid("Remote provider returned no setup result")),
    }
}
