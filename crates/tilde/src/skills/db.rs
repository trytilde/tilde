//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
//! Named parameters bind in the order they first appear in the SQL, not the order declared.
use crate::database::{DbError, DbResult, GenericClient};
use tilde_queries::queries::skills as q;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SourceRow {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub catalog_group: Option<String>,
    pub repository_url: Option<String>,
    pub git_ref: Option<String>,
    pub git_path: String,
    pub commit_sha: Option<String>,
    pub sync_error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub skill_count: i32,
}
#[derive(Debug, Clone)]
pub struct SkillRow {
    pub id: Uuid,
    pub source_id: Uuid,
    pub source_name: String,
    pub source_kind: String,
    pub name: String,
    pub source_path: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub latest_id: Option<Uuid>,
}
#[derive(Debug, Clone)]
pub struct VersionRow {
    pub id: Uuid,
    pub skill_id: Uuid,
    pub number: i32,
    pub hash: Vec<u8>,
    pub description: String,
    pub message: String,
    pub commit_sha: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Debug, Clone)]
pub struct FileRow {
    pub version_id: Uuid,
    pub path: String,
    pub media_type: String,
    pub size_bytes: i64,
    pub sha256: Vec<u8>,
    pub executable: bool,
    pub content: Option<String>,
    pub object_key: Option<String>,
}
pub use q::agent_connections::Record as AgentConnectionRow;
pub use q::deployment_skills::Record as DeploymentSkillRow;
pub use q::runtime_skills::Record as RuntimeRow;
pub use q::source_skills::Record as SourceSkillRow;

// Several statements select the same columns; fold each record into one row type.
macro_rules! rows {
    ($target:ident $fields:tt, $($module:ident),*) => {
        $(rows!(@one $target $module $fields);)*
    };
    (@one $target:ident $module:ident { $($field:ident),* }) => {
        impl From<q::$module::Record> for $target {
            fn from(r: q::$module::Record) -> Self {
                Self { $($field: r.$field),* }
            }
        }
    };
}
rows!(
    SourceRow {
        id,
        slug,
        name,
        kind,
        catalog_group,
        repository_url,
        git_ref,
        git_path,
        commit_sha,
        sync_error,
        created_at,
        skill_count
    },
    source_get,
    sources_list,
    sources_all,
    agent_sources,
    connection_sources
);
rows!(
    SkillRow {
        id,
        source_id,
        source_name,
        source_kind,
        name,
        source_path,
        created_at,
        latest_id
    },
    skill_get,
    skills_list,
    skills_all,
    agent_single_skills,
    group_skills
);
rows!(
    VersionRow {
        id,
        skill_id,
        number,
        hash,
        description,
        message,
        commit_sha,
        created_at
    },
    version_get,
    version_latest,
    versions_list,
    versions_by_ids
);
rows!(
    FileRow {
        version_id,
        path,
        media_type,
        size_bytes,
        sha256,
        executable,
        content,
        object_key
    },
    files_list,
    file_get
);
fn all<R, T: From<R>>(rows: Vec<R>) -> Vec<T> {
    rows.into_iter().map(T::from).collect()
}

pub struct NewSource<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub name: &'a str,
    pub kind: &'a str,
    pub catalog_group: Option<&'a str>,
    pub repository_url: Option<&'a str>,
    pub git_ref: Option<&'a str>,
    pub git_path: &'a str,
    /// Set only for an agent's bundled source.
    pub agent_id: Option<Uuid>,
}
pub async fn source_insert_opt(
    db: &impl GenericClient,
    s: NewSource<'_>,
) -> DbResult<Option<Uuid>> {
    Ok(q::source_insert::run()
        .bind(
            db,
            &s.id,
            &s.slug,
            &s.name,
            &s.kind,
            &s.catalog_group,
            &s.repository_url,
            &s.git_ref,
            &s.git_path,
            &s.agent_id,
        )
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn source_get_opt(db: &impl GenericClient, id: Uuid) -> DbResult<Option<SourceRow>> {
    Ok(q::source_get::run()
        .bind(db, &id)
        .opt()
        .await?
        .map(Into::into))
}
pub async fn sources_list_all(db: &impl GenericClient) -> DbResult<Vec<SourceRow>> {
    Ok(all(q::sources_list::run().bind(db).all().await?))
}
pub async fn sources_all(db: &impl GenericClient) -> DbResult<Vec<SourceRow>> {
    Ok(all(q::sources_all::run().bind(db).all().await?))
}
pub async fn agent_sources_all(db: &impl GenericClient, agent: Uuid) -> DbResult<Vec<SourceRow>> {
    Ok(all(q::agent_sources::run().bind(db, &agent).all().await?))
}
pub async fn source_by_slug_opt(
    db: &impl GenericClient,
    slug: &str,
) -> DbResult<Option<(Uuid, String)>> {
    Ok(q::source_by_slug::run()
        .bind(db, &slug)
        .opt()
        .await?
        .map(|r| (r.id, r.kind)))
}
pub async fn source_by_catalog_opt(db: &impl GenericClient, group: &str) -> DbResult<Option<Uuid>> {
    Ok(q::source_by_catalog::run()
        .bind(db, &group)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn slug_taken(db: &impl GenericClient, slug: &str) -> DbResult<bool> {
    Ok(q::slug_taken::run().bind(db, &slug).one().await?.taken)
}
/// Lock the source row, returning its latest started sync generation.
pub async fn source_lock_one(db: &impl GenericClient, id: Uuid) -> DbResult<i64> {
    q::source_lock::run()
        .bind(db, &id)
        .opt()
        .await?
        .map(|r| r.sync_generation)
        .ok_or(DbError::NotFound)
}
pub async fn source_sync_start_one(db: &impl GenericClient, id: Uuid) -> DbResult<i64> {
    q::source_sync_start::run()
        .bind(db, &id)
        .opt()
        .await?
        .map(|r| r.sync_generation)
        .ok_or(DbError::NotFound)
}
/// Records the outcome only while `generation` is still the latest sync to have started.
pub async fn source_synced_execute(
    db: &impl GenericClient,
    id: Uuid,
    generation: i64,
    commit: Option<&str>,
    error: Option<&str>,
) -> DbResult<u64> {
    Ok(q::source_synced::run()
        .bind(db, &error, &commit, &id, &generation)
        .await?)
}
pub async fn source_delete_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::source_delete::run().bind(db, &id).await?)
}
pub async fn source_agents_all(db: &impl GenericClient, id: Uuid) -> DbResult<Vec<Uuid>> {
    Ok(q::source_agents::run()
        .bind(db, &id)
        .all()
        .await?
        .into_iter()
        .map(|r| r.agent_id)
        .collect())
}
/// (source, agent) pairs for the agents given each whole group.
pub async fn group_agents_all(
    db: &impl GenericClient,
    sources: &[Uuid],
) -> DbResult<Vec<(Uuid, Uuid)>> {
    Ok(q::group_agents::run()
        .bind(db, &sources)
        .all()
        .await?
        .into_iter()
        .map(|r| (r.source_id, r.agent_id))
        .collect())
}
/// (skill, agent) pairs for the agents given each skill.
pub async fn skill_agents_all(
    db: &impl GenericClient,
    skills: &[Uuid],
) -> DbResult<Vec<(Uuid, Uuid)>> {
    Ok(q::skill_agents::run()
        .bind(db, &skills)
        .all()
        .await?
        .into_iter()
        .map(|r| (r.skill_id, r.agent_id))
        .collect())
}
pub async fn skill_insert_opt(
    db: &impl GenericClient,
    id: Uuid,
    source: Uuid,
    name: &str,
    source_path: &str,
) -> DbResult<Option<Uuid>> {
    Ok(q::skill_insert::run()
        .bind(db, &id, &source, &name, &source_path)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn skill_by_name_opt(
    db: &impl GenericClient,
    source: Uuid,
    name: &str,
) -> DbResult<Option<Uuid>> {
    Ok(q::skill_by_name::run()
        .bind(db, &source, &name)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn source_skills_all(
    db: &impl GenericClient,
    source: Uuid,
) -> DbResult<Vec<SourceSkillRow>> {
    Ok(q::source_skills::run().bind(db, &source).all().await?)
}
pub async fn skills_absent_delete_execute(
    db: &impl GenericClient,
    source: Uuid,
    names: &[&str],
) -> DbResult<u64> {
    Ok(q::skills_absent_delete::run()
        .bind(db, &source, &names)
        .await?)
}
pub async fn skill_get_opt(db: &impl GenericClient, id: Uuid) -> DbResult<Option<SkillRow>> {
    Ok(q::skill_get::run()
        .bind(db, &id)
        .opt()
        .await?
        .map(Into::into))
}
pub async fn skills_list_all(
    db: &impl GenericClient,
    source: Option<Uuid>,
) -> DbResult<Vec<SkillRow>> {
    Ok(all(q::skills_list::run().bind(db, &source).all().await?))
}
pub async fn skills_all(db: &impl GenericClient) -> DbResult<Vec<SkillRow>> {
    Ok(all(q::skills_all::run().bind(db).all().await?))
}
pub async fn agent_single_skills_all(
    db: &impl GenericClient,
    agent: Uuid,
) -> DbResult<Vec<SkillRow>> {
    Ok(all(q::agent_single_skills::run()
        .bind(db, &agent)
        .all()
        .await?))
}
pub async fn group_skills_all(
    db: &impl GenericClient,
    sources: &[Uuid],
) -> DbResult<Vec<SkillRow>> {
    Ok(all(q::group_skills::run().bind(db, &sources).all().await?))
}
pub async fn skill_rename_execute(db: &impl GenericClient, id: Uuid, name: &str) -> DbResult<u64> {
    Ok(q::skill_rename::run().bind(db, &name, &id).await?)
}
pub async fn version_describe_execute(
    db: &impl GenericClient,
    id: Uuid,
    description: &str,
) -> DbResult<u64> {
    Ok(q::version_describe::run()
        .bind(db, &description, &id)
        .await?)
}
pub async fn skill_touch_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::skill_touch::run().bind(db, &id).await?)
}
pub async fn skill_delete_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::skill_delete::run().bind(db, &id).await?)
}
pub async fn version_insert_one(
    db: &impl GenericClient,
    id: Uuid,
    skill: Uuid,
    hash: &[u8],
    description: &str,
    message: &str,
    commit: Option<&str>,
) -> DbResult<Uuid> {
    Ok(q::version_insert::run()
        .bind(db, &id, &skill, &hash, &description, &message, &commit)
        .one()
        .await?
        .id)
}
pub async fn version_latest_opt(
    db: &impl GenericClient,
    skill: Uuid,
) -> DbResult<Option<VersionRow>> {
    Ok(q::version_latest::run()
        .bind(db, &skill)
        .opt()
        .await?
        .map(Into::into))
}
pub async fn version_get_opt(db: &impl GenericClient, id: Uuid) -> DbResult<Option<VersionRow>> {
    Ok(q::version_get::run()
        .bind(db, &id)
        .opt()
        .await?
        .map(Into::into))
}
pub async fn versions_list_all(db: &impl GenericClient, skill: Uuid) -> DbResult<Vec<VersionRow>> {
    Ok(all(q::versions_list::run().bind(db, &skill).all().await?))
}
pub async fn versions_by_ids_all(
    db: &impl GenericClient,
    ids: &[Uuid],
) -> DbResult<Vec<VersionRow>> {
    Ok(all(q::versions_by_ids::run().bind(db, &ids).all().await?))
}
/// One array insert per version. See the statement for how absent values travel.
pub async fn files_insert_execute(
    db: &impl GenericClient,
    version: Uuid,
    files: &[super::package::File],
) -> DbResult<u64> {
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    let media: Vec<&str> = files.iter().map(|f| f.media_type.as_str()).collect();
    let sizes: Vec<i64> = files.iter().map(|f| f.size as i64).collect();
    let digests: Vec<&[u8]> = files.iter().map(|f| f.digest.as_slice()).collect();
    let executables: Vec<bool> = files.iter().map(|f| f.executable).collect();
    let inline: Vec<bool> = files.iter().map(|f| f.content.is_some()).collect();
    let contents: Vec<&str> = files
        .iter()
        .map(|f| f.content.as_deref().unwrap_or(""))
        .collect();
    let keys: Vec<String> = files
        .iter()
        .map(|f| {
            if f.needs_object() {
                super::package::object_key(&f.digest)
            } else {
                String::new()
            }
        })
        .collect();
    Ok(q::files_insert::run()
        .bind(
            db,
            &version,
            &paths,
            &media,
            &sizes,
            &digests,
            &executables,
            &inline,
            &contents,
            &keys,
        )
        .await?)
}
pub async fn files_list_all(db: &impl GenericClient, versions: &[Uuid]) -> DbResult<Vec<FileRow>> {
    Ok(all(q::files_list::run().bind(db, &versions).all().await?))
}
pub async fn file_get_opt(
    db: &impl GenericClient,
    version: Uuid,
    path: &str,
) -> DbResult<Option<FileRow>> {
    Ok(q::file_get::run()
        .bind(db, &version, &path)
        .opt()
        .await?
        .map(Into::into))
}
pub async fn assign_source_insert_execute(
    db: &impl GenericClient,
    agent: Uuid,
    source: Uuid,
) -> DbResult<u64> {
    Ok(q::assign_source_insert::run()
        .bind(db, &agent, &source)
        .await?)
}
pub async fn assign_source_delete_execute(
    db: &impl GenericClient,
    agent: Uuid,
    source: Uuid,
) -> DbResult<u64> {
    Ok(q::assign_source_delete::run()
        .bind(db, &agent, &source)
        .await?)
}
/// How many groups were switched: zero when the agent does not have the group.
pub async fn assign_source_enable_one(
    db: &impl GenericClient,
    agent: Uuid,
    source: Uuid,
    enabled: bool,
) -> DbResult<i32> {
    Ok(q::assign_source_enable::run()
        .bind(db, &enabled, &agent, &source)
        .one()
        .await?
        .switched)
}
pub async fn skill_exclude_execute(
    db: &impl GenericClient,
    agent: Uuid,
    skill: Uuid,
    excluded: bool,
) -> DbResult<u64> {
    Ok(q::skill_exclude::run()
        .bind(db, &agent, &skill, &excluded)
        .await?)
}
pub async fn agent_exclusions_all(db: &impl GenericClient, agent: Uuid) -> DbResult<Vec<Uuid>> {
    Ok(q::agent_exclusions::run()
        .bind(db, &agent)
        .all()
        .await?
        .into_iter()
        .map(|r| r.skill_id)
        .collect())
}
pub async fn agent_disabled_sources_all(
    db: &impl GenericClient,
    agent: Uuid,
) -> DbResult<Vec<Uuid>> {
    Ok(q::agent_disabled_sources::run()
        .bind(db, &agent)
        .all()
        .await?
        .into_iter()
        .map(|r| r.source_id)
        .collect())
}
pub async fn assign_skill_insert_execute(
    db: &impl GenericClient,
    agent: Uuid,
    skill: Uuid,
) -> DbResult<u64> {
    Ok(q::assign_skill_insert::run()
        .bind(db, &agent, &skill)
        .await?)
}
pub async fn assign_skill_delete_execute(
    db: &impl GenericClient,
    agent: Uuid,
    skill: Uuid,
) -> DbResult<u64> {
    Ok(q::assign_skill_delete::run()
        .bind(db, &agent, &skill)
        .await?)
}
pub async fn runtime_skills_all(db: &impl GenericClient, agent: Uuid) -> DbResult<Vec<RuntimeRow>> {
    Ok(q::runtime_skills::run().bind(db, &agent).all().await?)
}
pub async fn connection_sources_all(
    db: &impl GenericClient,
    connection: Uuid,
) -> DbResult<Vec<SourceRow>> {
    Ok(all(q::connection_sources::run()
        .bind(db, &connection)
        .all()
        .await?))
}
pub async fn agent_connections_all(
    db: &impl GenericClient,
    agent: Uuid,
) -> DbResult<Vec<AgentConnectionRow>> {
    Ok(q::agent_connections::run().bind(db, &agent).all().await?)
}
pub async fn source_connections_all(db: &impl GenericClient, source: Uuid) -> DbResult<Vec<Uuid>> {
    Ok(q::source_connections::run()
        .bind(db, &source)
        .all()
        .await?
        .into_iter()
        .map(|r| r.connection_id)
        .collect())
}
pub async fn link_source_insert_execute(
    db: &impl GenericClient,
    connection: Uuid,
    source: Uuid,
) -> DbResult<u64> {
    Ok(q::link_source_insert::run()
        .bind(db, &connection, &source)
        .await?)
}
pub async fn link_source_delete_execute(
    db: &impl GenericClient,
    connection: Uuid,
    source: Uuid,
) -> DbResult<u64> {
    Ok(q::link_source_delete::run()
        .bind(db, &connection, &source)
        .await?)
}
pub async fn bundled_source_opt(db: &impl GenericClient, agent: Uuid) -> DbResult<Option<Uuid>> {
    Ok(q::bundled_source::run()
        .bind(db, &agent)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn version_by_hash_opt(
    db: &impl GenericClient,
    skill: Uuid,
    hash: &[u8],
) -> DbResult<Option<Uuid>> {
    Ok(q::version_by_hash::run()
        .bind(db, &skill, &hash)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn deployment_link_execute(
    db: &impl GenericClient,
    deployment: Uuid,
    version: Uuid,
    origin: &str,
) -> DbResult<u64> {
    Ok(q::deployment_link::run()
        .bind(db, &deployment, &version, &origin)
        .await?)
}
pub async fn deployment_skills_all(
    db: &impl GenericClient,
    deployment: Uuid,
) -> DbResult<Vec<DeploymentSkillRow>> {
    Ok(q::deployment_skills::run()
        .bind(db, &deployment)
        .all()
        .await?)
}
pub async fn deployment_for_opt(
    db: &impl GenericClient,
    invocation: Uuid,
) -> DbResult<Option<Uuid>> {
    Ok(q::deployment_for::run()
        .bind(db, &invocation)
        .opt()
        .await?
        .and_then(|r| r.deployment_id))
}
/// Rows shaped like [`runtime_skills_all`].
pub async fn deployment_runtime_skills_all(
    db: &impl GenericClient,
    deployment: Uuid,
) -> DbResult<Vec<RuntimeRow>> {
    Ok(q::deployment_runtime_skills::run()
        .bind(db, &deployment)
        .all()
        .await?
        .into_iter()
        .map(|r| RuntimeRow {
            id: r.id,
            name: r.name,
            source_slug: r.source_slug,
            version_id: r.version_id,
            description: r.description,
        })
        .collect())
}
