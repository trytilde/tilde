//! Typed PostgreSQL operations for local agents. Callers own transactions.
use super::Harness;
use crate::database::{DbError, DbResult, GenericClient};
use tilde_queries::queries::local as q;
use uuid::Uuid;

fn harness(value: &str) -> DbResult<Harness> {
    Harness::parse(value).ok_or_else(|| DbError::decode("unknown harness"))
}
pub async fn register_execute(
    db: &impl GenericClient,
    agent: Uuid,
    value: Harness,
) -> DbResult<()> {
    q::register::run().bind(db, &agent, &value.as_str()).await?;
    Ok(())
}
pub async fn remove_execute(db: &impl GenericClient, agent: Uuid) -> DbResult<()> {
    q::remove::run().bind(db, &agent).await?;
    Ok(())
}
/// The harness of each local agent among `agents`.
pub async fn harnesses_all(
    db: &impl GenericClient,
    agents: &[Uuid],
) -> DbResult<std::collections::HashMap<Uuid, Harness>> {
    q::harnesses::run()
        .bind(db, &agents)
        .all()
        .await?
        .into_iter()
        .map(|row| Ok((row.agent_id, harness(&row.harness)?)))
        .collect()
}
pub async fn harness_opt(db: &impl GenericClient, agent: Uuid) -> DbResult<Option<Harness>> {
    Ok(harnesses_all(db, &[agent]).await?.remove(&agent))
}
/// Whether the message was new.
pub async fn message_import_execute(
    db: &impl GenericClient,
    id: Uuid,
    thread: Uuid,
    participant: Uuid,
    text: &str,
    at: chrono::DateTime<chrono::Utc>,
) -> DbResult<bool> {
    Ok(q::message_import::run()
        .bind(db, &id, &thread, &participant, &text, &at)
        .await?
        > 0)
}
