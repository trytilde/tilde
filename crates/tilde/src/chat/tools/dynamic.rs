//! Dynamic discovery. Deferred tools stay out of the listed catalog; the agent searches them by
//! words, reads the schemas it needs and calls them through `tools.execute`, which the registry
//! rewrites into the inner call. Ranking is lexical on purpose: it needs no model, no embedding
//! store and no round trip, so it behaves identically wherever a catalog is assembled.
use super::{Context, Provider, ToolResult, audit};
use crate::chat::{Chat, Scope};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use secrecy::SecretString;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

pub const SEARCH: &str = "tools.search";
pub const SCHEMAS: &str = "tools.schemas";
pub const EXECUTE: &str = "tools.execute";

fn meta(
    name: &str,
    summary: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
) -> types::ToolDefinition {
    types::ToolDefinition {
        name: name.into(),
        provider_id: "tilde".into(),
        description: description.into(),
        summary: summary.into(),
        input_schema_json: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}).to_string(),
        annotations: types::ToolAnnotations { read_only, idempotent: read_only, ..Default::default() }.into(),
        ..Default::default()
    }
}
pub fn definitions() -> Vec<types::ToolDefinition> {
    vec![
        meta(
            SEARCH,
            "Looked for a tool",
            "Find tools by what they do. More tools exist than are listed; search before concluding something cannot be done.",
            json!({"query":{"type":"string","minLength":1,"maxLength":400},"limit":{"type":"integer","minimum":1,"maximum":25}}),
            &["query"],
            true,
        ),
        meta(
            SCHEMAS,
            "Read tool details",
            "Get the full input schema of tools found with tools.search, before calling them.",
            json!({"names":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":10}}),
            &["names"],
            true,
        ),
        meta(
            EXECUTE,
            "Ran a tool",
            "Call a tool found with tools.search by its exact name, with input matching its schema.",
            json!({"name":{"type":"string","minLength":1,"maxLength":128},"input":{"type":"object"}}),
            &["name", "input"],
            false,
        ),
    ]
}
/// The bundled tools the agent process registered for this invocation; the engine never runs them.
pub async fn bundled(chat: &Chat, scope: &Scope) -> ToolResult<Vec<types::ToolDefinition>> {
    if chat.local().is_some() {
        return Ok(vec![]);
    }
    Ok(crate::tools::db::bundled_tools_all(
        &chat
            .pg()?
            .get()
            .await
            .map_err(crate::chat::ChatError::from)?,
        scope.id,
    )
    .await
    .map_err(crate::chat::ChatError::from)?
    .into_iter()
    .map(bundled_definition)
    .collect())
}
/// A stored bundled tool as its definition.
pub fn bundled_definition(t: crate::tools::db::BundledToolRow) -> types::ToolDefinition {
    types::ToolDefinition {
        name: t.name,
        provider_id: "bundled".into(),
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
        display: audit::display_value(&t.display).into(),
        ..Default::default()
    }
}
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(str::to_ascii_lowercase)
        .collect()
}
/// A query word counts most in the name, then the summary, then the description. Prefixes match
/// so "search" finds "searching"; nothing else is stemmed.
fn score(query: &[String], tool: &types::ToolDefinition) -> u32 {
    let hit = |haystack: &[String], word: &String| {
        haystack
            .iter()
            .any(|h| h.starts_with(word.as_str()) || word.starts_with(h.as_str()))
    };
    let (name, summary, description) = (
        words(&tool.name),
        words(&tool.summary),
        words(&tool.description),
    );
    query
        .iter()
        .map(|w| {
            4 * u32::from(hit(&name, w))
                + 2 * u32::from(hit(&summary, w))
                + u32::from(hit(&description, w))
        })
        .sum()
}
/// `tools.search` and `tools.schemas`: read-only, audited like any call.
pub(super) async fn answer(
    chat: &Chat,
    scope: &Scope,
    call_id: Uuid,
    capability: SecretString,
    name: &str,
    input: &Value,
    entries: Vec<(types::ToolDefinition, Arc<dyn Provider>, bool)>,
) -> ToolResult<Value> {
    let definition = definitions()
        .into_iter()
        .find(|d| d.name == name)
        .ok_or_else(|| ConnectError::not_found("Unknown discovery tool"))?;
    let context = Context {
        chat: chat.clone(),
        scope: scope.clone(),
        call_id,
        capability,
    };
    let execution = audit::Execution::begin(&context, &definition, input).await?;
    let mut candidates: Vec<(types::ToolDefinition, bool)> = entries
        .into_iter()
        .filter(|(_, _, deferred)| *deferred)
        .map(|(d, _, _)| (d, false))
        .collect();
    candidates.extend(bundled(chat, scope).await?.into_iter().map(|d| (d, true)));
    let mut result = if name == SEARCH {
        let query = words(input["query"].as_str().unwrap_or_default());
        let mut ranked: Vec<_> = candidates
            .iter()
            .map(|(tool, bundled)| (score(&query, tool), tool, *bundled))
            .filter(|(score, _, _)| *score > 0)
            .collect();
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
        let limit = input["limit"].as_u64().unwrap_or(8).clamp(1, 25) as usize;
        Ok(json!({
            "tools": ranked.into_iter().take(limit).map(|(score, tool, bundled)| json!({
                "name": tool.name, "summary": tool.summary, "description": tool.description, "score": score,
                "runs": if bundled { "bundled with the agent: call it directly" } else { "through tools.execute" },
            })).collect::<Vec<_>>(),
            "searched": candidates.len(),
        }))
    } else {
        let wanted: Vec<&str> = input["names"]
            .as_array()
            .map(|n| n.iter().filter_map(Value::as_str).take(10).collect())
            .unwrap_or_default();
        if wanted.is_empty() {
            Err(ConnectError::invalid_argument("Tool names are required"))
        } else {
            Ok(
                json!({"tools": candidates.iter().filter(|(t, _)| wanted.contains(&t.name.as_str())).map(|(t, bundled)| json!({
                "name": t.name, "description": t.description,
                "input_schema": serde_json::from_str::<Value>(&t.input_schema_json).unwrap_or(Value::Null),
                "output_schema": serde_json::from_str::<Value>(&t.output_schema_json).unwrap_or(Value::Null),
                "read_only": t.annotations.as_option().is_some_and(|a| a.read_only),
                "destructive": t.annotations.as_option().is_some_and(|a| a.destructive),
                "background": t.detached, "bundled": bundled,
            })).collect::<Vec<_>>()}),
            )
        }
    };
    execution.finish(&mut result).await?;
    result
}

impl Chat {
    /// Replace this invocation's bundled tool definitions; the SDK calls it again whenever the
    /// agent's own tools change mid-run. Names that collide with the catalog would make
    /// `tools.execute` ambiguous, so reserved prefixes are refused here.
    pub async fn register_bundled_tools(
        &self,
        scope: &Scope,
        tools: Vec<types::ToolDefinition>,
    ) -> crate::chat::Result<()> {
        use crate::chat::ChatError;
        if self.local().is_some() {
            return Ok(());
        }
        if !crate::tools::bundled_valid(&tools) {
            return Err(ChatError::Invalid(crate::tools::BUNDLED_INVALID.into()));
        }
        let mut client = self.pg()?.get().await?;
        let tx = client.transaction().await?;
        crate::tools::db::bundled_tools_clear_execute(&tx, scope.id).await?;
        for tool in &tools {
            crate::tools::db::bundled_tool_insert_execute(
                &tx,
                scope.id,
                tool,
                audit::display_text(tool.display),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
