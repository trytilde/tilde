//! Declarative apply of an agent's inference aliases (see `crate::apply`): each alias names one
//! inference connection, and an agent's connection carries at most one alias.
use crate::apply::{Change, Refs};
use crate::connections::model::{Assignment, Capability};
use crate::connections::service::Connections;
use crate::database::Transaction;
use crate::error::Error;
use crate::proto::tilde::management::v1 as m;
use uuid::Uuid;

pub(crate) async fn aliases(
    tx: &Transaction<'_>,
    agent: Uuid,
    agent_key: &str,
    refs: &Refs,
    specs: &[m::InferenceAliasSpec],
) -> Result<Vec<Change>, Error> {
    let mut existing = super::db::agent_aliases_all(tx, agent).await?;
    let mut changes = Vec::new();
    // Deletes first, so an alias may move onto the connection a deleted one leaves.
    let ordered = specs
        .iter()
        .filter(|s| s.delete)
        .chain(specs.iter().filter(|s| !s.delete));
    for spec in ordered {
        let key = format!("{agent_key}/{}", spec.alias);
        let current = existing.iter().find(|a| a.alias == spec.alias);
        if spec.delete {
            if let Some(current) = current {
                unassign(tx, current.connection_id, agent).await?;
                changes.push(Change::remove("inference_alias", key));
            }
            // Gone now: a later entry of the same alias must not unassign its old connection again.
            existing.retain(|a| a.alias != spec.alias);
            continue;
        }
        let connection = refs.connection(&spec.connection)?;
        let change = match current {
            Some(current) if current.connection_id == connection.id => {
                changes.push(Change::existing("inference_alias", key));
                continue;
            }
            Some(current) => {
                unassign(tx, current.connection_id, agent).await?;
                let mut change = Change::existing("inference_alias", key);
                change.field("connection", &current.slug, &connection.slug);
                change
            }
            None => {
                let mut change = Change::create("inference_alias", key);
                change.field("connection", "", &connection.slug);
                change
            }
        };
        crate::connections::db::connection_lock_execute(tx, &connection.id.to_string()).await?;
        let row = crate::connections::db::connection_get_opt(tx, connection.id)
            .await?
            .ok_or(Error::Missing("Connection"))?;
        let assignment = Assignment {
            capability: Capability::Inference,
            agent_id: agent,
            alias: Some(spec.alias.clone()),
        };
        Connections::assign_in_transaction(tx, &row, &assignment).await?;
        changes.push(change);
    }
    Ok(changes)
}

async fn unassign(tx: &Transaction<'_>, connection: Uuid, agent: Uuid) -> Result<(), Error> {
    crate::connections::db::connection_lock_execute(tx, &connection.to_string()).await?;
    crate::connections::db::assignment_delete_execute(tx, connection, "inference", agent).await?;
    Ok(())
}
