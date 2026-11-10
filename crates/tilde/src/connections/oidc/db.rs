//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
use crate::database::{DbResult, GenericClient};
use tilde_queries::queries::oidc as q;
use uuid::Uuid;

pub use q::key_list::Record as KeyRow;

pub async fn key_list_all(db: &impl GenericClient, published_days: i32) -> DbResult<Vec<KeyRow>> {
    Ok(q::key_list::run().bind(db, &published_days).all().await?)
}
pub async fn key_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    sealed: &[u8],
    fresh_days: i32,
) -> DbResult<u64> {
    Ok(q::key_insert::run()
        .bind(db, &id, &sealed, &fresh_days)
        .await?)
}
pub async fn key_prune_execute(db: &impl GenericClient, published_days: i32) -> DbResult<u64> {
    Ok(q::key_prune::run().bind(db, &published_days).await?)
}
