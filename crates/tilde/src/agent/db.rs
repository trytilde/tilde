//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};

fn avatar_row(r: tilde_queries::queries::agent::avatar::Record) -> DbResult<crate::agent::Agent> {
    Ok(crate::agent::Agent {
        id: r.id,
        name: r.name,
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

pub async fn avatar_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<crate::agent::Agent> {
    avatar_row(
        tilde_queries::queries::agent::avatar::run()
            .bind(db, &p2, &p1)
            .opt()
            .await?
            .ok_or(DbError::NotFound)?,
    )
}

fn create_row(r: tilde_queries::queries::agent::create::Record) -> DbResult<crate::agent::Agent> {
    Ok(crate::agent::Agent {
        id: r.id,
        name: r.name,
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

pub async fn create_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: &str,
    p4: &serde_json::Value,
) -> DbResult<Option<crate::agent::Agent>> {
    tilde_queries::queries::agent::create::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .opt()
        .await?
        .map(create_row)
        .transpose()
}

pub async fn delete_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::agent::delete::run()
        .bind(db, &p1)
        .await?)
}

fn get_row(r: tilde_queries::queries::agent::get::Record) -> DbResult<crate::agent::Agent> {
    Ok(crate::agent::Agent {
        id: r.id,
        name: r.name,
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

pub async fn get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<crate::agent::Agent>> {
    tilde_queries::queries::agent::get::run()
        .bind(db, &p1)
        .opt()
        .await?
        .map(get_row)
        .transpose()
}

pub struct GetForCreateRow {
    pub id: uuid::Uuid,
    pub name: String,
    pub concurrency_policy: String,
    pub avatar_seed: uuid::Uuid,
    pub avatar_key: Option<String>,
    pub paused: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub capabilities: tokio_postgres::types::Json<crate::iam::capabilities::Capabilities>,
}

fn get_for_create_row(
    r: tilde_queries::queries::agent::get_for_create::Record,
) -> DbResult<GetForCreateRow> {
    Ok(GetForCreateRow {
        id: r.id,
        name: r.name,
        concurrency_policy: r.concurrency_policy,
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

pub async fn get_for_create_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<GetForCreateRow>> {
    tilde_queries::queries::agent::get_for_create::run()
        .bind(db, &p1)
        .opt()
        .await?
        .map(get_for_create_row)
        .transpose()
}

pub use tilde_queries::queries::agent::lifecycle::Record as LifecycleRow;

pub async fn lifecycle_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: bool,
) -> DbResult<Option<LifecycleRow>> {
    Ok(tilde_queries::queries::agent::lifecycle::run()
        .bind(db, &p2, &p1)
        .opt()
        .await?)
}

fn list_row(r: tilde_queries::queries::agent::list::Record) -> DbResult<crate::agent::Agent> {
    Ok(crate::agent::Agent {
        id: r.id,
        name: r.name,
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

pub async fn list_all(
    db: &impl GenericClient,
    p1: Option<chrono::DateTime<chrono::Utc>>,
    p2: Option<uuid::Uuid>,
    p3: i64,
    p4: &str,
    caller: &crate::iam::authz::Access,
) -> DbResult<Vec<crate::agent::Agent>> {
    tilde_queries::queries::agent::list::run()
        .bind(
            db,
            &p1,
            &p2,
            &p4,
            &caller.admin,
            &caller.user,
            &caller.api_key,
            &caller.groups,
            &p3,
        )
        .all()
        .await?
        .into_iter()
        .map(list_row)
        .collect()
}

pub async fn unassign_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::agent::unassign::run()
        .bind(db, &p1)
        .await?)
}

fn update_row(r: tilde_queries::queries::agent::update::Record) -> DbResult<crate::agent::Agent> {
    Ok(crate::agent::Agent {
        id: r.id,
        name: r.name,
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

pub async fn update_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: Option<&str>,
    p3: Option<&str>,
    p4: Option<&serde_json::Value>,
) -> DbResult<Option<crate::agent::Agent>> {
    tilde_queries::queries::agent::update::run()
        .bind(db, &p2, &p3, &p4, &p1)
        .opt()
        .await?
        .map(update_row)
        .transpose()
}

pub async fn health_cleanup_execute(
    db: &impl GenericClient,
    p1: chrono::DateTime<chrono::Utc>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::agent_health::cleanup::run()
        .bind(db, &p1)
        .await?)
}

pub use tilde_queries::queries::agent_health::history::Record as HealthHistoryRow;

pub async fn health_history_all(
    db: &impl GenericClient,
    p1: &[uuid::Uuid],
    p2: chrono::DateTime<chrono::Utc>,
) -> DbResult<Vec<HealthHistoryRow>> {
    Ok(tilde_queries::queries::agent_health::history::run()
        .bind(db, &p2, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::agent_health::lock::Record as HealthLockRow;

pub async fn health_lock_one(db: &impl GenericClient) -> DbResult<HealthLockRow> {
    tilde_queries::queries::agent_health::lock::run()
        .bind(db)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::agent_health::metrics::Record as HealthMetricsRow;

pub async fn health_metrics_all(
    db: &impl GenericClient,
    p1: &[uuid::Uuid],
    p2: chrono::DateTime<chrono::Utc>,
) -> DbResult<Vec<HealthMetricsRow>> {
    Ok(tilde_queries::queries::agent_health::metrics::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

/// Persist the aggregate routing-health sample, including unavailable agents.
pub async fn health_sample_execute(
    db: &impl GenericClient,
    at: chrono::DateTime<chrono::Utc>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::agent_health::sample::run()
        .bind(db, &at)
        .await?)
}
