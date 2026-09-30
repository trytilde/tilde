//! Chat mutations coordinated under the registry's agent lock, before thread locks.
use super::*;
use crate::database::{Pool, Transaction};

/// Terminalize running invocations on pause, retaining pending work until resume.
/// Deletion additionally cancels pending invocations.
pub(crate) async fn stop_invocations(
    pool: &Pool,
    tx: &Transaction<'_>,
    agent: Uuid,
    deleting: bool,
) -> Result<()> {
    let invocations = crate::chat::db::agent_invocations_all(tx, agent, deleting).await?;
    for invocation in invocations {
        crate::chat::db::thread_lock_one(tx, invocation.thread_id).await?;
        let Some(row) =
            crate::chat::db::invocation_finish_opt(tx, invocation.id, "canceled").await?
        else {
            continue;
        };
        let status = if deleting { "canceled" } else { "waiting" };
        crate::chat::db::run_status_execute(tx, row.run_id, status).await?;
        activity(
            pool,
            tx,
            row.thread_id,
            "invocation.ended",
            invocation.id,
            "canceled",
        )
        .await?;
        if !deleting {
            requeue_inputs(pool, tx, invocation.id, row.run_id).await?;
        }
    }
    Ok(())
}

/// Historical participants remain addressable in snapshots but no longer receive work.
pub(crate) async fn retire(pool: &Pool, tx: &Transaction<'_>, agent: Uuid) -> Result<()> {
    let rows = crate::chat::db::agent_participants_all(tx, agent).await?;
    for row in rows {
        crate::chat::db::thread_lock_one(tx, row.thread_id).await?;
        crate::chat::db::participant_active_execute(tx, row.thread_id, row.id, false).await?;
        crate::chat::db::typing_clear_execute(tx, row.thread_id, row.id).await?;
        activity(pool, tx, row.thread_id, "participant.left", row.id, "").await?;
    }
    Ok(())
}

/// Dispatch pending events into a fresh invocation. Inputs are consumed by API policy,
/// never acknowledged or drained by an agent's implementation. Caller holds the thread lock.
pub(crate) async fn requeue_inputs(
    pool: &Pool,
    tx: &Transaction<'_>,
    invocation: Uuid,
    run_id: Uuid,
) -> Result<()> {
    let inputs = crate::chat::db::inputs_pending_all(tx, invocation).await?;
    if inputs.is_empty() {
        return Ok(());
    }
    let state = crate::chat::db::run_state_one(tx, run_id).await?;
    let count = if state.concurrency_policy == "queue" {
        1
    } else {
        inputs.len()
    };
    let selected = &inputs[..count];
    let last = selected.last().expect("nonempty input batch");
    let run = Uuid::new_v4();
    let next = Uuid::new_v4();
    let objective = selected
        .iter()
        .map(|input| input.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let key = format!("event:{}:{}", invocation, selected[0].id);
    crate::chat::db::run_create_one(
        tx,
        run,
        state.thread_id,
        state.agent_id,
        &(objective),
        None::<Uuid>,
        &(key),
    )
    .await?;
    crate::chat::access::db::run_source_execute(
        tx,
        run,
        last.source_identity_id,
        last.channel_origin,
    )
    .await?;
    let deployment = super::pin_deployment(tx, state.thread_id, state.agent_id).await?;
    crate::chat::db::invocation_create_execute(
        tx,
        next,
        run,
        state.thread_id,
        state.agent_id,
        &(crate::telemetry::tracing::context::capture().0),
        &(crate::telemetry::tracing::context::capture().1),
        deployment,
    )
    .await?;
    crate::chat::db::invocation_cutoff_execute(tx, next, last.history_through_message_id).await?;
    crate::chat::db::input_move_execute(tx, invocation, next).await?;
    for input in selected {
        crate::chat::db::input_accept_execute(tx, next, input.id).await?;
    }
    activity(pool, tx, state.thread_id, "invocation.pending", next, "").await?;
    Ok(())
}
