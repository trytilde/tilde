//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbResult, GenericClient};
use chrono::{DateTime, Utc};
use tilde_queries::queries::routines as q;
use uuid::Uuid;

pub use q::routine_due::Record as DueRow;
pub use q::routine_list::Record as RoutineRow;
pub use q::routine_signal_matches::Record as MatchRow;
pub use q::signal_connection::Record as SignalConnectionRow;

pub async fn routine_list_all(
    db: &impl GenericClient,
    agent: Option<Uuid>,
    id: Option<Uuid>,
) -> DbResult<Vec<RoutineRow>> {
    Ok(q::routine_list::run().bind(db, &agent, &id).all().await?)
}
pub async fn routine_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    agent: Uuid,
    name: &str,
    prompt: &str,
    thread_title: &str,
    enabled: bool,
    schedule: Option<&str>,
    connection: Option<Uuid>,
    signal_type: Option<&str>,
    next_run_at: Option<DateTime<Utc>>,
) -> DbResult<u64> {
    Ok(q::routine_insert::run()
        .bind(
            db,
            &id,
            &agent,
            &name,
            &prompt,
            &thread_title,
            &enabled,
            &schedule,
            &connection,
            &signal_type,
            &next_run_at,
        )
        .await?)
}
pub async fn routine_update_execute(
    db: &impl GenericClient,
    id: Uuid,
    name: &str,
    prompt: &str,
    thread_title: &str,
    enabled: bool,
    schedule: Option<&str>,
    connection: Option<Uuid>,
    signal_type: Option<&str>,
    next_run_at: Option<DateTime<Utc>>,
) -> DbResult<u64> {
    // Parameters bind in order of first appearance in the statement: the ID comes last.
    Ok(q::routine_update::run()
        .bind(
            db,
            &name,
            &prompt,
            &thread_title,
            &enabled,
            &schedule,
            &connection,
            &signal_type,
            &next_run_at,
            &id,
        )
        .await?)
}
pub async fn routine_delete_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::routine_delete::run().bind(db, &id).await?)
}
pub async fn routine_due_all(db: &impl GenericClient, limit: i64) -> DbResult<Vec<DueRow>> {
    Ok(q::routine_due::run().bind(db, &limit).all().await?)
}
pub async fn routine_reschedule_execute(
    db: &impl GenericClient,
    id: Uuid,
    next_run_at: Option<DateTime<Utc>>,
) -> DbResult<u64> {
    Ok(q::routine_reschedule::run()
        .bind(db, &next_run_at, &id)
        .await?)
}
pub async fn routine_signal_matches_all(
    db: &impl GenericClient,
    connection: Uuid,
    signal_type: &str,
) -> DbResult<Vec<MatchRow>> {
    Ok(q::routine_signal_matches::run()
        .bind(db, &connection, &signal_type)
        .all()
        .await?)
}
pub async fn run_claim_opt(
    db: &impl GenericClient,
    id: Uuid,
    routine: Uuid,
    fire_key: &str,
) -> DbResult<Option<Uuid>> {
    Ok(q::run_claim::run()
        .bind(db, &id, &routine, &fire_key)
        .opt()
        .await?
        .map(|row| row.id))
}
pub async fn run_finish_execute(
    db: &impl GenericClient,
    id: Uuid,
    thread: Option<Uuid>,
    error: Option<&str>,
) -> DbResult<u64> {
    Ok(q::run_finish::run().bind(db, &thread, &error, &id).await?)
}
pub async fn signal_connection_opt(
    db: &impl GenericClient,
    id: Uuid,
) -> DbResult<Option<SignalConnectionRow>> {
    Ok(q::signal_connection::run().bind(db, &id).opt().await?)
}
