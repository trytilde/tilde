//! An agent's tools. An agent uses tools from sources: a tool-capable installation connection
//! (including an instance of a tool host's provider) or a tool host that needs no credentials.
//! For each source the agent picks the tools it uses, offered as `{source slug}.{tool name}`;
//! the agent's `tools_invoke` capability still filters those names. Connections and tool hosts
//! only hold credentials and describe their tools: which tools an agent gets is its own
//! configuration, so two agents can use one connection with different tools.
//!
//! A sandbox blueprint owns sources the same way; their tools are called by processes inside its
//! sandboxes (`crate::sandboxes`). An agent with a sandbox has the sandbox itself as a source of
//! the fixed sandbox tools, added and removed with the agent's sandbox setting.
//!
//! Sidecar-hosted agents get the same catalog. Managed provider tools and tools of MCP-served
//! connections run on the sidecar: its replicated configuration carries their catalog entries
//! (`Tools::sidecar_tools`) and their connections' credentials, and a change to either pushes a
//! new configuration. Tool hosts (which dial the gateway), sandbox tools and providers that sign
//! with the gateway's own keys run on the gateway; the sidecar lists and calls them through the
//! registry relay (`relay`). `Callee::on_sidecar` draws the line.
use crate::chat::{
    Scope,
    tools::{Context, InputStream, Provider, ToolResult},
};
use crate::connections::{
    model::{ProviderKind, invalid},
    service::Connections,
};
use crate::error::Error;
use crate::proto::tilde::agent_event_ingress::v1 as sidecar;
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::{StreamExt, future::BoxFuture};
use serde_json::Value;
use uuid::Uuid;

pub(crate) mod apply;
pub mod builtin;
pub mod db;
pub mod hosts;
pub mod mcp;
pub mod mcp_catalog;
pub mod personal;
pub mod providers;
pub mod relay;
pub mod rpc;

/// One owner's use of one source.
pub struct Source {
    pub id: Uuid,
    pub owner: Owner,
    pub target: Target,
    pub slug: String,
    pub tools: Vec<db::AgentToolRow>,
}
/// Who uses a source: an agent, or a sandbox blueprint for the processes inside its sandboxes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Agent(Uuid),
    Blueprint(Uuid),
}
impl Owner {
    pub(crate) fn columns(self) -> (Option<Uuid>, Option<Uuid>) {
        match self {
            Owner::Agent(id) => (Some(id), None),
            Owner::Blueprint(id) => (None, Some(id)),
        }
    }
}
/// What a source's tools come from. `Sandbox` is the owning agent's sandbox.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Connection(Uuid),
    ToolHost(Uuid),
    Sandbox,
}
impl Target {
    pub(crate) fn columns(self) -> (Option<Uuid>, Option<Uuid>) {
        match self {
            Target::Connection(id) => (Some(id), None),
            Target::ToolHost(id) => (None, Some(id)),
            Target::Sandbox => (None, None),
        }
    }
}
/// An agent's settings for one of its tools. Empty wording keeps the tool's own; `display` is how
/// its calls show in end-user chats (traces always keep the full call).
#[derive(Clone, Default)]
pub struct ToolSettings {
    pub is_async: bool,
    pub summary: String,
    pub description: String,
    pub display: buffa::EnumValue<types::ToolDisplay>,
}
/// The agent's wording and display applied to a tool's catalog definition.
fn apply(
    definition: &mut types::ToolDefinition,
    summary: String,
    description: String,
    display: &str,
) {
    if !summary.is_empty() {
        definition.summary = summary;
    }
    if !description.is_empty() {
        definition.description = description;
    }
    definition.display = crate::chat::tools::audit::display_value(display).into();
}
#[derive(Default)]
pub struct Filter {
    pub id: Option<Uuid>,
    pub agent: Option<Uuid>,
    pub connection: Option<Uuid>,
    pub tool_host: Option<Uuid>,
    pub blueprint: Option<Uuid>,
}

/// Prefixes of built-in catalog names; a source with one could shadow them. `sandbox` is the
/// agent's sandbox source.
const RESERVED_SLUGS: [&str; 10] = [
    "tools", "agents", "thread", "user", "goals", "tasks", "channel", "sandbox", "memory", "wiki",
];
fn fixed() -> Error {
    invalid("The sandbox's tools are fixed; remove the sandbox in Capabilities instead")
}
/// `value` in the catalog-name alphabet: lowercase for slugs, case kept for tool names.
fn catalog_safe(value: &str, lowercase: bool, max: usize) -> String {
    let mut out = String::new();
    for c in value.chars() {
        let c = if lowercase { c.to_ascii_lowercase() } else { c };
        let keep = c.is_ascii_alphanumeric() || c == '-' || (c == '_' && !out.ends_with('_'));
        if keep {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches(['_', '-']);
    out.chars()
        .take(max)
        .collect::<String>()
        .trim_end_matches(['_', '-'])
        .to_owned()
}

/// A tool's catalog name within its source. Two tool names that differ only in characters a
/// catalog name cannot hold would collide, and are refused rather than renamed.
fn tool_name_for(tool: &str, others: &[(&str, String)]) -> Result<String, Error> {
    let name = catalog_safe(tool, false, 64);
    if name.is_empty() {
        return Err(invalid("This tool's name cannot be offered to agents"));
    }
    if others
        .iter()
        .any(|(other, taken)| *other != tool && *taken == name)
    {
        return Err(invalid(
            "Another of this source's tools has the same catalog name",
        ));
    }
    Ok(name)
}

/// The name a connection's source slug is derived from: its name, prefixed by its provider
/// unless the name already says it.
fn source_name(provider: &str, name: &str) -> String {
    if catalog_safe(name, true, 32) == catalog_safe(provider, true, 32) {
        name.to_owned()
    } else {
        format!("{provider} {name}")
    }
}
fn slug_base(name: &str) -> String {
    match catalog_safe(name, true, 28) {
        slug if slug.is_empty() || RESERVED_SLUGS.contains(&slug.as_str()) => {
            format!("{slug}_tools").trim_start_matches('_').to_owned()
        }
        slug => slug,
    }
}
/// Insert a source with the first free slug from `base`. Column checks own the slug alphabet; a
/// slug the owner already uses inserts nothing.
pub(crate) async fn insert_source(
    tx: &tokio_postgres::Transaction<'_>,
    owner: Owner,
    target: Target,
    base: &str,
) -> Result<Uuid, Error> {
    let (agent, blueprint) = owner.columns();
    let (connection, tool_host) = target.columns();
    let sandbox = agent.filter(|_| target == Target::Sandbox);
    let id = Uuid::new_v4();
    for n in 1..100 {
        let slug = if n == 1 {
            base.to_owned()
        } else {
            format!("{base}_{n}")
        };
        let inserted = db::source_insert_execute(
            tx, id, agent, connection, tool_host, &slug, blueprint, sandbox,
        )
        .await?;
        if inserted == 1 {
            return Ok(id);
        }
    }
    Err(invalid("Too many sources use this name"))
}

/// Unauthenticated `tools/list` answers by server URL, failures included, so browsing the
/// catalog doesn't dial a server on every panel open.
type Probes =
    std::collections::HashMap<String, (std::time::Instant, Option<Vec<types::ToolDefinition>>)>;
const PROBE_TTL: std::time::Duration = std::time::Duration::from_secs(600);

/// One catalog entry: the definition the agent sees, what it calls, and whether it is deferred.
pub(crate) type Entry = (types::ToolDefinition, Callee, bool);
/// Each live invocation's catalog, built on its first ListTools or InvokeTool and reused until
/// the invocation ends (at most `CATALOG_TTL`). A run makes many calls in quick succession, so
/// the database is read once per invocation; a tool removed from the agent, or a source that
/// stops being ready, takes effect for its next invocation. Credentials are still resolved, and
/// every provider still checks its own state, on each call.
type Catalogs = std::collections::HashMap<Uuid, (std::time::Instant, std::sync::Arc<Vec<Entry>>)>;
const CATALOG_TTL: std::time::Duration = std::time::Duration::from_secs(900);

#[derive(Clone)]
pub struct Tools {
    pub(crate) connections: Connections,
    pub hosts: hosts::ToolHosts,
    pub sandboxes: crate::sandboxes::Sandboxes,
    pub memory: crate::memory::Memory,
    pub wikis: crate::wikis::Wikis,
    probes: std::sync::Arc<std::sync::Mutex<Probes>>,
    catalogs: std::sync::Arc<std::sync::Mutex<Catalogs>>,
}
/// What a catalog entry calls once its name has been resolved.
#[derive(Clone)]
pub(crate) enum Callee {
    Connection(db::AgentFunctionRow),
    /// A discovered tool of an MCP-served connection: connection, provider, type and tool.
    Mcp(Uuid, String, String, String),
    Host(hosts::Target, String),
    /// A fixed sandbox tool, run in the invocation's sandbox.
    Sandbox(String),
}
impl Callee {
    /// Whether a sidecar runs the tool itself, with its replicated copy of the connection's
    /// credentials. Tool hosts dial the gateway, sandboxes and other gateway-only sources are
    /// reached from it, and some providers sign with the gateway's own keys, so those stay there.
    pub(crate) fn on_sidecar(&self) -> bool {
        match self {
            Callee::Connection(row) => !providers::needs_gateway(&row.provider_id, &row.type_id),
            Callee::Mcp(..) => true,
            Callee::Host(..) | Callee::Sandbox(_) => false,
        }
    }
}
/// Who a call is made for, as tool hosts are told.
pub(crate) struct Caller {
    pub call_id: Uuid,
    pub agent: Option<Uuid>,
    pub thread: Option<Uuid>,
    /// The invocation's run; a sandbox's process has none.
    #[allow(dead_code)]
    pub run: Option<Uuid>,
    pub sandbox: Option<Uuid>,
}
/// Tool text limits in characters, as Postgres `length()` checks them: every definition, agent
/// override and audit record uses these, so text a setting accepts never fails at audit.
pub const SUMMARY_CHARS: usize = 256;
/// The largest tool output: providers stop reading an upstream body past it, tool hosts may not
/// answer with more, and the audit record holds at most this much. An output that still ends up
/// larger fails the call (`OUTPUT_TOO_LARGE`) rather than its audit.
pub const MAX_OUTPUT: usize = 1024 * 1024;
pub const OUTPUT_TOO_LARGE: &str = "The tool's output is larger than 1 MiB; narrow the request";
pub const DESCRIPTION_CHARS: usize = 4096;
pub const BUNDLED_INVALID: &str = "Bundled tools need a unique name, a description, a summary \
     of at most 256 characters and JSON object schemas";
/// Whether bundled tool definitions can be stored, as an invocation registers them or a
/// deployment declares them: at most 256, unique names outside the catalog's `tools.` prefix,
/// a description, a short summary and JSON object schemas (the output schema may be empty).
pub fn bundled_valid(tools: &[types::ToolDefinition]) -> bool {
    let object = |json: &str| serde_json::from_str::<Value>(json).is_ok_and(|v| v.is_object());
    let mut names = std::collections::BTreeSet::new();
    tools.len() <= 256
        && tools.iter().all(|tool| {
            !tool.name.is_empty()
                && tool.name.len() <= 128
                && !tool.name.starts_with("tools.")
                && !tool.description.is_empty()
                && tool.description.chars().count() <= DESCRIPTION_CHARS
                && tool.summary.chars().count() <= SUMMARY_CHARS
                && tool.input_schema_json.len() <= 64 * 1024
                && tool.output_schema_json.len() <= 64 * 1024
                && object(&tool.input_schema_json)
                && (tool.output_schema_json.is_empty() || object(&tool.output_schema_json))
                && names.insert(tool.name.as_str())
        })
}
/// Record the bundled tools a deployment's code declares, with where each was found.
pub async fn register_declared(
    tx: &tokio_postgres::Transaction<'_>,
    deployment: Uuid,
    tools: &[(types::ToolDefinition, String)],
) -> Result<(), Error> {
    for (tool, origin) in tools {
        db::deployment_tool_insert_execute(
            tx,
            deployment,
            tool,
            crate::chat::tools::audit::display_text(tool.display),
            origin,
        )
        .await?;
    }
    Ok(())
}
/// The bundled tools a deployment declared.
pub async fn declared(
    client: &impl crate::database::GenericClient,
    deployment: Uuid,
) -> Result<Vec<crate::proto::tilde::management::v1::DeclaredTool>, Error> {
    Ok(db::deployment_tools_all(client, deployment)
        .await?
        .into_iter()
        .map(|t| crate::proto::tilde::management::v1::DeclaredTool {
            name: t.name,
            description: t.description,
            summary: t.summary,
            input_schema_json: t.input_schema_json,
            output_schema_json: t.output_schema_json,
            annotations: types::ToolAnnotations {
                read_only: t.read_only,
                destructive: t.destructive,
                idempotent: t.idempotent,
                open_world: t.open_world,
                ..Default::default()
            }
            .into(),
            display: crate::chat::tools::audit::display_value(&t.display).into(),
            origin: t.origin,
            ..Default::default()
        })
        .collect())
}

impl Tools {
    /// `runtime_url` is the origin sandbox processes dial back to.
    pub fn new(connections: Connections, runtime_url: String) -> Self {
        Self {
            hosts: hosts::ToolHosts::new(connections.clone()),
            sandboxes: crate::sandboxes::Sandboxes::new(connections.clone(), runtime_url),
            memory: crate::memory::Memory::new(connections.clone()),
            wikis: crate::wikis::Wikis::new(connections.clone()),
            connections,
            probes: Default::default(),
            catalogs: Default::default(),
        }
    }
    pub async fn sources(&self, filter: Filter) -> Result<Vec<Source>, Error> {
        let client = self.connections.pool.get().await?;
        let rows = db::source_list_all(
            &client,
            filter.id,
            filter.agent,
            filter.connection,
            filter.tool_host,
            filter.blueprint,
        )
        .await?;
        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut tools = db::agent_tool_list_all(&client, &ids).await?;
        rows.into_iter()
            .map(|r| {
                Ok(Source {
                    tools: tools.extract_if(.., |t| t.source_id == r.id).collect(),
                    id: r.id,
                    owner: match (r.agent_id, r.sandbox_blueprint_id) {
                        (Some(id), None) => Owner::Agent(id),
                        (None, Some(id)) => Owner::Blueprint(id),
                        _ => return Err(invalid("Invalid tool source")),
                    },
                    target: match (r.connection_id, r.tool_host_id, r.sandbox) {
                        (Some(id), None, false) => Target::Connection(id),
                        (None, Some(id), false) => Target::ToolHost(id),
                        (None, None, true) => Target::Sandbox,
                        _ => return Err(invalid("Invalid tool source")),
                    },
                    slug: r.slug,
                })
            })
            .collect()
    }
    pub async fn source(&self, id: Uuid) -> Result<Source, Error> {
        self.sources(Filter {
            id: Some(id),
            ..Default::default()
        })
        .await?
        .pop()
        .ok_or(Error::NotFound)
    }
    /// The tools `target` offers, and the name its slug is derived from.
    async fn offered(&self, target: Target) -> Result<(Vec<types::ToolDefinition>, String), Error> {
        match target {
            Target::Connection(id) => {
                let row = db::tool_connection_opt(&self.connections.pool.get().await?, id)
                    .await?
                    .ok_or_else(|| {
                        invalid(
                            "Connection not found, personal, or its type does not provide tools",
                        )
                    })?;
                let name = source_name(&row.provider_id, &row.name);
                Ok((self.provider_tools(id).await?, name))
            }
            Target::ToolHost(id) => {
                let host = self.hosts.get(id).await?;
                if host.provider_id.is_some() {
                    return Err(invalid(
                        "This tool host needs credentials: use one of its instances",
                    ));
                }
                Ok((host.tools, host.name))
            }
            Target::Sandbox => Ok((crate::sandboxes::tools::definitions(), "sandbox".into())),
        }
    }
    /// Give an agent or a sandbox blueprint a source with `tool_names`. Its slug comes from the
    /// source's name, made unique among the owner's sources; an owner uses each source once. An
    /// agent's sandbox source comes only with its sandbox setting.
    pub async fn add_source(
        &self,
        owner: Owner,
        target: Target,
        tool_names: &[String],
    ) -> Result<Source, Error> {
        if target == Target::Sandbox {
            return Err(invalid("Give the agent a sandbox to add its sandbox tools"));
        }
        let (offered, name) = self.offered(target).await?;
        let mut named: Vec<(&str, String)> = Vec::new();
        for tool in tool_names {
            if !offered.iter().any(|t| &t.name == tool) {
                return Err(invalid("The source has no such tool"));
            }
            named.push((tool, tool_name_for(tool, &named)?));
        }
        let (connection, tool_host) = target.columns();
        let mut client = self.connections.pool.get().await?;
        let tx = client.transaction().await?;
        let (agent, blueprint) = owner.columns();
        if db::source_of_agent_opt(&tx, agent, connection, tool_host, blueprint)
            .await?
            .is_some()
        {
            return Err(invalid("This source is already in use here"));
        }
        let id = insert_source(&tx, owner, target, &slug_base(&name)).await?;
        for (tool, name) in &named {
            db::agent_tool_set_execute(&tx, id, tool, name, &ToolSettings::default()).await?;
        }
        tx.commit().await?;
        drop(client);
        self.source(id).await
    }
    /// The tools the agent ships in its code, as its latest invocation that registered any left
    /// them, with that invocation.
    pub async fn bundled(
        &self,
        agent: Uuid,
    ) -> Result<
        Option<(
            db::BundledToolsLatestRow,
            Vec<crate::proto::tilde::types::v1::ToolDefinition>,
        )>,
        Error,
    > {
        let client = self.connections.pool.get().await?;
        let Some(latest) = db::bundled_tools_latest_opt(&client, agent).await? else {
            return Ok(None);
        };
        let tools = db::bundled_tools_all(&client, latest.id)
            .await?
            .into_iter()
            .map(crate::chat::tools::dynamic::bundled_definition)
            .collect();
        Ok(Some((latest, tools)))
    }
    /// Whether the agent's tools are dynamic: kept out of the listed catalog, found with
    /// tools.search and called through tools.execute. One mode covers all the agent's tools.
    pub async fn mode(&self, agent: Uuid) -> Result<bool, Error> {
        let mode = db::tool_mode_get_opt(&self.connections.pool.get().await?, agent)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(mode == "dynamic")
    }
    pub async fn set_mode(&self, agent: Uuid, dynamic: bool) -> Result<(), Error> {
        match db::tool_mode_set_execute(
            &self.connections.pool.get().await?,
            agent,
            if dynamic { "dynamic" } else { "direct" },
        )
        .await?
        {
            0 => Err(Error::NotFound),
            _ => Ok(()),
        }
    }
    pub async fn remove_source(&self, source: Uuid) -> Result<(), Error> {
        match db::source_delete_execute(&self.connections.pool.get().await?, source).await? {
            0 => Err(Error::NotFound),
            _ => Ok(()),
        }
    }
    /// Add a tool of the source to the agent, or replace all of the agent's settings for it. An
    /// agent's sandbox tools are fixed: every one, as the sandbox defines it.
    pub async fn set_tool(
        &self,
        source: Uuid,
        tool_name: &str,
        settings: &ToolSettings,
    ) -> Result<Source, Error> {
        if settings.summary.chars().count() > SUMMARY_CHARS {
            return Err(invalid("A tool summary is at most 256 characters"));
        }
        if settings.description.chars().count() > DESCRIPTION_CHARS {
            return Err(invalid("A tool description is at most 4096 characters"));
        }
        let current = self.source(source).await?;
        if current.target == Target::Sandbox {
            return Err(fixed());
        }
        if !self
            .offered(current.target)
            .await?
            .0
            .iter()
            .any(|t| t.name == tool_name)
        {
            return Err(invalid("The source has no such tool"));
        }
        let others: Vec<(&str, String)> = current
            .tools
            .iter()
            .filter(|t| t.tool_name != tool_name)
            .map(|t| (t.tool_name.as_str(), t.name.clone()))
            .collect();
        let name = tool_name_for(tool_name, &others)?;
        db::agent_tool_set_execute(
            &self.connections.pool.get().await?,
            source,
            tool_name,
            &name,
            settings,
        )
        .await?;
        self.source(source).await
    }
    pub async fn remove_tool(&self, source: Uuid, tool_name: &str) -> Result<Source, Error> {
        if self.source(source).await?.target == Target::Sandbox {
            return Err(fixed());
        }
        db::agent_tool_remove_execute(&self.connections.pool.get().await?, source, tool_name)
            .await?;
        self.source(source).await
    }
    /// The tools a connection offers: its provider's, or for an MCP-served type the discovered
    /// snapshot (discovering once if there is none). Errors when its type offers no tools.
    pub async fn provider_tools(
        &self,
        connection: Uuid,
    ) -> Result<Vec<types::ToolDefinition>, Error> {
        let row = db::tool_connection_opt(&self.connections.pool.get().await?, connection)
            .await?
            .ok_or_else(|| {
                invalid("Connection not found, personal, or its type does not provide tools")
            })?;
        if let Some(host) = row.tool_host_id {
            return Ok(self.hosts.get(host).await?.tools);
        }
        if row.mcp {
            let stored = mcp::stored(&self.connections.pool, &row.provider_id, connection).await?;
            return if stored.is_empty() && row.status == "ready" {
                Ok(self.refresh_connection_tools(connection).await?.0)
            } else {
                Ok(stored)
            };
        }
        Ok(providers::provider(&row.provider_id, &row.type_id)
            .ok_or_else(|| invalid("No installed tool provider for this connection type"))?
            .tools())
    }
    /// What a catalog provider offers before any connection, across its connection types: its
    /// built-in tools, its tool host's, or its MCP server's (`mcp_catalog_tools`). Deduplicated
    /// by name.
    pub async fn catalog_tools(&self, provider: &str) -> Result<Vec<types::ToolDefinition>, Error> {
        let definition = self.connections.provider(provider).await?;
        let mut tools: Vec<types::ToolDefinition> = Vec::new();
        for typ in &definition.connection_types {
            let offered = if let Some(host) = db::host_of_provider_opt(
                &self.connections.pool.get().await?,
                provider,
                hosts::LIVENESS_SECS,
            )
            .await?
            {
                self.hosts.get(host.id).await?.tools
            } else if let Some(server) = &typ.mcp {
                self.mcp_catalog_tools(provider, &server.url, &definition.kind)
                    .await?
            } else {
                providers::provider(provider, &typ.id)
                    .map(|p| p.tools())
                    .unwrap_or_default()
            };
            for tool in offered {
                if !tools.iter().any(|t| t.name == tool.name) {
                    tools.push(tool);
                }
            }
        }
        Ok(tools)
    }
    /// What an MCP server offers, before a given connection lists it: what the installation's
    /// connections discovered, else what the server lists without credentials, else, for a
    /// curated server, its advertised snapshot, which can lag behind the server.
    async fn mcp_catalog_tools(
        &self,
        provider: &str,
        url: &str,
        kind: &ProviderKind,
    ) -> Result<Vec<types::ToolDefinition>, Error> {
        let discovered = mcp::provider_tools(&self.connections.pool, provider).await?;
        if !discovered.is_empty() {
            return Ok(discovered);
        }
        let key = url.to_owned();
        let cached = self
            .probes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .filter(|(at, _)| at.elapsed() < PROBE_TTL)
            .map(|(_, tools)| tools.clone());
        let probed = match cached {
            Some(tools) => tools,
            None => {
                let tools = mcp::probe(provider, url, &self.connections.endpoints).await;
                self.probes
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(key, (std::time::Instant::now(), tools.clone()));
                tools
            }
        };
        Ok(match probed {
            Some(tools) => tools,
            None if matches!(kind, ProviderKind::BuiltIn) => mcp_catalog::advertised(provider),
            None => Vec::new(),
        })
    }
    /// Sample whether each MCP server added by URL answers, for its health history beside tool
    /// hosts. One replica samples at a time; servers are checked concurrently, ten seconds each.
    pub async fn sample_mcp_health(&self) -> Result<(), Error> {
        let mut client = self.connections.pool.get().await?;
        let tx = client.transaction().await?;
        if !db::mcp_health_lock_one(&tx).await? {
            return Ok(());
        }
        let servers = db::mcp_servers_all(&tx).await?;
        let endpoints = &self.connections.endpoints;
        let checks = futures::future::join_all(
            servers
                .iter()
                .map(|server| mcp::reachable(&server.url, endpoints)),
        )
        .await;
        // A server with several URLs is healthy only when every one answers.
        let mut healthy: std::collections::BTreeMap<&str, bool> = Default::default();
        for (server, up) in servers.iter().zip(checks) {
            *healthy.entry(&server.provider_id).or_insert(true) &= up;
        }
        let now = chrono::Utc::now();
        for (provider, up) in healthy {
            db::mcp_health_sample_execute(&tx, provider, now, up).await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// Sample MCP server health every five minutes until shutdown; hourly buckets need no more.
    pub async fn run_mcp_health(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut poll = tokio::time::interval(std::time::Duration::from_secs(300));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                _ = poll.tick() => {
                    if let Err(error) = self.sample_mcp_health().await {
                        tracing::warn!(%error, "MCP server health sweep failed");
                    }
                }
            }
        }
    }
    /// Twelve hourly buckets of each MCP server's health, oldest first.
    pub async fn mcp_health(
        &self,
        providers: &[String],
    ) -> Result<std::collections::BTreeMap<String, Vec<types::AgentHealthHour>>, Error> {
        let rows = db::mcp_health_history_all(
            &self.connections.pool.get().await?,
            providers,
            chrono::Utc::now(),
        )
        .await?;
        let mut out: std::collections::BTreeMap<String, Vec<types::AgentHealthHour>> =
            Default::default();
        for row in rows {
            out.entry(row.provider_id)
                .or_default()
                .push(types::AgentHealthHour {
                    hour_start: buffa_types::google::protobuf::Timestamp {
                        seconds: row.hour_start.timestamp(),
                        ..Default::default()
                    }
                    .into(),
                    total_checks: row.total_checks as u32,
                    failed_checks: row.failed_checks as u32,
                    ..Default::default()
                });
        }
        Ok(out)
    }
    /// An MCP-served connection's tools: the stored discovery, discovering once if empty.
    pub(crate) async fn mcp_tools(
        &self,
        connection: Uuid,
        provider: &str,
        typ: &str,
    ) -> Result<Vec<types::ToolDefinition>, Error> {
        let stored = mcp::stored(&self.connections.pool, provider, connection).await?;
        if !stored.is_empty() {
            return Ok(stored);
        }
        let target = self.mcp_target(connection, provider, typ).await?;
        let values = self.connections.resolve(connection).await?;
        Ok(mcp::discover(
            &self.connections.pool,
            &self.connections.endpoints,
            &target,
            &values,
        )
        .await?
        .0)
    }
    /// The MCP server a connection's type names; its tools are served from there.
    pub(crate) async fn mcp_target(
        &self,
        connection: Uuid,
        provider: &str,
        typ: &str,
    ) -> Result<mcp::Target, Error> {
        let server = self
            .connections
            .provider(provider)
            .await?
            .connection_types
            .into_iter()
            .find(|t| t.id == typ)
            .and_then(|t| t.mcp)
            .ok_or_else(|| invalid("This connection type is not served by an MCP server"))?;
        Ok(mcp::Target {
            connection,
            provider: provider.to_owned(),
            server,
        })
    }
    /// Rediscover an MCP-served connection's tools; the flag reports whether they changed.
    pub async fn refresh_connection_tools(
        &self,
        connection: Uuid,
    ) -> Result<(Vec<types::ToolDefinition>, bool), Error> {
        let row = db::tool_connection_opt(&self.connections.pool.get().await?, connection)
            .await?
            .filter(|row| row.mcp)
            .ok_or_else(|| invalid("Only MCP-served connections discover their tools"))?;
        let target = self
            .mcp_target(row.id, &row.provider_id, &row.type_id)
            .await?;
        let values = self.connections.resolve(row.id).await?;
        mcp::discover(
            &self.connections.pool,
            &self.connections.endpoints,
            &target,
            &values,
        )
        .await
    }
    /// The invocation's catalog: loaded once per invocation (`Catalogs`).
    pub(crate) async fn catalog(&self, scope: &Scope) -> ToolResult<std::sync::Arc<Vec<Entry>>> {
        let cached = self
            .catalogs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&scope.id)
            .filter(|(at, _)| at.elapsed() < CATALOG_TTL)
            .map(|(_, entries)| entries.clone());
        if let Some(entries) = cached {
            return Ok(entries);
        }
        let entries = std::sync::Arc::new(self.load(Owner::Agent(scope.agent_id)).await?);
        let mut catalogs = self.catalogs.lock().unwrap_or_else(|e| e.into_inner());
        catalogs.retain(|_, (at, _)| at.elapsed() < CATALOG_TTL);
        catalogs.insert(scope.id, (std::time::Instant::now(), entries.clone()));
        Ok(entries)
    }
    /// An agent's or sandbox blueprint's tools on ready sources, as catalog definitions paired
    /// with what is needed to call them. A blueprint's are never deferred.
    pub(crate) async fn load(&self, owner: Owner) -> ToolResult<Vec<Entry>> {
        let (agent, blueprint) = owner.columns();
        let dynamic = match agent {
            Some(agent) => self.mode(agent).await?,
            None => false,
        };
        let client = self.connections.pool.get().await.map_err(Error::from)?;
        let rows = db::agent_functions_all(&client, agent, blueprint)
            .await
            .map_err(Error::from)?;
        let hosted = db::agent_host_functions_all(&client, agent, hosts::LIVENESS_SECS, blueprint)
            .await
            .map_err(Error::from)?;
        let proxied = db::agent_mcp_functions_all(&client, agent, blueprint)
            .await
            .map_err(Error::from)?;
        let sandboxed = match agent {
            Some(agent) => db::agent_sandbox_functions_all(&client, agent)
                .await
                .map_err(Error::from)?,
            None => vec![],
        };
        drop(client);
        let mut out = Vec::new();
        let fixed = crate::sandboxes::tools::definitions();
        for row in sandboxed {
            let Some(mut definition) = fixed.iter().find(|t| t.name == row.tool_name).cloned()
            else {
                continue;
            };
            definition.name = format!("{}.{}", row.slug, row.name);
            definition.detached = row.is_async;
            apply(&mut definition, row.summary, row.description, &row.display);
            out.push((definition, Callee::Sandbox(row.tool_name), dynamic));
        }
        out.extend(proxied.into_iter().map(|row| mcp_entry(row, dynamic)));
        for row in hosted {
            let mut definition = types::ToolDefinition {
                name: format!("{}.{}", row.slug, row.name),
                provider_id: "tool_host".into(),
                description: row.tool_description,
                summary: row.tool_summary,
                input_schema_json: row.input_schema_json,
                output_schema_json: row.output_schema_json,
                detached: row.is_async,
                annotations: types::ToolAnnotations {
                    read_only: row.read_only,
                    destructive: row.destructive,
                    idempotent: row.idempotent,
                    open_world: row.open_world,
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            };
            apply(&mut definition, row.summary, row.description, &row.display);
            let target = hosts::Target {
                host: row.tool_host_id,
                function_arn: row.function_arn,
                connection: row.connection_id.zip(row.connection_type),
            };
            out.push((definition, Callee::Host(target, row.tool_name), dynamic));
        }
        out.extend(
            rows.into_iter()
                .filter_map(|row| connection_entry(row, dynamic)),
        );
        Ok(out)
    }
}
/// A managed provider's tool as the agent's catalog offers it. A provider release may drop a
/// tool an agent still uses; it then disappears from the catalog instead of failing every
/// ListTools.
fn connection_entry(row: db::AgentFunctionRow, dynamic: bool) -> Option<Entry> {
    let mut definition = providers::provider(&row.provider_id, &row.type_id)
        .and_then(|p| p.tools().into_iter().find(|t| t.name == row.tool_name))?;
    definition.name = format!("{}.{}", row.slug, row.name);
    definition.provider_id = row.provider_id.clone();
    definition.detached = row.is_async;
    apply(
        &mut definition,
        row.summary.clone(),
        row.description.clone(),
        &row.display,
    );
    Some((definition, Callee::Connection(row), dynamic))
}
/// A discovered tool of an MCP-served connection as the agent's catalog offers it.
fn mcp_entry(row: db::AgentMcpFunctionRow, dynamic: bool) -> Entry {
    let mut definition = types::ToolDefinition {
        name: format!("{}.{}", row.slug, row.name),
        provider_id: row.provider_id.clone(),
        description: row.tool_description,
        input_schema_json: row.input_schema_json,
        output_schema_json: row.output_schema_json,
        detached: row.is_async,
        annotations: types::ToolAnnotations {
            read_only: row.read_only,
            destructive: row.destructive,
            idempotent: row.idempotent,
            open_world: row.open_world,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };
    apply(&mut definition, row.summary, row.description, &row.display);
    (
        definition,
        Callee::Mcp(
            row.connection_id,
            row.provider_id,
            row.type_id,
            row.tool_name,
        ),
        dynamic,
    )
}
/// Tool input is checked against the tool's own schema before any provider sees it.
pub(crate) fn validate(definition: &types::ToolDefinition, input: &Value) -> ToolResult<()> {
    let schema: Value = serde_json::from_str(&definition.input_schema_json)
        .map_err(|_| ConnectError::internal("Invalid provider schema"))?;
    if !jsonschema::validator_for(&schema)
        .map_err(|_| ConnectError::internal("Invalid provider schema"))?
        .is_valid(input)
    {
        return Err(ConnectError::invalid_argument(
            "Input does not match this tool's schema",
        ));
    }
    Ok(())
}
impl Tools {
    /// Call a connection's, MCP server's or tool host's tool. The caller has authorized the call
    /// and validated its input.
    pub(crate) async fn run(
        &self,
        callee: Callee,
        caller: Caller,
        input: Value,
    ) -> ToolResult<Value> {
        let row = match callee {
            Callee::Connection(row) => row,
            Callee::Mcp(connection, provider, typ, tool_name) => {
                let target = self.mcp_target(connection, &provider, &typ).await?;
                let values = self.connections.resolve(connection).await?;
                return mcp::call(
                    &self.connections.endpoints,
                    &target.server,
                    &values,
                    &tool_name,
                    input,
                )
                .await;
            }
            Callee::Host(target, tool_name) => {
                let id = |id: Option<Uuid>| id.map(|id| id.to_string()).unwrap_or_default();
                return self
                    .hosts
                    .call(
                        &target,
                        crate::proto::tilde::tool_host::v1::ToolCallRequest {
                            call_id: caller.call_id.to_string(),
                            name: tool_name,
                            input_json: input.to_string(),
                            agent_id: id(caller.agent),
                            thread_id: id(caller.thread),
                            sandbox_id: caller.sandbox.map(|s| s.to_string()),
                            ..Default::default()
                        },
                    )
                    .await;
            }
            Callee::Sandbox(_) => {
                return Err(ConnectError::internal(
                    "Sandbox tools run in the invocation",
                ));
            }
        };
        let provider = providers::provider(&row.provider_id, &row.type_id)
            .ok_or_else(|| ConnectError::not_found("No installed tool provider"))?;
        let access = self.connections.access(row.connection_id).await?;
        provider
            .invoke(&access, caller.call_id, &row.tool_name, input)
            .await
    }
}
impl Tools {
    /// The agent's tools a sidecar runs itself, for its replicated configuration, with the
    /// connections they need credentials for. Built from the same rows and wording as the
    /// gateway's catalog, without the sources only the gateway can reach.
    pub(crate) async fn sidecar_tools(
        &self,
        agent: Uuid,
    ) -> Result<(Vec<sidecar::SidecarTool>, Vec<Uuid>), Error> {
        let dynamic = self.mode(agent).await?;
        let client = self.connections.pool.get().await?;
        let rows = db::agent_functions_all(&client, Some(agent), None).await?;
        let proxied = db::agent_mcp_functions_all(&client, Some(agent), None).await?;
        drop(client);
        let entries = rows
            .into_iter()
            .filter_map(|row| connection_entry(row, dynamic))
            .chain(proxied.into_iter().map(|row| mcp_entry(row, dynamic)))
            .filter(|(_, callee, _)| callee.on_sidecar());
        let mut tools = Vec::new();
        let mut connections = Vec::new();
        for (definition, callee, deferred) in entries {
            let (connection, tool_name, mcp) = match callee {
                Callee::Connection(row) => (row.connection_id, row.tool_name, None),
                Callee::Mcp(connection, provider, typ, tool_name) => {
                    // A type no longer served by an MCP server drops its tools, not the rest.
                    let server = match self.mcp_target(connection, &provider, &typ).await {
                        Ok(target) => target.server,
                        Err(error) => {
                            tracing::warn!(%connection, %error, "MCP tool left out of a sidecar configuration");
                            continue;
                        }
                    };
                    let mcp = sidecar::McpServer {
                        credential: server.credential.as_str().into(),
                        credential_name: server.credential.name().unwrap_or_default().into(),
                        credential_prefix: server.credential.prefix().into(),
                        url: server.url,
                        ..Default::default()
                    };
                    (connection, tool_name, Some(mcp))
                }
                _ => continue,
            };
            if !connections.contains(&connection) {
                connections.push(connection);
            }
            tools.push(sidecar::SidecarTool {
                definition: definition.into(),
                deferred,
                connection_id: connection.to_string(),
                tool_name,
                mcp: mcp.into(),
                ..Default::default()
            });
        }
        Ok((tools, connections))
    }
    /// The invocation's tools only the gateway can run, listed and deferred, for a sidecar.
    pub(crate) async fn gateway_tools(
        &self,
        scope: &Scope,
    ) -> ToolResult<(Vec<types::ToolDefinition>, Vec<types::ToolDefinition>)> {
        let (mut listed, mut deferred) = (Vec::new(), Vec::new());
        for (definition, callee, dynamic) in self.catalog(scope).await?.iter() {
            if !callee.on_sidecar() {
                if *dynamic { &mut deferred } else { &mut listed }.push(definition.clone());
            }
        }
        Ok((listed, deferred))
    }
    /// Run one of those tools for a sidecar. The relay verified the invocation; its capability
    /// ceiling is checked again here as the sidecar's registry did.
    pub(crate) async fn call_gateway_tool(
        &self,
        scope: &Scope,
        call_id: Uuid,
        name: &str,
        input: Value,
    ) -> ToolResult<Value> {
        if !scope
            .capabilities
            .permits(crate::iam::capabilities::Capability::ToolsInvoke, name)
        {
            return Err(ConnectError::permission_denied(
                "Tool is not available to this agent",
            ));
        }
        let (definition, callee, _) = self
            .catalog(scope)
            .await?
            .iter()
            .find(|(definition, callee, _)| definition.name == name && !callee.on_sidecar())
            .cloned()
            .ok_or_else(|| ConnectError::not_found("Tool is not available to this agent"))?;
        validate(&definition, &input)?;
        self.dispatch(scope, call_id, callee, input).await
    }
    async fn dispatch(
        &self,
        scope: &Scope,
        call_id: Uuid,
        callee: Callee,
        input: Value,
    ) -> ToolResult<Value> {
        if let Callee::Sandbox(tool_name) = callee {
            return self.sandboxes.invoke(scope, &tool_name, input).await;
        }
        self.run(
            callee,
            Caller {
                call_id,
                agent: Some(scope.agent_id),
                thread: Some(scope.thread_id),
                run: Some(scope.run_id),
                sandbox: None,
            },
            input,
        )
        .await
    }
}
impl Provider for Tools {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            Ok(self
                .catalog(scope)
                .await?
                .iter()
                .filter(|(_, _, dynamic)| !dynamic)
                .map(|(definition, _, _)| definition.clone())
                .collect())
        })
    }
    fn deferred<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            Ok(self
                .catalog(scope)
                .await?
                .iter()
                .filter(|(_, _, dynamic)| *dynamic)
                .map(|(definition, _, _)| definition.clone())
                .collect())
        })
    }
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        input: Value,
        mut chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            // Managed tools take complete input; the registry already refused streamed frames.
            while let Some(chunk) = chunks.next().await {
                chunk?;
            }
            // The agent's use of the tool is checked against the invocation's catalog.
            let (definition, callee, _) = self
                .catalog(context.scope())
                .await?
                .iter()
                .find(|(definition, _, _)| definition.name == name)
                .cloned()
                .ok_or_else(|| ConnectError::not_found("Tool is not available to this agent"))?;
            validate(&definition, &input)?;
            context.authorize().await?;
            self.dispatch(context.scope(), context.call_id, callee, input)
                .await
        })
    }
}
