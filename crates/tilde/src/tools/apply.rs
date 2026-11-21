//! Declarative apply of an agent's tool sources (see `crate::apply`): one source per connection,
//! whose tools become exactly the spec's (`*`: every tool it offers). Preflight has checked that the connection offers them;
//! settings people gave a kept tool (summary, display, async) stay.
use super::{
    Owner, Target, ToolSettings, db, insert_source, slug_base, source_name, tool_name_for,
};
use crate::apply::{Change, Refs};
use crate::database::Transaction;
use crate::error::Error;
use crate::proto::tilde::management::v1 as m;
use uuid::Uuid;

pub(crate) async fn sources(
    tx: &Transaction<'_>,
    agent: Uuid,
    agent_key: &str,
    refs: &Refs,
    specs: &[m::ToolSourceSpec],
) -> Result<Vec<Change>, Error> {
    let mut changes = Vec::new();
    for spec in specs {
        let key = format!("{agent_key}/{}", spec.connection.slug);
        let connection = refs.connection(&spec.connection)?;
        let existing =
            db::source_of_agent_opt(tx, Some(agent), Some(connection.id), None, None).await?;
        if spec.delete {
            if let Some(source) = existing {
                db::source_delete_execute(tx, source).await?;
                changes.push(Change::remove("tool_source", key));
            }
            continue;
        }
        let (source, mut change, current) = match existing {
            Some(source) => {
                let tools = db::agent_tool_list_all(tx, &[source]).await?;
                let current: Vec<String> = tools.into_iter().map(|t| t.tool_name).collect();
                (source, Change::existing("tool_source", key), current)
            }
            None => {
                let (provider, name) = connection.slug.split_once('/').unwrap_or_default();
                let base = slug_base(&source_name(provider, name));
                let source = insert_source(
                    tx,
                    Owner::Agent(agent),
                    Target::Connection(connection.id),
                    &base,
                )
                .await?;
                (source, Change::create("tool_source", key), Vec::new())
            }
        };
        let mut wanted = match spec.tools.iter().any(|t| t == "*") {
            true => refs.offered(&spec.connection).to_vec(),
            false => spec.tools.clone(),
        };
        wanted.sort();
        wanted.dedup();
        let mut sorted = current.clone();
        sorted.sort();
        change.field("tools", sorted.join(", "), wanted.join(", "));
        for tool in current.iter().filter(|t| !wanted.contains(t)) {
            db::agent_tool_remove_execute(tx, source, tool).await?;
        }
        let mut named: Vec<(&str, String)> = Vec::new();
        for tool in &wanted {
            named.push((tool, tool_name_for(tool, &named)?));
        }
        for (tool, name) in named
            .iter()
            .filter(|(t, _)| !current.iter().any(|c| c == t))
        {
            db::agent_tool_set_execute(tx, source, tool, name, &ToolSettings::default()).await?;
        }
        changes.push(change);
    }
    Ok(changes)
}
