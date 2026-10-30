//! The built-in wiki tools, offered to an agent with at least one linked wiki; the write tools
//! only with a read-write link. Wikis are named, never chosen by id, and resolved here from the
//! agent's links.
use super::{
    AGENT_LIST_PAGES, Body, CONTENT_MAX, GREP_LINES, PATH_MAX, PATTERN_MAX, QUERY_MAX,
    SEARCH_RESULTS, TITLE_MAX, Wikis, Write, db,
};
use crate::chat::Scope;
use crate::chat::tools::{Context, InputStream, Provider, ToolResult};
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::rest::{Hints, definition};
use connectrpc::ConnectError;
use futures::{StreamExt, future::BoxFuture};
use serde_json::{Value, json};
use uuid::Uuid;

const READ: Hints = Hints {
    read_only: true,
    destructive: false,
};
const WRITE: Hints = Hints {
    read_only: false,
    destructive: false,
};
const GUIDE: &str = "A wiki holds durable reference pages people read too (how things work, runbooks, decisions, guides), written in Markdown; keep short facts about people and conversations in memory instead. Read a page before changing it, keep its structure and headings, and change only what needs changing. Link other pages by the page's path from the wiki root, e.g. [Deploys](/runbooks/deploys). Never store secrets, credentials or tokens.";
/// Lines `wiki.read` returns at most when no range is given.
const READ_LINES: usize = 2000;

pub struct WikiTools(pub Wikis);

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
/// "Your wikis: …", describing each wiki a tool works on.
fn wikis_text(links: &[&db::AgentWikiRow]) -> String {
    let wikis: Vec<String> = links
        .iter()
        .map(|l| {
            let own = if l.own { "your own, " } else { "" };
            let access = if l.access == "read_write" {
                "read-write"
            } else {
                "read-only"
            };
            let about = if l.description.is_empty() {
                String::new()
            } else {
                format!(": {}", l.description)
            };
            format!("{} ({own}{access}){about}", l.wiki_name)
        })
        .collect();
    format!("Your wikis: {}.", wikis.join("; "))
}
/// The `wiki` parameter, offered only when there is a choice.
fn names(links: &[&db::AgentWikiRow]) -> Value {
    json!({
        "type":"string",
        "enum":links.iter().map(|l| l.wiki_name.as_str()).collect::<Vec<_>>(),
        "description":"Which wiki, by name.",
    })
}
fn path() -> Value {
    json!({"type":"string","minLength":1,"maxLength":PATH_MAX,"description":"The page's path: slash-separated segments of letters, digits, '-', '_' and '.' (e.g. runbooks/deploys)."})
}
fn revision() -> Value {
    json!({"type":"integer","minimum":0,"description":"The revision you read (wiki.read returns it); 0 to create a page. A write is refused when the page changed since: read it again and reapply your change."})
}

/// The agent's wiki tools: none without a linked wiki, the write tools only with a read-write
/// link. Listing and calling both go through here.
fn definitions(links: &[db::AgentWikiRow]) -> Vec<types::ToolDefinition> {
    if links.is_empty() {
        return vec![];
    }
    let all: Vec<_> = links.iter().collect();
    let writable: Vec<_> = links.iter().filter(|l| l.access == "read_write").collect();
    let mut search = json!({"query":{"type":"string","minLength":1,"maxLength":QUERY_MAX}});
    let mut grep = json!({
        "pattern":{"type":"string","minLength":1,"maxLength":PATTERN_MAX},
        "regex":{"type":"boolean","description":"Treat the pattern as a regular expression; a literal by default."},
        "ignore_case":{"type":"boolean"}
    });
    let mut read = json!({
        "path":path(),
        "from_line":{"type":"integer","minimum":1,"description":"The first line to return (1-based)."},
        "lines":{"type":"integer","minimum":1,"maximum":READ_LINES,"description":format!("How many lines to return; {READ_LINES} by default.")}
    });
    let mut list = json!({"prefix":{"type":"string","maxLength":PATH_MAX,"description":"Only the page at this path and those below it."}});
    let mut required_read = vec!["path"];
    let read_wikis = wikis_text(&all);
    if all.len() > 1 {
        for properties in [&mut search, &mut grep, &mut list] {
            properties["wiki"] = names(&all);
        }
        read["wiki"] = names(&all);
        required_read.push("wiki");
    }
    let mut out = vec![
        definition(
            "tilde",
            "wiki.search",
            "Searched the wiki",
            &format!(
                "Find wiki pages by keywords in their title and content, best match first, with an excerpt of each. Search before writing a page, and read a page before relying on it. {read_wikis}"
            ),
            schema(search, &["query"]),
            READ,
        ),
        definition(
            "tilde",
            "wiki.grep",
            "Searched wiki lines",
            &format!(
                "List lines of wiki pages containing a literal (or matching a regular expression), with each page's path and the line number, at most {GREP_LINES}. Use it to find exact names, commands or references. {read_wikis}"
            ),
            schema(grep, &["pattern"]),
            READ,
        ),
        definition(
            "tilde",
            "wiki.read",
            "Read a wiki page",
            &format!(
                "Read a wiki page's Markdown and its current revision, which a change must name. Long pages can be read in parts. {read_wikis}"
            ),
            schema(read, &required_read),
            READ,
        ),
        definition(
            "tilde",
            "wiki.list",
            "Listed wiki pages",
            &format!(
                "List pages' paths and titles in path order, every page or those under `prefix`. {read_wikis}"
            ),
            schema(list, &[]),
            READ,
        ),
    ];
    if writable.is_empty() {
        return out;
    }
    let mut write = json!({
        "path":path(),
        "title":{"type":"string","minLength":1,"maxLength":TITLE_MAX,"description":"Needed to create a page; omit to keep the title."},
        "content":{"type":"string","maxLength":CONTENT_MAX,"description":"The page's full Markdown, replacing what it has. Needed to create a page."},
        "edits":{"type":"array","minItems":1,"maxItems":50,"items":{"type":"object","properties":{
            "old":{"type":"string","minLength":1,"maxLength":CONTENT_MAX,"description":"Text the page has exactly once, copied from wiki.read."},
            "new":{"type":"string","maxLength":CONTENT_MAX}
        },"required":["old","new"],"additionalProperties":false},"description":"Replacements applied in order, instead of `content`, to change part of a page."},
        "expected_revision":revision()
    });
    let mut delete = json!({"path":path(),"expected_revision":revision()});
    // Writes and deletes need the same fields.
    let mut required = vec!["path", "expected_revision"];
    if writable.len() > 1 {
        write["wiki"] = names(&writable);
        delete["wiki"] = names(&writable);
        required.push("wiki");
    }
    let write_wikis = wikis_text(&writable);
    out.extend([
        definition(
            "tilde",
            "wiki.write",
            "Wrote a wiki page",
            &format!(
                "Create a page (expected_revision 0, with a title and content), or change one at the revision you read: prefer `edits` for small changes, and send the full `content` only to rewrite it. {GUIDE} {write_wikis}"
            ),
            schema(write, &required),
            WRITE,
        ),
        definition(
            "tilde",
            "wiki.delete",
            "Deleted a wiki page",
            &format!(
                "Delete a page at the revision you read; its history is kept. Only delete pages that are wrong or replaced. {write_wikis}"
            ),
            schema(delete, &required),
            Hints {
                read_only: false,
                destructive: true,
            },
        ),
    ]);
    out
}

/// The links a call may use: every one, or the one its `wiki` names (the schema only offers
/// names the agent has).
fn named<'a>(
    links: impl Iterator<Item = &'a db::AgentWikiRow>,
    input: &Value,
) -> Vec<&'a db::AgentWikiRow> {
    links
        .filter(|l| input["wiki"].as_str().is_none_or(|w| l.wiki_name == w))
        .collect()
}
fn one<'a>(
    links: impl Iterator<Item = &'a db::AgentWikiRow>,
    input: &Value,
) -> ToolResult<&'a db::AgentWikiRow> {
    named(links, input)
        .into_iter()
        .next()
        .ok_or_else(|| ConnectError::not_found("No such wiki"))
}
fn missing(path: &str) -> ConnectError {
    ConnectError::not_found(format!("No page at {path}; wiki.list shows the pages"))
}

impl WikiTools {
    pub async fn list(&self, agent: Uuid) -> ToolResult<Vec<types::ToolDefinition>> {
        Ok(definitions(&self.0.agent_wikis(agent).await?))
    }
    /// One call in an already authorized invocation scope; `call_id` keys a created page.
    pub async fn call(
        &self,
        scope: &Scope,
        call_id: Uuid,
        name: &str,
        input: Value,
    ) -> ToolResult<Value> {
        let links = self.0.agent_wikis(scope.agent_id).await?;
        // Listing and calling share one gate: a tool absent from the list cannot be called.
        let definition = definitions(&links)
            .into_iter()
            .find(|d| d.name == name)
            .ok_or_else(|| ConnectError::not_found("Tool is not available to this agent"))?;
        let schema: Value = serde_json::from_str(&definition.input_schema_json)
            .map_err(|_| ConnectError::internal("Invalid built-in schema"))?;
        if !jsonschema::validator_for(&schema)
            .map_err(|_| ConnectError::internal("Invalid built-in schema"))?
            .is_valid(&input)
        {
            return Err(ConnectError::invalid_argument(
                "Input does not match this tool's schema",
            ));
        }
        let at = input["path"].as_str().unwrap_or_default();
        let write = Write {
            call: call_id,
            agent: Some(scope.agent_id),
            thread: Some(scope.thread_id),
        };
        match name {
            "wiki.search" => {
                let wikis: Vec<Uuid> = named(links.iter(), &input)
                    .iter()
                    .map(|l| l.wiki_id)
                    .collect();
                let found = self
                    .0
                    .search(
                        &wikis,
                        input["query"].as_str().unwrap_or_default(),
                        SEARCH_RESULTS,
                    )
                    .await?;
                // The excerpt's private-use marks read as Markdown bold for agents.
                Ok(json!({"pages": found.iter().map(|p| json!({
                    "wiki": p.wiki_name, "path": p.path, "title": p.title,
                    "excerpt": p.excerpt.replace(['\u{e000}', '\u{e001}'], "**"),
                })).collect::<Vec<_>>()}))
            }
            "wiki.grep" => {
                let wikis: Vec<Uuid> = named(links.iter(), &input)
                    .iter()
                    .map(|l| l.wiki_id)
                    .collect();
                let (lines, more) = self
                    .0
                    .grep(
                        &wikis,
                        input["pattern"].as_str().unwrap_or_default(),
                        input["regex"].as_bool().unwrap_or(false),
                        input["ignore_case"].as_bool().unwrap_or(false),
                        GREP_LINES,
                    )
                    .await?;
                let mut out = json!({"lines": lines.iter().map(|l| {
                    let mut line = json!({"wiki": l.wiki_name, "path": l.path, "line": l.line_number, "text": l.line});
                    if l.truncated {
                        line["truncated"] = json!(true);
                    }
                    line
                }).collect::<Vec<_>>()});
                if more {
                    out["more"] = json!("More lines matched; narrow the pattern.");
                }
                Ok(out)
            }
            "wiki.read" => {
                let wiki = one(links.iter(), &input)?;
                let page = self
                    .0
                    .page_at(wiki.wiki_id, at)
                    .await?
                    .ok_or_else(|| missing(at))?;
                let all: Vec<&str> = page.content.lines().collect();
                let from = input["from_line"].as_u64().unwrap_or(1).max(1) as usize;
                if from > all.len().max(1) {
                    return Err(ConnectError::invalid_argument(format!(
                        "The page has {} lines; from_line is past its end",
                        all.len()
                    )));
                }
                let count = input["lines"].as_u64().map_or(READ_LINES, |n| n as usize);
                let part: Vec<&str> = all.iter().skip(from - 1).take(count).copied().collect();
                Ok(json!({
                    "wiki": page.wiki_name, "path": page.path, "title": page.title,
                    "revision": page.revision, "content": part.join("\n"),
                    "from": from, "to": from - 1 + part.len(), "total": all.len(),
                }))
            }
            "wiki.list" => {
                let (mut pages, mut more) = (Vec::new(), false);
                for wiki in named(links.iter(), &input) {
                    let left = AGENT_LIST_PAGES - pages.len() as i64;
                    let (found, cut) = self
                        .0
                        .pages(wiki.wiki_id, input["prefix"].as_str(), left.max(0))
                        .await?;
                    more |= cut;
                    pages.extend(found.into_iter().map(
                        |p| json!({"wiki": wiki.wiki_name, "path": p.path, "title": p.title}),
                    ));
                }
                let mut out = json!({"pages": pages});
                if more {
                    out["more"] = json!(format!(
                        "Only {AGENT_LIST_PAGES} pages are listed; narrow with prefix."
                    ));
                }
                Ok(out)
            }
            "wiki.write" => {
                let wiki = one(links.iter().filter(|l| l.access == "read_write"), &input)?;
                let expected = input["expected_revision"].as_i64().unwrap_or_default() as i32;
                let title = input["title"].as_str();
                let content = input["content"].as_str();
                let edits = input["edits"].as_array().map(|edits| {
                    edits
                        .iter()
                        .map(|e| {
                            (
                                e["old"].as_str().unwrap_or_default().to_owned(),
                                e["new"].as_str().unwrap_or_default().to_owned(),
                            )
                        })
                        .collect::<Vec<_>>()
                });
                let page = match (expected, content, edits) {
                    (_, Some(_), Some(_)) => {
                        return Err(ConnectError::invalid_argument(
                            "Send content or edits, not both",
                        ));
                    }
                    (0, Some(content), None) => {
                        let title = title.ok_or_else(|| {
                            ConnectError::invalid_argument("A new page needs a title")
                        })?;
                        self.0
                            .create_page(&write, wiki.wiki_id, at, title, content)
                            .await?
                    }
                    (0, None, _) => {
                        return Err(ConnectError::invalid_argument(
                            "A new page needs its content",
                        ));
                    }
                    (_, content, edits) => {
                        let page = self
                            .0
                            .page_at(wiki.wiki_id, at)
                            .await?
                            .ok_or_else(|| missing(at))?;
                        let body = match (content, edits) {
                            (Some(content), _) => Some(Body::Replace(content.to_owned())),
                            (None, Some(edits)) => Some(Body::Edits(edits)),
                            (None, None) => None,
                        };
                        self.0
                            .update_page(&write, page.id, expected, title, body)
                            .await?
                    }
                };
                Ok(json!({"wiki": page.wiki_name, "path": page.path, "revision": page.revision}))
            }
            "wiki.delete" => {
                let wiki = one(links.iter().filter(|l| l.access == "read_write"), &input)?;
                let expected = input["expected_revision"].as_i64().unwrap_or_default() as i32;
                self.0
                    .delete_at(&write, wiki.wiki_id, at, expected)
                    .await
                    .map_err(|e| match e {
                        crate::error::Error::Missing(_) => missing(at),
                        e => e.into(),
                    })?;
                Ok(json!({"deleted": at}))
            }
            _ => Err(ConnectError::not_found("Unknown wiki tool")),
        }
    }
}

impl Provider for WikiTools {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(self.list(scope.agent_id))
    }
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        input: Value,
        mut chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            while let Some(chunk) = chunks.next().await {
                chunk?;
            }
            context.authorize().await?;
            self.call(context.scope(), context.call_id, name, input)
                .await
        })
    }
}
