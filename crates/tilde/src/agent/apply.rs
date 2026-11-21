//! Declarative apply of an agent's own fields (see `crate::apply`). The agent is found by key,
//! else by the spec's ID (an agent made by hand is then given the key), else created.
use super::{Agents, ConcurrencyPolicy, CreateAgent, validate_description, validate_name};
use crate::apply::Change;
use crate::database::Transaction;
use crate::error::Error;
use crate::iam::capabilities::Capabilities;
use crate::proto::tilde::management::v1 as m;
use uuid::Uuid;

pub(crate) async fn agent(
    tx: &Transaction<'_>,
    agents: &Agents,
    spec: &m::AgentSpec,
) -> Result<(Uuid, Change), Error> {
    // Absent capabilities, like absent policy and tool mode, keep the agent's own.
    let capabilities = spec
        .capabilities
        .as_option()
        .map(|wire| {
            let capabilities = Capabilities::from_wire(wire.clone())?;
            capabilities.validate()?;
            Ok::<_, Error>(capabilities)
        })
        .transpose()?;
    let to_json = |capabilities: &Capabilities| {
        serde_json::to_value(capabilities)
            .map_err(|_| Error::Invalid("Invalid capabilities".into()))
    };
    let name = validate_name(&spec.name)?;
    let description = validate_description(&spec.description)?;
    let policy = spec
        .concurrency_policy
        .map(|p| ConcurrencyPolicy::from_wire(p.to_i32()))
        .transpose()?;
    let mode = match spec.tool_mode.and_then(|mode| mode.as_known()) {
        Some(m::ToolMode::Dynamic) => Some("dynamic"),
        Some(m::ToolMode::Direct) => Some("direct"),
        _ => None,
    };
    let id = spec
        .id
        .as_deref()
        .map(|id| Uuid::parse_str(id).map_err(|_| Error::Invalid("Invalid agent ID".into())))
        .transpose()?;
    let key = spec.key.as_str();
    let (id, change, policy, mode, caps) = match super::db::apply_get_opt(tx, key, id).await? {
        Some(row) => {
            let caps = match &capabilities {
                Some(capabilities) => to_json(capabilities)?,
                None => row.capabilities.clone(),
            };
            // Absent policy and tool mode keep the agent's own.
            let policy = policy.map_or(row.concurrency_policy.clone(), |p| p.as_str().into());
            let mode = mode.map_or(row.tool_mode.clone(), String::from);
            let mut change = Change::existing("agent", key);
            change.field("key", row.key.as_deref().unwrap_or_default(), key);
            change.field("name", &row.name, &name);
            change.field("description", &row.description, &description);
            // JSON equality: stored objects come back with jsonb's key order.
            if row.capabilities != caps {
                change.field("capabilities", &row.capabilities, &caps);
            }
            change.field("concurrency_policy", &row.concurrency_policy, &policy);
            change.field("tool_mode", &row.tool_mode, &mode);
            if !change.changed() {
                return Ok((row.id, change));
            }
            (row.id, change, policy, mode, caps)
        }
        None => {
            let policy = policy.unwrap_or_default();
            let capabilities = capabilities.unwrap_or_default();
            let caps = to_json(&capabilities)?;
            let input = CreateAgent {
                concurrency_policy: policy,
                capabilities,
                id: id.unwrap_or_else(Uuid::new_v4),
                name: name.clone(),
                description: description.clone(),
            };
            let agent = agents
                .insert(tx, &input, &name, &description)
                .await?
                .ok_or_else(|| Error::Exists("Another agent has this agent's ID".into()))?;
            let mut change = Change::create("agent", key);
            change.field("name", "", &name);
            let mode = mode.unwrap_or("direct").to_owned();
            (agent.id, change, policy.as_str().to_owned(), mode, caps)
        }
    };
    super::db::apply_update_execute(tx, key, &name, &description, &policy, &mode, &caps, id)
        .await?;
    Ok((id, change))
}
