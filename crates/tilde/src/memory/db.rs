//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
//! Generated binds take parameters in the order they first appear in each query.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbResult, GenericClient};
use chrono::{DateTime, Utc};
use tilde_queries::queries::memory as q;
use uuid::Uuid;

pub use q::agent_banks::Record as AgentBankRow;
pub use q::bank_list::Record as BankRow;
pub use q::browse::Run as PathRow;
pub use q::jobs_claim::Run as JobRow;
pub use q::memories_get::Record as MemoryRow;
pub use q::memory_status::Record as StatusRow;
pub use q::revisions_list::Record as RevisionRow;
pub use q::settings_get::Run as SettingsRow;

pub async fn bank_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    name: &str,
    description: &str,
    agent: Option<Uuid>,
    scope: &str,
) -> DbResult<u64> {
    Ok(q::bank_insert::run()
        .bind(db, &id, &name, &description, &agent, &scope)
        .await?)
}
pub async fn bank_list_all(
    db: &impl GenericClient,
    id: Option<Uuid>,
    search: Option<&str>,
) -> DbResult<Vec<BankRow>> {
    Ok(q::bank_list::run().bind(db, &id, &search).all().await?)
}
pub async fn bank_update_execute(
    db: &impl GenericClient,
    id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
) -> DbResult<u64> {
    Ok(q::bank_update::run()
        .bind(db, &name, &description, &id)
        .await?)
}
pub async fn bank_delete_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::bank_delete::run().bind(db, &id).await?)
}
pub async fn agent_live_one(db: &impl GenericClient, agent: Uuid) -> DbResult<bool> {
    Ok(q::agent_live::run().bind(db, &agent).one().await?)
}
/// A deleted agent's own banks and its links.
pub async fn agent_memory_delete_execute(db: &impl GenericClient, agent: Uuid) -> DbResult<u64> {
    Ok(q::agent_memory_delete::run().bind(db, &agent).await?)
}
pub async fn link_set_execute(
    db: &impl GenericClient,
    agent: Uuid,
    bank: Uuid,
    access: &str,
) -> DbResult<u64> {
    Ok(q::link_set::run().bind(db, &agent, &bank, &access).await?)
}
pub async fn link_delete_execute(
    db: &impl GenericClient,
    agent: Uuid,
    bank: Uuid,
) -> DbResult<u64> {
    Ok(q::link_delete::run().bind(db, &agent, &bank).await?)
}
pub async fn agent_banks_all(db: &impl GenericClient, agent: Uuid) -> DbResult<Vec<AgentBankRow>> {
    Ok(q::agent_banks::run().bind(db, &agent).all().await?)
}
pub async fn memory_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    bank: Uuid,
    draft: &super::Draft,
    source: &super::Source,
    supersedes: Option<Uuid>,
) -> DbResult<u64> {
    Ok(q::memory_insert::run()
        .bind(
            db,
            &id,
            &bank,
            &draft.kind,
            &draft.path,
            &draft.title,
            &draft.content,
            &draft.valid_from,
            &draft.ongoing,
            &draft.valid_to,
            &draft.importance,
            &if source.agent.is_some() {
                "explicit"
            } else {
                "owner"
            },
            &source.identity,
            &source.agent,
            &supersedes,
            &source.thread,
        )
        .await?)
}
pub async fn memory_update_execute(
    db: &impl GenericClient,
    id: Uuid,
    change: &super::Change,
) -> DbResult<u64> {
    Ok(q::memory_update::run()
        .bind(
            db,
            &change.kind,
            &change.path,
            &change.title,
            &change.content,
            &change.clear_validity,
            &change.valid_from,
            &change.ongoing,
            &change.valid_to,
            &change.importance,
            &id,
        )
        .await?)
}
pub async fn memory_forget_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::memory_forget::run().bind(db, &id).await?)
}
pub async fn memory_status_opt(db: &impl GenericClient, id: Uuid) -> DbResult<Option<StatusRow>> {
    Ok(q::memory_status::run().bind(db, &id).opt().await?)
}
pub async fn revision_insert_execute(
    db: &impl GenericClient,
    memory: Uuid,
    operation: &str,
    agent: Option<Uuid>,
) -> DbResult<u64> {
    Ok(q::revision_insert::run()
        .bind(db, &operation, &agent, &memory)
        .await?)
}
pub async fn revisions_list_all(
    db: &impl GenericClient,
    memory: Uuid,
) -> DbResult<Vec<RevisionRow>> {
    Ok(q::revisions_list::run().bind(db, &memory).all().await?)
}
/// One memory's embedding, or every live memory's when `memory` is None.
pub async fn job_enqueue_execute(db: &impl GenericClient, memory: Option<Uuid>) -> DbResult<u64> {
    Ok(q::job_enqueue::run().bind(db, &memory).await?)
}
pub async fn memories_get_all(
    db: &impl GenericClient,
    ids: &[Uuid],
    version: Option<i32>,
) -> DbResult<Vec<MemoryRow>> {
    Ok(q::memories_get::run()
        .bind(db, &version, &ids)
        .all()
        .await?)
}
pub async fn memory_list_all(
    db: &impl GenericClient,
    banks: &[Uuid],
    filter: &super::Filter,
    person: super::Person,
    limit: i64,
) -> DbResult<Vec<Uuid>> {
    let (identity, everyone) = person.params();
    Ok(q::memory_list::run()
        .bind(
            db,
            &banks,
            &filter.kind,
            &filter.path,
            &filter.from,
            &filter.to,
            &everyone,
            &identity,
            &limit,
        )
        .all()
        .await?)
}
pub async fn recall_text_all(
    db: &impl GenericClient,
    banks: &[Uuid],
    query: &str,
    filter: &super::Filter,
    person: super::Person,
    limit: i64,
) -> DbResult<Vec<Uuid>> {
    let (identity, everyone) = person.params();
    Ok(q::recall_text::run()
        .bind(
            db,
            &query,
            &banks,
            &filter.kind,
            &filter.path,
            &filter.from,
            &filter.to,
            &everyone,
            &identity,
            &limit,
        )
        .all()
        .await?)
}
/// Call inside a transaction that ran `vector_session_one`.
pub async fn recall_vector_all(
    db: &impl GenericClient,
    banks: &[Uuid],
    vector: &str,
    dims: i32,
    version: i32,
    filter: &super::Filter,
    person: super::Person,
    limit: i64,
) -> DbResult<Vec<Uuid>> {
    let (identity, everyone) = person.params();
    Ok(match dims {
        1024 => {
            q::recall_vector_1024::run()
                .bind(
                    db,
                    &banks,
                    &version,
                    &filter.kind,
                    &filter.path,
                    &filter.from,
                    &filter.to,
                    &everyone,
                    &identity,
                    &vector,
                    &limit,
                )
                .all()
                .await?
        }
        _ => {
            q::recall_vector_1536::run()
                .bind(
                    db,
                    &banks,
                    &version,
                    &filter.kind,
                    &filter.path,
                    &filter.from,
                    &filter.to,
                    &everyone,
                    &identity,
                    &vector,
                    &limit,
                )
                .all()
                .await?
        }
    })
}
pub async fn vector_session_one(db: &impl GenericClient) -> DbResult<()> {
    q::vector_session::run().bind(db).one().await?;
    Ok(())
}
pub async fn browse_all(
    db: &impl GenericClient,
    banks: &[Uuid],
    path: &str,
    person: super::Person,
    limit: i64,
) -> DbResult<Vec<PathRow>> {
    let (identity, everyone) = person.params();
    Ok(q::browse::run()
        .bind(db, &path, &banks, &everyone, &identity, &limit)
        .all()
        .await?)
}
pub async fn at_path_all(
    db: &impl GenericClient,
    banks: &[Uuid],
    path: &str,
    person: super::Person,
    limit: i64,
) -> DbResult<Vec<Uuid>> {
    let (identity, everyone) = person.params();
    Ok(q::at_path::run()
        .bind(db, &banks, &path, &everyone, &identity, &limit)
        .all()
        .await?)
}
pub async fn settings_get_opt(db: &impl GenericClient) -> DbResult<Option<SettingsRow>> {
    Ok(q::settings_get::run().bind(db).opt().await?)
}
pub async fn settings_set_one(
    db: &impl GenericClient,
    connection: Uuid,
    model: &str,
    dims: i32,
) -> DbResult<i32> {
    Ok(q::settings_set::run()
        .bind(db, &connection, &model, &dims)
        .one()
        .await?)
}
pub async fn settings_clear_execute(db: &impl GenericClient) -> DbResult<u64> {
    Ok(q::settings_clear::run().bind(db).await?)
}
pub async fn jobs_claim_all(
    db: &impl GenericClient,
    limit: i64,
    lease_secs: f64,
) -> DbResult<Vec<JobRow>> {
    Ok(q::jobs_claim::run()
        .bind(db, &lease_secs, &limit)
        .all()
        .await?)
}
pub async fn job_done_execute(
    db: &impl GenericClient,
    memory: Uuid,
    requested_at: DateTime<Utc>,
    vector: &str,
    dims: i32,
    version: i32,
) -> DbResult<u64> {
    Ok(q::job_done::run()
        .bind(db, &memory, &requested_at, &vector, &dims, &version)
        .await?)
}
pub async fn job_failed_execute(
    db: &impl GenericClient,
    memory: Uuid,
    requested_at: DateTime<Utc>,
    retry_secs: f64,
) -> DbResult<u64> {
    Ok(q::job_failed::run()
        .bind(db, &retry_secs, &memory, &requested_at)
        .await?)
}
