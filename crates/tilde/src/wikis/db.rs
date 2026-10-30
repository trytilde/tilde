//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
//! Generated binds take parameters in the order they first appear in each query.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbResult, GenericClient};
use tilde_queries::queries::wikis as q;
use uuid::Uuid;

pub use q::agent_wikis::Record as AgentWikiRow;
pub use q::grep::Run as GrepRow;
pub use q::page_get::Record as PageRow;
pub use q::pages_list::Run as PageSummaryRow;
pub use q::revision_by_call::Run as CallRow;
pub use q::revision_get::Record as RevisionContentRow;
pub use q::revisions_list::Record as RevisionRow;
pub use q::search::Run as SearchRow;
pub use q::wiki_list::Record as WikiRow;

pub async fn wiki_list_all(
    db: &impl GenericClient,
    id: Option<Uuid>,
    search: Option<&str>,
) -> DbResult<Vec<WikiRow>> {
    Ok(q::wiki_list::run().bind(db, &id, &search).all().await?)
}
pub async fn wiki_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    name: &str,
    description: &str,
    agent: Option<Uuid>,
) -> DbResult<u64> {
    Ok(q::wiki_insert::run()
        .bind(db, &id, &name, &description, &agent)
        .await?)
}
pub async fn wiki_update_execute(
    db: &impl GenericClient,
    id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
) -> DbResult<u64> {
    Ok(q::wiki_update::run()
        .bind(db, &name, &description, &id)
        .await?)
}
pub async fn wiki_delete_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::wiki_delete::run().bind(db, &id).await?)
}
pub async fn agent_live_one(db: &impl GenericClient, agent: Uuid) -> DbResult<bool> {
    Ok(q::agent_live::run().bind(db, &agent).one().await?)
}
/// A deleted agent's own wikis and its links.
pub async fn agent_wiki_delete_execute(db: &impl GenericClient, agent: Uuid) -> DbResult<u64> {
    Ok(q::agent_wiki_delete::run().bind(db, &agent).await?)
}
pub async fn agent_wikis_all(db: &impl GenericClient, agent: Uuid) -> DbResult<Vec<AgentWikiRow>> {
    Ok(q::agent_wikis::run().bind(db, &agent).all().await?)
}
pub async fn link_set_execute(
    db: &impl GenericClient,
    agent: Uuid,
    wiki: Uuid,
    access: &str,
) -> DbResult<u64> {
    Ok(q::link_set::run().bind(db, &agent, &wiki, &access).await?)
}
pub async fn link_delete_execute(
    db: &impl GenericClient,
    agent: Uuid,
    wiki: Uuid,
) -> DbResult<u64> {
    Ok(q::link_delete::run().bind(db, &agent, &wiki).await?)
}
pub async fn pages_list_all(
    db: &impl GenericClient,
    wiki: Uuid,
    prefix: Option<&str>,
    limit: i64,
) -> DbResult<Vec<PageSummaryRow>> {
    Ok(q::pages_list::run()
        .bind(db, &wiki, &prefix, &limit)
        .all()
        .await?)
}
pub async fn page_by_id_opt(db: &impl GenericClient, id: Uuid) -> DbResult<Option<PageRow>> {
    Ok(q::page_get::run()
        .bind(db, &Some(id), &None::<Uuid>, &None::<&str>)
        .opt()
        .await?)
}
pub async fn page_by_path_opt(
    db: &impl GenericClient,
    wiki: Uuid,
    path: &str,
) -> DbResult<Option<PageRow>> {
    Ok(q::page_get::run()
        .bind(db, &None::<Uuid>, &Some(wiki), &Some(path))
        .opt()
        .await?)
}
pub async fn page_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    wiki: Uuid,
    path: &str,
    title: &str,
    content: &str,
) -> DbResult<u64> {
    Ok(q::page_insert::run()
        .bind(db, &id, &wiki, &path, &title, &content)
        .await?)
}
/// The new revision, or none when the page is no longer at `expected`.
pub async fn page_update_opt(
    db: &impl GenericClient,
    id: Uuid,
    expected: i32,
    path: &str,
    title: &str,
    content: &str,
) -> DbResult<Option<i32>> {
    Ok(q::page_update::run()
        .bind(db, &path, &title, &content, &id, &expected)
        .opt()
        .await?)
}
/// Whether the page was deleted at `expected`.
pub async fn page_delete_execute(
    db: &impl GenericClient,
    id: Uuid,
    expected: i32,
) -> DbResult<bool> {
    Ok(q::page_delete::run()
        .bind(db, &id, &expected)
        .opt()
        .await?
        .is_some())
}
pub async fn revision_insert_execute(
    db: &impl GenericClient,
    page: &PageRow,
    revision: i32,
    operation: &str,
    write: &super::Write,
) -> DbResult<u64> {
    Ok(q::revision_insert::run()
        .bind(
            db,
            &page.id,
            &page.wiki_id,
            &revision,
            &operation,
            &page.path,
            &page.title,
            &page.content,
            &write.agent,
            &write.thread,
            &write.call,
        )
        .await?)
}
pub async fn revisions_list_all(db: &impl GenericClient, page: Uuid) -> DbResult<Vec<RevisionRow>> {
    Ok(q::revisions_list::run().bind(db, &page).all().await?)
}
pub async fn revision_get_opt(
    db: &impl GenericClient,
    page: Uuid,
    revision: i32,
) -> DbResult<Option<RevisionContentRow>> {
    Ok(q::revision_get::run()
        .bind(db, &page, &revision)
        .opt()
        .await?)
}
pub async fn search_all(
    db: &impl GenericClient,
    wikis: &[Uuid],
    query: &str,
    limit: i64,
) -> DbResult<Vec<SearchRow>> {
    Ok(q::search::run()
        .bind(db, &query, &wikis, &limit)
        .all()
        .await?)
}
/// Sets the statement timeout grep runs under, for the current transaction.
pub async fn grep_session_one(db: &impl GenericClient) -> DbResult<String> {
    Ok(q::grep_session::run().bind(db).one().await?)
}
pub async fn grep_all(
    db: &impl GenericClient,
    wikis: &[Uuid],
    pattern: &str,
    ignore_case: bool,
    limit: i64,
) -> DbResult<Vec<GrepRow>> {
    Ok(q::grep::run()
        .bind(db, &wikis, &ignore_case, &pattern, &limit)
        .all()
        .await?)
}
pub async fn revision_by_call_opt(
    db: &impl GenericClient,
    call: Uuid,
) -> DbResult<Option<CallRow>> {
    Ok(q::revision_by_call::run().bind(db, &call).opt().await?)
}
