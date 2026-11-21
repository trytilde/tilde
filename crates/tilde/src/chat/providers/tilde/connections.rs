//! The identity's own connections (personal tool federation, `tools::personal`) over the native
//! provider: an application's user lists the providers the agent may use on their behalf,
//! connects an account on Tilde's setup page (the same brokering page as `user.connect_tool`'s
//! link), and lists, reconnects or removes what they connected. The connection is owned by the
//! identity's chat user, so the agent sees its tools only in that user's conversations.
use super::db;
use crate::chat::Chat;
use crate::connections::{model::Started, service::Connections};
use crate::deployment::tokens::IngressClaims;
use crate::error::Error;
use crate::iam::capabilities::Capabilities;
use crate::proto::tilde::provider::tilde::v1 as wire;
use uuid::Uuid;

/// Connection setup tokens expire after ten minutes (`connections`).
const SETUP_SECONDS: i64 = 600;

fn user(claims: &IngressClaims) -> Result<Uuid, Error> {
    claims.user_id.ok_or(Error::Denied)
}
fn connections(chat: &Chat) -> Result<&Connections, Error> {
    chat.channels
        .as_ref()
        .map(|channels| &channels.connections)
        .ok_or_else(|| Error::Invalid("Connections are not configured".into()))
}
fn setup(started: Started) -> wire::ConnectionSetup {
    wire::ConnectionSetup {
        connection_id: started.connection.id.to_string(),
        setup_url: started.brokering_url,
        expires_at: chrono::Utc::now().timestamp() + SETUP_SECONDS,
        ..Default::default()
    }
}

pub(crate) async fn connectable(
    chat: &Chat,
    claims: &IngressClaims,
) -> Result<Vec<crate::tools::db::ToolProviderTypeRow>, Error> {
    user(claims)?;
    let pool = chat.pg()?;
    let capabilities = db::agent_capabilities(&pool.get().await?, claims.agent_id)
        .await?
        .ok_or(Error::NotFound)?;
    let capabilities: Capabilities =
        serde_json::from_value(capabilities).map_err(|_| Error::Denied)?;
    crate::tools::personal::connectable(pool, &capabilities).await
}

pub(crate) async fn providers(
    chat: &Chat,
    claims: &IngressClaims,
) -> Result<Vec<wire::ConnectableProvider>, Error> {
    let mut out = Vec::new();
    for row in connectable(chat, claims).await? {
        let provider = connections(chat)?.provider(&row.provider_id).await?;
        out.push(wire::ConnectableProvider {
            id: row.provider_id,
            name: provider.name,
            icon_url: provider.icon_url.unwrap_or_default(),
            ..Default::default()
        });
    }
    Ok(out)
}

pub(crate) async fn start(
    chat: &Chat,
    claims: &IngressClaims,
    provider: &str,
) -> Result<wire::ConnectionSetup, Error> {
    let typ = connectable(chat, claims)
        .await?
        .into_iter()
        .find(|t| t.provider_id == provider)
        .ok_or_else(|| Error::Invalid("This agent may not use that provider personally".into()))?;
    let started =
        crate::tools::personal::start(connections(chat)?, user(claims)?, provider, &typ.type_id)
            .await?;
    Ok(setup(started))
}

pub(crate) async fn list(
    chat: &Chat,
    claims: &IngressClaims,
) -> Result<Vec<wire::UserConnection>, Error> {
    let rows = db::user_connections(&chat.pg()?.get().await?, user(claims)?).await?;
    Ok(rows
        .into_iter()
        .map(|c| wire::UserConnection {
            id: c.id.to_string(),
            provider_id: c.provider_id,
            name: c.name,
            status: c.status,
            ..Default::default()
        })
        .collect())
}

/// One of the caller's own connections; another's is not found.
async fn owned(chat: &Chat, claims: &IngressClaims, id: &str) -> Result<Uuid, Error> {
    let id = Uuid::parse_str(id).map_err(|_| Error::Invalid("Invalid connection".into()))?;
    db::user_connections(&chat.pg()?.get().await?, user(claims)?)
        .await?
        .into_iter()
        .any(|c| c.id == id)
        .then_some(id)
        .ok_or(Error::NotFound)
}

pub(crate) async fn reconnect(
    chat: &Chat,
    claims: &IngressClaims,
    id: &str,
) -> Result<wire::ConnectionSetup, Error> {
    let id = owned(chat, claims, id).await?;
    Ok(setup(connections(chat)?.reconnect(id).await?))
}

pub(crate) async fn delete(chat: &Chat, claims: &IngressClaims, id: &str) -> Result<(), Error> {
    let id = owned(chat, claims, id).await?;
    connections(chat)?.disconnect(id).await
}
