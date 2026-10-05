//! Provider/type capabilities are declared here. No connection has an enable/disable capability flag.
use super::model::*;
use crate::database::Pool;
use crate::error::Error;
use std::collections::BTreeMap;

pub mod agentmail;
pub mod github;
pub mod inference;
pub mod linq;
pub mod slack;
pub mod telnyx;
pub mod tilde;
pub mod whatsapp;

pub fn builtins() -> Vec<Provider> {
    let mut providers: Vec<Provider> = vec![
        github::definition(),
        slack::definition(),
        agentmail::definition(),
        linq::definition(),
        whatsapp::definition(),
        telnyx::definition(),
        tilde::definition(),
    ]
    .into_iter()
    .chain(inference::definitions())
    .chain([
        crate::tools::providers::tavily::definition(),
        crate::tools::providers::google::mail::definition(),
        crate::tools::providers::google::calendar::definition(),
        crate::tools::providers::google::drive::definition(),
        crate::tools::providers::google::docs::definition(),
        crate::tools::providers::google::sheets::definition(),
        crate::tools::providers::google::search_console::definition(),
        crate::tools::providers::google::analytics::definition(),
        crate::tools::providers::sentry::definition(),
        crate::tools::providers::posthog::definition(),
        crate::tools::providers::firecrawl::definition(),
        crate::tools::providers::stripe::definition(),
        crate::tools::providers::payload::definition(),
        crate::tools::providers::e2b::definition(),
        crate::tools::providers::aws::definition(),
        crate::tools::providers::modal::definition(),
    ])
    .collect();
    crate::tools::mcp_catalog::extend(&mut providers);
    providers
}

/// Register or replace a configured/remote definition atomically. Clients cannot replace built-in entries; startup reconciles shipped definitions.
/// A registered definition declares no capability without an adapter. Two adapters exist: a tool
/// host publishing its provider (`tool_host`, which stays that host's alone), and an MCP server,
/// so a type naming one may declare the tool capability.
pub async fn register(
    pool: &Pool,
    provider: Provider,
    builtin: bool,
    remote_authorization: Option<Vec<u8>>,
    remote_authorization_id: Option<uuid::Uuid>,
    tool_host: Option<uuid::Uuid>,
) -> Result<Provider, Error> {
    provider.validate()?;
    // A type emits signals exactly when the signals module has a source for it.
    if provider.connection_types.iter().any(|typ| {
        typ.capabilities.contains(&Capability::Signal)
            != crate::signals::source(&provider.id, &typ.id).is_some()
    }) {
        return Err(invalid(
            "The signal capability needs a signal source for the connection type",
        ));
    }
    if !builtin
        && (matches!(provider.kind, ProviderKind::BuiltIn)
            || builtins().iter().any(|entry| entry.id == provider.id))
    {
        return Err(invalid(
            "Built-in provider IDs and app-provisioning drivers cannot be registered by clients",
        ));
    }
    if !builtin
        && provider.connection_types.iter().any(|typ| {
            let adapted = tool_host.is_some() || typ.mcp.is_some();
            !adapted && !typ.capabilities.is_empty()
                || adapted && typ.capabilities != [Capability::Tool]
                || tool_host.is_some() && typ.mcp.is_some()
        })
    {
        return Err(invalid(
            "Capabilities require an installed adapter; a custom credential definition alone cannot provide one",
        ));
    }
    let mut tx_client = pool.get().await?;
    let tx = tx_client.transaction().await?;
    crate::connections::db::provider_lock_execute(&tx, &(provider.id)).await?;
    if let Some(current) = crate::connections::db::provider_get_opt(&tx, &(provider.id)).await? {
        if (current.kind == "built_in") != builtin {
            return Err(invalid(
                "Built-in providers cannot be replaced by registered providers",
            ));
        }
        if current.tool_host_id != tool_host {
            return Err(invalid(match tool_host {
                Some(_) => "Another provider already uses this ID",
                None => "This provider belongs to a tool host, which publishes its definition",
            }));
        }
    }
    crate::connections::db::provider_insert_execute(
        &tx,
        &(provider.id),
        &(provider.name),
        provider.kind.as_str(),
        provider
            .kind
            .remote()
            .map(|remote| remote.endpoint.as_str()),
        provider
            .kind
            .remote()
            .and_then(|remote| remote.ui_url.as_deref()),
        remote_authorization.as_deref(),
        remote_authorization_id,
        &provider.categories,
        provider.icon_url.as_deref(),
        provider.instructions.as_deref(),
        provider.account_name_label.as_deref(),
        tool_host,
    )
    .await?;
    crate::connections::db::provider_clear_details_execute(&tx, &(provider.id)).await?;
    for typ in &provider.connection_types {
        let default = OAuth::standard("");
        let oauth = typ.oauth().unwrap_or(&default);
        let token_url = typ.oauth().map(|o| o.token_url.as_str());
        crate::connections::db::types_insert_execute(
            &tx,
            &(provider.id),
            &(typ.id),
            &(typ.name),
            typ.driver().as_str(),
            typ.capabilities.contains(&Capability::Channel),
            oauth.authorization_url.as_deref(),
            token_url,
            oauth.client_auth.as_str(),
            oauth.pkce,
            &oauth.scopes,
            &(oauth.scope_separator),
            &(oauth.access_token_path),
            &(oauth.refresh_token_path),
            &(oauth.expires_in_path),
            &(oauth.scope_path),
            oauth.success_path.as_deref(),
            match &typ.credential_source {
                CredentialSource::Static { schema } => Some(schema),
                CredentialSource::OAuth {
                    additional_schema, ..
                } => additional_schema.as_ref(),
                _ => None,
            },
            typ.capabilities.contains(&Capability::Inference),
            typ.capabilities.contains(&Capability::Tool),
            typ.mcp.as_ref().map(|server| server.url.as_str()),
            typ.mcp.as_ref().map(|server| server.credential.as_str()),
            typ.mcp.as_ref().and_then(|server| server.credential.name()),
            typ.mcp
                .as_ref()
                .map(|server| server.credential.prefix())
                .unwrap_or(""),
            typ.oauth().map(|o| o.client).unwrap_or_default().as_str(),
            typ.capabilities.contains(&Capability::Signal),
        )
        .await?;
        for field in &oauth.result_fields {
            crate::connections::db::result_fields_insert_execute(
                &tx,
                &(provider.id),
                &(typ.id),
                &(field.key),
                &(field.path),
                field.required,
            )
            .await?;
        }
        for (phase, parameters) in [
            ("authorization", &oauth.authorization_parameters),
            ("token", &oauth.token_parameters),
        ] {
            for (name, value) in parameters {
                crate::connections::db::parameters_insert_execute(
                    &tx,
                    &(provider.id),
                    &(typ.id),
                    phase,
                    name,
                    value,
                )
                .await?;
            }
        }
    }
    let type_ids = provider
        .connection_types
        .iter()
        .map(|typ| typ.id.clone())
        .collect::<Vec<_>>();
    crate::connections::db::provider_remove_types_execute(&tx, &(provider.id), &type_ids).await?;
    tx.commit().await?;
    drop(tx_client);
    Ok(provider)
}
pub async fn seed(pool: &Pool) -> Result<(), Error> {
    for provider in builtins() {
        if !provider.connection_types.is_empty() {
            register(pool, provider, true, None, None, None).await?;
        }
    }
    Ok(())
}
/// Read the current provider definition.
pub async fn get(pool: &Pool, id: &str) -> Result<Provider, Error> {
    let mut snapshot_client = pool.get().await?;
    let snapshot = snapshot_client.transaction().await?;
    crate::connections::db::provider_snapshot_execute(&snapshot).await?;
    let head = crate::connections::db::provider_get_opt(&snapshot, id)
        .await?
        .ok_or_else(|| invalid("Provider not found"))?;
    let mut types = vec![];
    for row in crate::connections::db::types_list_all(&snapshot, id).await? {
        let mut authorization_parameters = BTreeMap::new();
        let mut token_parameters = BTreeMap::new();
        for param in
            crate::connections::db::parameters_list_all(&snapshot, id, &(row.type_id)).await?
        {
            if param.phase == "authorization" {
                authorization_parameters.insert(param.name, param.value);
            } else {
                token_parameters.insert(param.name, param.value);
            }
        }
        let result_fields =
            crate::connections::db::result_fields_list_all(&snapshot, id, &(row.type_id))
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
                client: OAuthClient::parse(&row.oauth_client)?,
                host_published: head.tool_host_id.is_some(),
            })
        } else {
            None
        };
        types.push(ConnectionType {
            id: row.type_id,
            name: row.name,

            capabilities: [
                (row.channel_capable, Capability::Channel),
                (row.inference_capable, Capability::Inference),
                (row.tool_capable, Capability::Tool),
                (row.signal_capable, Capability::Signal),
            ]
            .into_iter()
            .filter_map(|(capable, cap)| capable.then_some(cap))
            .collect(),
            credential_source: CredentialSource::from_storage(
                Driver::parse(&row.driver)?,
                row.credential_schema,
                oauth,
            )?,
            mcp: row
                .mcp_credential
                .map(|kind| {
                    Ok::<_, Error>(McpServer {
                        url: row
                            .mcp_url
                            .clone()
                            .ok_or_else(|| invalid("MCP server URL missing"))?,
                        credential: McpCredential::from_storage(
                            &kind,
                            row.mcp_credential_name,
                            row.mcp_credential_prefix,
                        )?,
                    })
                })
                .transpose()?,
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
    drop(snapshot_client);
    Ok(provider)
}
/// Which providers a list returns; every filter applies in SQL before the page.
#[derive(Default)]
pub struct ProviderFilter<'a> {
    pub search: Option<&'a str>,
    pub capability: Option<Capability>,
    pub category: Option<&'a str>,
    pub source: Option<ProviderSource>,
}
/// `Catalog` leaves out MCP servers added by URL and providers published by tool hosts;
/// `McpServer` keeps only the former.
#[derive(Clone, Copy)]
pub enum ProviderSource {
    Catalog,
    McpServer,
    ToolHost,
}
impl ProviderSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::McpServer => "mcp_server",
            Self::ToolHost => "tool_host",
        }
    }
}
pub async fn list(
    pool: &Pool,
    after: &str,
    filter: &ProviderFilter<'_>,
    size: u32,
) -> Result<(Vec<Provider>, String), Error> {
    let size = if size == 0 { 50 } else { size.min(100) };
    let mut rows = crate::connections::db::provider_list_all(
        &pool.get().await?,
        after,
        filter,
        i64::from(size) + 1,
    )
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
        ("tilde", "application") => &tilde::Tilde,
        _ => inference::runtime(id).unwrap_or(&runtime::Configured),
    }
}
