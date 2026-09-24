//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};

pub use tilde_queries::queries::system::ready::Record as ReadyRow;

pub async fn ready_one(db: &impl GenericClient) -> DbResult<ReadyRow> {
    tilde_queries::queries::system::ready::run()
        .bind(db)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn test_age_connection_setup_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::testing::age_connection_setup::run()
        .bind(db, &p1)
        .await?)
}

pub use tilde_queries::queries::testing::connection_setup_lifetime::Record as TestConnectionSetupLifetimeRow;

pub async fn test_connection_setup_lifetime_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<TestConnectionSetupLifetimeRow> {
    tilde_queries::queries::testing::connection_setup_lifetime::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::testing::connection_setup_material::Record as TestConnectionSetupMaterialRow;

pub async fn test_connection_setup_material_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<TestConnectionSetupMaterialRow> {
    tilde_queries::queries::testing::connection_setup_material::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn test_live_channel_ready_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: bool,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::testing::live_channel_ready::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub async fn test_live_chat_lease_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::testing::live_chat_lease::run()
        .bind(db, &p1)
        .await?)
}
