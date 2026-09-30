//! Modal sandboxes: create, inspect and terminate sandboxes in the connection's workspace, and
//! work inside one through its task command router. Every file operation is a shell command in
//! the sandbox (`bash -lc`), so a sandbox needs only a POSIX userland; file contents cross as
//! base64 inside the command line, which bounds a single write to what one argument can carry
//! (about 96 KiB). Each call looks up the sandbox's task and router access afresh.
mod grpc;
use super::sandbox_patch as patch;

use super::{
    ToolProvider,
    rest::{Hints, definition as tool},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{
    categories::{CATEGORY_DEVELOPER_TOOLS, CATEGORY_SANDBOX},
    model,
};
use crate::proto::tilde::types::v1 as types;
use base64::Engine as _;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use grpc::*;
use serde_json::{Value, json};
use std::time::Duration;

const DEFAULT_APP: &str = "tilde-sandbox-tools";
const FILE_TIMEOUT_MS: u64 = 10_000;

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("Modal workspace".into()),
        icon_url: Some("/provider-icons/modal_sandbox.svg".into()),
        instructions: Some(
            "Modal sandbox management, filesystem, patch and command execution. Create a token with `modal token new` or in the Modal dashboard under Settings > API Tokens."
                .into(),
        ),
        id: "modal".into(),
        name: "Modal".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_SANDBOX.into(), CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Modal token".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "api_key_id":{"type":"string","title":"Token ID","minLength":1,"writeOnly":true,"description":"Usually starts with ak-."},
                    "api_key_secret":{"type":"string","title":"Token secret","minLength":1,"writeOnly":true,"description":"Usually starts with as-."},
                    "workspace_id":{"type":"string","title":"Workspace ID","minLength":1,"description":"Usually starts with ac-."}
                },"required":["api_key_id","api_key_secret","workspace_id"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool],
        }],
    }
}

pub struct Modal;

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn sandbox_id() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"description":"ID returned by create_sandbox (sb-…)."})
}
fn path() -> Value {
    json!({"type":"string","minLength":1,"maxLength":4096,"description":"Absolute path inside the sandbox."})
}
const READ: Hints = Hints {
    read_only: true,
    destructive: false,
};
const WRITE: Hints = Hints {
    read_only: false,
    destructive: false,
};
const DESTROY: Hints = Hints {
    read_only: false,
    destructive: true,
};

impl ToolProvider for Modal {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        // A tool on an existing sandbox.
        let on = |name: &str,
                  summary: &str,
                  description: &str,
                  mut properties: Value,
                  required: &[&str],
                  hints: Hints| {
            properties["sandbox_id"] = sandbox_id();
            let mut all = vec!["sandbox_id"];
            all.extend_from_slice(required);
            tool(
                "modal",
                name,
                summary,
                description,
                schema(properties, &all),
                hints,
            )
        };
        vec![
            tool(
                "modal",
                "create_sandbox",
                "Created a Modal sandbox",
                "Create a Modal sandbox from a Python 3.13 Debian image with build tools and uv. It stops after `timeout_secs` (default 300).",
                schema(
                    json!({
                        "app_name":{"type":"string","minLength":1,"maxLength":64,"description":"Modal app that owns the sandbox; created if missing. Defaults to tilde-sandbox-tools."},
                        "timeout_secs":{"type":"integer","minimum":1,"maximum":86400}
                    }),
                    &[],
                ),
                WRITE,
            ),
            on(
                "get_sandbox",
                "Checked a Modal sandbox",
                "Wait until a sandbox is running and return its task ID.",
                json!({}),
                &[],
                READ,
            ),
            on(
                "terminate_sandbox",
                "Terminated a Modal sandbox",
                "Terminate a sandbox. Its filesystem is lost.",
                json!({}),
                &[],
                DESTROY,
            ),
            on(
                "read_file",
                "Read a sandbox file",
                "Read a UTF-8 text file.",
                json!({"path":path()}),
                &["path"],
                READ,
            ),
            on(
                "write_file",
                "Wrote a sandbox file",
                "Create or overwrite a text file, creating parent directories.",
                json!({"path":path(),"contents":{"type":"string","maxLength":65536}}),
                &["path", "contents"],
                WRITE,
            ),
            on(
                "delete_file",
                "Deleted a sandbox file",
                "Delete a file. Deleting a missing file succeeds.",
                json!({"path":path()}),
                &["path"],
                DESTROY,
            ),
            on(
                "list_dir",
                "Listed a sandbox directory",
                "List the entries directly under a directory (default /tmp) with their kind (file, dir, symlink, other) and file sizes.",
                json!({"path":path()}),
                &[],
                READ,
            ),
            on(
                "stat",
                "Inspected a sandbox path",
                "Report whether a path exists, its kind and size.",
                json!({"path":path()}),
                &["path"],
                READ,
            ),
            on(
                "exec_command",
                "Ran a sandbox command",
                "Run a shell command with bash -lc and return its exit code, stdout and stderr. `timed_out` is set when the command was killed at its timeout (default 60 s).",
                json!({
                    "cmd":{"type":"string","minLength":1,"maxLength":65536},
                    "workdir":path(),
                    "timeout_ms":{"type":"integer","minimum":1000,"maximum":3600000}
                }),
                &["cmd"],
                WRITE,
            ),
            on(
                "apply_patch",
                "Patched sandbox files",
                "Apply a patch in the apply_patch format: '*** Begin Patch', then hunks '*** Add File: path' (lines prefixed +), '*** Delete File: path' or '*** Update File: path' (optionally '*** Move to: path', then @@ sections of ' ', '-' and '+' lines), then '*** End Patch'.",
                json!({"patch":{"type":"string","minLength":1,"maxLength":262144}}),
                &["patch"],
                WRITE,
            ),
        ]
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let mut api = Api::new(access)?;
            if name == "create_sandbox" {
                let timeout = input["timeout_secs"].as_u64().unwrap_or(300) as u32;
                let app = input["app_name"].as_str().unwrap_or(DEFAULT_APP);
                return create(&mut api, app, timeout).await;
            }
            let id = text(&input, "sandbox_id")?;
            match name {
                "get_sandbox" => {
                    return Ok(json!({"sandbox_id": id, "task_id": task(&mut api, id).await?}));
                }
                "terminate_sandbox" => {
                    let _: SandboxTerminateResponse = api
                        .unary(
                            "SandboxTerminateV2",
                            SandboxTerminateRequest {
                                sandbox_id: id.into(),
                            },
                            true,
                        )
                        .await?;
                    return Ok(json!({"terminated": true}));
                }
                "read_file" | "write_file" | "delete_file" | "list_dir" | "stat"
                | "exec_command" | "apply_patch" => {}
                _ => return Err(ConnectError::not_found("Unsupported Modal tool")),
            }
            let sandbox = Sandbox::open(api, id).await?;
            match name {
                "read_file" => Ok(json!({"contents": sandbox.read(text(&input, "path")?).await?})),
                "write_file" => {
                    let contents = text(&input, "contents")?;
                    sandbox.write(text(&input, "path")?, contents).await?;
                    Ok(json!({"bytes_written": contents.len()}))
                }
                "delete_file" => {
                    sandbox.delete(text(&input, "path")?).await?;
                    Ok(json!({"deleted": true}))
                }
                "list_dir" => {
                    let path = input["path"].as_str().unwrap_or("/tmp");
                    let out = sandbox
                        .check(
                            "list directory",
                            format!(
                                "find {} -mindepth 1 -maxdepth 1 -printf '%f\\t%y\\t%s\\n'",
                                quote(path)
                            ),
                        )
                        .await?;
                    let entries: Vec<Value> = out
                        .lines()
                        .filter(|line| !line.is_empty())
                        .map(|line| {
                            let mut parts = line.splitn(3, '\t');
                            let name = parts.next().unwrap_or_default();
                            let kind = kind(parts.next().unwrap_or_default());
                            let size = parts
                                .next()
                                .and_then(|s| s.parse::<u64>().ok())
                                .filter(|_| kind == "file");
                            json!({"name": name, "kind": kind, "size": size})
                        })
                        .collect();
                    Ok(json!({"entries": entries}))
                }
                "stat" => {
                    let out = sandbox.check("stat path", format!(
                        "p={}; if [ -e \"$p\" ] || [ -L \"$p\" ]; then t=o; [ -d \"$p\" ] && t=d; [ -L \"$p\" ] && t=l; [ -f \"$p\" ] && t=f; s=$(stat -c %s -- \"$p\"); printf '1\\t%s\\t%s\\n' \"$t\" \"$s\"; else printf '0\\t\\t0\\n'; fi",
                        quote(text(&input, "path")?)
                    )).await?;
                    let mut parts = out.lines().next().unwrap_or_default().splitn(3, '\t');
                    Ok(if parts.next() == Some("1") {
                        json!({"exists": true, "kind": kind(parts.next().unwrap_or_default()), "size": parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0)})
                    } else {
                        json!({"exists": false, "kind": "", "size": 0})
                    })
                }
                "exec_command" => {
                    let exec = sandbox
                        .exec(
                            text(&input, "cmd")?,
                            input["workdir"].as_str(),
                            input["timeout_ms"].as_u64(),
                        )
                        .await?;
                    Ok(
                        json!({"exit_code": exec.exit_code, "stdout": exec.stdout, "stderr": exec.stderr, "timed_out": exec.timed_out}),
                    )
                }
                _ => apply_patch(&sandbox, text(&input, "patch")?).await,
            }
        })
    }
}
fn text<'a>(input: &'a Value, key: &str) -> ToolResult<&'a str> {
    input[key]
        .as_str()
        .ok_or_else(|| ConnectError::invalid_argument(format!("`{key}` is required")))
}
fn kind(code: &str) -> &'static str {
    match code {
        "d" => "dir",
        "l" => "symlink",
        "f" => "file",
        _ => "other",
    }
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
fn failed(result: &Option<GenericResult>) -> Option<String> {
    result
        .as_ref()
        .filter(|r| r.status > 1)
        .map(|r| r.exception.chars().take(2048).collect())
}

/// The app (created if missing), the default image (built if missing) and the sandbox.
async fn create(api: &mut Api<'_>, app: &str, timeout_secs: u32) -> ToolResult<Value> {
    let app: AppGetOrCreateResponse = api
        .unary(
            "AppGetOrCreate",
            AppGetOrCreateRequest {
                app_name: app.into(),
                environment_name: String::new(),
                object_creation_type: 1,
            },
            false,
        )
        .await?;
    let image: ImageGetOrCreateResponse = api
        .unary(
            "ImageGetOrCreate",
            ImageGetOrCreateRequest {
                image: Some(Image {
                    dockerfile_commands: [
                        "FROM python:3.13.3-slim-bookworm",
                        "RUN apt-get update",
                        "RUN apt-get install -y gcc gfortran build-essential",
                        "RUN pip install --upgrade pip wheel uv",
                        "RUN echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections",
                        "CMD [\"sleep\", \"172800\"]",
                    ]
                    .map(String::from)
                    .into(),
                    version: "2025.06".into(),
                    image_registry_config: Some(ImageRegistryConfig::default()),
                }),
                app_id: app.app_id.clone(),
                force_build: false,
                namespace: 3,
                builder_version: "2025.06".into(),
            },
            false,
        )
        .await?;
    if let Some(error) = failed(&image.result) {
        return Err(ConnectError::unknown(format!(
            "Modal image build failed: {error}"
        )));
    }
    if image.result.as_ref().is_none_or(|r| r.status == 0) {
        // Building: follow the build log stream until Modal reports a result.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(900);
        let mut last_entry_id = String::new();
        'build: loop {
            if tokio::time::Instant::now() > deadline {
                return Err(ConnectError::deadline_exceeded(
                    "The Modal image is still building",
                ));
            }
            let updates: Vec<ImageJoinStreamingResponse> = api
                .call(
                    "ImageJoinStreaming",
                    ImageJoinStreamingRequest {
                        image_id: image.image_id.clone(),
                        timeout: 55.0,
                        last_entry_id: last_entry_id.clone(),
                        include_logs_for_finished: true,
                    },
                    false,
                    grpc::UNARY_TIMEOUT,
                )
                .await?;
            for update in updates {
                if let Some(error) = failed(&update.result) {
                    return Err(ConnectError::unknown(format!(
                        "Modal image build failed: {error}"
                    )));
                }
                if update.result.is_some_and(|r| r.status == 1) {
                    break 'build;
                }
                if !update.entry_id.is_empty() {
                    last_entry_id = update.entry_id;
                }
            }
        }
    }
    let created: SandboxCreateV2Response = api
        .unary(
            "SandboxCreateV2",
            SandboxCreateV2Request {
                app_id: app.app_id,
                definition: Some(grpc::Sandbox {
                    image_id: image.image_id,
                    timeout_secs,
                }),
            },
            true,
        )
        .await?;
    Ok(json!({"sandbox_id": created.sandbox_id, "task_id": created.task_id}))
}
async fn task(api: &mut Api<'_>, sandbox_id: &str) -> ToolResult<String> {
    let response: SandboxGetTaskIdResponse = api
        .unary(
            "SandboxGetTaskIdV2",
            SandboxGetTaskIdRequest {
                sandbox_id: sandbox_id.into(),
                timeout: Some(55.0),
                wait_until_ready: true,
            },
            true,
        )
        .await?;
    response.task_id.filter(|t| !t.is_empty()).ok_or_else(|| {
        ConnectError::failed_precondition(format!("Modal sandbox {sandbox_id} is not running"))
    })
}

struct Sandbox<'a> {
    api: Api<'a>,
    task_id: String,
    router: RouterAccess,
}
struct Exec {
    exit_code: i32,
    stdout: String,
    stderr: String,
    timed_out: bool,
}
impl<'a> Sandbox<'a> {
    async fn open(mut api: Api<'a>, id: &str) -> ToolResult<Self> {
        let task_id = task(&mut api, id).await?;
        let access: SandboxGetCommandRouterAccessResponse = api
            .unary(
                "SandboxGetCommandRouterAccess",
                SandboxGetCommandRouterAccessRequest {
                    sandbox_id: id.into(),
                },
                true,
            )
            .await?;
        Ok(Self {
            api,
            task_id,
            router: RouterAccess {
                url: access.url,
                jwt: access.jwt.into(),
            },
        })
    }
    /// Start the command, drain stdout then stderr (each stream ends with the process), then
    /// read its exit status.
    async fn exec(
        &self,
        cmd: &str,
        workdir: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> ToolResult<Exec> {
        let timeout_secs = timeout_ms.map_or(60, |ms| (ms / 1000).max(1) as u32);
        let wait = Duration::from_secs(u64::from(timeout_secs) + 30);
        let exec_id = uuid::Uuid::new_v4().to_string();
        let _: Vec<TaskExecStartResponse> = self
            .api
            .router(
                &self.router,
                "TaskExecStart",
                TaskExecStartRequest {
                    task_id: self.task_id.clone(),
                    exec_id: exec_id.clone(),
                    command_args: vec!["bash".into(), "-lc".into(), cmd.into()],
                    stdout_config: 1,
                    stderr_config: 1,
                    timeout_secs: Some(timeout_secs),
                    workdir: workdir.map(Into::into),
                },
                grpc::UNARY_TIMEOUT,
            )
            .await?;
        let mut output = [Vec::new(), Vec::new()];
        for (fd, out) in output.iter_mut().enumerate() {
            let chunks: Vec<TaskExecStdioReadResponse> = self
                .api
                .router(
                    &self.router,
                    "TaskExecStdioRead",
                    TaskExecStdioReadRequest {
                        task_id: self.task_id.clone(),
                        exec_id: exec_id.clone(),
                        offset: 0,
                        file_descriptor: fd as i32,
                    },
                    wait,
                )
                .await?;
            *out = chunks.into_iter().flat_map(|c| c.data).collect();
        }
        let status: Vec<TaskExecWaitResponse> = self
            .api
            .router(
                &self.router,
                "TaskExecWait",
                TaskExecWaitRequest {
                    task_id: self.task_id.clone(),
                    exec_id,
                },
                wait,
            )
            .await?;
        let status = status
            .into_iter()
            .next()
            .ok_or_else(|| ConnectError::unknown("Modal returned no exit status"))?;
        let [stdout, stderr] = output;
        Ok(Exec {
            exit_code: status
                .code
                .or(status.signal.map(|signal| 128 + signal))
                .unwrap_or(1),
            // Killed by SIGKILL without an exit code: the command hit its timeout.
            timed_out: status.code.is_none() && status.signal == Some(9) && timeout_ms.is_some(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    }
    /// Run a file operation, failing with its stderr on a non-zero exit.
    async fn check(&self, op: &str, cmd: String) -> ToolResult<String> {
        let exec = self.exec(&cmd, None, Some(FILE_TIMEOUT_MS)).await?;
        if exec.exit_code != 0 {
            return Err(ConnectError::unknown(format!(
                "Failed to {op} (exit {}): {}",
                exec.exit_code,
                exec.stderr.chars().take(2048).collect::<String>()
            )));
        }
        Ok(exec.stdout)
    }
    async fn read(&self, path: &str) -> ToolResult<String> {
        let out = self
            .check("read file", format!("base64 -w0 -- {}", quote(path)))
            .await?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(out.trim())
            .map_err(|_| ConnectError::unknown("Modal returned unreadable file bytes"))?;
        String::from_utf8(bytes)
            .map_err(|_| ConnectError::invalid_argument(format!("{path} is not a UTF-8 text file")))
    }
    async fn write(&self, path: &str, contents: &str) -> ToolResult<()> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(contents);
        let mkdir = match path.rsplit_once('/') {
            Some((parent, _)) if !parent.is_empty() => format!("mkdir -p -- {} && ", quote(parent)),
            _ => String::new(),
        };
        self.check(
            "write file",
            format!(
                "{mkdir}base64 -d > {} <<'__TILDE_EOF__'\n{encoded}\n__TILDE_EOF__",
                quote(path)
            ),
        )
        .await
        .map(drop)
    }
    async fn delete(&self, path: &str) -> ToolResult<()> {
        self.check("delete file", format!("rm -f -- {}", quote(path)))
            .await
            .map(drop)
    }
}

async fn apply_patch(sandbox: &Sandbox<'_>, text: &str) -> ToolResult<Value> {
    let hunks = patch::parse(text)?;
    let (mut added, mut modified, mut deleted) = (vec![], vec![], vec![]);
    for hunk in hunks {
        match hunk {
            patch::Hunk::Add { path, contents } => {
                sandbox.write(&path, &contents).await?;
                added.push(path);
            }
            patch::Hunk::Delete { path } => {
                sandbox.delete(&path).await?;
                deleted.push(path);
            }
            patch::Hunk::Update {
                path,
                move_to,
                chunks,
            } => {
                let original = sandbox.read(&path).await?;
                let updated = patch::update(&original, &path, &chunks)?;
                match move_to {
                    Some(to) => {
                        sandbox.write(&to, &updated).await?;
                        sandbox.delete(&path).await?;
                        added.push(to);
                        deleted.push(path);
                    }
                    None => {
                        sandbox.write(&path, &updated).await?;
                        modified.push(path);
                    }
                }
            }
        }
    }
    Ok(json!({"added": added, "modified": modified, "deleted": deleted}))
}
