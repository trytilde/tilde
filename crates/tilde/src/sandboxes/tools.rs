//! The fixed sandbox tools an agent with a sandbox gets as its `sandbox` source. Each is one
//! operation run by the sandbox's `tilde sandbox connect` process, except apply_patch, which is
//! parsed here and applied through its file operations.
use super::Sandboxes;
use crate::chat::tools::ToolResult;
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::{
    rest::{Hints, definition},
    sandbox_patch::{self, Hunk},
};
use connectrpc::ConnectError;
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

const READ: Hints = Hints {
    read_only: true,
    destructive: false,
};
const WRITE: Hints = Hints {
    read_only: false,
    destructive: false,
};
/// File operations answer quickly; commands are given their own timeout plus this margin.
const OPERATION_TIMEOUT: Duration = Duration::from_secs(60);
const MARGIN: Duration = Duration::from_secs(30);

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn path(description: &str) -> Value {
    json!({"type":"string","minLength":1,"maxLength":4096,"description":description})
}

pub fn definitions() -> Vec<types::ToolDefinition> {
    let file = "A path in the sandbox; relative paths resolve against its working directory.";
    [
        (
            "exec",
            "Ran a sandbox command",
            "Run a shell command (bash -lc) in the sandbox and return its exit code and the last 256 KiB of stdout and stderr. `cwd` defaults to the sandbox's working directory. `timeout_ms` defaults to 120000 and is at most 600000; the command is killed when it passes. With `background`, a `job_id` is returned at once and exec_output reads the output.",
            schema(
                json!({
                    "command":{"type":"string","minLength":1,"maxLength":65536},
                    "cwd":path("Working directory for the command."),
                    "timeout_ms":{"type":"integer","minimum":1000,"maximum":600000},
                    "background":{"type":"boolean"}
                }),
                &["command"],
            ),
            WRITE,
        ),
        (
            "exec_output",
            "Read a background command",
            "Read a background command's output so far and whether it is still running, waiting up to `wait_ms` (default 0, at most 600000) for it to finish.",
            schema(
                json!({
                    "job_id":{"type":"string","minLength":1,"maxLength":64},
                    "wait_ms":{"type":"integer","minimum":0,"maximum":600000}
                }),
                &["job_id"],
            ),
            READ,
        ),
        (
            "read_file",
            "Read a sandbox file",
            "Read a UTF-8 text file. `offset` is the first line to read (1-based) and `limit` the number of lines; without them the whole file is read (at most 900 KiB).",
            schema(
                json!({
                    "path":path(file),
                    "offset":{"type":"integer","minimum":1},
                    "limit":{"type":"integer","minimum":1}
                }),
                &["path"],
            ),
            READ,
        ),
        (
            "write_file",
            "Wrote a sandbox file",
            "Create or replace a text file, creating its parent directories.",
            schema(
                json!({"path":path(file),"content":{"type":"string","maxLength":921600}}),
                &["path", "content"],
            ),
            WRITE,
        ),
        (
            "edit_file",
            "Edited a sandbox file",
            "Replace `old_string` with `new_string` in a text file. `old_string` must occur exactly once unless `replace_all` is set.",
            schema(
                json!({
                    "path":path(file),
                    "old_string":{"type":"string","minLength":1},
                    "new_string":{"type":"string"},
                    "replace_all":{"type":"boolean"}
                }),
                &["path", "old_string", "new_string"],
            ),
            WRITE,
        ),
        (
            "apply_patch",
            "Patched sandbox files",
            "Apply a patch to files in the sandbox. Format: '*** Begin Patch', then hunks '*** Add File: path' (+ lines), '*** Delete File: path', or '*** Update File: path' (optional '*** Move to: path', then '@@ context' chunks of ' ', '-', '+' lines), then '*** End Patch'.",
            schema(
                json!({"patch":{"type":"string","minLength":1,"maxLength":1048576}}),
                &["patch"],
            ),
            WRITE,
        ),
        (
            "list_dir",
            "Listed a sandbox directory",
            "List the entries directly under a directory (default: the working directory) with their kind (file, dir, symlink, other) and file sizes; at most 1000.",
            schema(json!({"path":path("The directory.")}), &[]),
            READ,
        ),
        (
            "glob",
            "Found sandbox files",
            "Find files matching a glob pattern such as `src/**/*.rs` under `path` (default: the working directory), respecting .gitignore; at most 1000 paths, relative to `path`.",
            schema(
                json!({"pattern":{"type":"string","minLength":1,"maxLength":1024},"path":path("The directory to search.")}),
                &["pattern"],
            ),
            READ,
        ),
        (
            "grep",
            "Searched sandbox files",
            "Search file contents for a regular expression under `path` (default: the working directory), respecting .gitignore and skipping binary files. `glob` limits the files searched. Returns each matching line's path, line number and text; at most `max_results` (default 200, at most 1000).",
            schema(
                json!({
                    "pattern":{"type":"string","minLength":1,"maxLength":1024},
                    "path":path("The file or directory to search."),
                    "glob":{"type":"string","minLength":1,"maxLength":1024},
                    "ignore_case":{"type":"boolean"},
                    "max_results":{"type":"integer","minimum":1,"maximum":1000}
                }),
                &["pattern"],
            ),
            READ,
        ),
    ]
    .into_iter()
    .map(|(name, summary, description, schema, hints)| {
        definition("sandbox", name, summary, description, schema, hints)
    })
    .collect()
}

/// Run one of the agent's sandbox tools in `sandbox`. Inputs were validated against the tools'
/// schemas, which the sandbox process's operations share.
pub(super) async fn invoke(
    sandboxes: &Sandboxes,
    sandbox: Uuid,
    invocation: Uuid,
    tool: &str,
    input: Value,
) -> ToolResult<Value> {
    let waits = |key: &str, default: u64| {
        Duration::from_millis(input[key].as_u64().unwrap_or(default)) + MARGIN
    };
    let timeout = match tool {
        "exec" if input["background"] == true => OPERATION_TIMEOUT,
        "exec" => waits("timeout_ms", 120_000),
        "exec_output" => waits("wait_ms", 0),
        "apply_patch" => return apply_patch(sandboxes, sandbox, invocation, &input).await,
        "read_file" | "write_file" | "edit_file" | "list_dir" | "glob" | "grep" => {
            OPERATION_TIMEOUT
        }
        _ => return Err(ConnectError::not_found("No such sandbox tool")),
    };
    sandboxes
        .operate(sandbox, invocation, tool, &input, timeout)
        .await
}

async fn apply_patch(
    sandboxes: &Sandboxes,
    sandbox: Uuid,
    invocation: Uuid,
    input: &Value,
) -> ToolResult<Value> {
    let operate = |name: &'static str, input: Value| async move {
        sandboxes
            .operate(sandbox, invocation, name, &input, OPERATION_TIMEOUT)
            .await
    };
    let read = |path: String| async move {
        let read = operate("read_file", json!({"path":path})).await?;
        Ok::<_, ConnectError>(read["content"].as_str().unwrap_or_default().to_owned())
    };
    let (mut added, mut modified, mut deleted) = (vec![], vec![], vec![]);
    for hunk in sandbox_patch::parse(input["patch"].as_str().unwrap_or_default())? {
        match hunk {
            Hunk::Add { path, contents } => {
                operate("write_file", json!({"path":path,"content":contents})).await?;
                added.push(path);
            }
            Hunk::Delete { path } => {
                operate("delete_file", json!({"path":path})).await?;
                deleted.push(path);
            }
            Hunk::Update {
                path,
                move_to,
                chunks,
            } => {
                let updated = sandbox_patch::update(&read(path.clone()).await?, &path, &chunks)?;
                let target = move_to.clone().unwrap_or_else(|| path.clone());
                operate("write_file", json!({"path":target,"content":updated})).await?;
                match move_to {
                    Some(to) => {
                        operate("delete_file", json!({"path":path})).await?;
                        added.push(to);
                        deleted.push(path);
                    }
                    None => modified.push(path),
                }
            }
        }
    }
    Ok(json!({"added":added,"modified":modified,"deleted":deleted}))
}
