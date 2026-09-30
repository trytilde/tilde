//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};

fn agent_lock_row(
    r: tilde_queries::queries::iam::agent_lock::Record,
) -> DbResult<crate::agent::Agent> {
    Ok(crate::agent::Agent {
        id: r.id,
        name: r.name,
        description: r.description,
        concurrency_policy: serde_json::from_value(serde_json::Value::String(r.concurrency_policy))
            .map_err(crate::database::DbError::decode)?,
        avatar_seed: r.avatar_seed,
        avatar_key: r.avatar_key,
        paused: r.paused,

        created_at: r.created_at,
        updated_at: r.updated_at,
        capabilities: tokio_postgres::types::Json(
            serde_json::from_value(r.capabilities).map_err(crate::database::DbError::decode)?,
        ),
    })
}

pub async fn agent_lock_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<crate::agent::Agent>> {
    tilde_queries::queries::iam::agent_lock::run()
        .bind(db, &p1)
        .opt()
        .await?
        .map(agent_lock_row)
        .transpose()
}

pub use tilde_queries::queries::iam::inference_connections::Record as InferenceConnectionRow;

pub async fn inference_connections_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<InferenceConnectionRow>> {
    Ok(tilde_queries::queries::iam::inference_connections::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub struct InvocationLiveRow {
    pub participant_id: uuid::Uuid,
    pub capabilities: tokio_postgres::types::Json<crate::iam::capabilities::Capabilities>,
}

fn invocation_live_row(
    r: tilde_queries::queries::iam::invocation_live::Record,
) -> DbResult<InvocationLiveRow> {
    Ok(InvocationLiveRow {
        participant_id: r.participant_id,
        capabilities: tokio_postgres::types::Json(
            serde_json::from_value(r.capabilities).map_err(crate::database::DbError::decode)?,
        ),
    })
}

pub async fn invocation_live_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
) -> DbResult<Option<InvocationLiveRow>> {
    tilde_queries::queries::iam::invocation_live::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .opt()
        .await?
        .map(invocation_live_row)
        .transpose()
}

pub use tilde_queries::queries::iam::invocation_trace_live::Record as InvocationTraceLiveRow;

pub async fn invocation_trace_live_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
    p5: f64,
) -> DbResult<InvocationTraceLiveRow> {
    tilde_queries::queries::iam::invocation_trace_live::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::iam::key_get::Record as KeyGetRow;

pub async fn key_get_one(db: &impl GenericClient) -> DbResult<KeyGetRow> {
    tilde_queries::queries::iam::key_get::run()
        .bind(db)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn key_insert_execute(db: &impl GenericClient, p1: &[u8]) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::key_insert::run()
        .bind(db, &p1)
        .await?)
}
