//! The built-in skill tools: what an agent's runtime `SkillService` does (`WriteSkill`,
//! `AssignSkill`, `UnassignSkill`), as tools a model calls. Each is offered only when the agent's
//! capabilities make it usable and authorizes exactly like its RPC: `skills.write` needs
//! `skills.edit` on the editor source; `skills.assign` needs `agents.edit_skills` on the receiving
//! agent plus `skills.read` (or `skills.edit`) on the source; `skills.unassign` needs
//! `agents.edit_skills` alone. Gateway-only, like the other built-in tools.
use super::{Skills, Upload};
use crate::chat::Scope;
use crate::chat::tools::{Context, InputStream, Provider, ToolResult};
use crate::iam::capabilities::Capability;
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::rest::{Hints, definition};
use connectrpc::ConnectError;
use futures::{StreamExt, future::BoxFuture};
use serde_json::{Value, json};

pub const WRITE: &str = "skills.write";
pub const ASSIGN: &str = "skills.assign";
pub const UNASSIGN: &str = "skills.unassign";
/// Files one `skills.write` may send: text only, each at most the inline limit.
const MAX_FILES: usize = 64;
const MAX_CONTENT: usize = 256 * 1024;

pub struct SkillTools(pub Skills);

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn slug() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"description":"A skill source's slug."})
}
/// What `skills.assign` and `skills.unassign` take: an agent and a whole source or one skill.
fn assignment(verb: &str) -> Value {
    json!({
        "type":"object",
        "properties":{
            "agent_id":{"type":"string","format":"uuid","description":format!("The agent to {verb}; your own ID for yourself.")},
            "source":slug(),
            "skill":{"type":"string","minLength":3,"maxLength":200,"description":"One skill, as source/name."}
        },
        // Exactly one of `source` and `skill`, checked by the call: model APIs refuse a
        // top-level oneOf.
        "required":["agent_id"],
        "additionalProperties":false
    })
}

/// The agent's skill tools; listing and calling both go through here.
fn definitions(scope: &Scope) -> Vec<types::ToolDefinition> {
    let can = |capability| scope.capabilities.grants(capability);
    let change = Hints {
        read_only: false,
        destructive: false,
    };
    let mut out = Vec::new();
    if can(Capability::SkillsEdit) {
        out.push(definition(
            "tilde",
            WRITE,
            "Wrote a skill",
            "Create a skill in an editor skill source you may edit, or replace it: the files you send are the whole skill. SKILL.md is required and starts with front matter (`name:`, and `description:`, the one line an agent sees before deciding to read it), then the Markdown instructions. Sending the same files again changes nothing. Agents given the source get the new version on their next invocation, with no redeploy; give it with skills.assign.",
            schema(
                json!({
                    "source":slug(),
                    "name":{"type":"string","pattern":"^[a-z0-9][a-z0-9_-]{0,63}$","description":"The skill's name: lowercase letters, digits, '-' and '_'."},
                    "files":{"type":"array","minItems":1,"maxItems":MAX_FILES,"items":{"type":"object","properties":{
                        "path":{"type":"string","minLength":1,"maxLength":512,"description":"Relative to the skill's folder, e.g. SKILL.md or scripts/check.sh."},
                        "content":{"type":"string","maxLength":MAX_CONTENT},
                        "executable":{"type":"boolean"}
                    },"required":["path","content"],"additionalProperties":false}},
                    "message":{"type":"string","maxLength":500,"description":"What changed, kept with the version."}
                }),
                &["source", "name", "files"],
            ),
            change,
        ));
    }
    if can(Capability::AgentsEditSkills)
        && (can(Capability::SkillsRead) || can(Capability::SkillsEdit))
    {
        out.push(definition(
            "tilde",
            ASSIGN,
            "Gave skills to an agent",
            "Give an agent a whole skill source (switched on: every skill in it, including ones written later) or one skill of it. Giving what it already has changes nothing.",
            assignment("give the skills to"),
            change,
        ));
    }
    if can(Capability::AgentsEditSkills) {
        out.push(definition(
            "tilde",
            UNASSIGN,
            "Took skills from an agent",
            "Take a skill source, with every skill given from it, or one skill away from an agent. Taking what it does not have changes nothing.",
            assignment("take the skills from"),
            Hints {
                read_only: false,
                destructive: true,
            },
        ));
    }
    out
}

/// Whether the agent may see and assign the source's skills; `skills.edit` implies `skills.read`.
fn require_readable(scope: &Scope, source: uuid::Uuid) -> ToolResult<()> {
    let id = source.to_string();
    if scope.capabilities.permits(Capability::SkillsRead, &id)
        || scope.capabilities.permits(Capability::SkillsEdit, &id)
    {
        Ok(())
    } else {
        Err(ConnectError::permission_denied(
            "Capability does not permit this operation",
        ))
    }
}

impl SkillTools {
    async fn call(&self, scope: &Scope, name: &str, input: Value) -> ToolResult<Value> {
        // Listing and calling share one gate: a tool absent from the list cannot be called.
        let definition = definitions(scope)
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
        let text = |field: &str| input[field].as_str().unwrap_or_default();
        match name {
            WRITE => {
                let (source, _) = self.0.source_by_slug(text("source")).await?;
                scope
                    .capabilities
                    .require(Capability::SkillsEdit, &source.to_string())?;
                let files = input["files"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|f| Upload::Data {
                        path: f["path"].as_str().unwrap_or_default().to_owned(),
                        data: f["content"]
                            .as_str()
                            .unwrap_or_default()
                            .as_bytes()
                            .to_vec(),
                        executable: f["executable"].as_bool().unwrap_or(false),
                    })
                    .collect();
                let (_, written) = self
                    .0
                    .write(source, text("name"), files, text("message"), false)
                    .await?;
                Ok(json!({
                    "skill": format!("{}/{}", text("source"), text("name")),
                    "version": written.number,
                    "created": written.created,
                }))
            }
            ASSIGN | UNASSIGN => {
                let pool = &self.0.pool;
                let agent = crate::tools::builtin::acted_on(
                    pool,
                    scope,
                    text("agent_id"),
                    Capability::AgentsEditSkills,
                )
                .await?;
                let assign = name == ASSIGN;
                match (input["source"].as_str(), input["skill"].as_str()) {
                    (Some(slug), None) => {
                        let (source, _) = self.0.source_by_slug(slug).await?;
                        if assign {
                            require_readable(scope, source)?;
                            // A whole source means every skill in it, switched on.
                            self.0.assign_source(agent, source).await?;
                            self.0.enable_source(agent, source, true).await?;
                        } else {
                            self.0.unassign_source(agent, source).await?;
                        }
                    }
                    (None, Some(address)) => {
                        let (source, skill) = self.0.skill_by_address(address).await?;
                        if assign {
                            require_readable(scope, source)?;
                            self.0.assign_skill(agent, skill).await?;
                        } else {
                            self.0.unassign_skill(agent, skill).await?;
                        }
                    }
                    _ => {
                        return Err(ConnectError::invalid_argument("Name a source or a skill"));
                    }
                }
                Ok(json!({"agent_id": agent, "assigned": assign}))
            }
            _ => Err(ConnectError::not_found("Unknown skill tool")),
        }
    }
}

impl Provider for SkillTools {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move { Ok(definitions(scope)) })
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
            self.call(context.scope(), name, input).await
        })
    }
}
