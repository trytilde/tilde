//! Local agents: agents that run in a harness on a person's machine (Claude Code, Codex, the
//! Claude and ChatGPT desktop apps). Tilde never runs or wakes one, so it has no deployments,
//! inference, prompts or channels. Its harness, through the `tilde local` hooks installed in
//! it, reports sessions (`LocalAgentService.Ingest`), installs the agent's skills
//! (`ListSkills`, `ReadSkillFile`) and is served the agent's tools over MCP (`mcp`).
//!
//! Everything here is served on the agent's ingress under the agent's application key (the
//! `api_key` of its Tilde connection): the harness is one more application speaking for the
//! agent. Each harness declares what its adapter supports ([`Harness::features`]); the web
//! app shows only those parts of the agent.
//!
//! Not built yet: logs (no harness's OpenTelemetry export is accepted), and adapters that read
//! an account instead of a machine (claude.ai and ChatGPT conversations), which would report
//! through the same `Ingest`.
pub(crate) mod db;
mod ingest;
mod mcp;
mod rpc;

use crate::proto::tilde::types::v1 as types;
use crate::{chat::Chat, deployment::Deployments, error::Error};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

const SERVICE: &str = "tilde.local.v1.LocalAgentService/";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Harness {
    ClaudeCode,
    Codex,
    ClaudeDesktop,
    ChatgptDesktop,
}
impl Harness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude_code",
            Self::Codex => "codex",
            Self::ClaudeDesktop => "claude_desktop",
            Self::ChatgptDesktop => "chatgpt_desktop",
        }
    }
    pub(crate) fn parse(value: &str) -> Option<Self> {
        [
            Self::ClaudeCode,
            Self::Codex,
            Self::ClaudeDesktop,
            Self::ChatgptDesktop,
        ]
        .into_iter()
        .find(|harness| harness.as_str() == value)
    }
    /// `None` for a managed agent (UNSPECIFIED).
    pub fn from_wire(value: Option<types::Harness>) -> Result<Option<Self>, Error> {
        Ok(Some(match value {
            Some(types::Harness::HARNESS_UNSPECIFIED) => return Ok(None),
            Some(types::Harness::HARNESS_CLAUDE_CODE) => Self::ClaudeCode,
            Some(types::Harness::HARNESS_CODEX) => Self::Codex,
            Some(types::Harness::HARNESS_CLAUDE_DESKTOP) => Self::ClaudeDesktop,
            Some(types::Harness::HARNESS_CHATGPT_DESKTOP) => Self::ChatgptDesktop,
            None => return Err(Error::Invalid("Unknown harness".into())),
        }))
    }
    /// What the harness's adapter supports. The coding harnesses keep transcripts on the
    /// machine and run hooks, so their sessions are reported and their skills installed; the
    /// desktop apps keep conversations in the vendor's account and only take MCP tools.
    pub fn features(self) -> types::HarnessFeatures {
        let coding = matches!(self, Self::ClaudeCode | Self::Codex);
        types::HarnessFeatures {
            sessions: coding,
            traces: coding,
            logs: false,
            tools: true,
            skills: coding,
            ..Default::default()
        }
    }
    pub fn wire(self) -> types::LocalAgent {
        types::LocalAgent {
            harness: match self {
                Self::ClaudeCode => types::Harness::HARNESS_CLAUDE_CODE,
                Self::Codex => types::Harness::HARNESS_CODEX,
                Self::ClaudeDesktop => types::Harness::HARNESS_CLAUDE_DESKTOP,
                Self::ChatgptDesktop => types::Harness::HARNESS_CHATGPT_DESKTOP,
            }
            .into(),
            features: self.features().into(),
            ..Default::default()
        }
    }
}

/// The local agent a request was authenticated for.
#[derive(Clone)]
struct Caller {
    agent: Uuid,
    harness: Harness,
    /// The agent's Tilde connection, which its threads and identities belong to.
    connection: Uuid,
}

/// Whether `path` below `/agents/{agent}/` is served here.
pub(crate) fn serves(path: &str) -> bool {
    path == "mcp" || path.starts_with(SERVICE)
}

/// One request of a local agent's harness: its application key, then the service or MCP.
pub(crate) async fn serve(
    deployments: &Deployments,
    chat: &Chat,
    agent: Uuid,
    path: &str,
    request: axum::extract::Request,
) -> Result<Response, Error> {
    let key = request
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(Error::Denied)?;
    let client = deployments.pool.get().await?;
    let connection = deployments.application_key(&client, agent, key).await?;
    let harness = db::harness_opt(&client, agent)
        .await?
        .ok_or(Error::Denied)?;
    drop(client);
    let caller = Caller {
        agent,
        harness,
        connection,
    };
    if path == "mcp" {
        return Ok(mcp::handle(chat, &caller, request).await);
    }
    let (mut parts, body) = request.into_parts();
    parts.uri = format!("/{path}").parse().map_err(|_| Error::Denied)?;
    parts.extensions.insert(caller);
    use tower::ServiceExt as _;
    Ok(rpc::router(deployments.clone(), chat.clone())
        .oneshot(axum::extract::Request::from_parts(parts, body))
        .await
        .into_response())
}
