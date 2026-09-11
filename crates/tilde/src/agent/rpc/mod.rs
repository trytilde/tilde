use crate::proto::tilde::types::v1 as types;
pub mod management;
pub mod runtime;
use crate::{agent::Agents, error::Error};
use uuid::Uuid;
pub(super) fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Agent ID must be a UUID".into()))
}
pub(super) fn wire(
    agent: crate::agent::Agent,
    metrics: Option<types::AgentMetrics>,
    avatar_url: Option<String>,
) -> types::Agent {
    let timestamp =
        |date: chrono::DateTime<chrono::Utc>| buffa_types::google::protobuf::Timestamp {
            seconds: date.timestamp(),
            nanos: date.timestamp_subsec_nanos() as i32,
            ..Default::default()
        };
    let mut result = types::Agent {
        avatar_seed: agent.avatar_seed.to_string(),
        avatar_url,
        paused: agent.paused,
        capabilities: agent.capabilities.wire().into(),
        id: agent.id.to_string(),
        name: agent.name,
        endpoint_url: agent.endpoint_url,
        created_at: timestamp(agent.created_at).into(),
        updated_at: timestamp(agent.updated_at).into(),
        ..Default::default()
    };
    if let Some(metrics) = metrics {
        result.metrics = metrics.into();
    }
    result
}

pub(super) async fn project(
    agents: &Agents,
    agent: crate::agent::Agent,
) -> Result<types::Agent, Error> {
    let metrics = agents.metrics(&[agent.id]).await?.remove(&agent.id);
    let avatar_url = agents.avatar_url(&agent).await?;
    Ok(wire(agent, metrics, avatar_url))
}
