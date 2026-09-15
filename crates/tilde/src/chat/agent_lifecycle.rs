//! Chat mutations coordinated under the registry's agent lock, before thread locks.
use super::*;

/// Terminalize running invocations on pause, retaining pending work until resume.
/// Deletion additionally cancels pending invocations.
pub(crate) async fn stop_invocations(
    tx: &mut Transaction<'_, Postgres>,
    agent: Uuid,
    deleting: bool,
) -> Result<()> {
    let invocations =
        sqlx::query_file!("../../queries/chat/agent_invocations.sql", agent, deleting)
            .fetch_all(&mut **tx)
            .await?;
    for invocation in invocations {
        sqlx::query_file!("../../queries/chat/thread_lock.sql", invocation.thread_id)
            .fetch_one(&mut **tx)
            .await?;
        let Some(row) = sqlx::query_file!(
            "../../queries/chat/invocation_finish.sql",
            invocation.id,
            "canceled"
        )
        .fetch_optional(&mut **tx)
        .await?
        else {
            continue;
        };
        let status = if deleting { "canceled" } else { "waiting" };
        sqlx::query_file!("../../queries/chat/run_status.sql", row.run_id, status)
            .execute(&mut **tx)
            .await?;
        activity(
            tx,
            row.thread_id,
            "invocation.ended",
            invocation.id,
            "canceled",
        )
        .await?;
        if !deleting {
            requeue_inputs(tx, invocation.id, row.run_id).await?;
        }
    }
    Ok(())
}

/// Historical participants remain addressable in snapshots but no longer receive work.
pub(crate) async fn retire(tx: &mut Transaction<'_, Postgres>, agent: Uuid) -> Result<()> {
    let rows = sqlx::query_file!("../../queries/chat/agent_participants.sql", agent)
        .fetch_all(&mut **tx)
        .await?;
    for row in rows {
        sqlx::query_file!("../../queries/chat/thread_lock.sql", row.thread_id)
            .fetch_one(&mut **tx)
            .await?;
        sqlx::query_file!(
            "../../queries/chat/participant_active.sql",
            row.thread_id,
            row.id,
            false
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query_file!("../../queries/chat/typing_clear.sql", row.thread_id, row.id)
            .execute(&mut **tx)
            .await?;
        activity(tx, row.thread_id, "participant.left", row.id, "").await?;
    }
    Ok(())
}

/// Dispatch pending events into a fresh invocation. Inputs are consumed by API policy,
/// never acknowledged or drained by an agent's implementation. Caller holds the thread lock.
pub(crate) async fn requeue_inputs(
    tx: &mut Transaction<'_, Postgres>,
    invocation: Uuid,
    run_id: Uuid,
) -> Result<()> {
    let inputs = sqlx::query_file!("../../queries/chat/inputs_pending.sql", invocation)
        .fetch_all(&mut **tx)
        .await?;
    if inputs.is_empty() {
        return Ok(());
    }
    let state = sqlx::query_file!("../../queries/chat/run_state.sql", run_id)
        .fetch_one(&mut **tx)
        .await?;
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
    sqlx::query_file!(
        "../../queries/chat/run_create.sql",
        run,
        state.thread_id,
        state.agent_id,
        objective,
        None::<Uuid>,
        key
    )
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query_file!(
        "../../queries/channel_access/run_source.sql",
        run,
        last.source_identity_id,
        last.channel_origin
    )
    .execute(&mut **tx)
    .await?;
    let deployment = super::pin_deployment(tx, state.thread_id, state.agent_id).await?;
    sqlx::query_file!(
        "../../queries/chat/invocation_create.sql",
        next,
        run,
        state.thread_id,
        state.agent_id,
        crate::telemetry::context::capture().0,
        crate::telemetry::context::capture().1,
        deployment
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query_file!(
        "../../queries/chat/invocation_cutoff.sql",
        next,
        last.history_through_message_id
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query_file!("../../queries/chat/input_move.sql", invocation, next)
        .execute(&mut **tx)
        .await?;
    for input in selected {
        sqlx::query_file!("../../queries/chat/input_accept.sql", next, input.id)
            .execute(&mut **tx)
            .await?;
    }
    activity(tx, state.thread_id, "invocation.pending", next, "").await?;
    Ok(())
}
