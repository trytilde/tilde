//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};
use tilde_queries::queries::prompts as q;
use uuid::Uuid;

pub use q::deployment_versions::Record as DeploymentVersionRow;
pub use q::prompt_get::Record as PromptRow;
pub use q::prompts_list::Record as PromptListRow;
pub use q::sections_list::Record as SectionRow;
pub use q::usage::Record as UsageRow;
pub use q::versions_by_ids::Record as VersionRow;

pub async fn prompt_upsert_one(
    db: &impl GenericClient,
    id: Uuid,
    agent: Uuid,
    name: &str,
) -> DbResult<Uuid> {
    Ok(q::prompt_upsert::run()
        .bind(db, &id, &agent, &name)
        .one()
        .await?
        .id)
}
pub async fn prompt_lock_one(db: &impl GenericClient, id: Uuid) -> DbResult<Uuid> {
    Ok(q::prompt_lock::run()
        .bind(db, &id)
        .opt()
        .await?
        .ok_or(DbError::NotFound)?
        .id)
}
pub async fn prompt_get_opt(db: &impl GenericClient, id: Uuid) -> DbResult<Option<PromptRow>> {
    Ok(q::prompt_get::run().bind(db, &id).opt().await?)
}
pub async fn prompts_list_all(
    db: &impl GenericClient,
    agent: Uuid,
) -> DbResult<Vec<PromptListRow>> {
    Ok(q::prompts_list::run().bind(db, &agent).all().await?)
}
pub async fn invocation_deployments_all(
    db: &impl GenericClient,
    invocations: &[Uuid],
) -> DbResult<Vec<(Uuid, Uuid)>> {
    Ok(q::invocation_deployments::run()
        .bind(db, &invocations)
        .all()
        .await?
        .into_iter()
        .map(|r| (r.invocation_id, r.deployment_id))
        .collect())
}
pub async fn deployment_link_execute(
    db: &impl GenericClient,
    deployment: Uuid,
    version: Uuid,
) -> DbResult<u64> {
    Ok(q::deployment_link::run()
        .bind(db, &deployment, &version)
        .await?)
}
pub async fn deployment_versions_all(
    db: &impl GenericClient,
    deployment: Uuid,
) -> DbResult<Vec<DeploymentVersionRow>> {
    Ok(q::deployment_versions::run()
        .bind(db, &deployment)
        .all()
        .await?)
}
pub async fn version_by_stamp_opt(
    db: &impl GenericClient,
    agent: Uuid,
    name: &str,
    hash: &[u8],
) -> DbResult<Option<Uuid>> {
    Ok(q::version_by_stamp::run()
        .bind(db, &agent, &name, &hash)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn request_links_insert_execute(
    db: &impl GenericClient,
    requests: &[Uuid],
    versions: &[Uuid],
) -> DbResult<u64> {
    Ok(q::request_links_insert::run()
        .bind(db, &requests, &versions)
        .await?)
}
pub async fn version_by_hash_opt(
    db: &impl GenericClient,
    prompt: Uuid,
    hash: &[u8],
) -> DbResult<Option<Uuid>> {
    Ok(q::version_by_hash::run()
        .bind(db, &prompt, &hash)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn version_insert_one(
    db: &impl GenericClient,
    id: Uuid,
    prompt: Uuid,
    hash: &[u8],
    template: &str,
    config: &str,
    variables: &[String],
    format: &str,
    origin: &str,
    deployment: Uuid,
) -> DbResult<Uuid> {
    Ok(q::version_insert::run()
        .bind(
            db,
            &id,
            &prompt,
            &hash,
            &template,
            &config,
            &variables,
            &format,
            &origin,
            &deployment,
        )
        .one()
        .await?
        .id)
}
pub async fn versions_list_all(
    db: &impl GenericClient,
    prompt: Uuid,
) -> DbResult<Vec<q::versions_list::Record>> {
    Ok(q::versions_list::run().bind(db, &prompt).all().await?)
}
pub async fn versions_by_ids_all(
    db: &impl GenericClient,
    ids: &[Uuid],
) -> DbResult<Vec<VersionRow>> {
    Ok(q::versions_by_ids::run().bind(db, &ids).all().await?)
}
pub async fn sections_insert_execute(
    db: &impl GenericClient,
    version: Uuid,
    names: &[&str],
    hashes: &[Vec<u8>],
    contents: &[&str],
) -> DbResult<u64> {
    Ok(q::sections_insert::run()
        .bind(db, &version, &names, &hashes, &contents)
        .await?)
}
pub async fn sections_list_all(
    db: &impl GenericClient,
    versions: &[Uuid],
) -> DbResult<Vec<SectionRow>> {
    Ok(q::sections_list::run().bind(db, &versions).all().await?)
}
pub async fn usage_all(db: &impl GenericClient, prompt: Uuid) -> DbResult<Vec<UsageRow>> {
    Ok(q::usage::run().bind(db, &prompt).all().await?)
}
