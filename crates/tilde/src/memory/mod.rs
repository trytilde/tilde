//! Memory: banks of short facts agents recall and write through the built-in memory tools
//! (`tools`), and people curate through `MemoryService` (`rpc`).
//!
//! A bank is shared, or an agent's own (created with it, linked to it alone and deleted with
//! it). Agents reach banks only through links, read or read-write; in a per-person bank an agent
//! reads and writes only the memories of the verified person it acts for, plus bank-wide ones
//! people wrote. A memory people wrote or edited (origin `owner`) is never changed by an agent.
//! A memory is superseded at most once; the old one stays, hidden from recall even if its
//! successor is later forgotten. Forgetting is a soft delete. Every change is a revision, and a
//! memory an agent writes records the conversation it came from.
//!
//! Recall fuses ranked lists with reciprocal rank fusion: full text (any of the query's words)
//! and, once embedding settings exist, vector similarity among memories embedded under the
//! current settings. A background worker (`run`) embeds memories through the settings' inference
//! connection from a queue that a write, an edit or a settings change fills.
pub mod db;
pub mod embed;
pub mod rpc;
pub mod synthesis;
pub mod tools;

pub use synthesis::Evidence;

use crate::connections::{model::invalid, service::Connections};
use crate::error::Error;
use crate::inference::{Provider, Upstream};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

pub const KINDS: [&str; 8] = [
    "profile",
    "preferences",
    "entities",
    "events",
    "cases",
    "patterns",
    "tools",
    "skills",
];
/// What each kind holds, in `KINDS` order: told to agents and to the synthesis model.
const KIND_HINTS: [&str; 8] = [
    "who someone is",
    "how someone likes things",
    "facts about a thing",
    "something that happened",
    "a problem and its resolution",
    "an approach that works",
    "how to use a tool",
    "a procedure for a recurring task",
];
/// "profile: who someone is. preferences: …" for every kind.
pub fn kind_guide() -> String {
    KINDS
        .iter()
        .zip(KIND_HINTS)
        .map(|(kind, hint)| format!("{kind}: {hint}."))
        .collect::<Vec<_>>()
        .join(" ")
}
/// Candidates each recall list contributes before fusion (the vector session searches at least
/// as many neighbours), and the fusion constant.
const CANDIDATES: i64 = 50;
const RRF_K: f64 = 60.0;
/// Embedding dimensions with an index of their own.
const DIMS: [i32; 2] = [1024, 1536];
const NAME_MAX: usize = 128;
const DESCRIPTION_MAX: usize = 2048;
const TITLE_MAX: usize = 256;
const CONTENT_MAX: usize = 16384;
/// What browse returns at most: child paths, and memories at the path.
const BROWSE_PATHS: i64 = 200;
const BROWSE_MEMORIES: i64 = 50;
/// Paths the bank page's tree shows at most.
const BANK_PATHS: i64 = 2000;
/// Embedding jobs claimed together, and how long a claim is held.
const BATCH: i64 = 32;
const LEASE_SECS: f64 = 300.0;

#[derive(Clone)]
pub struct Memory {
    connections: Connections,
    /// Clients for the connections embeddings and synthesis use, rebuilt when credentials change.
    upstreams: Arc<Mutex<HashMap<Uuid, Arc<Upstream>>>>,
}
/// A new memory.
pub struct Draft {
    pub kind: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    /// Holds from `valid_from` on, with no end yet.
    pub ongoing: bool,
    pub importance: f32,
}
/// A person's edit; absent fields keep their value.
#[derive(Default)]
pub struct Change {
    pub kind: Option<String>,
    pub path: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub ongoing: bool,
    pub clear_validity: bool,
    pub importance: Option<f32>,
}
#[derive(Default)]
pub struct Filter {
    pub kind: Option<String>,
    /// This path and below.
    pub path: Option<String>,
    /// Memories whose validity overlaps [from, to]; either end may be open. Timeless memories
    /// are left out when either is set.
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    /// The bank page narrows by origin and by person.
    pub origin: Option<String>,
    pub identity: Option<Uuid>,
}
/// Whose memories in per-person banks count: everyone's (a person managing the bank), or one
/// person's and bank-wide ones (an agent acting for that person, or for nobody).
#[derive(Clone, Copy)]
pub enum Person {
    Everyone,
    Only(Option<Uuid>),
}
impl Person {
    fn params(self) -> (Option<Uuid>, bool) {
        match self {
            Self::Everyone => (None, true),
            Self::Only(identity) => (identity, false),
        }
    }
}
/// What browse finds; `more_*` when a cap cut the list short.
pub struct Browse {
    pub paths: Vec<(String, i64)>,
    pub memories: Vec<db::MemoryRow>,
    pub more_paths: bool,
    pub more_memories: bool,
}
/// Who writes a memory: a person through management (origin `owner`), an agent through its
/// tools (`explicit`) or Tilde's synthesis (`synthesized`). Only people change people's memories.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Actor {
    Person,
    Agent(Uuid),
    /// Synthesis, in a batch.
    Synthesizer(Uuid),
}
impl Actor {
    pub fn agent(self) -> Option<Uuid> {
        match self {
            Self::Agent(agent) => Some(agent),
            _ => None,
        }
    }
    fn batch(self) -> Option<Uuid> {
        match self {
            Self::Synthesizer(batch) => Some(batch),
            _ => None,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Agent(_) => "agent",
            Self::Synthesizer(_) => "synthesizer",
        }
    }
    fn origin(self) -> &'static str {
        match self {
            Self::Person => "owner",
            Self::Agent(_) => "explicit",
            Self::Synthesizer(_) => "synthesized",
        }
    }
}
/// Where a memory being written comes from: who writes it, the person it is for in a per-person
/// bank and, for an agent, the conversation it writes in.
pub struct Source {
    pub identity: Option<Uuid>,
    pub actor: Actor,
    pub thread: Option<Uuid>,
}

fn text(value: &str, what: &str, max: usize) -> Result<String, Error> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max {
        return Err(invalid(&format!(
            "The {what} must be 1 to {max} characters"
        )));
    }
    Ok(value.to_owned())
}
fn kind(value: &str) -> Result<String, Error> {
    KINDS
        .iter()
        .find(|k| **k == value)
        .map(|k| (*k).to_owned())
        .ok_or_else(|| invalid(&format!("The kind is one of {}", KINDS.join(", "))))
}
/// Slash-separated segments of letters, digits, `-`, `_` and `.`; outer slashes are dropped.
fn path(value: &str) -> Result<String, Error> {
    let value = value.trim().trim_matches('/');
    let valid = value.len() <= 512
        && (value.is_empty()
            || value.split('/').all(|s| {
                !s.is_empty()
                    && s.len() <= 64
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            }));
    if !valid {
        return Err(invalid(
            "A path is slash-separated segments of letters, digits, '-', '_' and '.'",
        ));
    }
    Ok(value.to_owned())
}
fn importance(value: f32) -> Result<f32, Error> {
    if !(0.0..=1.0).contains(&value) {
        return Err(invalid("Importance is between 0 and 1"));
    }
    Ok(value)
}
/// From `from`: a moment without `to`, the period up to `to`, or with no end when `ongoing`.
fn validity(
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    ongoing: bool,
) -> Result<(), Error> {
    match (from, to) {
        (None, _) if ongoing => Err(invalid("An ongoing memory needs a start")),
        (_, Some(_)) if ongoing => Err(invalid("An ongoing memory has no end")),
        (None, Some(_)) => Err(invalid("A memory valid until a time needs a start too")),
        (Some(from), Some(to)) if to < from => {
            Err(invalid("A memory's validity ends after it starts"))
        }
        _ => Ok(()),
    }
}
impl Draft {
    fn validated(self) -> Result<Self, Error> {
        validity(self.valid_from, self.valid_to, self.ongoing)?;
        Ok(Self {
            kind: kind(&self.kind)?,
            path: path(&self.path)?,
            title: text(&self.title, "title", TITLE_MAX)?,
            content: text(&self.content, "content", CONTENT_MAX)?,
            importance: importance(self.importance)?,
            ..self
        })
    }
}
fn scope(value: &str) -> Result<&'static str, Error> {
    match value {
        "shared" => Ok("shared"),
        "per_person" => Ok("per_person"),
        _ => Err(invalid("A bank is shared or per_person")),
    }
}
fn access(value: &str) -> Result<&'static str, Error> {
    match value {
        "read" => Ok("read"),
        "read_write" => Ok("read_write"),
        _ => Err(invalid("Access is read or read_write")),
    }
}
/// Seconds before retrying after `attempts` failures: doubling, up to an hour.
fn backoff(attempts: i32) -> f64 {
    2f64.powi(attempts.clamp(0, 12)).min(3600.0)
}
/// Writes memory `id` in `tx`: the row, its revision (and the replaced memory's), and an
/// embedding request. Callers check the rules first.
async fn insert(
    tx: &crate::database::Transaction<'_>,
    id: Uuid,
    bank: Uuid,
    draft: &Draft,
    source: &Source,
    supersedes: Option<Uuid>,
) -> crate::database::DbResult<()> {
    db::memory_insert_execute(tx, id, bank, draft, source, supersedes).await?;
    db::revision_insert_execute(tx, id, "create", source.actor).await?;
    if let Some(old) = supersedes {
        db::revision_insert_execute(tx, old, "supersede", source.actor).await?;
    }
    db::job_enqueue_execute(tx, Some(id)).await?;
    Ok(())
}
fn owners() -> Error {
    invalid("A person wrote this memory; agents cannot change or forget it")
}
/// Whether `actor` may replace memory `old` in `bank`: never a person's memory unless a person
/// replaces it, and never a memory already replaced.
fn replaceable(old: &db::StatusRow, bank: Uuid, actor: Actor) -> Result<(), Error> {
    if old.deleted || old.bank_id != bank {
        return Err(Error::Missing("Memory"));
    }
    if actor != Actor::Person && old.origin == "owner" {
        return Err(owners());
    }
    if old.superseded {
        return Err(superseded());
    }
    Ok(())
}
fn superseded() -> Error {
    invalid("This memory was already replaced; recall the current one and change that instead")
}

impl Memory {
    pub fn new(connections: Connections) -> Self {
        Self {
            connections,
            upstreams: Arc::default(),
        }
    }
    async fn client(&self) -> Result<crate::database::Client, Error> {
        Ok(self.connections.pool.get().await?)
    }
    /// Deleted agents keep their row; they get no banks or links.
    async fn require_live(&self, agent: Uuid) -> Result<(), Error> {
        if !db::agent_live_one(&self.client().await?, agent).await? {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub async fn banks(&self, search: Option<&str>) -> Result<Vec<db::BankRow>, Error> {
        Ok(db::bank_list_all(&self.client().await?, None, search).await?)
    }
    pub async fn bank(&self, id: Uuid) -> Result<db::BankRow, Error> {
        db::bank_list_all(&self.client().await?, Some(id), None)
            .await?
            .into_iter()
            .next()
            .ok_or(Error::Missing("Memory bank"))
    }
    /// With an agent, the bank is the agent's own and linked to it read-write.
    pub async fn create_bank(
        &self,
        name: &str,
        description: &str,
        scope_name: &str,
        agent: Option<Uuid>,
    ) -> Result<db::BankRow, Error> {
        let name = text(name, "name", NAME_MAX)?;
        if description.chars().count() > DESCRIPTION_MAX {
            return Err(invalid(&format!(
                "The description is at most {DESCRIPTION_MAX} characters"
            )));
        }
        if let Some(agent) = agent {
            self.require_live(agent).await?;
        }
        let id = Uuid::new_v4();
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        let inserted = match db::bank_insert_execute(
            &tx,
            id,
            &name,
            description.trim(),
            agent,
            scope(scope_name)?,
        )
        .await
        {
            Ok(inserted) => inserted,
            Err(e) if e.is_unique_violation() => {
                return Err(Error::Exists("This agent already has its own bank".into()));
            }
            Err(e) => return Err(e.into()),
        };
        if inserted == 0 {
            return Err(invalid("Another memory bank has this name"));
        }
        if let Some(agent) = agent {
            db::link_set_execute(&tx, agent, id, "read_write").await?;
        }
        tx.commit().await?;
        drop(client);
        self.bank(id).await
    }
    pub async fn update_bank(
        &self,
        id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<db::BankRow, Error> {
        let name = name.map(|n| text(n, "name", NAME_MAX)).transpose()?;
        if description.is_some_and(|d| d.chars().count() > DESCRIPTION_MAX) {
            return Err(invalid(&format!(
                "The description is at most {DESCRIPTION_MAX} characters"
            )));
        }
        let updated = db::bank_update_execute(
            &self.client().await?,
            id,
            name.as_deref(),
            description.map(str::trim),
        )
        .await?;
        let bank = self.bank(id).await?;
        if updated == 0 {
            return Err(invalid("Another memory bank has this name"));
        }
        Ok(bank)
    }
    pub async fn delete_bank(&self, id: Uuid) -> Result<(), Error> {
        if db::bank_delete_execute(&self.client().await?, id).await? == 0 {
            return Err(Error::Missing("Memory bank"));
        }
        Ok(())
    }

    pub async fn agent_banks(&self, agent: Uuid) -> Result<Vec<db::AgentBankRow>, Error> {
        Ok(db::agent_banks_all(&self.client().await?, agent).await?)
    }
    pub async fn set_link(
        &self,
        agent: Uuid,
        bank: Uuid,
        access_name: &str,
    ) -> Result<db::AgentBankRow, Error> {
        let row = self.bank(bank).await?;
        if row.agent_id.is_some_and(|owner| owner != agent) {
            return Err(invalid("This bank is another agent's own"));
        }
        self.require_live(agent).await?;
        db::link_set_execute(&self.client().await?, agent, bank, access(access_name)?).await?;
        self.agent_banks(agent)
            .await?
            .into_iter()
            .find(|l| l.bank_id == bank)
            .ok_or(Error::Missing("Memory bank"))
    }
    pub async fn remove_link(&self, agent: Uuid, bank: Uuid) -> Result<(), Error> {
        if self.bank(bank).await?.agent_id == Some(agent) {
            return Err(invalid(
                "This is the agent's own bank; delete the bank instead",
            ));
        }
        db::link_delete_execute(&self.client().await?, agent, bank).await?;
        Ok(())
    }

    /// Live memories by id, in the order given.
    async fn load(&self, ids: &[Uuid]) -> Result<Vec<db::MemoryRow>, Error> {
        let client = self.client().await?;
        let version = db::settings_get_opt(&client).await?.map(|s| s.version);
        let mut rows: HashMap<Uuid, db::MemoryRow> = db::memories_get_all(&client, ids, version)
            .await?
            .into_iter()
            .map(|m| (m.id, m))
            .collect();
        Ok(ids.iter().filter_map(|id| rows.remove(id)).collect())
    }
    pub async fn memory(&self, id: Uuid) -> Result<db::MemoryRow, Error> {
        self.load(&[id])
            .await?
            .pop()
            .ok_or(Error::Missing("Memory"))
    }
    /// A memory forgotten ones included, for checking a change before it is made.
    pub async fn status(&self, id: Uuid) -> Result<db::StatusRow, Error> {
        db::memory_status_opt(&self.client().await?, id)
            .await?
            .ok_or(Error::Missing("Memory"))
    }
    pub async fn revisions(&self, id: Uuid) -> Result<Vec<db::RevisionRow>, Error> {
        Ok(db::revisions_list_all(&self.client().await?, id).await?)
    }

    /// Writes memory `id`, superseding `supersedes` when given, with its revision and embedding
    /// request in one transaction. An id already written is a retry and changes nothing; tools
    /// pass their call id. An agent cannot supersede a person's memory, and no memory is
    /// superseded twice.
    pub async fn create(
        &self,
        id: Uuid,
        bank: Uuid,
        draft: Draft,
        source: Source,
        supersedes: Option<Uuid>,
    ) -> Result<Uuid, Error> {
        let draft = draft.validated()?;
        // An existing id is a retry only when it is the same write.
        let retry = async || -> Result<Option<Uuid>, Error> {
            Ok(
                match db::memory_status_opt(&self.client().await?, id).await? {
                    None => None,
                    Some(m)
                        if m.bank_id == bank
                            && m.written_by_agent_id == source.actor.agent()
                            && m.supersedes_id == supersedes =>
                    {
                        Some(id)
                    }
                    Some(_) => return Err(invalid("This id belongs to another memory")),
                },
            )
        };
        if let Some(id) = retry().await? {
            return Ok(id);
        }
        if source.identity.is_some() && self.bank(bank).await?.scope != "per_person" {
            return Err(invalid("Only per-person banks keep memories for a person"));
        }
        if let Some(old) = supersedes {
            replaceable(&self.status(old).await?, bank, source.actor)?;
        }
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        match insert(&tx, id, bank, &draft, &source, supersedes).await {
            // A concurrent retry, or a concurrent superseding of the same memory.
            Err(error) if error.is_unique_violation() => {
                drop(tx);
                drop(client);
                return retry().await?.ok_or_else(superseded);
            }
            result => result?,
        };
        tx.commit().await?;
        Ok(id)
    }
    /// A person's edit, which makes the memory theirs.
    pub async fn update(&self, id: Uuid, change: Change) -> Result<db::MemoryRow, Error> {
        let change = Change {
            kind: change.kind.as_deref().map(kind).transpose()?,
            path: change.path.as_deref().map(path).transpose()?,
            title: change
                .title
                .as_deref()
                .map(|t| text(t, "title", TITLE_MAX))
                .transpose()?,
            content: change
                .content
                .as_deref()
                .map(|c| text(c, "content", CONTENT_MAX))
                .transpose()?,
            importance: change.importance.map(importance).transpose()?,
            ..change
        };
        validity(change.valid_from, change.valid_to, change.ongoing)?;
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        if db::memory_update_execute(&tx, id, &change).await? == 0 {
            return Err(Error::Missing("Memory"));
        }
        db::revision_insert_execute(&tx, id, "update", Actor::Person).await?;
        db::job_enqueue_execute(&tx, Some(id)).await?;
        tx.commit().await?;
        drop(client);
        self.memory(id).await
    }
    /// Forgets a memory. For an agent: never a person's memory, never one already replaced, and
    /// forgetting what it already forgot is a retry that succeeds.
    pub async fn forget(&self, id: Uuid, agent: Option<Uuid>) -> Result<(), Error> {
        let status = self.status(id).await?;
        if status.deleted {
            return match agent {
                Some(agent) if status.forgotten_by == Some(agent) => Ok(()),
                _ => Err(Error::Missing("Memory")),
            };
        }
        if agent.is_some() && status.origin == "owner" {
            return Err(owners());
        }
        if agent.is_some() && status.superseded {
            return Err(superseded());
        }
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        db::revision_insert_execute(&tx, id, "forget", agent.map_or(Actor::Person, Actor::Agent))
            .await?;
        if db::memory_forget_execute(&tx, id).await? == 0 {
            return Err(Error::Missing("Memory"));
        }
        tx.commit().await?;
        Ok(())
    }

    /// Memories in `banks` best matching `query`, at most `limit`, each with how it was found
    /// (`keywords`, `meaning`); without a query, the newest, found by neither.
    pub async fn recall(
        &self,
        banks: &[Uuid],
        query: Option<&str>,
        filter: &Filter,
        person: Person,
        limit: i64,
    ) -> Result<Vec<(db::MemoryRow, Vec<&'static str>)>, Error> {
        if let (Some(from), Some(to)) = (filter.from, filter.to)
            && from > to
        {
            return Err(invalid("A period ends after it starts"));
        }
        let Some(query) = query.map(str::trim).filter(|q| !q.is_empty()) else {
            return Ok(self
                .list(banks, filter, person, None, limit)
                .await?
                .into_iter()
                .map(|m| (m, vec![]))
                .collect());
        };
        let client = self.client().await?;
        let mut lists = vec![(
            "keywords",
            db::recall_text_all(&client, banks, query, filter, person, CANDIDATES).await?,
        )];
        let settings = db::settings_get_opt(&client).await?;
        // The pooled connection is not held across the provider call.
        drop(client);
        if let Some(settings) = settings {
            // Recall still answers from full text when the embedding provider fails.
            match self.query_vector(&settings, query).await {
                Ok(vector) => {
                    let mut client = self.client().await?;
                    let tx = client.transaction().await?;
                    db::vector_session_one(&tx).await?;
                    let nearest = db::recall_vector_all(
                        &tx,
                        banks,
                        &vector,
                        settings.dims,
                        settings.version,
                        filter,
                        person,
                        CANDIDATES,
                    )
                    .await?;
                    tx.commit().await?;
                    lists.push(("meaning", nearest));
                }
                Err(error) => tracing::warn!(%error, "Memory recall without embeddings"),
            }
        }
        let mut found: HashMap<Uuid, (f64, Vec<&'static str>)> = HashMap::new();
        for (how, list) in &lists {
            for (rank, id) in list.iter().enumerate() {
                let entry = found.entry(*id).or_default();
                entry.0 += 1.0 / (RRF_K + rank as f64 + 1.0);
                entry.1.push(how);
            }
        }
        let ids: Vec<Uuid> = found.keys().copied().collect();
        let mut recalled = self.load(&ids).await?;
        // Relevance first; importance only orders memories that score the same.
        recalled.sort_by(|a, b| {
            found[&b.id]
                .0
                .total_cmp(&found[&a.id].0)
                .then(b.importance.total_cmp(&a.importance))
        });
        recalled.truncate(limit.max(0) as usize);
        Ok(recalled
            .into_iter()
            .map(|m| {
                let how = found.remove(&m.id).map(|f| f.1).unwrap_or_default();
                (m, how)
            })
            .collect())
    }
    /// Memories in `banks` newest first, after memory `after` (the previous page's last).
    pub async fn list(
        &self,
        banks: &[Uuid],
        filter: &Filter,
        person: Person,
        after: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<db::MemoryRow>, Error> {
        let ids =
            db::memory_list_all(&self.client().await?, banks, filter, person, after, limit).await?;
        self.load(&ids).await
    }
    /// Every path in the bank holding memories, with how many it holds exactly, at most
    /// `BANK_PATHS`.
    pub async fn paths(&self, bank: Uuid) -> Result<Vec<db::PathCountRow>, Error> {
        Ok(db::memory_paths_all(&self.client().await?, bank, BANK_PATHS).await?)
    }
    /// The people a per-person bank keeps memories for.
    pub async fn people(&self, bank: Uuid) -> Result<Vec<db::PersonRow>, Error> {
        Ok(db::memory_people_all(&self.client().await?, bank).await?)
    }
    /// The paths directly below `at` with how many memories each holds (itself and below), and
    /// the newest memories at `at`, each capped (`BROWSE_PATHS`, `BROWSE_MEMORIES`).
    pub async fn browse(&self, banks: &[Uuid], at: &str, person: Person) -> Result<Browse, Error> {
        let at = path(at)?;
        let client = self.client().await?;
        let mut paths: Vec<(String, i64)> =
            db::browse_all(&client, banks, &at, person, BROWSE_PATHS + 1)
                .await?
                .into_iter()
                .map(|r| (r.path, r.count))
                .collect();
        let mut ids = db::at_path_all(&client, banks, &at, person, BROWSE_MEMORIES + 1).await?;
        drop(client);
        let more_paths = paths.len() as i64 > BROWSE_PATHS;
        let more_memories = ids.len() as i64 > BROWSE_MEMORIES;
        paths.truncate(BROWSE_PATHS as usize);
        ids.truncate(BROWSE_MEMORIES as usize);
        Ok(Browse {
            paths,
            memories: self.load(&ids).await?,
            more_paths,
            more_memories,
        })
    }

    pub async fn settings(&self) -> Result<Option<db::SettingsRow>, Error> {
        Ok(db::settings_get_opt(&self.client().await?).await?)
    }
    /// Embeds a probe text first, so a model that fails or returns another dimension is refused,
    /// then asks for every memory's embedding under the new settings.
    pub async fn set_settings(
        &self,
        connection: Uuid,
        model: &str,
        dims: i32,
    ) -> Result<db::SettingsRow, Error> {
        if !DIMS.contains(&dims) {
            return Err(invalid("Embeddings have 1024 or 1536 dimensions"));
        }
        let model = text(model, "model", TITLE_MAX)?;
        let upstream = self.upstream(connection).await?;
        embed::embed(
            &upstream,
            &model,
            dims as usize,
            &["Tilde memory".to_owned()],
            false,
        )
        .await
        .map_err(|e| e.error)?;
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        db::settings_set_one(&tx, connection, &model, dims).await?;
        db::job_enqueue_execute(&tx, None).await?;
        tx.commit().await?;
        drop(client);
        self.settings()
            .await?
            .ok_or(Error::Missing("Embedding settings"))
    }
    pub async fn clear_settings(&self) -> Result<(), Error> {
        db::settings_clear_execute(&self.client().await?).await?;
        Ok(())
    }
    /// The price sheet's embedding models for the connection's provider, with their native
    /// dimension when it is one memory supports (0 otherwise).
    pub async fn models(&self, connection: Uuid) -> Result<Vec<(String, u32)>, Error> {
        let (provider, _) = self.embedding_connection(connection).await?;
        Ok(crate::inference::prices::embedding_models(provider.id())?
            .into_iter()
            .map(|(model, size)| {
                (
                    model,
                    if DIMS.contains(&(size as i32)) {
                        size
                    } else {
                        0
                    },
                )
            })
            .collect())
    }
    /// The connection's provider and credential version, if it is a ready inference connection
    /// of a provider `supported` accepts; `refused` explains which do.
    async fn inference_connection(
        &self,
        connection: Uuid,
        supported: fn(Provider) -> bool,
        refused: &str,
    ) -> Result<(Provider, i64), Error> {
        let row = self.connections.get(connection).await?;
        let provider = Provider::parse(&row.provider_id)
            .filter(|p| row.inference_capable && supported(*p))
            .ok_or_else(|| invalid(refused))?;
        if row.status != "ready" {
            return Err(invalid("The connection is not ready"));
        }
        Ok((provider, row.credential_version))
    }
    async fn embedding_connection(&self, connection: Uuid) -> Result<(Provider, i64), Error> {
        self.inference_connection(
            connection,
            embed::supported,
            "Embeddings come from an OpenAI, Azure OpenAI, Vercel AI Gateway, Baseten, Voyage or Cohere connection",
        )
        .await
    }
    /// A client for the connection, reused until its credentials change.
    async fn client_for(
        &self,
        connection: Uuid,
        (provider, version): (Provider, i64),
    ) -> Result<Arc<Upstream>, Error> {
        if let Some(cached) = self
            .upstreams
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&connection)
            .filter(|u| u.version == version)
        {
            return Ok(cached.clone());
        }
        let values = self.connections.resolve(connection).await?;
        let upstream = Arc::new(Upstream::new(connection, version, provider, &values)?);
        self.upstreams
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(connection, upstream.clone());
        Ok(upstream)
    }
    async fn upstream(&self, connection: Uuid) -> Result<Arc<Upstream>, Error> {
        let checked = self.embedding_connection(connection).await?;
        self.client_for(connection, checked).await
    }
    async fn query_vector(&self, settings: &db::SettingsRow, query: &str) -> Result<String, Error> {
        let upstream = self.upstream(settings.connection_id).await?;
        let vectors = embed::embed(
            &upstream,
            &settings.model,
            settings.dims as usize,
            &[query.to_owned()],
            true,
        )
        .await
        .map_err(|e| e.error)?;
        Ok(embed::literal(&vectors[0]))
    }

    /// Embeds one batch of due memories; returns how many were attempted. When the provider
    /// rejects a batch, each memory is tried alone so only the one it refuses backs off; when it
    /// is unavailable, the whole batch backs off.
    pub async fn embed_pending(&self) -> Result<usize, Error> {
        let client = self.client().await?;
        let Some(settings) = db::settings_get_opt(&client).await? else {
            return Ok(0);
        };
        let jobs = db::jobs_claim_all(&client, BATCH, LEASE_SECS).await?;
        drop(client);
        if jobs.is_empty() {
            return Ok(0);
        }
        let upstream = match self.upstream(settings.connection_id).await {
            Ok(upstream) => upstream,
            Err(error) => {
                tracing::warn!(%error, "Memory embeddings unavailable");
                self.back_off(&jobs).await?;
                return Ok(jobs.len());
            }
        };
        let embed = async |jobs: &[db::JobRow]| {
            let texts: Vec<String> = jobs
                .iter()
                .map(|j| format!("{}\n{}", j.title, j.content))
                .collect();
            embed::embed(
                &upstream,
                &settings.model,
                settings.dims as usize,
                &texts,
                false,
            )
            .await
        };
        let mut done: Vec<(&db::JobRow, Vec<f32>)> = Vec::new();
        let mut failed: Vec<&db::JobRow> = Vec::new();
        match embed(&jobs).await {
            Ok(vectors) => done.extend(jobs.iter().zip(vectors)),
            Err(e) if e.rejected && jobs.len() > 1 => {
                for job in &jobs {
                    match embed(std::slice::from_ref(job)).await {
                        Ok(mut vectors) => done.push((job, vectors.remove(0))),
                        Err(e) => {
                            tracing::warn!(error=%e.error, memory=%job.memory_id, "Memory embedding failed");
                            failed.push(job);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error=%e.error, "Memory embeddings failed");
                failed.extend(&jobs);
            }
        }
        let client = self.client().await?;
        for (job, vector) in done {
            db::job_done_execute(
                &client,
                job.memory_id,
                job.requested_at,
                &embed::literal(&vector),
                settings.dims,
                settings.version,
            )
            .await?;
        }
        drop(client);
        self.back_off(failed).await?;
        Ok(jobs.len())
    }
    /// Retry failed jobs later, exponentially up to an hour.
    async fn back_off<'a>(
        &self,
        jobs: impl IntoIterator<Item = &'a db::JobRow>,
    ) -> Result<(), Error> {
        let client = self.client().await?;
        for job in jobs {
            db::job_failed_execute(
                &client,
                job.memory_id,
                job.requested_at,
                backoff(job.attempts),
            )
            .await?;
        }
        Ok(())
    }
    /// The embedding and synthesis workers, until shutdown.
    pub async fn run(self, shutdown: tokio::sync::watch::Receiver<bool>) {
        tokio::join!(
            self.clone().embed_worker(shutdown.clone()),
            self.synthesis_worker(shutdown)
        );
    }
    async fn embed_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        loop {
            let wait = match self.embed_pending().await {
                Ok(0) => Duration::from_secs(2),
                Ok(_) => Duration::ZERO,
                Err(error) => {
                    tracing::warn!(%error, "Memory embedding worker failed");
                    Duration::from_secs(10)
                }
            };
            tokio::select! {
                _ = shutdown.changed() => return,
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }
    async fn synthesis_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        loop {
            let wait = match self.synthesize_pending().await {
                Ok(false) => Duration::from_secs(5),
                Ok(true) => Duration::ZERO,
                Err(error) => {
                    tracing::warn!(%error, "Memory synthesis worker failed");
                    Duration::from_secs(10)
                }
            };
            tokio::select! {
                _ = shutdown.changed() => return,
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }
}
