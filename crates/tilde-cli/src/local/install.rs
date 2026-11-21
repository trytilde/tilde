//! Wiring a harness to `tilde local`, and unwiring it. Each harness keeps its hooks and MCP
//! servers its own way:
//! - Claude Code: hooks in `~/.claude/settings.json`; MCP servers through `claude mcp`.
//! - Codex: hooks in `~/.codex/hooks.json`, which Codex runs only once the person has trusted
//!   them (`/hooks`); MCP servers through `codex mcp`.
//! - Claude Desktop: MCP servers in `claude_desktop_config.json`; it has no hooks.
//!
//! The harness is given this executable's path and a subcommand, never the agent's key: the
//! key stays in `tilde local`'s own file.
use super::{Connection, Harness, home, state_dir};
use anyhow::{Context, Result, bail};
use clap::Args;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct Install {
    harness: Harness,
    /// The gateway the agent is registered on.
    #[arg(long, env = "TILDE_URL")]
    url: String,
    /// The local agent this harness is, from its page in the registry.
    #[arg(long, env = "TILDE_AGENT_ID")]
    agent_id: String,
    /// The agent's key, from its page in the registry. Asked for when absent.
    #[arg(long, env = "TILDE_AGENT_KEY", hide_env_values = true)]
    key: Option<String>,
}

/// The harness's own configuration directory.
pub fn harness_dir(harness: Harness) -> Result<PathBuf> {
    let from_env = |name: &str| std::env::var_os(name).map(PathBuf::from);
    Ok(match harness {
        Harness::ClaudeCode => from_env("CLAUDE_CONFIG_DIR").unwrap_or(home()?.join(".claude")),
        Harness::Codex => from_env("CODEX_HOME").unwrap_or(home()?.join(".codex")),
        Harness::ClaudeDesktop => {
            if cfg!(target_os = "macos") {
                home()?.join("Library/Application Support/Claude")
            } else if cfg!(windows) {
                from_env("APPDATA")
                    .context("APPDATA is not set")?
                    .join("Claude")
            } else {
                from_env("XDG_CONFIG_HOME")
                    .unwrap_or(home()?.join(".config"))
                    .join("Claude")
            }
        }
    })
}
/// A name safe as one path segment: what skill names and agent ids are.
pub fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The hook events `tilde local hook` handles for a harness.
fn events(harness: Harness) -> &'static [&'static str] {
    match harness {
        Harness::ClaudeCode => &["SessionStart", "Stop", "SessionEnd"],
        // Codex gives SessionEnd one second; every turn's Stop already reports the session.
        Harness::Codex => &["SessionStart", "Stop"],
        Harness::ClaudeDesktop => &[],
    }
}
fn hooks_file(harness: Harness) -> Result<PathBuf> {
    Ok(harness_dir(harness)?.join(match harness {
        Harness::Codex => "hooks.json",
        _ => "settings.json",
    }))
}
/// The harness's own CLI and its arguments to add or remove the `tilde` MCP server.
fn mcp_cli(harness: Harness, add: Option<&str>) -> (&'static str, Vec<String>) {
    let words = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    let server = |mut args: Vec<String>| {
        if let Some(exe) = add {
            args.extend(words(&["--", exe, "local", "mcp", harness.name()]));
        }
        args
    };
    let verb = if add.is_some() { "add" } else { "remove" };
    match harness {
        Harness::Codex => ("codex", server(words(&["mcp", verb, "tilde"]))),
        _ => (
            "claude",
            server(words(&["mcp", verb, "--scope", "user", "tilde"])),
        ),
    }
}

pub async fn install(args: Install) -> Result<()> {
    let harness = args.harness;
    if !plain_name(&args.agent_id) {
        bail!("--agent-id is the agent's ID from its page in the registry");
    }
    let key = match args.key {
        Some(key) => key,
        None => {
            eprint!("Agent key: ");
            let mut line = String::new();
            std::io::stdin().read_line(&mut line)?;
            line
        }
    };
    let connection = Connection {
        url: crate::api::base_url(&args.url)?,
        agent_id: args.agent_id,
        key: key.trim().to_owned(),
    };
    // One call the gateway answers only for this agent's key, before anything is changed.
    let answered = reqwest::Client::new()
        .post(format!("{}/mcp", connection.base()))
        .header(http::header::AUTHORIZATION, connection.bearer()?)
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}))
        .send()
        .await
        .with_context(|| format!("{} is unreachable", connection.url))?;
    if !answered.status().is_success() {
        bail!(
            "{} refused the key for this agent ({}). Check the agent ID and key on the agent's \
             page, and that it is a local agent.",
            connection.url,
            answered.status()
        );
    }
    connection.save(harness)?;
    let exe = std::env::current_exe().context("Could not find this executable")?;
    let exe = exe.to_string_lossy();
    if harness.has_hooks() {
        let command = format!("\"{exe}\" local hook {}", harness.name());
        set_hooks(&hooks_file(harness)?, harness, Some(&command))?;
        let skills = harness_dir(harness)?.join("skills");
        if let Err(error) = super::skills::sync(&connection, &skills).await {
            eprintln!("Skills were not installed ({error:#}); the next session retries.");
        }
    }
    if harness == Harness::ClaudeDesktop {
        let server = json!({"command": exe, "args": ["local", "mcp", harness.name()]});
        set_desktop_server(&harness_dir(harness)?, Some(server))?;
    } else {
        let _ = run(mcp_cli(harness, None));
        let (program, arguments) = mcp_cli(harness, Some(&exe));
        if !run((program, arguments.clone())) {
            eprintln!(
                "Could not add the MCP server. Add it yourself with:\n  {program} {}",
                arguments.join(" ")
            );
        }
    }
    eprintln!(
        "Connected {} to agent {}.",
        harness.name(),
        connection.agent_id
    );
    match harness {
        Harness::Codex => {
            eprintln!("Codex runs new hooks only once you trust them: open Codex and run /hooks.")
        }
        Harness::ClaudeDesktop => eprintln!("Restart Claude Desktop to load the agent's tools."),
        Harness::ClaudeCode => eprintln!("New Claude Code sessions report to Tilde."),
    }
    Ok(())
}

pub fn uninstall(harness: Harness) -> Result<()> {
    if harness.has_hooks() {
        set_hooks(&hooks_file(harness)?, harness, None)?;
        // The skills this command installed carry its marker.
        let skills = harness_dir(harness)?.join("skills");
        for entry in std::fs::read_dir(skills).into_iter().flatten().flatten() {
            if entry.path().join(".tilde").is_file() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    if harness == Harness::ClaudeDesktop {
        set_desktop_server(&harness_dir(harness)?, None)?;
    } else {
        let _ = run(mcp_cli(harness, None));
    }
    let _ = std::fs::remove_file(Connection::path(harness)?);
    let _ = std::fs::remove_dir_all(state_dir()?.join("sessions").join(harness.name()));
    eprintln!("Disconnected {}.", harness.name());
    Ok(())
}

fn run((program, arguments): (&str, Vec<String>)) -> bool {
    std::process::Command::new(program)
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// A JSON settings file as an object; one that does not exist is empty.
fn read(path: &Path) -> Result<Map<String, Value>> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Map::new()),
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("{} is not a JSON object", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(error) => Err(error).with_context(|| format!("Could not read {}", path.display())),
    }
}
/// Written beside the file and moved over it, so the harness never reads half a file. A
/// symlinked settings file (a dotfiles checkout) is written where it points.
fn write(path: &Path, root: Map<String, Value>) -> Result<()> {
    std::fs::create_dir_all(path.parent().context("settings directory")?)?;
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    let staged = target.with_extension("tilde-tmp");
    let mut text = serde_json::to_string_pretty(&root)?;
    text.push('\n');
    std::fs::write(&staged, text)
        .and_then(|()| std::fs::rename(&staged, &target))
        .with_context(|| format!("Could not write {}", path.display()))
}

/// Put `command` on each of the harness's hook events (or, with none, take it off), leaving
/// every other hook in the file as it is. Claude Code and Codex share the shape:
/// `{"hooks": {"<Event>": [{"hooks": [{"type": "command", "command": "..."}]}]}}`.
fn set_hooks(path: &Path, harness: Harness, command: Option<&str>) -> Result<()> {
    let mut root = read(path)?;
    if command.is_none() && !root.contains_key("hooks") {
        return Ok(());
    }
    let ours = format!("local hook {}", harness.name());
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .with_context(|| format!("\"hooks\" in {} is not an object", path.display()))?;
    for event in events(harness) {
        let mut groups = match hooks.remove(*event) {
            Some(Value::Array(groups)) => groups,
            None => vec![],
            Some(_) => bail!("\"hooks.{event}\" in {} is not a list", path.display()),
        };
        groups.retain(|group| {
            !group["hooks"].as_array().is_some_and(|handlers| {
                handlers
                    .iter()
                    .any(|h| h["command"].as_str().is_some_and(|c| c.contains(&ours)))
            })
        });
        if let Some(command) = command {
            groups.push(json!({"hooks": [{"type": "command", "command": command}]}));
        }
        if !groups.is_empty() {
            hooks.insert(event.to_string(), Value::Array(groups));
        }
    }
    if hooks.is_empty() {
        root.remove("hooks");
    }
    write(path, root)
}

/// Set or remove the `tilde` entry of Claude Desktop's `mcpServers`.
fn set_desktop_server(dir: &Path, server: Option<Value>) -> Result<()> {
    let path = dir.join("claude_desktop_config.json");
    if server.is_none() && !path.exists() {
        return Ok(());
    }
    let mut root = read(&path)?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .with_context(|| format!("\"mcpServers\" in {} is not an object", path.display()))?;
    match server {
        Some(server) => servers.insert("tilde".into(), server),
        None => servers.remove("tilde"),
    };
    write(&path, root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_are_added_once_and_removed_leaving_the_file_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let theirs = json!({"theme": "dark", "hooks": {"Stop": [
            {"hooks": [{"type": "command", "command": "notify-send done"}]}]}, "alwaysThinking": true});
        let original = format!("{}\n", serde_json::to_string_pretty(&theirs).unwrap());
        std::fs::write(&path, &original).unwrap();
        let command = "\"/bin/tilde\" local hook claude-code";
        set_hooks(&path, Harness::ClaudeCode, Some(command)).unwrap();
        // Installing again (a moved binary, say) replaces the entry instead of doubling it.
        set_hooks(&path, Harness::ClaudeCode, Some(command)).unwrap();
        let installed: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let stop = installed["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0]["hooks"][0]["command"], "notify-send done");
        assert_eq!(stop[1]["hooks"][0]["command"], command);
        assert_eq!(
            installed["hooks"]["SessionStart"].as_array().unwrap().len(),
            1
        );

        // Their hook, their other settings and the order of their keys are untouched.
        set_hooks(&path, Harness::ClaudeCode, None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
}
