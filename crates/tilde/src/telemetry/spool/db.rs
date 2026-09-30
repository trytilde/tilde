//! Typed PostgreSQL operations for telemetry object pointers.
use crate::database::{DbResult, GenericClient};
use tilde_queries::queries::spool as q;

pub use q::capacity::Record as Capacity;
pub async fn capacity(db: &impl GenericClient, queue: &str, agent: &str) -> DbResult<Capacity> {
    Ok(q::capacity::run().bind(db, &agent, &queue).one().await?)
}
/// Held until the surrounding transaction ends.
pub async fn lock(db: &impl GenericClient, queue: &str) -> DbResult<()> {
    q::lock::run().bind(db, &queue).one().await?;
    Ok(())
}
/// Zero when the batch is already pending in this queue.
pub async fn insert(
    db: &impl GenericClient,
    key: &str,
    queue: &str,
    agent: &str,
    batch: &str,
    bytes: i64,
) -> DbResult<u64> {
    Ok(q::insert::run()
        .bind(db, &key, &queue, &agent, &batch, &bytes)
        .await?)
}
pub async fn claim(
    db: &impl GenericClient,
    queue: &str,
    lease_seconds: i32,
) -> DbResult<Option<String>> {
    Ok(q::claim::run()
        .bind(db, &lease_seconds, &queue)
        .opt()
        .await?
        .map(|r| r.object_key))
}
pub async fn complete(db: &impl GenericClient, key: &str) -> DbResult<u64> {
    Ok(q::complete::run().bind(db, &key).await?)
}
pub async fn expire(db: &impl GenericClient, retention_seconds: i32) -> DbResult<u64> {
    Ok(q::expire::run().bind(db, &retention_seconds).await?)
}
pub async fn pending(db: &impl GenericClient, queue: &str) -> DbResult<i64> {
    Ok(q::pending::run().bind(db, &queue).one().await?.objects)
}
pub async fn release(db: &impl GenericClient, key: &str, delay_seconds: i32) -> DbResult<u64> {
    Ok(q::release::run().bind(db, &delay_seconds, &key).await?)
}
