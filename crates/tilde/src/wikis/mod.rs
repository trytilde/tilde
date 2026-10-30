//! Wikis: trees of Markdown pages people write through `WikiService` (`rpc`) and agents read and
//! edit through the built-in wiki tools (`tools`). Memory banks hold short facts an agent recalls;
//! a wiki holds the durable reference pages people read too.
//!
//! A wiki is shared, or an agent's own (created with it, linked to it alone and deleted with it).
//! Agents reach wikis only through links, read or read-write. Every change to a page is a
//! revision. A write names the revision it read; when the page has moved on since, the write is
//! refused as stale and the writer reads it again, so concurrent edits never overwrite each
//! other. Deleting a page removes it and keeps its history. Pages are found by path, by ranked
//! full text over titles and content, and by grep over their lines.
pub mod db;
pub mod rpc;
pub mod tools;

use crate::connections::{model::invalid, service::Connections};
use crate::error::Error;
use uuid::Uuid;

const NAME_MAX: usize = 128;
const DESCRIPTION_MAX: usize = 2048;
pub const PATH_MAX: usize = 512;
pub const TITLE_MAX: usize = 256;
pub const CONTENT_MAX: usize = 262_144;
pub const PATTERN_MAX: usize = 256;
pub const QUERY_MAX: usize = 500;
/// What search and grep return at most: results, lines, and bytes of lines, so a response stays
/// far below the relay's and management's message limits.
pub const SEARCH_RESULTS: i64 = 20;
pub const GREP_LINES: i64 = 200;
const GREP_BYTES: usize = 128 * 1024;
/// Pages a listing returns at most: agents get fewer and narrow with a prefix.
pub const LIST_PAGES: i64 = 5000;
pub const AGENT_LIST_PAGES: i64 = 500;

#[derive(Clone)]
pub struct Wikis {
    connections: Connections,
}
/// One write to a page: its call id (an agent's tool call id, a fresh id for a person), and the
/// agent and conversation it came from (both unset for a person).
pub struct Write {
    pub call: Uuid,
    pub agent: Option<Uuid>,
    pub thread: Option<Uuid>,
}
/// A page's new content: all of it, or edits each replacing one exact, unique passage.
pub enum Body {
    Replace(String),
    Edits(Vec<(String, String)>),
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
fn content(value: &str) -> Result<String, Error> {
    if value.chars().count() > CONTENT_MAX {
        return Err(invalid(&format!(
            "A page is at most {CONTENT_MAX} characters"
        )));
    }
    Ok(value.to_owned())
}
fn description(value: &str) -> Result<&str, Error> {
    if value.chars().count() > DESCRIPTION_MAX {
        return Err(invalid(&format!(
            "The description is at most {DESCRIPTION_MAX} characters"
        )));
    }
    Ok(value.trim())
}
/// Slash-separated segments of letters, digits, `-`, `_` and `.`; outer slashes are dropped.
pub fn path(value: &str) -> Result<String, Error> {
    let value = value.trim().trim_matches('/');
    let valid = !value.is_empty()
        && value.len() <= PATH_MAX
        && value.split('/').all(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        });
    if !valid {
        return Err(invalid(
            "A page path is slash-separated segments of letters, digits, '-', '_' and '.'",
        ));
    }
    Ok(value.to_owned())
}
fn access(value: &str) -> Result<&'static str, Error> {
    match value {
        "read" => Ok("read"),
        "read_write" => Ok("read_write"),
        _ => Err(invalid("Access is read or read_write")),
    }
}
/// Applies `edits` in order; each `old` must occur exactly once in the text as it then is.
fn edit(text: &str, edits: &[(String, String)]) -> Result<String, Error> {
    let mut text = text.to_owned();
    for (index, (old, new)) in edits.iter().enumerate() {
        let found = if old.is_empty() {
            0
        } else {
            text.matches(old.as_str()).count()
        };
        if found != 1 {
            return Err(invalid(&format!(
                "Edit {}: its old text must match the page exactly once, and matches it {found} times; read the page again",
                index + 1
            )));
        }
        text = text.replacen(old.as_str(), new, 1);
    }
    content(&text)
}
fn stale(current: i32) -> Error {
    Error::Stale(format!(
        "The page changed since you read it; it is now at revision {current}. Read it again and reapply your change."
    ))
}
fn taken() -> Error {
    Error::Exists("The wiki already has a page at this path; read it and update it instead".into())
}
/// A grep pattern as a PostgreSQL regular expression.
fn pattern(value: &str, regex: bool) -> Result<String, Error> {
    if value.is_empty() || value.chars().count() > PATTERN_MAX {
        return Err(invalid(&format!(
            "The pattern must be 1 to {PATTERN_MAX} characters"
        )));
    }
    // `***=` makes the rest of a PostgreSQL regular expression a literal.
    Ok(if regex {
        value.to_owned()
    } else {
        format!("***={value}")
    })
}
/// A retried call that is not the same write: its id names another page or operation.
fn reused() -> Error {
    invalid("This call id was already used for a different write")
}
/// The current revision, for a stale-write error.
async fn current(db: &impl crate::database::GenericClient, id: Uuid) -> Result<Error, Error> {
    Ok(match db::page_by_id_opt(db, id).await? {
        Some(page) => stale(page.revision),
        None => Error::Missing("Wiki page"),
    })
}

impl Wikis {
    pub fn new(connections: Connections) -> Self {
        Self { connections }
    }
    async fn client(&self) -> Result<crate::database::Client, Error> {
        Ok(self.connections.pool.get().await?)
    }
    /// Deleted agents keep their row; they get no wikis or links.
    async fn require_live(&self, agent: Uuid) -> Result<(), Error> {
        if !db::agent_live_one(&self.client().await?, agent).await? {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub async fn wikis(&self, search: Option<&str>) -> Result<Vec<db::WikiRow>, Error> {
        Ok(db::wiki_list_all(&self.client().await?, None, search).await?)
    }
    pub async fn wiki(&self, id: Uuid) -> Result<db::WikiRow, Error> {
        db::wiki_list_all(&self.client().await?, Some(id), None)
            .await?
            .into_iter()
            .next()
            .ok_or(Error::Missing("Wiki"))
    }
    /// With an agent, the wiki is the agent's own and linked to it read-write.
    pub async fn create_wiki(
        &self,
        name: &str,
        about: &str,
        agent: Option<Uuid>,
    ) -> Result<db::WikiRow, Error> {
        let name = text(name, "name", NAME_MAX)?;
        let about = description(about)?;
        if let Some(agent) = agent {
            self.require_live(agent).await?;
        }
        let id = Uuid::new_v4();
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        match db::wiki_insert_execute(&tx, id, &name, about, agent).await {
            Ok(0) => return Err(Error::Exists("Another wiki has this name".into())),
            Ok(_) => {}
            Err(e) if e.is_unique_violation() => {
                return Err(Error::Exists("This agent already has its own wiki".into()));
            }
            Err(e) => return Err(e.into()),
        }
        if let Some(agent) = agent {
            db::link_set_execute(&tx, agent, id, "read_write").await?;
        }
        tx.commit().await?;
        drop(client);
        self.wiki(id).await
    }
    pub async fn update_wiki(
        &self,
        id: Uuid,
        name: Option<&str>,
        about: Option<&str>,
    ) -> Result<db::WikiRow, Error> {
        let name = name.map(|n| text(n, "name", NAME_MAX)).transpose()?;
        let about = about.map(description).transpose()?;
        let updated =
            db::wiki_update_execute(&self.client().await?, id, name.as_deref(), about).await?;
        let wiki = self.wiki(id).await?;
        if updated == 0 {
            return Err(Error::Exists("Another wiki has this name".into()));
        }
        Ok(wiki)
    }
    pub async fn delete_wiki(&self, id: Uuid) -> Result<(), Error> {
        if db::wiki_delete_execute(&self.client().await?, id).await? == 0 {
            return Err(Error::Missing("Wiki"));
        }
        Ok(())
    }

    pub async fn agent_wikis(&self, agent: Uuid) -> Result<Vec<db::AgentWikiRow>, Error> {
        Ok(db::agent_wikis_all(&self.client().await?, agent).await?)
    }
    pub async fn set_link(
        &self,
        agent: Uuid,
        wiki: Uuid,
        access_name: &str,
    ) -> Result<db::AgentWikiRow, Error> {
        if self
            .wiki(wiki)
            .await?
            .agent_id
            .is_some_and(|owner| owner != agent)
        {
            return Err(invalid("This wiki is another agent's own"));
        }
        self.require_live(agent).await?;
        db::link_set_execute(&self.client().await?, agent, wiki, access(access_name)?).await?;
        self.agent_wikis(agent)
            .await?
            .into_iter()
            .find(|l| l.wiki_id == wiki)
            .ok_or(Error::Missing("Wiki"))
    }
    pub async fn remove_link(&self, agent: Uuid, wiki: Uuid) -> Result<(), Error> {
        if self.wiki(wiki).await?.agent_id == Some(agent) {
            return Err(invalid(
                "This is the agent's own wiki; delete the wiki instead",
            ));
        }
        db::link_delete_execute(&self.client().await?, agent, wiki).await?;
        Ok(())
    }

    /// Pages in path order, at most `limit`; `true` when more exist.
    pub async fn pages(
        &self,
        wiki: Uuid,
        prefix: Option<&str>,
        limit: i64,
    ) -> Result<(Vec<db::PageSummaryRow>, bool), Error> {
        // An empty or slash-only prefix lists every page.
        let prefix = prefix
            .filter(|p| !p.trim().trim_matches('/').is_empty())
            .map(path)
            .transpose()?;
        let mut pages =
            db::pages_list_all(&self.client().await?, wiki, prefix.as_deref(), limit + 1).await?;
        let more = pages.len() as i64 > limit;
        pages.truncate(limit as usize);
        Ok((pages, more))
    }
    pub async fn page(&self, id: Uuid) -> Result<db::PageRow, Error> {
        db::page_by_id_opt(&self.client().await?, id)
            .await?
            .ok_or(Error::Missing("Wiki page"))
    }
    pub async fn page_at(&self, wiki: Uuid, at: &str) -> Result<Option<db::PageRow>, Error> {
        Ok(db::page_by_path_opt(&self.client().await?, wiki, &path(at)?).await?)
    }
    pub async fn revisions(&self, page: Uuid) -> Result<Vec<db::RevisionRow>, Error> {
        Ok(db::revisions_list_all(&self.client().await?, page).await?)
    }
    pub async fn revision(
        &self,
        page: Uuid,
        revision: i32,
    ) -> Result<db::RevisionContentRow, Error> {
        db::revision_get_opt(&self.client().await?, page, revision)
            .await?
            .ok_or(Error::Missing("Wiki page revision"))
    }
    /// A retry of a write that already landed (its call id has a revision) returns that write's
    /// result instead of writing again, if it is the same write.
    pub async fn create_page(
        &self,
        write: &Write,
        wiki: Uuid,
        at: &str,
        title: &str,
        body: &str,
    ) -> Result<db::PageRow, Error> {
        let (at, title, body) = (path(at)?, text(title, "title", TITLE_MAX)?, content(body)?);
        let created = |done: db::CallRow| {
            if done.operation == "create" && done.wiki_id == wiki && done.path == at {
                Ok(done.page_id)
            } else {
                Err(reused())
            }
        };
        let mut client = self.client().await?;
        if let Some(done) = db::revision_by_call_opt(&client, write.call).await? {
            drop(client);
            return self.page(created(done)?).await;
        }
        let tx = client.transaction().await?;
        let id = Uuid::new_v4();
        if db::page_insert_execute(&tx, id, wiki, &at, &title, &body).await? == 0 {
            return Err(taken());
        }
        let page = db::page_by_id_opt(&tx, id)
            .await?
            .ok_or(Error::Missing("Wiki page"))?;
        if db::revision_insert_execute(&tx, &page, 1, "create", write).await? == 0 {
            // The same call landed concurrently; its page stands.
            drop(tx);
            return match db::revision_by_call_opt(&client, write.call).await? {
                Some(done) => {
                    let id = created(done)?;
                    drop(client);
                    self.page(id).await
                }
                None => Err(Error::Missing("Wiki page")),
            };
        }
        tx.commit().await?;
        Ok(page)
    }
    /// Changes the title or content of the page at `expected`.
    pub async fn update_page(
        &self,
        write: &Write,
        id: Uuid,
        expected: i32,
        title: Option<&str>,
        body: Option<Body>,
    ) -> Result<db::PageRow, Error> {
        let title = title.map(|t| text(t, "title", TITLE_MAX)).transpose()?;
        let body = match body {
            Some(Body::Replace(body)) => Some(Body::Replace(content(&body)?)),
            other => other,
        };
        self.change(write, id, expected, "update", |page| {
            Ok(db::PageRow {
                title: title.unwrap_or_else(|| page.title.clone()),
                content: match body {
                    None => page.content.clone(),
                    Some(Body::Replace(body)) => body,
                    Some(Body::Edits(edits)) => edit(&page.content, &edits)?,
                },
                ..page.clone()
            })
        })
        .await
    }
    /// Gives the page at `expected` a new path; retries are recognised as for updates.
    pub async fn move_page(
        &self,
        write: &Write,
        id: Uuid,
        expected: i32,
        to: &str,
    ) -> Result<db::PageRow, Error> {
        let to = path(to)?;
        self.change(write, id, expected, "move", |page| {
            Ok(db::PageRow {
                path: to,
                ..page.clone()
            })
        })
        .await
    }
    /// `next` is the page as the write leaves it, computed once from the page at `expected`; it
    /// fails only when edits do not apply.
    async fn change(
        &self,
        write: &Write,
        id: Uuid,
        expected: i32,
        operation: &str,
        next: impl FnOnce(&db::PageRow) -> Result<db::PageRow, Error>,
    ) -> Result<db::PageRow, Error> {
        let mut client = self.client().await?;
        if let Some(done) = db::revision_by_call_opt(&client, write.call).await? {
            if done.page_id != id || done.operation != operation || done.revision != expected + 1 {
                return Err(reused());
            }
            drop(client);
            return self.page(id).await;
        }
        let tx = client.transaction().await?;
        let page = db::page_by_id_opt(&tx, id)
            .await?
            .ok_or(Error::Missing("Wiki page"))?;
        if page.revision != expected {
            return Err(stale(page.revision));
        }
        let next = next(&page)?;
        let revision =
            match db::page_update_opt(&tx, id, expected, &next.path, &next.title, &next.content)
                .await
            {
                Ok(Some(revision)) => revision,
                Ok(None) => {
                    drop(tx);
                    return Err(current(&client, id).await?);
                }
                Err(e) if e.is_unique_violation() => return Err(taken()),
                Err(e) => return Err(e.into()),
            };
        let next = db::PageRow { revision, ..next };
        if db::revision_insert_execute(&tx, &next, revision, operation, write).await? == 0 {
            // The same call landed concurrently; keep its change, not a second one.
            drop(tx);
            drop(client);
            return self.page(id).await;
        }
        tx.commit().await?;
        drop(client);
        self.page(id).await
    }
    /// Deletes the page at `expected`, keeping its history.
    pub async fn delete_page(&self, write: &Write, id: Uuid, expected: i32) -> Result<(), Error> {
        let mut client = self.client().await?;
        if let Some(done) = db::revision_by_call_opt(&client, write.call).await? {
            return if done.page_id == id
                && done.operation == "delete"
                && done.revision == expected + 1
            {
                Ok(())
            } else {
                Err(reused())
            };
        }
        let tx = client.transaction().await?;
        let page = db::page_by_id_opt(&tx, id)
            .await?
            .ok_or(Error::Missing("Wiki page"))?;
        if !db::page_delete_execute(&tx, id, expected).await? {
            drop(tx);
            return Err(current(&client, id).await?);
        }
        if db::revision_insert_execute(&tx, &page, expected + 1, "delete", write).await? == 0 {
            return Ok(());
        }
        tx.commit().await?;
        Ok(())
    }
    /// Deletes the page at a path, as an agent's tool does; a retried call succeeds.
    pub async fn delete_at(
        &self,
        write: &Write,
        wiki: Uuid,
        at: &str,
        expected: i32,
    ) -> Result<(), Error> {
        let at = path(at)?;
        if let Some(done) = db::revision_by_call_opt(&self.client().await?, write.call).await? {
            return if done.wiki_id == wiki
                && done.operation == "delete"
                && done.path == at
                && done.revision == expected + 1
            {
                Ok(())
            } else {
                Err(reused())
            };
        }
        match self.page_at(wiki, &at).await? {
            Some(page) => self.delete_page(write, page.id, expected).await,
            None => Err(Error::Missing("Wiki page")),
        }
    }

    /// Pages matching any of the query's words, best first.
    pub async fn search(
        &self,
        wikis: &[Uuid],
        query: &str,
        limit: i64,
    ) -> Result<Vec<db::SearchRow>, Error> {
        let query = text(query, "query", QUERY_MAX)?;
        Ok(db::search_all(&self.client().await?, wikis, &query, limit).await?)
    }
    /// Lines matching `value` (a literal, or a regular expression with `regex`), at most `limit`;
    /// `true` when more matched.
    pub async fn grep(
        &self,
        wikis: &[Uuid],
        value: &str,
        regex: bool,
        ignore_case: bool,
        limit: i64,
    ) -> Result<(Vec<db::GrepRow>, bool), Error> {
        let pattern = pattern(value, regex)?;
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        db::grep_session_one(&tx).await?;
        let mut lines = match db::grep_all(&tx, wikis, &pattern, ignore_case, limit + 1).await {
            Ok(lines) => lines,
            Err(e) if e.is_invalid_regex() => {
                return Err(invalid("The pattern is not a valid regular expression"));
            }
            Err(e) if e.is_query_canceled() => {
                return Err(invalid("The pattern took too long; make it more specific"));
            }
            Err(e) => return Err(e.into()),
        };
        tx.commit().await?;
        let mut more = lines.len() as i64 > limit;
        lines.truncate(limit as usize);
        let mut bytes = 0;
        if let Some(cut) = lines.iter().position(|l| {
            bytes += l.path.len() + l.line.len();
            bytes > GREP_BYTES
        }) {
            lines.truncate(cut);
            more = true;
        }
        Ok((lines, more))
    }
}
