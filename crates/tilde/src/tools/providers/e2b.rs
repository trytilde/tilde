//! E2B sandboxes: management over the E2B REST API and file and command operations inside a
//! running sandbox through its envd daemon. Each sandbox operation first connects the sandbox
//! (`POST /sandboxes/{id}/connect`, which also extends its TTL) for an envd access token, then
//! calls `https://49983-{id}.{sandboxDomain}`: `/files` for file bytes and Connect RPC
//! (`filesystem.Filesystem/*`, `process.Process/Start`) for the rest.
use super::{
    ToolProvider,
    rest::{self, Rest, Verb},
    sandbox_patch::{self, Hunk},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{
    categories::{CATEGORY_DEVELOPER_TOOLS, CATEGORY_SANDBOX},
    model,
};
use crate::proto::tilde::types::v1 as types;
use base64::Engine;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Map, Value, json};

const BASE: &str = "https://api.e2b.app";

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("API key name".into()),
        icon_url: Some("/provider-icons/e2b_sandbox.svg".into()),
        instructions: Some(
            "Connect E2B so agents can create sandboxes, edit files and run commands in them. Create an API key (e2b_...) in the E2B dashboard."
                .into(),
        ),
        id: "e2b".into(),
        name: "E2B".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_SANDBOX.into(), CATEGORY_DEVELOPER_TOOLS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "E2B API".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{"api_key":{"type":"string","title":"API key","description":"Starts with e2b_.","minLength":1,"writeOnly":true}},"required":["api_key"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool],
        }],
    }
}

pub struct E2b;

const fn spec(
    name: &'static str,
    summary: &'static str,
    description: &'static str,
    verb: Verb,
    path: &'static str,
) -> Rest {
    Rest {
        name,
        summary,
        description,
        verb,
        path,
        query: &[],
    }
}
/// Other input fields are the E2B request: the JSON body of writes, the query of reads.
const SPECS: &[Rest] = &[
    spec(
        "e2b_get_health",
        "Checked E2B health",
        "Check the E2B API health endpoint.",
        Verb::Get,
        "/health",
    ),
    spec(
        "e2b_create_sandbox",
        "Created an E2B sandbox",
        "Create a sandbox from a template, e.g. {\"templateID\":\"base\",\"timeout\":300}.",
        Verb::Post,
        "/sandboxes",
    ),
    spec(
        "e2b_connect_sandbox",
        "Connected an E2B sandbox",
        "Connect to an existing sandbox, resuming it if paused, and extend its TTL.",
        Verb::Post,
        "/sandboxes/{sandbox_id}/connect",
    ),
    spec(
        "e2b_get_sandbox",
        "Read an E2B sandbox",
        "Get details for a sandbox.",
        Verb::Get,
        "/sandboxes/{sandbox_id}",
    ),
    spec(
        "e2b_list_sandboxes",
        "Listed E2B sandboxes",
        "List running and paused sandboxes.",
        Verb::Get,
        "/sandboxes",
    ),
    spec(
        "e2b_delete_sandbox",
        "Deleted an E2B sandbox",
        "Terminate a sandbox.",
        Verb::Delete,
        "/sandboxes/{sandbox_id}",
    ),
    spec(
        "e2b_pause_sandbox",
        "Paused an E2B sandbox",
        "Pause a sandbox.",
        Verb::Post,
        "/sandboxes/{sandbox_id}/pause",
    ),
    spec(
        "e2b_refresh_sandbox",
        "Refreshed an E2B sandbox",
        "Refresh a sandbox's TTL.",
        Verb::Post,
        "/sandboxes/{sandbox_id}/refresh",
    ),
    spec(
        "e2b_set_sandbox_timeout",
        "Set an E2B sandbox timeout",
        "Set a sandbox's timeout in seconds, e.g. {\"timeout\":600}.",
        Verb::Post,
        "/sandboxes/{sandbox_id}/timeout",
    ),
    spec(
        "e2b_get_sandbox_logs",
        "Read E2B sandbox logs",
        "Get a sandbox's logs.",
        Verb::Get,
        "/sandboxes/{sandbox_id}/logs",
    ),
    spec(
        "e2b_get_sandbox_metrics",
        "Read E2B sandbox metrics",
        "Get a sandbox's metrics.",
        Verb::Get,
        "/sandboxes/{sandbox_id}/metrics",
    ),
    spec(
        "e2b_get_sandbox_events",
        "Read E2B sandbox events",
        "Get a sandbox's lifecycle events.",
        Verb::Get,
        "/events/sandboxes/{sandbox_id}",
    ),
    spec(
        "e2b_list_team_sandbox_events",
        "Listed E2B sandbox events",
        "List sandbox lifecycle events for the team.",
        Verb::Get,
        "/events/sandboxes",
    ),
    spec(
        "e2b_list_sandboxes_metrics",
        "Listed E2B sandbox metrics",
        "Get metrics for several sandboxes.",
        Verb::Get,
        "/metrics/sandboxes",
    ),
    spec(
        "e2b_get_team_metrics",
        "Read E2B team metrics",
        "Get aggregate team metrics.",
        Verb::Get,
        "/metrics",
    ),
    spec(
        "e2b_get_team_metric_max",
        "Read an E2B team metric maximum",
        "Get the maximum of a team metric.",
        Verb::Get,
        "/metrics/max",
    ),
    spec(
        "e2b_create_template",
        "Created an E2B template",
        "Create a template.",
        Verb::Post,
        "/templates",
    ),
    spec(
        "e2b_list_templates",
        "Listed E2B templates",
        "List templates.",
        Verb::Get,
        "/templates",
    ),
    spec(
        "e2b_update_template",
        "Updated an E2B template",
        "Update a template.",
        Verb::Patch,
        "/templates/{template_id}",
    ),
    spec(
        "e2b_start_template_build",
        "Started an E2B template build",
        "Start a template build.",
        Verb::Post,
        "/templates/{template_id}/builds",
    ),
    spec(
        "e2b_get_template_build_status",
        "Read E2B template builds",
        "Get a template's build status.",
        Verb::Get,
        "/templates/{template_id}/builds",
    ),
    spec(
        "e2b_get_template_files",
        "Read E2B template files",
        "Get a template's files.",
        Verb::Get,
        "/templates/{template_id}/files",
    ),
    spec(
        "e2b_create_webhook",
        "Created an E2B webhook",
        "Create a lifecycle webhook.",
        Verb::Post,
        "/webhooks",
    ),
    spec(
        "e2b_list_webhooks",
        "Listed E2B webhooks",
        "List lifecycle webhooks.",
        Verb::Get,
        "/webhooks",
    ),
    spec(
        "e2b_get_webhook",
        "Read an E2B webhook",
        "Get a webhook.",
        Verb::Get,
        "/webhooks/{webhook_id}",
    ),
    spec(
        "e2b_update_webhook",
        "Updated an E2B webhook",
        "Update a webhook.",
        Verb::Patch,
        "/webhooks/{webhook_id}",
    ),
    spec(
        "e2b_delete_webhook",
        "Deleted an E2B webhook",
        "Delete a webhook.",
        Verb::Delete,
        "/webhooks/{webhook_id}",
    ),
];

fn sandbox_id() -> Value {
    // It becomes part of the envd host name.
    json!({"type":"string","pattern":"^[A-Za-z0-9-]{1,128}$"})
}
fn path() -> Value {
    json!({"type":"string","minLength":1,"maxLength":4096})
}

impl ToolProvider for E2b {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let mut tools: Vec<_> = SPECS
            .iter()
            .map(|spec| {
                let ids: Vec<&str> = ["sandbox_id", "template_id", "webhook_id"]
                    .into_iter()
                    .filter(|id| spec.path.contains(&format!("{{{id}}}")))
                    .collect();
                let properties: Map<String, Value> = ids
                    .iter()
                    .map(|id| ((*id).to_owned(), if *id == "sandbox_id" { sandbox_id() } else { json!({"type":"string","minLength":1}) }))
                    .collect();
                rest::definition(
                    "e2b",
                    spec.name,
                    spec.summary,
                    spec.description,
                    json!({"type":"object","properties":properties,"required":ids,"additionalProperties":true}),
                    spec.verb.hints(),
                )
            })
            .collect();
        let read = rest::Hints {
            read_only: true,
            destructive: false,
        };
        let write = rest::Hints::default();
        let destructive = rest::Hints {
            read_only: false,
            destructive: true,
        };
        let object = |properties: Value, required: &[&str]| {
            let mut properties = properties;
            properties["sandbox_id"] = sandbox_id();
            let required: Vec<&str> = [&["sandbox_id"], required].concat();
            json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
        };
        for (name, summary, description, schema, hints) in [
            (
                "e2b_read_file",
                "Read a sandbox file",
                "Read a UTF-8 file in a running sandbox.",
                object(json!({"path":path()}), &["path"]),
                read,
            ),
            (
                "e2b_write_file",
                "Wrote a sandbox file",
                "Write a file in a running sandbox, creating parent directories and replacing any existing file.",
                object(
                    json!({"path":path(),"contents":{"type":"string"}}),
                    &["path", "contents"],
                ),
                write,
            ),
            (
                "e2b_delete_file",
                "Deleted a sandbox file",
                "Delete a file or directory in a running sandbox; a missing path is not an error.",
                object(json!({"path":path()}), &["path"]),
                destructive,
            ),
            (
                "e2b_list_dir",
                "Listed a sandbox directory",
                "List the entries directly under a directory (default: the working directory).",
                object(json!({"path":path()}), &[]),
                read,
            ),
            (
                "e2b_stat",
                "Inspected a sandbox path",
                "Report whether a path exists, its kind (file, dir, other) and size.",
                object(json!({"path":path()}), &["path"]),
                read,
            ),
            (
                "e2b_exec_command",
                "Ran a sandbox command",
                "Run a bash command in a running sandbox and return its exit code, stdout and stderr.",
                object(
                    json!({"cmd":{"type":"string","minLength":1},"workdir":path(),"timeout_ms":{"type":"integer","minimum":1000,"maximum":600000,"description":"Default 60000."}}),
                    &["cmd"],
                ),
                write,
            ),
            (
                "e2b_apply_patch",
                "Patched sandbox files",
                "Apply a patch to files in a running sandbox. Format: '*** Begin Patch', then hunks '*** Add File: path' (+ lines), '*** Delete File: path', or '*** Update File: path' (optional '*** Move to: path', then '@@ context' chunks of ' ', '-', '+' lines), then '*** End Patch'.",
                object(json!({"patch":{"type":"string","minLength":1}}), &["patch"]),
                write,
            ),
        ] {
            tools.push(rest::definition(
                "e2b",
                name,
                summary,
                description,
                schema,
                hints,
            ));
        }
        tools
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            if let Some(spec) = SPECS.iter().find(|spec| spec.name == name) {
                let request = rest::request(access, "e2b_api", BASE, spec, input)?;
                return rest::send(request.header("X-API-Key", access.secret("api_key")?)).await;
            }
            let field = |key: &str| input[key].as_str().unwrap_or_default();
            let envd = Envd::connect(access, field("sandbox_id")).await?;
            match name {
                "e2b_read_file" => Ok(json!({"contents": envd.read(field("path")).await?})),
                "e2b_write_file" => {
                    envd.write(field("path"), field("contents")).await?;
                    Ok(json!({"bytes_written": field("contents").len()}))
                }
                "e2b_delete_file" => {
                    envd.remove(field("path")).await?;
                    Ok(json!({"deleted": true}))
                }
                "e2b_list_dir" => envd.list(input["path"].as_str().unwrap_or(".")).await,
                "e2b_stat" => envd.stat(field("path")).await,
                "e2b_exec_command" => {
                    envd.exec(
                        field("cmd"),
                        input["workdir"].as_str(),
                        input["timeout_ms"].as_u64().unwrap_or(60_000),
                    )
                    .await
                }
                "e2b_apply_patch" => {
                    let (mut added, mut modified, mut deleted) = (vec![], vec![], vec![]);
                    for hunk in sandbox_patch::parse(field("patch"))? {
                        match hunk {
                            Hunk::Add { path, contents } => {
                                envd.write(&path, &contents).await?;
                                added.push(path);
                            }
                            Hunk::Delete { path } => {
                                envd.remove(&path).await?;
                                deleted.push(path);
                            }
                            Hunk::Update {
                                path,
                                move_to,
                                chunks,
                            } => {
                                let updated = sandbox_patch::update(
                                    &envd.read(&path).await?,
                                    &path,
                                    &chunks,
                                )?;
                                match move_to {
                                    Some(to) => {
                                        envd.write(&to, &updated).await?;
                                        envd.remove(&path).await?;
                                        added.push(to);
                                        deleted.push(path);
                                    }
                                    None => {
                                        envd.write(&path, &updated).await?;
                                        modified.push(path);
                                    }
                                }
                            }
                        }
                    }
                    Ok(json!({"added":added,"modified":modified,"deleted":deleted}))
                }
                _ => Err(ConnectError::not_found("Unsupported E2B tool")),
            }
        })
    }
}

/// A connected sandbox's envd daemon.
struct Envd<'a> {
    access: &'a Access,
    base: String,
    sandbox: &'a str,
    token: Option<SecretString>,
}
impl<'a> Envd<'a> {
    async fn connect(access: &'a Access, sandbox: &'a str) -> ToolResult<Self> {
        let spec = spec("", "", "", Verb::Post, "/sandboxes/{sandbox_id}/connect");
        let request = rest::request(
            access,
            "e2b_api",
            BASE,
            &spec,
            json!({"sandbox_id":sandbox,"timeout":300}),
        )?;
        let mut connected =
            rest::send(request.header("X-API-Key", access.secret("api_key")?)).await?;
        let data = &mut connected["data"];
        let token = match data["envdAccessToken"].take() {
            Value::String(token) => Some(SecretString::from(token)),
            _ => None,
        };
        let base = match access.endpoints.0.get("e2b_envd") {
            Some(base) => base.clone(),
            None => format!(
                "https://49983-{sandbox}.{}",
                data["sandboxDomain"].as_str().unwrap_or("e2b.app")
            ),
        };
        Ok(Self {
            access,
            base,
            sandbox,
            token,
        })
    }
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let request = self
            .access
            .http
            .client
            .request(method, format!("{}/{path}", self.base))
            .header("E2b-Sandbox-Id", self.sandbox)
            .header("E2b-Sandbox-Port", "49983");
        match &self.token {
            Some(token) => request.header("X-Access-Token", token.expose_secret()),
            None => request,
        }
    }
    async fn read(&self, path: &str) -> ToolResult<String> {
        let response = self
            .request(reqwest::Method::GET, "files")
            .query(&[("path", path)])
            .send()
            .await
            .map_err(|_| ConnectError::unavailable("The sandbox could not be reached"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ConnectError::unavailable("The sandbox response was interrupted"))?;
        if !status.is_success() {
            return Err(ConnectError::unknown(format!(
                "Reading {path} failed with {}: {}",
                status.as_u16(),
                String::from_utf8_lossy(&bytes)
                    .chars()
                    .take(2048)
                    .collect::<String>()
            )));
        }
        String::from_utf8(bytes.into())
            .map_err(|_| ConnectError::invalid_argument(format!("{path} is not UTF-8 text")))
    }
    async fn write(&self, path: &str, contents: &str) -> ToolResult<()> {
        if let Some((parent, _)) = path
            .rsplit_once('/')
            .filter(|(parent, _)| !parent.is_empty())
        {
            match self
                .unary("filesystem.Filesystem/MakeDir", json!({"path":parent}))
                .await?
            {
                Ok(_) => {}
                Err(error) if error["code"] == "already_exists" => {}
                Err(error) => return Err(envd_error(error)),
            }
        }
        rest::send(
            self.request(reqwest::Method::POST, "files")
                .query(&[("path", path)])
                .header("content-type", "application/octet-stream")
                .body(contents.to_owned()),
        )
        .await
        .map(drop)
    }
    async fn remove(&self, path: &str) -> ToolResult<()> {
        match self
            .unary("filesystem.Filesystem/Remove", json!({"path":path}))
            .await?
        {
            Err(error) if error["code"] != "not_found" => Err(envd_error(error)),
            _ => Ok(()),
        }
    }
    async fn list(&self, path: &str) -> ToolResult<Value> {
        let listed = self
            .unary(
                "filesystem.Filesystem/ListDir",
                json!({"path":path,"depth":1}),
            )
            .await?
            .map_err(envd_error)?;
        let entries: Vec<Value> = listed["entries"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|entry| {
                let kind = kind(&entry["type"]);
                json!({"name":entry["name"],"kind":kind,"size":if kind == "file" { size(&entry["size"]) } else { None }})
            })
            .collect();
        Ok(json!({"entries":entries}))
    }
    async fn stat(&self, path: &str) -> ToolResult<Value> {
        match self
            .unary("filesystem.Filesystem/Stat", json!({"path":path}))
            .await?
        {
            Ok(stat) => {
                let entry = stat.get("entry").unwrap_or(&stat);
                Ok(
                    json!({"exists":true,"kind":kind(&entry["type"]),"size":size(&entry["size"]).unwrap_or(0)}),
                )
            }
            Err(error) if error["code"] == "not_found" => {
                Ok(json!({"exists":false,"kind":"","size":0}))
            }
            Err(error) => Err(envd_error(error)),
        }
    }
    /// A Connect unary call. The inner error is envd's Connect error, which callers may expect.
    async fn unary(&self, method: &str, body: Value) -> ToolResult<Result<Value, Value>> {
        let response = self
            .request(reqwest::Method::POST, method)
            .header("connect-protocol-version", "1")
            .json(&body)
            .send()
            .await
            .map_err(|_| ConnectError::unavailable("The sandbox could not be reached"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ConnectError::unavailable("The sandbox response was interrupted"))?;
        let value: Value = serde_json::from_slice(&bytes).unwrap_or_else(
            |_| json!({"code":"unknown","message":String::from_utf8_lossy(&bytes)}),
        );
        Ok(if status.is_success() {
            Ok(value)
        } else {
            Err(value)
        })
    }
    /// `process.Process/Start` is server streaming: one request frame, then output and end
    /// events until the process exits.
    async fn exec(&self, cmd: &str, workdir: Option<&str>, timeout_ms: u64) -> ToolResult<Value> {
        self.run(cmd, workdir, timeout_ms, &Map::new()).await
    }
    async fn run(
        &self,
        cmd: &str,
        workdir: Option<&str>,
        timeout_ms: u64,
        envs: &Map<String, Value>,
    ) -> ToolResult<Value> {
        let request = serde_json::to_vec(&json!({"process":{"cmd":"/bin/bash","args":["-l","-c",cmd],"cwd":workdir,"envs":envs},"stdin":false}))
            .map_err(|_| ConnectError::internal("Invalid command"))?;
        let mut frame = vec![0];
        frame.extend_from_slice(&(request.len() as u32).to_be_bytes());
        frame.extend(request);
        let response = self
            .request(reqwest::Method::POST, "process.Process/Start")
            .header("connect-protocol-version", "1")
            .header("content-type", "application/connect+json")
            .header("connect-timeout-ms", timeout_ms.to_string())
            .timeout(std::time::Duration::from_millis(timeout_ms + 10_000))
            .body(frame)
            .send()
            .await
            .map_err(|_| ConnectError::unavailable("The sandbox could not be reached"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ConnectError::unavailable("The command output was interrupted"))?;
        if !status.is_success() {
            return Err(ConnectError::unknown(format!(
                "The sandbox returned {}: {}",
                status.as_u16(),
                String::from_utf8_lossy(&bytes)
                    .chars()
                    .take(2048)
                    .collect::<String>()
            )));
        }
        let (mut stdout, mut stderr, mut exit_code, mut timed_out) = (vec![], vec![], 0, false);
        let decode = |text: &Value| {
            text.as_str()
                .map(|text| base64::engine::general_purpose::STANDARD.decode(text))
                .transpose()
                .map_err(|_| ConnectError::unknown("Invalid command output"))
        };
        let mut rest = &bytes[..];
        while rest.len() >= 5 {
            let flags = rest[0];
            let len = u32::from_be_bytes([rest[1], rest[2], rest[3], rest[4]]) as usize;
            let payload = rest
                .get(5..5 + len)
                .ok_or_else(|| ConnectError::unknown("Truncated command output"))?;
            rest = &rest[5 + len..];
            let message: Value = serde_json::from_slice(payload).unwrap_or_default();
            // The end-of-stream frame carries only trailers and a possible error.
            if flags & 0b10 != 0 {
                if !message["error"].is_null() {
                    return Err(ConnectError::unknown(format!(
                        "The command failed: {}",
                        message["error"]
                    )));
                }
                continue;
            }
            let event = message.get("event").unwrap_or(&message);
            stdout.extend(decode(&event["data"]["stdout"])?.unwrap_or_default());
            stderr.extend(decode(&event["data"]["stderr"])?.unwrap_or_default());
            if let Some(end) = event.get("end") {
                exit_code = end["exitCode"].as_i64().unwrap_or(exit_code);
                timed_out = end["status"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case("deadline_exceeded"));
                if let Some(error) = end["error"].as_str() {
                    stderr.extend_from_slice(error.as_bytes());
                }
            }
        }
        Ok(json!({
            "exit_code":exit_code,
            "stdout":String::from_utf8_lossy(&stdout),
            "stderr":String::from_utf8_lossy(&stderr),
            "timed_out":timed_out
        }))
    }
}

/// Blueprint sandboxes (`crate::sandboxes`): a VM from `template` that pauses, rather than being
/// killed, when its TTL runs out. Returns its ID.
pub async fn launch(access: &Access, template: &str) -> ToolResult<String> {
    let spec = spec("", "", "", Verb::Post, "/sandboxes");
    let request = rest::request(
        access,
        "e2b_api",
        BASE,
        &spec,
        json!({"templateID":template,"timeout":LIFECYCLE_TTL_SECS,"autoPause":true}),
    )?;
    let created = rest::send(request.header("X-API-Key", access.secret("api_key")?)).await?;
    created["data"]["sandboxID"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| ConnectError::unknown("E2B returned no sandbox ID"))
}
/// The TTL a blueprint sandbox gets at launch and every wake; it pauses when it runs out.
const LIFECYCLE_TTL_SECS: u64 = 3600;
/// Resume a paused sandbox (a running one only has its TTL extended). False when E2B no longer
/// has it.
pub async fn resume(access: &Access, sandbox: &str) -> ToolResult<bool> {
    lifecycle(
        access,
        Verb::Post,
        "/sandboxes/{sandbox_id}/connect",
        json!({"sandbox_id":sandbox,"timeout":LIFECYCLE_TTL_SECS}),
    )
    .await
}
/// Pause, keeping memory and processes. False when E2B no longer has it.
pub async fn pause(access: &Access, sandbox: &str) -> ToolResult<bool> {
    lifecycle(
        access,
        Verb::Post,
        "/sandboxes/{sandbox_id}/pause",
        json!({"sandbox_id":sandbox}),
    )
    .await
}
pub async fn kill(access: &Access, sandbox: &str) -> ToolResult<()> {
    lifecycle(
        access,
        Verb::Delete,
        "/sandboxes/{sandbox_id}",
        json!({"sandbox_id":sandbox}),
    )
    .await
    .map(drop)
}
/// Start a long-running shell command with `envs`, returning once envd reports it started. The
/// process outlives this call, as E2B's background commands do: the start stream is dropped
/// without a deadline that would end it.
pub async fn spawn(
    access: &Access,
    sandbox: &str,
    command: &str,
    envs: &Map<String, Value>,
) -> ToolResult<()> {
    let envd = Envd::connect(access, sandbox).await?;
    let request = serde_json::to_vec(
        &json!({"process":{"cmd":"/bin/bash","args":["-l","-c",command],"envs":envs},"stdin":false}),
    )
    .map_err(|_| ConnectError::internal("Invalid command"))?;
    let mut frame = vec![0];
    frame.extend_from_slice(&(request.len() as u32).to_be_bytes());
    frame.extend(request);
    let mut response = envd
        .request(reqwest::Method::POST, "process.Process/Start")
        .header("connect-protocol-version", "1")
        .header("content-type", "application/connect+json")
        .body(frame)
        .send()
        .await
        .map_err(|_| ConnectError::unavailable("The sandbox could not be reached"))?;
    if !response.status().is_success() {
        return Err(ConnectError::unknown(format!(
            "The sandbox returned {} starting a process",
            response.status().as_u16()
        )));
    }
    let mut bytes = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let chunk = tokio::time::timeout_at(deadline, response.chunk())
            .await
            .map_err(|_| ConnectError::deadline_exceeded("The process did not start in time"))?
            .map_err(|_| ConnectError::unavailable("The sandbox response was interrupted"))?;
        let Some(chunk) = chunk else {
            return Err(ConnectError::unknown("The process ended before it started"));
        };
        bytes.extend_from_slice(&chunk);
        while bytes.len() >= 5 {
            let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
            if bytes.len() < 5 + len {
                break;
            }
            let end_of_stream = bytes[0] & 0b10 != 0;
            let message: Value = serde_json::from_slice(&bytes[5..5 + len]).unwrap_or_default();
            bytes.drain(..5 + len);
            let event = message.get("event").unwrap_or(&message);
            if event.get("start").is_some() {
                return Ok(());
            }
            if end_of_stream || event.get("end").is_some() {
                return Err(ConnectError::unknown(format!(
                    "The process ended before it started: {}",
                    message.to_string().chars().take(512).collect::<String>()
                )));
            }
        }
    }
}
/// A lifecycle call: false on 404, and a pause of an already paused sandbox (409) succeeds.
async fn lifecycle(
    access: &Access,
    verb: Verb,
    path: &'static str,
    input: Value,
) -> ToolResult<bool> {
    let spec = spec("", "", "", verb, path);
    let request = rest::request(access, "e2b_api", BASE, &spec, input)?;
    let (status, bytes) =
        rest::response(request.header("X-API-Key", access.secret("api_key")?)).await?;
    match status.as_u16() {
        404 => Ok(false),
        409 => Ok(true),
        _ => rest::output(status, &bytes).map(|_| true),
    }
}

fn envd_error(error: Value) -> ConnectError {
    ConnectError::unknown(format!("The sandbox refused the operation: {error}"))
}
fn kind(value: &Value) -> &'static str {
    match value.as_str() {
        Some("FILE_TYPE_FILE" | "file") => "file",
        Some("FILE_TYPE_DIRECTORY" | "dir" | "directory") => "dir",
        _ => "other",
    }
}
/// Connect's JSON encodes 64-bit integers as strings.
fn size(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|v| v.parse().ok()))
}
