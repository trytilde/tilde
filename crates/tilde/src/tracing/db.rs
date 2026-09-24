//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};

pub use tilde_queries::queries::telemetry::capacity::Record as DeliveryCapacityRow;

pub async fn delivery_capacity_one(db: &impl GenericClient) -> DbResult<DeliveryCapacityRow> {
    tilde_queries::queries::telemetry::capacity::run()
        .bind(db)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn delivery_cleanup_execute(db: &impl GenericClient) -> DbResult<u64> {
    Ok(tilde_queries::queries::telemetry::cleanup::run()
        .bind(db)
        .await?)
}

pub async fn delivery_complete_execute(db: &impl GenericClient, p1: &[u8]) -> DbResult<u64> {
    Ok(tilde_queries::queries::telemetry::complete::run()
        .bind(db, &p1)
        .await?)
}

pub use tilde_queries::queries::telemetry::deadline::Record as DeliveryDeadlineRow;

pub async fn delivery_deadline_one(db: &impl GenericClient) -> DbResult<DeliveryDeadlineRow> {
    tilde_queries::queries::telemetry::deadline::run()
        .bind(db)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn delivery_discard_execute(db: &impl GenericClient) -> DbResult<u64> {
    Ok(tilde_queries::queries::telemetry::discard::run()
        .bind(db)
        .await?)
}

pub async fn delivery_enqueue_execute(
    db: &impl GenericClient,
    p1: &[u8],
    p2: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::telemetry::enqueue::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub use tilde_queries::queries::telemetry::exists::Record as DeliveryExistsRow;

pub async fn delivery_exists_one(
    db: &impl GenericClient,
    p1: &[u8],
) -> DbResult<DeliveryExistsRow> {
    tilde_queries::queries::telemetry::exists::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn delivery_lock_execute(db: &impl GenericClient) -> DbResult<u64> {
    tilde_queries::queries::telemetry::lock::run()
        .bind(db)
        .one()
        .await?;
    Ok(1)
}

pub use tilde_queries::queries::telemetry::pending::Record as DeliveryPendingRow;

pub async fn delivery_pending_opt(db: &impl GenericClient) -> DbResult<Option<DeliveryPendingRow>> {
    Ok(tilde_queries::queries::telemetry::pending::run()
        .bind(db)
        .opt()
        .await?)
}

pub async fn delivery_retry_execute(db: &impl GenericClient, p1: &[u8], p2: i32) -> DbResult<u64> {
    Ok(tilde_queries::queries::telemetry::retry::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub use tilde_queries::queries::tracing::agent_exists::Record as AgentExistsRow;

pub async fn agent_exists_one(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<AgentExistsRow> {
    tilde_queries::queries::tracing::agent_exists::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn execution_context_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::tracing::execution_context::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub use tilde_queries::queries::tracing::invocation::Record as InvocationRow;

pub async fn invocation_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<InvocationRow>> {
    Ok(tilde_queries::queries::tracing::invocation::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::tracing::session_metadata::Record as SessionMetadata;
pub async fn session_metadata_all(
    db: &impl GenericClient,
    agents: &[uuid::Uuid],
    sessions: &[uuid::Uuid],
) -> DbResult<Vec<SessionMetadata>> {
    Ok(tilde_queries::queries::tracing::session_metadata::run()
        .bind(db, &agents, &sessions)
        .all()
        .await?)
}
pub use tilde_queries::queries::tracing::session_identity_labels::Record as SessionIdentityLabel;
pub async fn session_identity_labels_all(
    db: &impl GenericClient,
    agent_id: uuid::Uuid,
    sessions: &[uuid::Uuid],
    identities: &[uuid::Uuid],
) -> DbResult<Vec<SessionIdentityLabel>> {
    Ok(
        tilde_queries::queries::tracing::session_identity_labels::run()
            .bind(db, &sessions, &identities, &agent_id)
            .all()
            .await?,
    )
}
