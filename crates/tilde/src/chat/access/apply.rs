//! Declarative apply of an agent's chat channels (see `crate::apply`): each spec assigns a channel
//! connection to the agent, or names the agent's own Tilde chat, and sets who may talk to it
//! there. A channel the spec does not name stays as it is; `delete` unassigns one. Preflight has
//! checked each mode against its provider and that no other agent has the connection.
use crate::apply::{Change, Refs};
use crate::connections::model::{Assignment, Capability};
use crate::connections::service::Connections;
use crate::database::Transaction;
use crate::error::Error;
use crate::proto::tilde::management::v1 as m;
use crate::proto::tilde::types::v1 as types;
use uuid::Uuid;

pub(crate) async fn channels(
    tx: &Transaction<'_>,
    connections: &Connections,
    agent: Uuid,
    agent_key: &str,
    refs: &Refs,
    specs: &[m::ChannelSpec],
) -> Result<(Vec<Change>, Vec<Uuid>), Error> {
    let mut changes = Vec::new();
    // Connections whose access this apply tightened: their runs from identities no longer
    // allowed are stopped once it commits (`Apply::apply`).
    let mut tightened = Vec::new();
    for spec in specs {
        let (connection, slug) = match spec.connection.as_option() {
            Some(reference) => {
                let connection = refs.connection(reference)?;
                (connection.id, connection.slug.as_str())
            }
            // Every agent has its Tilde chat (agent creation makes it, earlier in this apply).
            None => (
                crate::connections::db::tilde_connection_opt(tx, agent)
                    .await?
                    .ok_or(Error::Missing("The agent's Tilde chat connection"))?
                    .id,
                "tilde",
            ),
        };
        let key = format!("{agent_key}/{slug}");
        crate::connections::db::connection_lock_execute(tx, &connection.to_string()).await?;
        let route = super::db::route_opt(tx, connection, agent).await?;
        if spec.delete {
            if route.is_some() {
                crate::connections::db::assignment_delete_execute(tx, connection, "channel", agent)
                    .await?;
                changes.push(Change::remove("channel", key));
            }
            continue;
        }
        let mut change = match &route {
            Some(_) => Change::existing("channel", key),
            None => {
                let row = crate::connections::db::connection_get_opt(tx, connection)
                    .await?
                    .ok_or(Error::Missing("Connection"))?;
                let assignment = Assignment {
                    capability: Capability::Channel,
                    agent_id: agent,
                    alias: None,
                };
                Connections::assign_in_transaction(tx, &row, &assignment).await?;
                connections.sync_agent_identity(tx, &row).await?;
                let mut change = Change::create("channel", key);
                change.field("connection", "", slug);
                change
            }
        };
        // A new assignment starts in its provider's default mode (GitHub's is disabled).
        let current = match route {
            Some(route) => route.access_mode,
            None => {
                super::db::route_opt(tx, connection, agent)
                    .await?
                    .ok_or(Error::Missing("Channel"))?
                    .access_mode
            }
        };
        let wanted = spec.access.and_then(|mode| match mode.as_known() {
            Some(types::ChannelAccessMode::Private) => Some("private"),
            Some(types::ChannelAccessMode::Public) => Some("public"),
            Some(types::ChannelAccessMode::Disabled) => Some("disabled"),
            _ => None,
        });
        if let Some(wanted) = wanted.filter(|wanted| *wanted != current) {
            super::db::policy_execute(tx, connection, agent, wanted).await?;
            change.field("access", &current, wanted);
            if rank(wanted) < rank(&current) {
                tightened.push(connection);
            }
        }
        changes.push(change);
    }
    Ok((changes, tightened))
}

/// How open a mode is: moving to a lower rank tightens access.
fn rank(mode: &str) -> u8 {
    match mode {
        "public" => 2,
        "private" => 1,
        _ => 0,
    }
}
