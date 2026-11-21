//! Declarative apply of an agent's routines (see `crate::apply`), found by name within the agent.
//! A spec without `enabled` creates its routine disabled and then leaves enabling to people.
//! Preflight has checked each signal trigger's connection and type.
use super::{Filter, RoutineInput, Trigger, check, columns, db, filters_by_routine, write_filters};
use crate::apply::{Change, Refs};
use crate::database::Transaction;
use crate::error::Error;
use crate::proto::tilde::management::v1 as m;
use m::routine_trigger_spec::Kind;
use uuid::Uuid;

fn trigger(refs: &Refs, spec: &m::RoutineSpec) -> Result<Trigger, Error> {
    match spec.trigger.as_option().and_then(|t| t.kind.as_ref()) {
        Some(Kind::Cron(cron)) => Ok(Trigger::Cron {
            schedule: cron.schedule.trim().to_owned(),
        }),
        Some(Kind::Signal(signal)) => Ok(Trigger::Signal {
            connection_id: refs.connection(&signal.connection)?.id,
            signal_type: signal.signal_type.clone(),
            filters: signal
                .filters
                .iter()
                .map(|f| Filter {
                    path: f.key.trim().to_owned(),
                    values: f.values.clone(),
                })
                .collect(),
        }),
        None => Err(Error::Invalid(format!(
            "Routine {} needs a cron or a signal trigger",
            spec.name
        ))),
    }
}
/// A trigger as the plan shows it.
fn describe(trigger: &Trigger, refs: &Refs, spec: Option<&m::RoutineSpec>) -> String {
    match trigger {
        Trigger::Cron { schedule } => format!("cron {schedule}"),
        Trigger::Signal {
            connection_id,
            signal_type,
            filters,
        } => {
            let connection = spec
                .and_then(|s| s.trigger.as_option())
                .and_then(|t| match &t.kind {
                    Some(Kind::Signal(signal)) => Some(signal.connection.slug.clone()),
                    _ => None,
                })
                .or_else(|| refs.slug_of(*connection_id).map(str::to_owned))
                .unwrap_or_else(|| connection_id.to_string());
            let filters: Vec<String> = filters
                .iter()
                .map(|f| format!("{} in [{}]", f.path, f.values.join(", ")))
                .collect();
            format!("{connection} {signal_type} {}", filters.join(" and "))
                .trim_end()
                .to_owned()
        }
    }
}

pub(crate) async fn routines(
    tx: &Transaction<'_>,
    agent: Uuid,
    agent_key: &str,
    refs: &Refs,
    specs: &[m::RoutineSpec],
) -> Result<Vec<Change>, Error> {
    let rows = db::routine_list_all(tx, agent).await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut filters = filters_by_routine(db::filter_list_all(tx, &ids).await?);
    let mut changes = Vec::new();
    for spec in specs {
        let name = spec.name.trim();
        let key = format!("{agent_key}/{name}");
        let current = rows.iter().find(|r| r.name == name);
        if spec.delete {
            if let Some(row) = current {
                db::routine_delete_execute(tx, row.id).await?;
                changes.push(Change::remove("routine", key));
            }
            continue;
        }
        let input = RoutineInput {
            name: name.to_owned(),
            prompt: spec.prompt.clone(),
            thread_title: spec.thread_title.trim().to_owned(),
            // Absent: a new routine starts disabled, an existing one keeps its state.
            enabled: spec
                .enabled
                .unwrap_or_else(|| current.is_some_and(|r| r.enabled)),
            trigger: trigger(refs, spec)?,
        };
        let next = check(&input)?;
        let after = describe(&input.trigger, refs, Some(spec));
        let (schedule, connection, signal_type) = columns(&input.trigger);
        let change = match current {
            Some(row) => {
                let existing =
                    super::routine_model(row.clone(), filters.remove(&row.id).unwrap_or_default());
                let mut change = Change::existing("routine", key);
                change.field("prompt", &row.prompt, &input.prompt);
                change.field("thread_title", &row.thread_title, &input.thread_title);
                change.field("trigger", describe(&existing.trigger, refs, None), &after);
                change.field("enabled", row.enabled, input.enabled);
                if change.changed() {
                    db::routine_update_execute(
                        tx,
                        row.id,
                        name,
                        &input.prompt,
                        &input.thread_title,
                        input.enabled,
                        schedule,
                        connection,
                        signal_type,
                        next,
                    )
                    .await?;
                    write_filters(tx, row.id, &input.trigger).await?;
                }
                change
            }
            None => {
                let id = Uuid::new_v4();
                db::routine_insert_execute(
                    tx,
                    id,
                    agent,
                    name,
                    &input.prompt,
                    &input.thread_title,
                    input.enabled,
                    schedule,
                    connection,
                    signal_type,
                    next,
                )
                .await?;
                write_filters(tx, id, &input.trigger).await?;
                let mut change = Change::create("routine", key);
                change.field("trigger", "", &after);
                change.field("enabled", "", input.enabled);
                change
            }
        };
        changes.push(change);
    }
    Ok(changes)
}
