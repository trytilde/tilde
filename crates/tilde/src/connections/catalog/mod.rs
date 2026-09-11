//! Provider/type capabilities are declared here. No connection has an enable/disable capability flag.
use super::model::*;
use crate::error::Error;
use sqlx::PgPool;
use std::collections::BTreeMap;

pub mod agentmail;
pub mod github;
pub mod linq;
pub mod slack;
pub mod telnyx;
pub mod whatsapp;

pub fn builtins() -> Vec<Provider> {
    vec![
        github::definition(),
        slack::definition(),
        agentmail::definition(),
        linq::definition(),
        whatsapp::definition(),
        telnyx::definition(),
    ]
}

/// Register or replace a configured/remote definition atomically. Clients cannot replace built-in entries; startup reconciles shipped definitions.
pub async fn register(
    pool: &PgPool,
    provider: Provider,
    builtin: bool,
    remote_authorization: Option<Vec<u8>>,
    remote_authorization_id: Option<uuid::Uuid>,
) -> Result<Provider, Error> {
    provider.validate()?;
    if !builtin
        && (matches!(provider.kind, ProviderKind::BuiltIn)
            || builtins().iter().any(|entry| entry.id == provider.id))
    {
        return Err(invalid(
            "Built-in provider IDs and app-provisioning drivers cannot be registered by clients",
        ));
    }
    if !builtin
        && provider
            .connection_types
            .iter()
            .any(|typ| !typ.capabilities.is_empty())
    {
        return Err(invalid(
            "Channel capability requires an installed channel adapter; a custom credential definition alone cannot provide one",
        ));
    }
    let mut tx = pool.begin().await?;
    sqlx::query_file!("../../queries/connections/provider_lock.sql", provider.id)
        .execute(&mut *tx)
        .await?;
    let current = sqlx::query_file!("../../queries/connections/provider_get.sql", provider.id)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(current) = current
        && (current.kind == "built_in") != builtin
    {
        return Err(invalid(
            "Built-in providers cannot be replaced by registered providers",
        ));
    }
    sqlx::query_file!(
        "../../queries/connections/provider_insert.sql",
        provider.id,
        provider.name,
        provider.kind.as_str(),
        provider
            .kind
            .remote()
            .map(|remote| remote.endpoint.as_str()),
        provider
            .kind
            .remote()
            .and_then(|remote| remote.ui_url.as_deref()),
        remote_authorization,
        remote_authorization_id,
        &provider.categories,
        provider.icon_url,
        provider.instructions,
        provider.account_name_label
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query_file!(
        "../../queries/connections/provider_clear_details.sql",
        provider.id
    )
    .execute(&mut *tx)
    .await?;
    for typ in &provider.connection_types {
        let default = OAuth::standard("");
        let oauth = typ.oauth().unwrap_or(&default);
        let token_url = typ.oauth().map(|o| o.token_url.as_str());
        sqlx::query_file!(
            "../../queries/connections/types_insert.sql",
            provider.id,
            typ.id,
            typ.name,
            typ.driver().as_str(),
            typ.capabilities.contains(&Capability::Channel),
            oauth.authorization_url,
            token_url,
            oauth.client_auth.as_str(),
            oauth.pkce,
            &oauth.scopes,
            oauth.scope_separator,
            oauth.access_token_path,
            oauth.refresh_token_path,
            oauth.expires_in_path,
            oauth.scope_path,
            oauth.success_path,
            match &typ.credential_source {
                CredentialSource::Static { schema } => Some(schema),
                CredentialSource::OAuth {
                    additional_schema, ..
                } => additional_schema.as_ref(),
                _ => None,
            }
        )
        .execute(&mut *tx)
        .await?;
        for field in &oauth.result_fields {
            sqlx::query_file!(
                "../../queries/connections/result_fields_insert.sql",
                provider.id,
                typ.id,
                field.key,
                field.path,
                field.required
            )
            .execute(&mut *tx)
            .await?;
        }
        for (phase, parameters) in [
            ("authorization", &oauth.authorization_parameters),
            ("token", &oauth.token_parameters),
        ] {
            for (name, value) in parameters {
                sqlx::query_file!(
                    "../../queries/connections/parameters_insert.sql",
                    provider.id,
                    typ.id,
                    phase,
                    name,
                    value
                )
                .execute(&mut *tx)
                .await?;
            }
        }
    }
    let type_ids = provider
        .connection_types
        .iter()
        .map(|typ| typ.id.clone())
        .collect::<Vec<_>>();
    sqlx::query_file!(
        "../../queries/connections/provider_remove_types.sql",
        provider.id,
        &type_ids
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(provider)
}
pub async fn seed(pool: &PgPool) -> Result<(), Error> {
    for provider in builtins() {
        register(pool, provider, true, None, None).await?;
    }
    Ok(())
}
/// Read the current provider definition.
pub async fn get(pool: &PgPool, id: &str) -> Result<Provider, Error> {
    let mut snapshot = pool.begin().await?;
    sqlx::query_file!("../../queries/connections/provider_snapshot.sql")
        .execute(&mut *snapshot)
        .await?;
    let head = sqlx::query_file!("../../queries/connections/provider_get.sql", id)
        .fetch_optional(&mut *snapshot)
        .await?
        .ok_or_else(|| invalid("Provider not found"))?;
    let mut types = vec![];
    for row in sqlx::query_file!("../../queries/connections/types_list.sql", id)
        .fetch_all(&mut *snapshot)
        .await?
    {
        let mut authorization_parameters = BTreeMap::new();
        let mut token_parameters = BTreeMap::new();
        for param in sqlx::query_file!(
            "../../queries/connections/parameters_list.sql",
            id,
            row.type_id
        )
        .fetch_all(&mut *snapshot)
        .await?
        {
            if param.phase == "authorization" {
                authorization_parameters.insert(param.name, param.value);
            } else {
                token_parameters.insert(param.name, param.value);
            }
        }
        let result_fields = sqlx::query_file!(
            "../../queries/connections/result_fields_list.sql",
            id,
            row.type_id
        )
        .fetch_all(&mut *snapshot)
        .await?
        .into_iter()
        .map(|field| ResultField {
            key: field.field_key,
            path: field.json_pointer,
            required: field.required,
        })
        .collect();
        let oauth = if let Some(token_url) = row.token_url {
            Some(OAuth {
                authorization_url: row.authorization_url,
                token_url,
                client_auth: ClientAuth::parse(&row.client_auth)?,
                pkce: row.pkce,
                scopes: row.scopes,
                scope_separator: row.scope_separator,
                authorization_parameters,
                token_parameters,
                access_token_path: row.access_token_path,
                refresh_token_path: row.refresh_token_path,
                expires_in_path: row.expires_in_path,
                scope_path: row.scope_path,
                success_path: row.success_path,
                result_fields,
            })
        } else {
            None
        };
        types.push(ConnectionType {
            id: row.type_id,
            name: row.name,

            capabilities: if row.channel_capable {
                vec![Capability::Channel]
            } else {
                vec![]
            },
            credential_source: CredentialSource::from_storage(
                Driver::parse(&row.driver)?,
                row.credential_schema,
                oauth,
            )?,
        });
    }
    let provider = Provider {
        account_name_label: head.account_name_label,
        icon_url: head.icon_url,
        instructions: head.instructions,
        id: id.into(),
        name: head.name,
        categories: head.categories,
        connection_types: types,
        kind: match head.kind.as_str() {
            "built_in" => ProviderKind::BuiltIn,
            "configured" => ProviderKind::Configured,
            "remote" => ProviderKind::Remote(RemoteProvider {
                endpoint: head
                    .remote_endpoint
                    .ok_or_else(|| invalid("Remote endpoint missing"))?,
                ui_url: head.remote_ui_url,
            }),
            _ => return Err(invalid("Invalid provider kind")),
        },
    };
    snapshot.commit().await?;
    Ok(provider)
}
pub async fn list(
    pool: &PgPool,
    after: &str,
    search: Option<&str>,
    size: u32,
) -> Result<(Vec<Provider>, String), Error> {
    let size = if size == 0 { 50 } else { size.min(100) };
    let mut rows = sqlx::query_file!(
        "../../queries/connections/provider_list.sql",
        after,
        search,
        i64::from(size) + 1
    )
    .fetch_all(pool)
    .await?;
    let more = rows.len() > size as usize;
    if more {
        rows.pop();
    }
    let next = if more {
        rows.last()
            .map(|r| r.provider_id.clone())
            .unwrap_or_default()
    } else {
        String::new()
    };
    let mut providers = vec![];
    for row in rows {
        providers.push(get(pool, &row.provider_id).await?);
    }
    Ok((providers, next))
}

/// Optional endpoint overrides for isolated provider tests. Production URLs belong to each provider.
#[derive(Clone, Default)]
pub struct Endpoints(pub BTreeMap<String, String>);
impl Endpoints {
    pub(super) fn get<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.0.get(key).map(String::as_str).unwrap_or(default)
    }
}
fn url(base: &str, segments: &[&str]) -> Result<url::Url, Error> {
    let mut url = endpoint(base)?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| invalid("Invalid provider base URL"))?;
        path.pop_if_empty();
        for segment in segments {
            path.push(segment);
        }
    }
    Ok(url)
}

pub(crate) mod runtime;
/// The single registration point for compiled catalog adapters. Custom definitions use standard drivers.
pub(crate) fn runtime(id: &str, type_id: &str) -> &'static dyn runtime::Runtime {
    match (id, type_id) {
        ("github", "github_app") => &github::Github,
        ("slack", "slack_app") => &slack::Slack,
        ("agentmail", "inbox") => &agentmail::Agentmail,
        ("linq", "account") => &linq::Linq,
        ("whatsapp", "meta") => &whatsapp::Whatsapp,
        ("telnyx", "whatsapp") => &telnyx::Telnyx,
        _ => &runtime::Configured,
    }
}
