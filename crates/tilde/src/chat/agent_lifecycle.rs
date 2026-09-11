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

/// Carry unaccepted input into one pending invocation without reopening a completed run.
/// The caller holds the thread lock and has terminalized the previous invocation.
pub(crate) async fn requeue_inputs(
    tx: &mut Transaction<'_, Postgres>,
    invocation: Uuid,
    run_id: Uuid,
) -> Result<()> {
    let inputs = sqlx::query_file!("../../queries/chat/inputs_pending.sql", invocation)
        .fetch_all(&mut **tx)
        .await?;
    if !inputs.is_empty() {
        let state = sqlx::query_file!("../../queries/chat/run_state.sql", run_id)
            .fetch_one(&mut **tx)
            .await?;
        let next_run = if state.status == "waiting" {
            sqlx::query_file!("../../queries/chat/run_resume.sql", run_id)
                .fetch_one(&mut **tx)
                .await?;
            run_id
        } else {
            let run = Uuid::new_v4();
            let objective = inputs
                .iter()
                .map(|v| v.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let key = format!("pending:{invocation}");
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
                state.source_identity_id,
                state.channel_origin
            )
            .execute(&mut **tx)
            .await?;
            run
        };
        let next = Uuid::new_v4();
        sqlx::query_file!(
            "../../queries/chat/invocation_create.sql",
            next,
            next_run,
            state.thread_id,
            state.agent_id,
            crate::telemetry::context::capture().0,
            crate::telemetry::context::capture().1
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query_file!("../../queries/chat/input_move.sql", invocation, next)
            .execute(&mut **tx)
            .await?;
        activity(tx, state.thread_id, "invocation.pending", next, "").await?;
    }
    Ok(())
}
