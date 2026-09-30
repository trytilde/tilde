//! Skills: directories with a `SKILL.md` whose front matter says when to use them, plus any
//! files they reference. Skills are grouped into sources, which carry the roles and are what
//! agents are usually given:
//!
//! - catalog: a group compiled into the engine ([`catalog`]), re-synced at startup, or a
//!   managed provider (a trusted GitHub repository narrowed by path scopes), synced like git;
//! - git: a GitHub repository ([`github`]), synced hourly and on request, pinned to a commit;
//! - editor: a collection authored in the UI, or by agents holding `skills.edit` on it;
//! - code: the skills an agent's own deployments ship, declared in its code and registered by
//!   `tilde deploy`. One per agent, without roles (the agent governs it) and never listed,
//!   assigned or linked; each deployment sees the versions it shipped, which shadow a given
//!   skill of the same name.
//!
//! Every distinct set of files is an immutable version. Files are content-addressed objects in
//! the skills bucket; small UTF-8 files are also kept inline. A sync that fails keeps the last
//! good skills. Agents list their skills (names and descriptions) through the runtime
//! `SkillService` and read files on demand: text inline, anything else by short-lived URL.
pub mod catalog;
pub mod db;
pub mod github;
pub mod package;
pub mod rpc;
use crate::proto::tilde::management::v1 as management;
use crate::{agent::avatar::ObjectStore, database::Pool, error::Error};
use futures::{StreamExt, TryStreamExt};
use package::{File, Package};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// How often git sources and managed providers are re-synced.
pub const SYNC_INTERVAL: Duration = Duration::from_secs(3600);
const DOWNLOAD_SECONDS: u64 = 900;
/// How long a managed provider's preview is reused before its repository is read again.
const PREVIEW_TTL: Duration = Duration::from_secs(3600);

#[derive(Clone)]
pub struct Skills {
    pool: Pool,
    store: Option<ObjectStore>,
    github: Option<github::GitHub>,
    previews: Arc<Previews>,
}
/// Previews of managed providers not yet enabled, by catalog id, with when they were read.
type Previews = Mutex<HashMap<&'static str, (Instant, Vec<CatalogSkill>)>>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Catalog,
    Git,
    Editor,
    Bundled,
}
impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Git => "git",
            Self::Editor => "editor",
            Self::Bundled => "bundled",
        }
    }
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "catalog" => Ok(Self::Catalog),
            "git" => Ok(Self::Git),
            "editor" => Ok(Self::Editor),
            "bundled" => Ok(Self::Bundled),
            _ => Err(Error::Invalid("Unknown skill source kind".into())),
        }
    }
}
/// A file as a caller supplies it: new bytes, or the current version's copy kept by path.
pub enum Upload {
    Data {
        path: String,
        data: Vec<u8>,
        executable: bool,
    },
    /// The current version's file at `from` (or `path`), now at `path`: a rename or move
    /// that needs no bytes sent again.
    Keep { path: String, from: Option<String> },
}
#[derive(Debug, Clone)]
pub struct StoredFile {
    pub path: String,
    pub media_type: String,
    pub size: i64,
    pub digest: Vec<u8>,
    pub executable: bool,
    pub content: Option<String>,
    pub download_url: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Version {
    pub row: db::VersionRow,
    pub files: Vec<StoredFile>,
}
#[derive(Debug, Clone)]
pub struct Skill {
    pub row: db::SkillRow,
    pub latest: Option<db::VersionRow>,
}
pub struct Written {
    pub version: Uuid,
    pub number: i32,
    pub created: bool,
}
pub struct RuntimeSkill {
    pub name: String,
    pub source: String,
    pub description: String,
    pub version_id: Uuid,
    /// Path, media type, size in bytes and executable bit of each file.
    pub files: Vec<(String, String, i64, bool)>,
    /// Shipped by the deployment rather than assigned through the registry.
    pub deployed: bool,
}
/// A catalog entry, its enabled source and its skills: a built-in group's from the binary, a
/// managed provider's once synced (or previewed, from [`Skills::catalog_group`]).
pub struct CatalogEntry {
    pub group: &'static catalog::Group,
    pub source: Option<Uuid>,
    pub skills: Vec<CatalogSkill>,
}
#[derive(Debug, Clone)]
pub struct CatalogSkill {
    pub name: String,
    pub description: String,
    /// The skill's directory in its repository, or a built-in's catalog directory.
    pub path: String,
}
/// A repository's skills found at a commit, before any file is fetched.
struct Scan<'a> {
    github: &'a github::GitHub,
    owner: String,
    repo: String,
    commit: String,
    found: Vec<package::Found>,
}
pub enum FileBody {
    Text(String, String),
    Url(String, String),
}

/// `skills.edit` targets and a slug-safe source slug derived from a display name.
fn slugify(name: &str) -> String {
    let slug = package::normalize_name(name).replace('_', "-");
    if slug.is_empty() {
        "skills".into()
    } else {
        slug
    }
}
/// The catalog entry a catalog source was enabled from.
fn catalog_group(source: &db::SourceRow) -> Option<&'static catalog::Origin> {
    source
        .catalog_group
        .as_deref()
        .and_then(catalog::get)
        .map(|g| &g.origin)
}
fn valid_source_name(name: &str) -> Result<&str, Error> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(Error::Invalid("Source names are 1-100 characters".into()));
    }
    Ok(name)
}

impl Skills {
    pub fn new(pool: Pool) -> Self {
        Self {
            pool,
            store: None,
            github: None,
            previews: Arc::default(),
        }
    }
    pub fn with_store(mut self, store: Option<ObjectStore>) -> Self {
        self.store = store;
        self
    }
    pub fn with_github(mut self, github: github::GitHub) -> Self {
        self.github = Some(github);
        self
    }

    // ---- sources ----

    async fn unique_slug(&self, name: &str) -> Result<String, Error> {
        let base = slugify(name);
        let db = self.pool.get().await?;
        for n in 1..100 {
            let slug = if n == 1 {
                base.clone()
            } else {
                let suffix = format!("-{n}");
                format!("{}{suffix}", &base[..base.len().min(63 - suffix.len())])
            };
            if !db::slug_taken(&db, &slug).await? {
                return Ok(slug);
            }
        }
        Err(Error::Invalid(
            "Choose a more distinctive source name".into(),
        ))
    }
    /// `None` when a unique slug or catalog group is already taken.
    async fn create_source(&self, source: db::NewSource<'_>) -> Result<Option<Uuid>, Error> {
        Ok(db::source_insert_opt(&self.pool.get().await?, source).await?)
    }
    /// Any source but an agent's bundled source, which is reached through its agent only.
    pub async fn source(&self, id: Uuid) -> Result<db::SourceRow, Error> {
        db::source_get_opt(&self.pool.get().await?, id)
            .await?
            .filter(|s| s.kind != Kind::Bundled.as_str())
            .ok_or(Error::NotFound)
    }
    pub async fn sources(&self) -> Result<Vec<db::SourceRow>, Error> {
        Ok(db::sources_list_all(&self.pool.get().await?).await?)
    }
    pub async fn source_agents(&self, id: Uuid) -> Result<Vec<Uuid>, Error> {
        Ok(db::source_agents_all(&self.pool.get().await?, id).await?)
    }
    /// The catalog with each entry's enabled source, if the installation has one. The catalog
    /// is public, so a provider's synced skills are listed whoever can view its source.
    pub async fn catalog(&self) -> Result<Vec<CatalogEntry>, Error> {
        let db = self.pool.get().await?;
        let mut entries = Vec::new();
        for group in catalog::GROUPS {
            let source = db::source_by_catalog_opt(&db, group.id).await?;
            entries.push(CatalogEntry {
                group,
                source,
                skills: self.catalog_skills(group, source).await?,
            });
        }
        Ok(entries)
    }
    /// One catalog entry. A managed provider not yet enabled is previewed live from its
    /// repository, scoped as a sync would be; nothing is stored and the result is reused for
    /// [`PREVIEW_TTL`].
    pub async fn catalog_group(&self, id: &str) -> Result<CatalogEntry, Error> {
        let group =
            catalog::get(id).ok_or_else(|| Error::Invalid("Unknown catalog group".into()))?;
        let source = db::source_by_catalog_opt(&self.pool.get().await?, group.id).await?;
        let skills = match (&group.origin, source) {
            (catalog::Origin::Repository(r), None) => self.preview(group.id, r).await?,
            _ => self.catalog_skills(group, source).await?,
        };
        Ok(CatalogEntry {
            group,
            source,
            skills,
        })
    }
    /// A built-in group's skills from the binary; a managed provider's once synced.
    async fn catalog_skills(
        &self,
        group: &catalog::Group,
        source: Option<Uuid>,
    ) -> Result<Vec<CatalogSkill>, Error> {
        Ok(match (&group.origin, source) {
            (catalog::Origin::Builtin(skills), _) => skills
                .iter()
                .filter_map(|files| {
                    let (path, md) = files.first()?;
                    let (name, description) = package::front_matter(md);
                    let dir = path.split('/').next().unwrap_or_default();
                    Some(CatalogSkill {
                        name: name.unwrap_or_else(|| dir.into()),
                        description,
                        path: dir.into(),
                    })
                })
                .collect(),
            (catalog::Origin::Repository(_), Some(id)) => self
                .skills(Some(id))
                .await?
                .into_iter()
                .map(|s| CatalogSkill {
                    description: s.latest.map(|v| v.description).unwrap_or_default(),
                    name: s.row.name,
                    path: s.row.source_path,
                })
                .collect(),
            (catalog::Origin::Repository(_), None) => Vec::new(),
        })
    }
    /// A managed provider's skills as its sync would name them, reading only each SKILL.md.
    async fn preview(
        &self,
        id: &'static str,
        repository: &catalog::Repository,
    ) -> Result<Vec<CatalogSkill>, Error> {
        if let Some((read, skills)) = self.previews.lock().unwrap().get(id)
            && read.elapsed() < PREVIEW_TTL
        {
            return Ok(skills.clone());
        }
        let scan = self
            .git_scan(repository.url, repository.branch, "", |p| {
                repository.allows(p)
            })
            .await?;
        let mut entrypoints = Vec::new();
        for skill in &scan.found {
            let path = skill
                .entries
                .iter()
                .find(|(relative, _)| relative == package::ENTRYPOINT)
                .map(|(_, entry)| entry.path.as_str())
                .unwrap_or_default();
            entrypoints.push(scan.github.file(
                &scan.owner,
                &scan.repo,
                &scan.commit,
                path,
                package::MAX_ENTRYPOINT_BYTES,
            ));
        }
        let files: Vec<Vec<u8>> = futures::stream::iter(entrypoints)
            .buffered(8)
            .try_collect()
            .await?;
        let read: Vec<(Option<String>, String)> = files
            .iter()
            .map(|md| {
                std::str::from_utf8(md)
                    .map(package::front_matter)
                    .unwrap_or_default()
            })
            .collect();
        let names = package::assign_names(
            read.iter()
                .zip(&scan.found)
                .map(|((name, _), skill)| (name.clone().unwrap_or_default(), skill.dir.clone()))
                .collect(),
        );
        let mut skills: Vec<CatalogSkill> = names
            .into_iter()
            .zip(read)
            .zip(scan.found)
            .map(|((name, (_, description)), skill)| CatalogSkill {
                name,
                description,
                path: skill.dir,
            })
            .collect();
        // Listed as the synced skills are, by name.
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        self.previews
            .lock()
            .unwrap()
            .insert(id, (Instant::now(), skills.clone()));
        Ok(skills)
    }
    /// Enable a catalog entry; the second call returns the existing source. A managed
    /// provider's repository stays in the catalog (`repository_url` is for git sources only).
    pub async fn enable_catalog(&self, group: &str) -> Result<(Uuid, bool), Error> {
        let entry =
            catalog::get(group).ok_or_else(|| Error::Invalid("Unknown catalog group".into()))?;
        if let Some(existing) = db::source_by_catalog_opt(&self.pool.get().await?, group).await? {
            return Ok((existing, false));
        }
        // A user source may already hold the entry's id as its slug (a git source named Zoom).
        let slug = self.unique_slug(entry.id).await?;
        let created = self
            .create_source(db::NewSource {
                id: Uuid::new_v4(),
                slug: &slug,
                name: entry.name,
                kind: Kind::Catalog.as_str(),
                catalog_group: Some(entry.id),
                repository_url: None,
                git_ref: None,
                git_path: "",
                agent_id: None,
            })
            .await?;
        let Some(id) = created else {
            // A concurrent request enabled the group first; theirs is the source.
            return db::source_by_catalog_opt(&self.pool.get().await?, group)
                .await?
                .map(|existing| (existing, false))
                .ok_or_else(|| Error::Invalid("That skill source already exists".into()));
        };
        self.sync(id).await?;
        Ok((id, true))
    }
    /// Add a GitHub repository and sync it once. A failing first sync is recorded on the
    /// source rather than refused, so the caller can fix access and retry.
    pub async fn add_git(
        &self,
        name: &str,
        repository_url: &str,
        git_ref: &str,
        git_path: &str,
    ) -> Result<Uuid, Error> {
        let name = valid_source_name(name)?;
        github::repository(repository_url)?;
        let git_ref = if git_ref.trim().is_empty() {
            "main"
        } else {
            git_ref.trim()
        };
        if !github::valid_ref(git_ref) {
            return Err(Error::Invalid("Invalid branch or tag".into()));
        }
        let git_path = git_path.trim().trim_matches('/');
        if !git_path.is_empty() && !package::valid_path(git_path) {
            return Err(Error::Invalid("Invalid repository path".into()));
        }
        let slug = self.unique_slug(name).await?;
        let id = self
            .create_source(db::NewSource {
                id: Uuid::new_v4(),
                slug: &slug,
                name,
                kind: Kind::Git.as_str(),
                catalog_group: None,
                repository_url: Some(repository_url.trim()),
                git_ref: Some(git_ref),
                git_path,
                agent_id: None,
            })
            .await?
            .ok_or_else(taken)?;
        self.sync(id).await?;
        Ok(id)
    }
    pub async fn create_editor(&self, name: &str) -> Result<Uuid, Error> {
        let name = valid_source_name(name)?;
        let slug = self.unique_slug(name).await?;
        self.create_source(db::NewSource {
            id: Uuid::new_v4(),
            slug: &slug,
            name,
            kind: Kind::Editor.as_str(),
            catalog_group: None,
            repository_url: None,
            git_ref: None,
            git_path: "",
            agent_id: None,
        })
        .await?
        .ok_or_else(taken)
    }
    /// Remove the source, its skills and their assignments.
    pub async fn delete_source(&self, id: Uuid) -> Result<(), Error> {
        self.source(id).await?;
        if db::source_delete_execute(&self.pool.get().await?, id).await? == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    // ---- sync ----

    /// Bring a catalog or git source up to date. Failures are recorded on the source and keep
    /// its current skills; an editor source has nothing to sync. Each sync claims a generation
    /// before fetching, so one that finishes after a newer sync started changes nothing.
    pub async fn sync(&self, id: Uuid) -> Result<(), Error> {
        let source = self.source(id).await?;
        let kind = Kind::parse(&source.kind)?;
        if matches!(kind, Kind::Editor | Kind::Bundled) {
            return Ok(());
        }
        let generation = db::source_sync_start_one(&self.pool.get().await?, id).await?;
        // Fetching, validating and storing can all fail on content; each is recorded alike.
        let result = async {
            let (packages, commit) = match (kind, catalog_group(&source)) {
                (Kind::Git, _) => {
                    let (packages, commit) = self
                        .git_packages(
                            source.repository_url.as_deref().unwrap_or_default(),
                            source.git_ref.as_deref().unwrap_or("main"),
                            &source.git_path,
                            |_| true,
                        )
                        .await?;
                    (packages, Some(commit))
                }
                (_, Some(catalog::Origin::Repository(r))) => {
                    let (packages, commit) = self
                        .git_packages(r.url, r.branch, "", |p| r.allows(p))
                        .await?;
                    (packages, Some(commit))
                }
                (_, Some(catalog::Origin::Builtin(skills))) => {
                    (Self::catalog_packages(skills)?, None)
                }
                (_, None) => {
                    return Err(Error::Invalid("This catalog group no longer exists".into()));
                }
            };
            self.apply(id, generation, packages, commit.as_deref())
                .await
        }
        .await;
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                let message = error.to_string();
                db::source_synced_execute(
                    &self.pool.get().await?,
                    id,
                    generation,
                    None,
                    Some(&message),
                )
                .await?;
                tracing::warn!(source = %id, %message, "Skill source sync failed");
                Ok(())
            }
        }
    }
    fn catalog_packages(skills: &[&[(&'static str, &'static str)]]) -> Result<Vec<Package>, Error> {
        let mut found = Vec::new();
        for files in skills {
            let dir = files[0].0.split('/').next().unwrap_or_default().to_owned();
            let files: Vec<File> = files
                .iter()
                .map(|(path, content)| {
                    File::new(
                        path.strip_prefix(&format!("{dir}/"))
                            .unwrap_or(path)
                            .to_owned(),
                        content.as_bytes().to_vec(),
                        false,
                    )
                })
                .collect();
            found.push((dir, files));
        }
        Self::name_packages(found)
    }
    /// Name each found skill from its front matter and build validated packages.
    fn name_packages(found: Vec<(String, Vec<File>)>) -> Result<Vec<Package>, Error> {
        let declared: Vec<(String, String)> = found
            .iter()
            .map(|(dir, files)| {
                let name = files
                    .iter()
                    .find(|f| f.path == package::ENTRYPOINT)
                    .and_then(|f| f.text())
                    .and_then(|md| package::front_matter(&md).0)
                    .unwrap_or_default();
                (name, dir.clone())
            })
            .collect();
        package::assign_names(declared)
            .into_iter()
            .zip(found)
            .map(|(name, (dir, files))| Package::new(name, dir, files))
            .collect()
    }
    /// A repository at `git_ref`: every SKILL.md under `root`, over the tree's files that
    /// `allows` keeps (a managed provider's scopes; everything for git sources).
    async fn git_scan(
        &self,
        url: &str,
        git_ref: &str,
        root: &str,
        allows: impl Fn(&str) -> bool,
    ) -> Result<Scan<'_>, Error> {
        let github = self
            .github
            .as_ref()
            .ok_or_else(|| Error::Invalid("Git skill sources are not configured".into()))?;
        let (owner, repo) = github::repository(url)?;
        let commit = github.commit(&owner, &repo, git_ref).await?;
        let mut tree = github.tree(&owner, &repo, &commit).await?;
        // Out-of-scope files are dropped before discovery, so an excluded skill neither appears
        // nor lends its files to an enclosing one.
        tree.retain(|e| allows(&e.path));
        let found = package::discover(&tree, root)?;
        Ok(Scan {
            github,
            owner,
            repo,
            commit,
            found,
        })
    }
    /// The skills of a repository at `git_ref`, fetched in full (see [`Self::git_scan`]).
    async fn git_packages(
        &self,
        url: &str,
        git_ref: &str,
        root: &str,
        allows: impl Fn(&str) -> bool,
    ) -> Result<(Vec<Package>, String), Error> {
        let Scan {
            github,
            owner,
            repo,
            commit,
            found,
        } = self.git_scan(url, git_ref, root, allows).await?;
        if found.is_empty() {
            return Err(Error::Invalid("No SKILL.md found at that path".into()));
        }
        // Identical blobs are fetched once, however many skills include them.
        let mut blobs: HashMap<String, Vec<u8>> = HashMap::new();
        let mut packages = Vec::new();
        for skill in found {
            let total: u64 = skill.entries.iter().map(|(_, e)| e.size).sum();
            if total > package::MAX_TOTAL_BYTES {
                return Err(Error::Invalid(format!(
                    "The skill at {} is larger than 64 MiB",
                    skill.dir
                )));
            }
            let mut files = Vec::new();
            for (path, entry) in skill.entries {
                if entry.size > package::MAX_FILE_BYTES {
                    return Err(Error::Invalid(format!(
                        "{} is larger than 10 MiB",
                        entry.path
                    )));
                }
                if !blobs.contains_key(&entry.id) {
                    let data = github
                        .file(&owner, &repo, &commit, &entry.path, package::MAX_FILE_BYTES)
                        .await?;
                    blobs.insert(entry.id.clone(), data);
                }
                files.push(File::new(path, blobs[&entry.id].clone(), entry.executable));
            }
            packages.push((skill.dir, files));
        }
        Ok((Self::name_packages(packages)?, commit))
    }
    /// Upload what object storage lacks. Content-addressed keys make repeats harmless.
    async fn upload(&self, files: &[File]) -> Result<(), Error> {
        for file in files.iter().filter(|f| f.needs_object()) {
            let Some(data) = &file.data else { continue };
            let store = self.store.as_ref().ok_or_else(|| {
                Error::Invalid(format!(
                    "{} is binary or larger than 256 KiB; configure ENGINE_SKILLS_S3_BUCKET to store it",
                    file.path
                ))
            })?;
            store
                .write(
                    reqwest::Method::PUT,
                    &package::object_key(&file.digest),
                    &file.media_type,
                    data.clone(),
                )
                .await?;
        }
        Ok(())
    }
    /// Make a synced source's skills exactly `packages`: new skills are created, changed ones
    /// get a version, missing ones are removed, all in one transaction after uploads. Skipped
    /// when a sync newer than `generation` has started.
    async fn apply(
        &self,
        source: Uuid,
        generation: i64,
        packages: Vec<Package>,
        commit: Option<&str>,
    ) -> Result<(), Error> {
        let current: HashMap<String, (Option<Vec<u8>>, Option<String>)> =
            db::source_skills_all(&self.pool.get().await?, source)
                .await?
                .into_iter()
                .map(|r| (r.name, (r.hash, r.description)))
                .collect();
        let changed: Vec<&Package> = packages
            .iter()
            .filter(|p| match current.get(&p.name) {
                Some((Some(hash), description)) => {
                    hash != &p.hash || description.as_deref() != Some(p.description.as_str())
                }
                _ => true,
            })
            .collect();
        for package in &changed {
            self.upload(&package.files).await?;
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        if db::source_lock_one(&tx, source).await? != generation {
            return Ok(());
        }
        for package in changed {
            let skill = match db::skill_by_name_opt(&tx, source, &package.name).await? {
                Some(id) => id,
                None => {
                    db::skill_insert_opt(&tx, Uuid::new_v4(), source, &package.name, &package.dir)
                        .await?
                        .ok_or(Error::Conflict)?
                }
            };
            Self::save_version(&tx, skill, package, "Synced", commit).await?;
        }
        let names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
        db::skills_absent_delete_execute(&tx, source, &names).await?;
        db::source_synced_execute(&tx, source, generation, commit, None).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Append a version unless the latest already has these files; then only its description,
    /// derived from SKILL.md, is brought up to date.
    async fn save_version(
        tx: &tokio_postgres::Transaction<'_>,
        skill: Uuid,
        package: &Package,
        message: &str,
        commit: Option<&str>,
    ) -> Result<(Uuid, i32, bool), Error> {
        if let Some(latest) = db::version_latest_opt(tx, skill).await?
            && latest.hash == package.hash
        {
            if latest.description != package.description {
                db::version_describe_execute(tx, latest.id, &package.description).await?;
            }
            return Ok((latest.id, latest.number, false));
        }
        let version = db::version_insert_one(
            tx,
            Uuid::new_v4(),
            skill,
            &package.hash,
            &package.description,
            message,
            commit,
        )
        .await?;
        db::files_insert_execute(tx, version, &package.files).await?;
        db::skill_touch_execute(tx, skill).await?;
        let number = db::version_get_opt(tx, version)
            .await?
            .map(|v| v.number)
            .unwrap_or(1);
        Ok((version, number, true))
    }
    /// Re-sync every built-in catalog source against this binary's catalog. Managed providers
    /// are left to the worker, so startup never waits on GitHub.
    pub async fn reconcile(&self) -> Result<(), Error> {
        for source in db::sources_all(&self.pool.get().await?).await? {
            if source.kind == Kind::Catalog.as_str()
                && !matches!(catalog_group(&source), Some(catalog::Origin::Repository(_)))
            {
                self.sync(source.id).await?;
            }
        }
        Ok(())
    }
    /// Re-sync git sources and managed providers on an interval; each failure is recorded on
    /// its source.
    pub async fn worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(SYNC_INTERVAL);
        tick.tick().await;
        loop {
            tokio::select! {
                _ = tick.tick() => {}
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { break; }
                    continue;
                }
            }
            let sources = match self.pool.get().await {
                Ok(db) => db::sources_all(&db).await.unwrap_or_default(),
                Err(_) => continue,
            };
            let fetched = |s: &db::SourceRow| {
                s.kind == Kind::Git.as_str()
                    || matches!(catalog_group(s), Some(catalog::Origin::Repository(_)))
            };
            for source in sources.into_iter().filter(fetched) {
                if let Err(error) = self.sync(source.id).await {
                    tracing::warn!(source = %source.id, %error, "Skill source sync failed");
                }
            }
        }
    }

    // ---- editor skills ----

    /// Files for a new version: uploads replace, kept paths copy the current version's file.
    async fn files_from(
        &self,
        current: Option<Uuid>,
        uploads: Vec<Upload>,
    ) -> Result<Vec<File>, Error> {
        let previous: HashMap<String, db::FileRow> = match current {
            Some(version) => db::files_list_all(&self.pool.get().await?, &[version])
                .await?
                .into_iter()
                .map(|f| (f.path.clone(), f))
                .collect(),
            None => HashMap::new(),
        };
        uploads
            .into_iter()
            .map(|upload| match upload {
                Upload::Data {
                    path,
                    data,
                    executable,
                } => Ok(File::new(path, data, executable)),
                Upload::Keep { path, from } => {
                    let source = from.as_deref().unwrap_or(&path);
                    let row = previous.get(source).ok_or_else(|| {
                        Error::Invalid(format!("{source} has no content and no earlier version"))
                    })?;
                    Ok(File {
                        media_type: package::media_type(&path).to_owned(),
                        path,
                        size: row.size_bytes as u64,
                        digest: row.sha256.clone(),
                        executable: row.executable,
                        content: row.content.clone(),
                        data: None,
                    })
                }
            })
            .collect()
    }
    async fn editor_source(&self, source: Uuid) -> Result<(), Error> {
        let kind = self.source(source).await?.kind;
        if kind != Kind::Editor.as_str() {
            return Err(Error::Invalid(
                "Only editor sources are edited here; catalog and git sources follow their origin"
                    .into(),
            ));
        }
        Ok(())
    }
    /// Create the skill, or save a new version when it exists; identical files change nothing.
    pub async fn write(
        &self,
        source: Uuid,
        name: &str,
        uploads: Vec<Upload>,
        message: &str,
        create_only: bool,
    ) -> Result<(Uuid, Written), Error> {
        self.save(source, name, uploads, message, create_only, false)
            .await
    }
    /// `rename` makes an existing skill take the `name:` of its new SKILL.md front matter, in the
    /// same transaction as the version, so editing the name in the UI renames the skill.
    async fn save(
        &self,
        source: Uuid,
        name: &str,
        uploads: Vec<Upload>,
        message: &str,
        create_only: bool,
        rename: bool,
    ) -> Result<(Uuid, Written), Error> {
        self.editor_source(source).await?;
        if message.len() > 500 {
            return Err(Error::Invalid("Version message is too long".into()));
        }
        let existing = db::skill_by_name_opt(&self.pool.get().await?, source, name).await?;
        if existing.is_some() && create_only {
            return Err(Error::Invalid(format!(
                "A skill named {name} already exists in this source"
            )));
        }
        let latest = match existing {
            Some(skill) => db::version_latest_opt(&self.pool.get().await?, skill)
                .await?
                .map(|v| v.id),
            None => None,
        };
        let files = self.files_from(latest, uploads).await?;
        let target = match (rename, existing) {
            (true, Some(_)) => files
                .iter()
                .find(|f| f.path == package::ENTRYPOINT)
                .and_then(File::text)
                .and_then(|md| package::front_matter(&md).0)
                .unwrap_or_else(|| name.to_owned()),
            _ => name.to_owned(),
        };
        let package = Package::new(target.clone(), target.clone(), files)?;
        self.upload(&package.files).await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        db::source_lock_one(&tx, source).await?;
        let skill = match db::skill_by_name_opt(&tx, source, name).await? {
            Some(id) => id,
            None => db::skill_insert_opt(&tx, Uuid::new_v4(), source, name, name)
                .await?
                .ok_or(Error::Conflict)?,
        };
        // Kept files were resolved against `latest`; a version saved since would be undone.
        if db::version_latest_opt(&tx, skill).await?.map(|v| v.id) != latest {
            return Err(Error::Stale(
                "This skill changed while you were editing; reload and try again".into(),
            ));
        }
        if target != name && db::skill_rename_execute(&tx, skill, &target).await? == 0 {
            return Err(Error::Invalid(format!(
                "A skill named {target} already exists in this group"
            )));
        }
        let message = match (message.trim(), existing) {
            ("", None) => "Created",
            ("", Some(_)) => "Updated",
            (m, _) => m,
        };
        let (version, number, created) =
            Self::save_version(&tx, skill, &package, message, None).await?;
        tx.commit().await?;
        Ok((
            skill,
            Written {
                version,
                number,
                created,
            },
        ))
    }
    /// Save a new version of an editor skill; a changed front matter `name:` renames it.
    pub async fn update(&self, id: Uuid, uploads: Vec<Upload>) -> Result<(), Error> {
        let skill = self.skill(id).await?;
        self.save(
            skill.row.source_id,
            &skill.row.name,
            uploads,
            "",
            false,
            true,
        )
        .await?;
        Ok(())
    }
    pub async fn delete_skill(&self, id: Uuid) -> Result<(), Error> {
        let skill = self.skill(id).await?;
        self.editor_source(skill.row.source_id).await?;
        db::skill_delete_execute(&self.pool.get().await?, id).await?;
        Ok(())
    }

    // ---- reads ----

    async fn with_latest(&self, rows: Vec<db::SkillRow>) -> Result<Vec<Skill>, Error> {
        let ids: Vec<Uuid> = rows.iter().filter_map(|r| r.latest_id).collect();
        let versions = db::versions_by_ids_all(&self.pool.get().await?, &ids).await?;
        Ok(rows
            .into_iter()
            .map(|row| Skill {
                latest: row
                    .latest_id
                    .and_then(|id| versions.iter().find(|v| v.id == id).cloned()),
                row,
            })
            .collect())
    }
    pub async fn skills(&self, source: Option<Uuid>) -> Result<Vec<Skill>, Error> {
        let rows = db::skills_list_all(&self.pool.get().await?, source).await?;
        self.with_latest(rows).await
    }
    /// Which agents are given each of these whole groups.
    pub async fn group_agents(&self, sources: &[Uuid]) -> Result<HashMap<Uuid, Vec<Uuid>>, Error> {
        let mut agents: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for (source, agent) in db::group_agents_all(&self.pool.get().await?, sources).await? {
            agents.entry(source).or_default().push(agent);
        }
        Ok(agents)
    }
    /// Which agents are given each of these skills.
    pub async fn skill_agents(&self, skills: &[Uuid]) -> Result<HashMap<Uuid, Vec<Uuid>>, Error> {
        let mut agents: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for (skill, agent) in db::skill_agents_all(&self.pool.get().await?, skills).await? {
            agents.entry(skill).or_default().push(agent);
        }
        Ok(agents)
    }
    pub async fn skill(&self, id: Uuid) -> Result<Skill, Error> {
        let row = db::skill_get_opt(&self.pool.get().await?, id)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(self.with_latest(vec![row]).await?.remove(0))
    }
    pub async fn versions(&self, skill: Uuid) -> Result<Vec<db::VersionRow>, Error> {
        Ok(db::versions_list_all(&self.pool.get().await?, skill).await?)
    }
    async fn download_url(&self, key: &str) -> Option<String> {
        self.store
            .as_ref()?
            .signed_url(key, DOWNLOAD_SECONDS)
            .await
            .ok()
    }
    /// One version with its files: text inline, other files by short-lived URL.
    pub async fn version(&self, id: Uuid) -> Result<Version, Error> {
        let db = self.pool.get().await?;
        let row = db::version_get_opt(&db, id).await?.ok_or(Error::NotFound)?;
        let mut files = Vec::new();
        for f in db::files_list_all(&db, &[id]).await? {
            let download_url = match (&f.content, &f.object_key) {
                (None, Some(key)) => self.download_url(key).await,
                _ => None,
            };
            files.push(StoredFile {
                path: f.path,
                media_type: f.media_type,
                size: f.size_bytes,
                digest: f.sha256,
                executable: f.executable,
                content: f.content,
                download_url,
            });
        }
        Ok(Version { row, files })
    }

    // ---- assignments ----

    pub async fn agent_skills(
        &self,
        agent: Uuid,
    ) -> Result<(Vec<db::SourceRow>, Vec<Skill>), Error> {
        let db = self.pool.get().await?;
        let sources = db::agent_sources_all(&db, agent).await?;
        let singles = db::agent_single_skills_all(&db, agent).await?;
        drop(db);
        Ok((sources, self.with_latest(singles).await?))
    }
    /// Every skill in these groups.
    pub async fn group_skills(&self, sources: &[Uuid]) -> Result<Vec<Skill>, Error> {
        let rows = db::group_skills_all(&self.pool.get().await?, sources).await?;
        self.with_latest(rows).await
    }
    /// Bundled skills belong to their deployments and are never given out.
    async fn assignable_skill(&self, skill: Uuid) -> Result<(), Error> {
        match self.skill(skill).await {
            Ok(s) if s.row.source_kind != Kind::Bundled.as_str() => Ok(()),
            _ => Err(Error::NotFound),
        }
    }
    pub async fn assign_source(&self, agent: Uuid, source: Uuid) -> Result<(), Error> {
        self.source(source).await?;
        db::assign_source_insert_execute(&self.pool.get().await?, agent, source)
            .await
            .map_err(missing_reference)?;
        Ok(())
    }
    pub async fn unassign_source(&self, agent: Uuid, source: Uuid) -> Result<(), Error> {
        db::assign_source_delete_execute(&self.pool.get().await?, agent, source).await?;
        Ok(())
    }
    /// Switch an agent's group on (every skill in it, including later ones) or off (none of
    /// them) without removing it.
    pub async fn enable_source(
        &self,
        agent: Uuid,
        source: Uuid,
        enabled: bool,
    ) -> Result<(), Error> {
        match db::assign_source_enable_one(&self.pool.get().await?, agent, source, enabled).await? {
            0 => Err(Error::NotFound),
            _ => Ok(()),
        }
    }
    /// Switch one skill of an agent's enabled group off, or back on.
    pub async fn enable_skill(&self, agent: Uuid, skill: Uuid, enabled: bool) -> Result<(), Error> {
        self.assignable_skill(skill).await?;
        db::skill_exclude_execute(&self.pool.get().await?, agent, skill, !enabled)
            .await
            .map_err(missing_reference)?;
        Ok(())
    }
    /// Skills of the agent's groups that it switched off.
    pub async fn excluded_skills(&self, agent: Uuid) -> Result<Vec<Uuid>, Error> {
        Ok(db::agent_exclusions_all(&self.pool.get().await?, agent).await?)
    }
    pub async fn disabled_sources(&self, agent: Uuid) -> Result<Vec<Uuid>, Error> {
        Ok(db::agent_disabled_sources_all(&self.pool.get().await?, agent).await?)
    }
    pub async fn assign_skill(&self, agent: Uuid, skill: Uuid) -> Result<(), Error> {
        self.assignable_skill(skill).await?;
        db::assign_skill_insert_execute(&self.pool.get().await?, agent, skill)
            .await
            .map_err(missing_reference)?;
        Ok(())
    }
    pub async fn unassign_skill(&self, agent: Uuid, skill: Uuid) -> Result<(), Error> {
        db::assign_skill_delete_execute(&self.pool.get().await?, agent, skill).await?;
        Ok(())
    }

    // ---- connections ----

    /// The sources a connection links.
    pub async fn connection_sources(&self, connection: Uuid) -> Result<Vec<db::SourceRow>, Error> {
        Ok(db::connection_sources_all(&self.pool.get().await?, connection).await?)
    }
    /// Connections whose skills capability the agent holds.
    pub async fn agent_connections(
        &self,
        agent: Uuid,
    ) -> Result<Vec<db::AgentConnectionRow>, Error> {
        Ok(db::agent_connections_all(&self.pool.get().await?, agent).await?)
    }
    pub async fn source_connections(&self, source: Uuid) -> Result<Vec<Uuid>, Error> {
        Ok(db::source_connections_all(&self.pool.get().await?, source).await?)
    }
    pub async fn link_source(&self, connection: Uuid, source: Uuid, on: bool) -> Result<(), Error> {
        if on {
            self.source(source).await?;
        }
        let db = self.pool.get().await?;
        if on {
            db::link_source_insert_execute(&db, connection, source)
                .await
                .map_err(missing_reference)?;
        } else {
            db::link_source_delete_execute(&db, connection, source).await?;
        }
        Ok(())
    }

    // ---- runtime ----

    pub async fn source_by_slug(&self, slug: &str) -> Result<(Uuid, Kind), Error> {
        let (id, kind) = db::source_by_slug_opt(&self.pool.get().await?, slug)
            .await?
            .ok_or(Error::NotFound)?;
        Ok((id, Kind::parse(&kind)?))
    }
    /// `source/name` names one skill anywhere; returns its source and the skill.
    pub async fn skill_by_address(&self, address: &str) -> Result<(Uuid, Uuid), Error> {
        let (source, name) = address
            .split_once('/')
            .ok_or_else(|| Error::Invalid("Name a skill as source/name".into()))?;
        let (source, _) = self.source_by_slug(source).await?;
        let skill = db::skill_by_name_opt(&self.pool.get().await?, source, name)
            .await?
            .ok_or(Error::NotFound)?;
        Ok((source, skill))
    }
    /// Everything an agent is given, with each file's path, media type and size. With its own
    /// invocation the agent also sees the bundled skills that invocation's deployment shipped; code
    /// skills are the agent's implementation, so another agent's listing (no invocation) omits them.
    pub async fn for_agent(
        &self,
        agent: Uuid,
        invocation: Option<Uuid>,
    ) -> Result<Vec<RuntimeSkill>, Error> {
        let db = self.pool.get().await?;
        let mut rows = db::runtime_skills_all(&db, agent).await?;
        let mut deployed = Vec::new();
        if let Some(invocation) = invocation
            && let Some(deployment) = db::deployment_for_opt(&db, invocation).await?
        {
            let code = db::deployment_runtime_skills_all(&db, deployment).await?;
            rows.retain(|r| !code.iter().any(|c| c.name == r.name));
            deployed = code.iter().map(|c| c.name.clone()).collect();
            rows.extend(code);
        }
        let versions: Vec<Uuid> = rows.iter().map(|r| r.version_id).collect();
        let files = db::files_list_all(&db, &versions).await?;
        let mut skills: Vec<RuntimeSkill> = rows
            .into_iter()
            .map(|r| RuntimeSkill {
                files: files
                    .iter()
                    .filter(|f| f.version_id == r.version_id)
                    .map(|f| {
                        (
                            f.path.clone(),
                            f.media_type.clone(),
                            f.size_bytes,
                            f.executable,
                        )
                    })
                    .collect(),
                deployed: deployed.contains(&r.name),
                name: r.name,
                source: r.source_slug,
                description: r.description,
                version_id: r.version_id,
            })
            .collect();
        skills.sort_by(|a, b| (&a.name, &a.source).cmp(&(&b.name, &b.source)));
        Ok(skills)
    }
    /// One file of a skill the agent is given, addressed as `name` or `source/name`.
    pub async fn read(
        &self,
        agent: Uuid,
        invocation: Option<Uuid>,
        address: &str,
        path: &str,
    ) -> Result<FileBody, Error> {
        if !package::valid_path(path) {
            return Err(Error::Invalid("Invalid skill file path".into()));
        }
        let skills = self.for_agent(agent, invocation).await?;
        let matches: Vec<&RuntimeSkill> = match address.split_once('/') {
            Some((source, name)) => skills
                .iter()
                .filter(|s| s.source == source && s.name == name)
                .collect(),
            None => skills.iter().filter(|s| s.name == address).collect(),
        };
        let skill = match matches.as_slice() {
            [one] => *one,
            [] => return Err(Error::NotFound),
            _ => {
                return Err(Error::Invalid(format!(
                    "Several assigned skills are named {address}; use source/name"
                )));
            }
        };
        let file = db::file_get_opt(&self.pool.get().await?, skill.version_id, path)
            .await?
            .ok_or(Error::NotFound)?;
        match (file.content, file.object_key) {
            (Some(text), _) => Ok(FileBody::Text(text, file.media_type)),
            (None, Some(key)) => Ok(FileBody::Url(
                self.download_url(&key)
                    .await
                    .ok_or_else(|| Error::Invalid("Skill file storage is not configured".into()))?,
                file.media_type,
            )),
            (None, None) => Err(Error::NotFound),
        }
    }
    /// Every source with its skills at their latest version, for agents choosing what to assign.
    pub async fn catalogue(&self) -> Result<Vec<(db::SourceRow, Vec<Skill>)>, Error> {
        let db = self.pool.get().await?;
        let sources = db::sources_all(&db).await?;
        let skills = db::skills_all(&db).await?;
        drop(db);
        let skills = self.with_latest(skills).await?;
        Ok(sources
            .into_iter()
            .map(|source| {
                let own = skills
                    .iter()
                    .filter(|s| s.row.source_id == source.id)
                    .cloned()
                    .collect();
                (source, own)
            })
            .collect())
    }
}
impl Skills {
    // ---- bundled skills (deployments) ----

    fn store(&self) -> Result<&ObjectStore, Error> {
        self.store.as_ref().ok_or_else(|| {
            Error::Invalid("Configure ENGINE_SKILLS_S3_BUCKET to upload skill files".into())
        })
    }
    /// Which of these file digests object storage lacks, so `tilde deploy` uploads only those.
    pub async fn missing_files(&self, digests: &[String]) -> Result<Vec<String>, Error> {
        if digests.len() > package::MAX_FILES {
            return Err(Error::Invalid("Too many digests".into()));
        }
        let store = self.store()?;
        let mut missing = Vec::new();
        for digest in digests {
            let bytes = hex::decode(digest)
                .ok()
                .filter(|d| d.len() == 32)
                .ok_or_else(|| Error::Invalid("Invalid SHA-256 digest".into()))?;
            if store.size(&package::object_key(&bytes)).await?.is_none() {
                missing.push(digest.clone());
            }
        }
        Ok(missing)
    }
    /// Store one skill file by its content digest, which is returned as hex.
    pub async fn upload_file(&self, data: Vec<u8>) -> Result<String, Error> {
        if data.len() as u64 > package::MAX_FILE_BYTES {
            return Err(Error::Invalid("Skill files are at most 10 MiB".into()));
        }
        let digest = package::digest(&data);
        self.store()?
            .write(
                reqwest::Method::PUT,
                &package::object_key(&digest),
                "application/octet-stream",
                data,
            )
            .await?;
        Ok(hex::encode(digest))
    }
    /// Validate declared skills into packages and store the files they need in object storage,
    /// before the deployment's transaction. Files named by digest must already be uploaded.
    pub async fn prepare(
        &self,
        declared: Vec<management::DeclaredSkill>,
    ) -> Result<Vec<(Package, String)>, Error> {
        use management::declared_skill_file::Body;
        let mut packages: Vec<(Package, String)> = Vec::new();
        for skill in declared {
            if skill.origin.len() > 512 {
                return Err(Error::Invalid(format!(
                    "Skill {} origin is too long",
                    skill.name
                )));
            }
            let mut files = Vec::new();
            for f in skill.files {
                files.push(match f.body {
                    Some(Body::Content(text)) => File::new(f.path, text.into_bytes(), f.executable),
                    Some(Body::Data(data)) => File::new(f.path, data, f.executable),
                    Some(Body::Sha256(hex)) => {
                        let digest = hex::decode(&hex)
                            .ok()
                            .filter(|d| d.len() == 32)
                            .ok_or_else(|| {
                                Error::Invalid(format!("{} has an invalid digest", f.path))
                            })?;
                        let key = package::object_key(&digest);
                        let size = self.store()?.size(&key).await?.ok_or_else(|| {
                            Error::Invalid(format!("{} was not uploaded before deploying", f.path))
                        })?;
                        // A SKILL.md too large to send inline is read back for its description
                        // (and rewritten under the same content-addressed key, which is harmless).
                        if f.path == package::ENTRYPOINT && size <= package::MAX_ENTRYPOINT_BYTES {
                            let data = self.store()?.read(&key).await?;
                            if package::digest(&data) != digest {
                                return Err(Error::Invalid(format!(
                                    "{} does not match its digest",
                                    f.path
                                )));
                            }
                            files.push(File::new(f.path, data, f.executable));
                            continue;
                        }
                        File {
                            media_type: package::media_type(&f.path).to_owned(),
                            path: f.path,
                            size,
                            digest,
                            executable: f.executable,
                            content: None,
                            data: None,
                        }
                    }
                    None => return Err(Error::Invalid(format!("{} has no content", f.path))),
                });
            }
            let package = Package::new(skill.name.clone(), skill.name, files)?;
            match packages.iter().find(|(p, _)| p.name == package.name) {
                Some((p, _)) if p.hash == package.hash => continue,
                Some(_) => {
                    return Err(Error::Invalid(format!(
                        "Two different skills are named {}",
                        package.name
                    )));
                }
                None => {}
            }
            self.upload(&package.files).await?;
            packages.push((package, skill.origin));
        }
        Ok(packages)
    }
    /// Store a deployment's skills in the agent's bundled source inside the caller's transaction:
    /// a skill's version is reused whenever a deployment ships the same files again.
    pub async fn register_code(
        tx: &tokio_postgres::Transaction<'_>,
        agent: Uuid,
        deployment: Uuid,
        packages: &[(Package, String)],
    ) -> Result<(), Error> {
        if packages.is_empty() {
            return Ok(());
        }
        let source = match db::bundled_source_opt(tx, agent).await? {
            Some(id) => id,
            None => {
                let slug = format!("agent-{}", agent.simple());
                let created = db::source_insert_opt(
                    tx,
                    db::NewSource {
                        id: Uuid::new_v4(),
                        slug: &slug,
                        name: "Agent code",
                        kind: Kind::Bundled.as_str(),
                        catalog_group: None,
                        repository_url: None,
                        git_ref: None,
                        git_path: "",
                        agent_id: Some(agent),
                    },
                )
                .await?;
                match created {
                    Some(id) => id,
                    None => db::bundled_source_opt(tx, agent)
                        .await?
                        .ok_or(Error::Conflict)?,
                }
            }
        };
        db::source_lock_one(tx, source).await?;
        for (package, origin) in packages {
            let skill = match db::skill_by_name_opt(tx, source, &package.name).await? {
                Some(id) => id,
                None => {
                    db::skill_insert_opt(tx, Uuid::new_v4(), source, &package.name, &package.dir)
                        .await?
                        .ok_or(Error::Conflict)?
                }
            };
            let version = match db::version_by_hash_opt(tx, skill, &package.hash).await? {
                Some(id) => id,
                None => {
                    let version = db::version_insert_one(
                        tx,
                        Uuid::new_v4(),
                        skill,
                        &package.hash,
                        &package.description,
                        "Deployed",
                        None,
                    )
                    .await?;
                    db::files_insert_execute(tx, version, &package.files).await?;
                    db::skill_touch_execute(tx, skill).await?;
                    version
                }
            };
            db::deployment_link_execute(tx, deployment, version, origin).await?;
        }
        Ok(())
    }
    pub async fn for_deployment(
        &self,
        deployment: Uuid,
    ) -> Result<Vec<db::DeploymentSkillRow>, Error> {
        Ok(db::deployment_skills_all(&self.pool.get().await?, deployment).await?)
    }
}
/// Slugs are chosen free, so only a concurrent create with the same name takes one.
fn taken() -> Error {
    Error::Invalid("That skill source already exists".into())
}
/// An assignment insert only fails on a foreign key: the agent, source or skill is gone.
fn missing_reference(error: crate::database::DbError) -> Error {
    match error {
        crate::database::DbError::Postgres(ref e)
            if e.as_db_error().is_some_and(|e| {
                e.code() == &tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION
            }) =>
        {
            Error::NotFound
        }
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_entries_are_valid_and_their_icons_exist() {
        let web = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/public");
        for group in catalog::GROUPS {
            assert_eq!(catalog::get(group.id).map(|g| g.name), Some(group.name));
            assert!(
                web.join(&group.icon_url[1..]).is_file(),
                "{}",
                group.icon_url
            );
            match &group.origin {
                catalog::Origin::Builtin(skills) => {
                    for package in Skills::catalog_packages(skills).unwrap() {
                        assert!(
                            !package.description.is_empty(),
                            "{} lacks a description",
                            package.name
                        );
                    }
                }
                catalog::Origin::Repository(r) => {
                    assert!(github::repository(r.url).is_ok() && github::valid_ref(r.branch));
                }
            }
        }
        assert_eq!(slugify("My Team's Skills!"), "my-team-s-skills");
    }
}
