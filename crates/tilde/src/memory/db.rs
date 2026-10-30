//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
//! Generated binds take parameters in the order they first appear in each query.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbResult, GenericClient};
use chrono::{DateTime, Utc};
use tilde_queries::queries::memory as q;
use uuid::Uuid;

pub use q::agent_banks::Record as AgentBankRow;
pub use q::bank_list::Record as BankRow;
pub use q::bank_wikis::Run as BankWikiRow;
pub use q::batch_memories::Run as BatchMemoryRow;
pub use q::browse::Run as PathRow;
pub use q::evidence_list::Record as EvidenceRow;
pub use q::jobs_claim::Run as JobRow;
pub use q::memories_get::Record as MemoryRow;
pub use q::memory_paths::Run as PathCountRow;
pub use q::memory_people::Run as PersonRow;
pub use q::memory_status::Record as StatusRow;
pub use q::revisions_list::Record as RevisionRow;
pub use q::settings_get::Run as SettingsRow;
pub use q::synthesis_batches::Record as BatchRow;
pub use q::synthesis_claim::Record as ClaimRow;
pub use q::synthesis_context::Record as ContextRow;
pub use q::synthesis_get::Record as SynthesisRow;
pub use q::synthesis_messages::Record as MessageRow;
pub use q::synthesis_next_page::Record as PageRow;
pub use q::synthesis_pending::Record as PendingRow;

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
            &source.actor.origin(),
            &source.identity,
            &source.actor.agent(),
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
    actor: super::Actor,
) -> DbResult<u64> {
    Ok(q::revision_insert::run()
        .bind(
            db,
            &operation,
            &actor.name(),
            &actor.agent(),
            &actor.batch(),
            &memory,
        )
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
/// Newest first, after memory `after` when given.
pub async fn memory_list_all(
    db: &impl GenericClient,
    banks: &[Uuid],
    filter: &super::Filter,
    person: super::Person,
    after: Option<Uuid>,
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
            &filter.origin,
            &filter.identity,
            &everyone,
            &identity,
            &after,
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
            &filter.origin,
            &filter.identity,
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
                    &filter.origin,
                    &filter.identity,
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
                    &filter.origin,
                    &filter.identity,
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
pub async fn memory_paths_all(
    db: &impl GenericClient,
    bank: Uuid,
    limit: i64,
) -> DbResult<Vec<PathCountRow>> {
    Ok(q::memory_paths::run().bind(db, &bank, &limit).all().await?)
}
pub async fn memory_people_all(db: &impl GenericClient, bank: Uuid) -> DbResult<Vec<PersonRow>> {
    Ok(q::memory_people::run().bind(db, &bank).all().await?)
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

pub async fn synthesis_get_opt(
    db: &impl GenericClient,
    bank: Uuid,
) -> DbResult<Option<SynthesisRow>> {
    Ok(q::synthesis_get::run().bind(db, &bank).opt().await?)
}
pub async fn synthesis_set_execute(
    db: &impl GenericClient,
    bank: Uuid,
    connection: Uuid,
    model: &str,
    instructions: &str,
) -> DbResult<u64> {
    Ok(q::synthesis_set::run()
        .bind(db, &bank, &connection, &model, &instructions)
        .await?)
}
pub async fn synthesis_clear_execute(db: &impl GenericClient, bank: Uuid) -> DbResult<u64> {
    Ok(q::synthesis_clear::run().bind(db, &bank).await?)
}
pub async fn synthesis_run_now_execute(db: &impl GenericClient, bank: Uuid) -> DbResult<u64> {
    Ok(q::synthesis_run_now::run().bind(db, &bank).await?)
}
pub async fn synthesis_claim_opt(
    db: &impl GenericClient,
    lease_secs: f64,
) -> DbResult<Option<ClaimRow>> {
    Ok(q::synthesis_claim::run()
        .bind(db, &lease_secs)
        .opt()
        .await?)
}
pub async fn synthesis_done_execute(
    db: &impl GenericClient,
    bank: Uuid,
    again_secs: f64,
    ran: bool,
) -> DbResult<u64> {
    Ok(q::synthesis_done::run()
        .bind(db, &again_secs, &ran, &bank)
        .await?)
}
pub async fn synthesis_failed_execute(
    db: &impl GenericClient,
    bank: Uuid,
    again_secs: f64,
    error: &str,
) -> DbResult<u64> {
    Ok(q::synthesis_failed::run()
        .bind(db, &again_secs, &error, &bank)
        .await?)
}
pub async fn synthesis_next_thread_opt(
    db: &impl GenericClient,
    bank: Uuid,
    trigger_chars: i64,
) -> DbResult<Option<Uuid>> {
    Ok(q::synthesis_next_thread::run()
        .bind(db, &bank, &trigger_chars)
        .opt()
        .await?)
}
pub async fn synthesis_messages_all(
    db: &impl GenericClient,
    bank: Uuid,
    thread: Uuid,
    limit: i64,
) -> DbResult<Vec<MessageRow>> {
    Ok(q::synthesis_messages::run()
        .bind(db, &bank, &thread, &limit)
        .all()
        .await?)
}
pub async fn synthesis_context_all(
    db: &impl GenericClient,
    bank: Uuid,
    thread: Uuid,
    limit: i64,
) -> DbResult<Vec<ContextRow>> {
    Ok(q::synthesis_context::run()
        .bind(db, &bank, &thread, &limit)
        .all()
        .await?)
}
pub async fn synthesis_person_opt(db: &impl GenericClient, thread: Uuid) -> DbResult<Option<Uuid>> {
    Ok(q::synthesis_person::run().bind(db, &thread).opt().await?)
}
pub async fn synthesis_next_page_opt(
    db: &impl GenericClient,
    bank: Uuid,
    content_chars: i32,
) -> DbResult<Option<PageRow>> {
    Ok(q::synthesis_next_page::run()
        .bind(db, &content_chars, &bank)
        .opt()
        .await?)
}
pub async fn page_revision_opt(
    db: &impl GenericClient,
    page: Uuid,
    revision: i32,
) -> DbResult<Option<q::page_revision::Run>> {
    Ok(q::page_revision::run()
        .bind(db, &page, &revision)
        .opt()
        .await?)
}
pub async fn page_memories_all(
    db: &impl GenericClient,
    bank: Uuid,
    page: Uuid,
) -> DbResult<Vec<Uuid>> {
    Ok(q::page_memories::run()
        .bind(db, &bank, &page)
        .all()
        .await?
        .into_iter()
        .map(|r| r.id)
        .collect())
}
pub async fn gone_page_opt(db: &impl GenericClient, bank: Uuid) -> DbResult<Option<(Uuid, i32)>> {
    Ok(q::gone_page::run()
        .bind(db, &bank)
        .opt()
        .await?
        .map(|r| (r.page_id, r.revision)))
}
pub async fn gone_page_memories_all(
    db: &impl GenericClient,
    bank: Uuid,
    page: Uuid,
) -> DbResult<Vec<Uuid>> {
    Ok(q::gone_page_memories::run()
        .bind(db, &bank, &page)
        .all()
        .await?)
}
pub async fn synthesis_page_unmark_execute(
    db: &impl GenericClient,
    bank: Uuid,
    page: Uuid,
) -> DbResult<u64> {
    Ok(q::synthesis_page_unmark::run()
        .bind(db, &bank, &page)
        .await?)
}
pub async fn synthesis_thread_mark_execute(
    db: &impl GenericClient,
    bank: Uuid,
    thread: Uuid,
    through_at: DateTime<Utc>,
    through_id: Uuid,
) -> DbResult<u64> {
    Ok(q::synthesis_thread_mark::run()
        .bind(db, &bank, &thread, &through_at, &through_id)
        .await?)
}
pub async fn synthesis_page_mark_execute(
    db: &impl GenericClient,
    bank: Uuid,
    page: Uuid,
    revision: i32,
) -> DbResult<u64> {
    Ok(q::synthesis_page_mark::run()
        .bind(db, &bank, &page, &revision)
        .await?)
}
pub async fn synthesis_applied_one(db: &impl GenericClient, batch: Uuid) -> DbResult<bool> {
    Ok(q::synthesis_applied::run().bind(db, &batch).one().await?)
}
/// What a batch did, as recorded.
pub struct BatchRecord<'a> {
    pub id: Uuid,
    pub bank: Uuid,
    /// Where its source part starts.
    pub start: Uuid,
    pub thread: Option<Uuid>,
    pub page: Option<Uuid>,
    pub revision: Option<i32>,
    pub status: &'a str,
    /// A failure its content caused, which counts toward passing over its start.
    pub content_failure: bool,
    pub written: i32,
    pub superseded: i32,
    pub forgotten: i32,
    pub dropped: i32,
    pub notes: String,
}
/// None when the batch was already applied or skipped.
pub async fn synthesis_batch_set_opt(
    db: &impl GenericClient,
    b: &BatchRecord<'_>,
) -> DbResult<Option<Uuid>> {
    Ok(q::synthesis_batch_set::run()
        .bind(
            db,
            &b.id,
            &b.bank,
            &b.start,
            &b.thread,
            &b.page,
            &b.revision,
            &b.status,
            &b.content_failure,
            &b.written,
            &b.superseded,
            &b.forgotten,
            &b.dropped,
            &b.notes,
        )
        .opt()
        .await?)
}
pub async fn start_failures_one(db: &impl GenericClient, start: Uuid) -> DbResult<i32> {
    Ok(q::start_failures::run().bind(db, &start).one().await?)
}
pub async fn synthesis_release_execute(
    db: &impl GenericClient,
    bank: Uuid,
    error: &str,
) -> DbResult<u64> {
    Ok(q::synthesis_release::run().bind(db, &error, &bank).await?)
}
pub async fn page_lineage_one(db: &impl GenericClient, memory: Uuid, page: Uuid) -> DbResult<bool> {
    Ok(q::page_lineage::run()
        .bind(db, &memory, &page)
        .one()
        .await?)
}
pub async fn synthesis_batches_all(
    db: &impl GenericClient,
    bank: Uuid,
    limit: i64,
) -> DbResult<Vec<BatchRow>> {
    Ok(q::synthesis_batches::run()
        .bind(db, &bank, &limit)
        .all()
        .await?)
}
pub async fn batch_memories_all(
    db: &impl GenericClient,
    batches: &[Uuid],
) -> DbResult<Vec<BatchMemoryRow>> {
    Ok(q::batch_memories::run().bind(db, &batches).all().await?)
}
pub async fn synthesis_pending_all(
    db: &impl GenericClient,
    bank: Uuid,
) -> DbResult<Vec<PendingRow>> {
    Ok(q::synthesis_pending::run().bind(db, &bank).all().await?)
}
pub async fn evidence_insert_execute(
    db: &impl GenericClient,
    memory: Uuid,
    batch: Uuid,
    evidence: &super::Evidence,
) -> DbResult<u64> {
    let (thread, message, page, revision) = match *evidence {
        super::Evidence::Message { thread, message } => (Some(thread), Some(message), None, None),
        super::Evidence::Page { page, revision } => (None, None, Some(page), Some(revision)),
    };
    Ok(q::evidence_insert::run()
        .bind(db, &memory, &batch, &thread, &message, &page, &revision)
        .await?)
}
pub async fn evidence_list_all(
    db: &impl GenericClient,
    memories: &[Uuid],
) -> DbResult<Vec<EvidenceRow>> {
    Ok(q::evidence_list::run().bind(db, &memories).all().await?)
}
pub async fn bank_wikis_all(db: &impl GenericClient, bank: Uuid) -> DbResult<Vec<BankWikiRow>> {
    Ok(q::bank_wikis::run().bind(db, &bank).all().await?)
}
pub async fn bank_wiki_set_execute(
    db: &impl GenericClient,
    bank: Uuid,
    wiki: Uuid,
) -> DbResult<u64> {
    Ok(q::bank_wiki_set::run().bind(db, &bank, &wiki).await?)
}
pub async fn bank_wiki_delete_execute(
    db: &impl GenericClient,
    bank: Uuid,
    wiki: Uuid,
) -> DbResult<u64> {
    Ok(q::bank_wiki_delete::run().bind(db, &bank, &wiki).await?)
}
pub async fn link_retain_execute(
    db: &impl GenericClient,
    agent: Uuid,
    bank: Uuid,
    retain: bool,
) -> DbResult<u64> {
    Ok(q::link_retain::run()
        .bind(db, &retain, &agent, &bank)
        .await?)
}
pub async fn retain_start_execute(
    db: &impl GenericClient,
    agent: Uuid,
    bank: Uuid,
) -> DbResult<u64> {
    Ok(q::retain_start::run().bind(db, &bank, &agent).await?)
}
pub async fn retaining_agents_all(db: &impl GenericClient, bank: Uuid) -> DbResult<Vec<Uuid>> {
    Ok(q::retaining_agents::run().bind(db, &bank).all().await?)
}
