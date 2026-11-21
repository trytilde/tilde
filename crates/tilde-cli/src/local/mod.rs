//! `tilde local`: connects a harness on this machine to its local agent in a Tilde registry.
//!
//! A local agent is one Tilde never runs: it lives in Claude Code, Codex or Claude Desktop on
//! someone's machine. `install` stores the agent's key here and wires the harness to this
//! command: its hooks call `hook`, which reports each session's transcript to Tilde and keeps
//! the agent's skills in the harness's own skill folder, and its MCP configuration starts
//! `mcp`, which serves the agent's tools. Nothing runs in the background; the harness starts
//! all of it.
//!
//! Each harness is an adapter of three small parts: where its files live (`install`), how its
//! transcript reads (`transcript`), and which of sessions, skills and tools it supports. The
//! registry declares the same features per harness, and its pages follow them. ChatGPT Desktop
//! takes remote MCP servers only, so it has nothing to install here: its agent page gives the
//! server URL to add in ChatGPT.
mod hook;
mod install;
mod mcp;
mod skills;
mod transcript;

use crate::api::{base_url, transport};
use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use connectrpc::client::{ClientConfig, HttpClient};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tilde_contracts::services::tilde::local::v1::LocalAgentServiceClient;

#[derive(Args)]
pub struct Local {
    #[command(subcommand)]
    command: LocalCommand,
}

#[derive(Subcommand)]
enum LocalCommand {
    /// Connect a harness on this machine to its local agent.
    Install(install::Install),
    /// Disconnect a harness: its hooks, its MCP server and the stored key.
    Uninstall { harness: Harness },
    /// Run by the harness's hooks, with the hook's payload on stdin.
    #[command(hide = true)]
    Hook { harness: Harness },
    /// Run by the harness as its MCP server (stdio).
    #[command(hide = true)]
    Mcp { harness: Harness },
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Harness {
    ClaudeCode,
    Codex,
    ClaudeDesktop,
}
impl Harness {
    fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::ClaudeDesktop => "claude-desktop",
        }
    }
    /// Whether the harness keeps transcripts on this machine and runs hooks: its sessions are
    /// reported and its skills installed. Claude Desktop only takes the agent's tools.
    fn has_hooks(self) -> bool {
        matches!(self, Self::ClaudeCode | Self::Codex)
    }
}

const HOOK_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

fn home() -> Result<PathBuf> {
    std::env::home_dir().context("This machine has no home directory")
}
/// Where `tilde local` keeps each harness's connection and session cursors.
fn state_dir() -> Result<PathBuf> {
    Ok(match std::env::var_os("TILDE_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => home()?.join(".tilde"),
    }
    .join("local"))
}

/// A harness's connection to its agent, as `install` stored it.
#[derive(Serialize, Deserialize)]
struct Connection {
    url: String,
    agent_id: String,
    /// The agent's application key. The file is readable by its owner only.
    key: String,
}
impl Connection {
    fn path(harness: Harness) -> Result<PathBuf> {
        Ok(state_dir()?.join(format!("{}.json", harness.name())))
    }
    fn load(harness: Harness) -> Result<Self> {
        let path = Self::path(harness)?;
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "{} is not connected; run `tilde local install {0}`",
                harness.name()
            )
        })?;
        serde_json::from_str(&text).with_context(|| format!("{} is not valid", path.display()))
    }
    fn save(&self, harness: Harness) -> Result<()> {
        let path = Self::path(harness)?;
        std::fs::create_dir_all(path.parent().context("state directory")?)?;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        use std::io::Write as _;
        let written = options.open(&path).and_then(|mut file| {
            // A file that was already there keeps its mode unless it is set.
            #[cfg(unix)]
            file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
            file.write_all(serde_json::to_string_pretty(self)?.as_bytes())
        });
        written.with_context(|| format!("Could not write {}", path.display()))
    }
    /// The agent's ingress, which serves its local service and its MCP endpoint.
    fn base(&self) -> String {
        format!("{}/agents/{}", self.url, self.agent_id)
    }
    fn bearer(&self) -> Result<http::HeaderValue> {
        let mut value = http::HeaderValue::from_str(&format!("Bearer {}", self.key))
            .context("The agent key is not a valid header value")?;
        value.set_sensitive(true);
        Ok(value)
    }
    fn client(&self) -> Result<LocalAgentServiceClient<HttpClient>> {
        let base = base_url(&self.base())?;
        let mut headers = http::HeaderMap::new();
        headers.insert(http::header::AUTHORIZATION, self.bearer()?);
        let config = ClientConfig::new(base.parse().context("The gateway URL is not usable")?)
            .with_default_headers(headers);
        Ok(LocalAgentServiceClient::new(transport(&base)?, config))
    }
}

pub async fn run(args: Local) -> Result<()> {
    match args.command {
        LocalCommand::Install(args) => install::install(args).await,
        LocalCommand::Uninstall { harness } => install::uninstall(harness),
        LocalCommand::Hook { harness } => {
            // A hook must never get in the harness's way: it reports the failure and succeeds.
            if !harness.has_hooks() {
                bail!("{} has no hooks", harness.name());
            }
            // Nor may it hold the harness up: an unreachable gateway costs one deadline.
            match tokio::time::timeout(HOOK_DEADLINE, hook::run(harness)).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => eprintln!("tilde local: {error:#}"),
                Err(_) => eprintln!("tilde local: Tilde did not answer in time"),
            }
            Ok(())
        }
        LocalCommand::Mcp { harness } => mcp::serve(harness).await,
    }
}
