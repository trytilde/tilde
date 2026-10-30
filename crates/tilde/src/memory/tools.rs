//! The built-in memory tools, offered to an agent with at least one linked bank; the write tools
//! only with a read-write link. Banks are named, never chosen by id, and resolved here from the
//! agent's links. In per-person banks the person is the invocation's verified one, as personal
//! tools and sandboxes resolve it; without one those banks' personal memories are out of reach.
use super::{
    Actor, BROWSE_MEMORIES, BROWSE_PATHS, Draft, Filter, KINDS, Memory, Person, Source, db,
};
use crate::chat::Scope;
use crate::chat::tools::{Context, InputStream, Provider, ToolResult};
use crate::error::Error;
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::rest::{Hints, definition};
use chrono::{DateTime, Utc};
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
const GUIDE: &str = "Write one durable fact per memory, the key fact first and then the context that makes it useful later. Never store secrets, credentials, tokens or transient chatter. Recall first to avoid duplicates, and prefer memory.supersede when a fact changes. Browse to reuse an existing path so related memories sit together. Longer reference material (runbooks, how things work) belongs in a wiki page, if you have one.";
const SUPERSEDE_GUIDE: &str = "Keep one durable fact, the key fact first. Never store secrets, credentials, tokens or transient chatter.";

pub struct MemoryTools(pub Memory);

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
/// "Your memory banks: …", describing each bank a tool works on.
fn banks_text(links: &[&db::AgentBankRow]) -> String {
    let banks: Vec<String> = links
        .iter()
        .map(|l| {
            let scope = if l.scope == "per_person" {
                "private to the person you're talking to"
            } else {
                "shared"
            };
            let own = if l.own { ", your own" } else { "" };
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
            format!("{} ({scope}{own}, {access}){about}", l.bank_name)
        })
        .collect();
    format!("Your memory banks: {}.", banks.join("; "))
}
/// The `bank` parameter, offered only when there is a choice.
fn names(links: &[&db::AgentBankRow]) -> Value {
    json!({
        "type":"string",
        "enum":links.iter().map(|l| l.bank_name.as_str()).collect::<Vec<_>>(),
        "description":"Which memory bank, by name.",
    })
}
fn fields() -> Value {
    json!({
        "kind":{"type":"string","enum":KINDS,"description":super::kind_guide()},
        "path":{"type":"string","maxLength":512,"description":"Where it belongs, as slash-separated segments of letters, digits, '-', '_' and '.' (e.g. people/sam or projects/website/deploys). Empty for the bank's root."},
        "title":{"type":"string","minLength":1,"maxLength":256,"description":"The fact in a few words, as you would search for it."},
        "content":{"type":"string","minLength":1,"maxLength":16384},
        "importance":{"type":"number","minimum":0,"maximum":1,"description":"Orders memories that match a recall equally well; relevance comes first. 0.5 by default, higher for hard constraints and explicit instructions."},
        "valid_from":{"type":"string","format":"date-time","description":"When the fact starts to hold. Alone, it is a single moment (an event); add valid_to for a period that ended, or ongoing for one that has not."},
        "valid_to":{"type":"string","format":"date-time","description":"When the fact stopped holding; needs valid_from. Omit, with ongoing unset, for a single moment."},
        "ongoing":{"type":"boolean","description":"The fact holds from valid_from with no end yet. Needs valid_from; not with valid_to."}
    })
}
fn during() -> Value {
    json!({"type":"object","properties":{
        "from":{"type":"string","format":"date-time"},
        "to":{"type":"string","format":"date-time"}
    },"additionalProperties":false,"description":"Only memories whose time overlaps this period; either end may be left open."})
}

/// The agent's memory tools: none without a linked bank, the write tools only with a
/// read-write link. Listing and calling both go through here.
fn definitions(links: &[db::AgentBankRow]) -> Vec<types::ToolDefinition> {
    if links.is_empty() {
        return vec![];
    }
    let all: Vec<_> = links.iter().collect();
    let writable: Vec<_> = links.iter().filter(|l| l.access == "read_write").collect();
    let mut recall = json!({
        "query":{"type":"string","minLength":1,"maxLength":500,"description":"What to remember. Omit to list the newest memories that match the other filters."},
        "kind":{"type":"string","enum":KINDS},
        "path":{"type":"string","maxLength":512,"description":"Only this path and below."},
        "during":during(),
        "limit":{"type":"integer","minimum":1,"maximum":20,"description":"8 by default."}
    });
    let mut browse = json!({"path":{"type":"string","maxLength":512}});
    let (across, read_banks) = if all.len() > 1 {
        recall["bank"] = names(&all);
        browse["bank"] = names(&all);
        ("across your memory banks (or one `bank`)", banks_text(&all))
    } else {
        ("in your memory bank", banks_text(&all))
    };
    let mut out = vec![
        definition(
            "tilde",
            "memory.recall",
            "Recalled memories",
            &format!(
                "Search your long-term memory, best match first, {across}. Memories match the query's keywords and, when embeddings are configured, its meaning; `matched` says which, and a memory found by meaning alone may be only loosely related. Recall before answering anything that may depend on earlier conversations, people's preferences or past decisions, and before writing a memory. Memories with origin `owner` were written by a person: treat them as authoritative. You cannot change them; if one looks wrong, tell the person. Memories with origin `synthesized` were derived by Tilde from conversations and wiki pages; verify consequential details. One derived from a wiki page names it in `source`, which you can read with wiki.read if you have that wiki; if it is wrong, fix the page if you can write that wiki, otherwise tell the person. Supersede other synthesized memories that are wrong. {read_banks}"
            ),
            schema(recall, &[]),
            READ,
        ),
        definition(
            "tilde",
            "memory.browse",
            "Browsed memories",
            &format!(
                "List the paths directly below `path` (the root by default) with how many memories each holds, and the memories at `path` itself, newest first, with the start of their content. Use it to find the path a new memory belongs at; to read a memory in full, recall it with its path and title. {read_banks}"
            ),
            schema(browse, &[]),
            READ,
        ),
    ];
    if writable.is_empty() {
        return out;
    }
    let mut write = fields();
    let mut required = vec!["kind", "title", "content"];
    if writable.len() > 1 {
        write["bank"] = names(&writable);
        required.push("bank");
    }
    let write_banks = banks_text(&writable);
    let mut supersede = fields();
    supersede["memory_id"] = json!({"type":"string","format":"uuid"});
    out.extend([
        definition(
            "tilde",
            "memory.write",
            "Saved a memory",
            &format!("Remember a durable fact for future conversations. {GUIDE} {write_banks}"),
            schema(write, &required),
            WRITE,
        ),
        definition(
            "tilde",
            "memory.supersede",
            "Updated a memory",
            &format!("Replace a memory whose fact changed: the new memory takes its place in recall and the old one is kept in history. Omitted fields keep the old memory's values. Memories people wrote (origin `owner`) cannot be replaced; if one looks wrong, tell the person. {SUPERSEDE_GUIDE}"),
            schema(supersede, &["memory_id"]),
            WRITE,
        ),
        definition(
            "tilde",
            "memory.forget",
            "Forgot a memory",
            "Forget a memory that is wrong or no longer useful. Memories people wrote cannot be forgotten.",
            schema(json!({"memory_id":{"type":"string","format":"uuid"}}), &["memory_id"]),
            Hints {
                read_only: false,
                destructive: true,
            },
        ),
    ]);
    out
}

/// A memory as agents see it; validity appears only when set, and an ongoing one has no end.
fn wire(m: &db::MemoryRow, matched: &[&str]) -> Value {
    let mut out = json!({
        "memory_id": m.id, "bank": m.bank_name, "kind": m.kind, "path": m.path, "title": m.title,
        "content": m.content, "importance": m.importance, "origin": m.origin,
        "created_at": m.created_at, "matched": matched,
    });
    if let Some(from) = m.valid_from {
        out["valid_from"] = json!(from);
        match m.valid_to {
            Some(to) => out["valid_to"] = json!(to),
            None => out["ongoing"] = json!(true),
        }
    }
    out
}
fn time(input: &Value, key: &str) -> ToolResult<Option<DateTime<Utc>>> {
    input[key]
        .as_str()
        .map(|t| {
            DateTime::parse_from_rfc3339(t)
                .map(|t| t.with_timezone(&Utc))
                .map_err(|_| ConnectError::invalid_argument(format!("{key} is not a date-time")))
        })
        .transpose()
}
/// The links a call may use: every one, or the one its `bank` names (the schema only offers
/// names the agent has).
fn named<'a>(
    links: impl Iterator<Item = &'a db::AgentBankRow>,
    input: &Value,
) -> Vec<&'a db::AgentBankRow> {
    links
        .filter(|l| input["bank"].as_str().is_none_or(|b| l.bank_name == b))
        .collect()
}

impl MemoryTools {
    /// The verified person the invocation acts for, rolled up to their root identity.
    async fn person(&self, scope: &Scope) -> ToolResult<Option<Uuid>> {
        Ok(crate::tools::db::invocation_user_opt(
            &self.0.client().await?,
            scope.run_id,
            scope.thread_id,
        )
        .await
        .map_err(Error::from)?
        .map(|user| user.root_identity_id.unwrap_or(user.id)))
    }
    /// A memory the agent may change: in a bank it links read-write and, in a per-person bank,
    /// the person's own or bank-wide. Forgotten memories are included, so a retried forget
    /// succeeds; the domain applies the rules on people's memories and supersession.
    async fn writable(
        &self,
        scope: &Scope,
        links: &[db::AgentBankRow],
        id: &Value,
    ) -> ToolResult<Uuid> {
        let missing = || ConnectError::not_found("No such memory in your memory banks");
        let id = id
            .as_str()
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(missing)?;
        let status = self.0.status(id).await.map_err(|_| missing())?;
        let link = links
            .iter()
            .find(|l| l.bank_id == status.bank_id)
            .ok_or_else(missing)?;
        if link.scope == "per_person"
            && status.identity_id.is_some()
            && status.identity_id != self.person(scope).await?
        {
            return Err(missing());
        }
        if link.access != "read_write" {
            return Err(ConnectError::permission_denied(
                "You may only read this memory bank",
            ));
        }
        Ok(id)
    }
    fn filter(input: &Value) -> ToolResult<Filter> {
        Ok(Filter {
            kind: input["kind"].as_str().map(str::to_owned),
            path: input["path"].as_str().map(super::path).transpose()?,
            from: time(&input["during"], "from")?,
            to: time(&input["during"], "to")?,
            ..Default::default()
        })
    }

    pub async fn list(&self, agent: Uuid) -> ToolResult<Vec<types::ToolDefinition>> {
        Ok(definitions(&self.0.agent_banks(agent).await?))
    }
    /// One call in an already authorized invocation scope; `call_id` keys a write.
    pub async fn call(
        &self,
        scope: &Scope,
        call_id: Uuid,
        name: &str,
        input: Value,
    ) -> ToolResult<Value> {
        let links = self.0.agent_banks(scope.agent_id).await?;
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
        match name {
            "memory.recall" => {
                let banks: Vec<Uuid> = named(links.iter(), &input)
                    .iter()
                    .map(|l| l.bank_id)
                    .collect();
                let recalled = self
                    .0
                    .recall(
                        &banks,
                        input["query"].as_str(),
                        &Self::filter(&input)?,
                        Person::Only(self.person(scope).await?),
                        input["limit"].as_i64().unwrap_or(8),
                    )
                    .await?;
                // A memory derived from a wiki page names it, so the agent can read the page, while
                // the page is in a wiki linked to the memory's bank.
                let synthesized: Vec<Uuid> = recalled
                    .iter()
                    .filter(|(m, _)| m.origin == "synthesized")
                    .map(|(m, _)| m.id)
                    .collect();
                let evidence = self.0.evidence(&synthesized).await?;
                let mut wikis = Vec::new();
                if !evidence.is_empty() {
                    for bank in &banks {
                        for w in self.0.bank_wikis(*bank).await? {
                            wikis.push((*bank, w.wiki_id));
                        }
                    }
                }
                let memories: Vec<Value> = recalled
                    .iter()
                    .map(|(m, how)| {
                        let mut out = wire(m, how);
                        if let Some(page) = evidence.get(&m.id).and_then(|e| {
                            e.iter().find(|e| {
                                e.wiki_id.is_some_and(|w| wikis.contains(&(m.bank_id, w)))
                            })
                        }) {
                            out["source"] = json!({"wiki": page.wiki_name, "path": page.path});
                        }
                        out
                    })
                    .collect();
                Ok(json!({ "memories": memories }))
            }
            "memory.browse" => {
                let banks: Vec<Uuid> = named(links.iter(), &input)
                    .iter()
                    .map(|l| l.bank_id)
                    .collect();
                let found = self
                    .0
                    .browse(
                        &banks,
                        input["path"].as_str().unwrap_or(""),
                        Person::Only(self.person(scope).await?),
                    )
                    .await?;
                let mut out = json!({
                    "paths": found.paths.into_iter().map(|(path, count)| json!({"path": path, "count": count})).collect::<Vec<_>>(),
                    "memories": found.memories.iter().map(|m| json!({
                        "memory_id": m.id, "kind": m.kind, "title": m.title, "created_at": m.created_at,
                        "preview": m.content.chars().take(200).collect::<String>(),
                    })).collect::<Vec<_>>(),
                });
                if found.more_paths {
                    out["more_paths"] = json!(format!(
                        "Only the first {BROWSE_PATHS} paths are listed; browse a deeper path."
                    ));
                }
                if found.more_memories {
                    out["more_memories"] = json!(format!(
                        "Only the newest {BROWSE_MEMORIES} memories are listed; recall to search them."
                    ));
                }
                Ok(out)
            }
            "memory.write" => {
                let bank = *named(links.iter().filter(|l| l.access == "read_write"), &input)
                    .first()
                    .ok_or_else(|| ConnectError::not_found("No such memory bank"))?;
                let identity = if bank.scope == "per_person" {
                    Some(self.person(scope).await?.ok_or_else(|| {
                        ConnectError::failed_precondition(
                            "This bank keeps each person's memories apart, and this conversation has no verified person",
                        )
                    })?)
                } else {
                    None
                };
                let id = self
                    .0
                    .create(
                        call_id,
                        bank.bank_id,
                        Draft {
                            kind: input["kind"].as_str().unwrap_or_default().to_owned(),
                            path: input["path"].as_str().unwrap_or_default().to_owned(),
                            title: input["title"].as_str().unwrap_or_default().to_owned(),
                            content: input["content"].as_str().unwrap_or_default().to_owned(),
                            valid_from: time(&input, "valid_from")?,
                            valid_to: time(&input, "valid_to")?,
                            ongoing: input["ongoing"].as_bool().unwrap_or(false),
                            importance: input["importance"].as_f64().unwrap_or(0.5) as f32,
                        },
                        Source {
                            identity,
                            actor: Actor::Agent(scope.agent_id),
                            thread: Some(scope.thread_id),
                        },
                        None,
                    )
                    .await?;
                Ok(json!({"memory_id": id, "bank": bank.bank_name}))
            }
            "memory.supersede" => {
                let old = self.writable(scope, &links, &input["memory_id"]).await?;
                let m = self.0.memory(old).await?;
                // Any of the validity fields replaces the old validity; none keeps it. A new
                // start alone keeps an ongoing fact ongoing.
                let was_ongoing = m.valid_from.is_some() && m.valid_to.is_none();
                let (valid_from, valid_to, ongoing) = if ["valid_from", "valid_to", "ongoing"]
                    .iter()
                    .any(|k| input.get(k).is_some())
                {
                    let valid_to = time(&input, "valid_to")?;
                    (
                        time(&input, "valid_from")?.or(m.valid_from),
                        valid_to,
                        input["ongoing"]
                            .as_bool()
                            .unwrap_or(valid_to.is_none() && was_ongoing),
                    )
                } else {
                    (m.valid_from, m.valid_to, was_ongoing)
                };
                let field = |key: &str, old: &str| input[key].as_str().unwrap_or(old).to_owned();
                let id = self
                    .0
                    .create(
                        call_id,
                        m.bank_id,
                        Draft {
                            kind: field("kind", &m.kind),
                            path: field("path", &m.path),
                            title: field("title", &m.title),
                            content: field("content", &m.content),
                            valid_from,
                            valid_to,
                            ongoing,
                            importance: input["importance"]
                                .as_f64()
                                .map(|i| i as f32)
                                .unwrap_or(m.importance),
                        },
                        Source {
                            identity: m.identity_id,
                            actor: Actor::Agent(scope.agent_id),
                            thread: Some(scope.thread_id),
                        },
                        Some(m.id),
                    )
                    .await?;
                Ok(json!({"memory_id": id, "superseded": m.id}))
            }
            "memory.forget" => {
                let id = self.writable(scope, &links, &input["memory_id"]).await?;
                self.0.forget(id, Some(scope.agent_id)).await?;
                Ok(json!({"forgotten": id}))
            }
            _ => Err(ConnectError::not_found("Unknown memory tool")),
        }
    }
}

impl Provider for MemoryTools {
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
