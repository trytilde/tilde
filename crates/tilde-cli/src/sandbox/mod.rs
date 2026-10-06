//! `tilde sandbox`: the process Tilde starts inside a sandbox VM, and the commands that reach it.
//!
//! `connect` exchanges the single-use enrollment token for a session token held only in memory,
//! then keeps a Connect stream open to the gateway: operation frames run concurrently and are
//! answered through Respond, and a silent or broken stream is redialled with the session token
//! (a paused and resumed VM looks exactly like that). Every registration carries the
//! blueprint's environment, which commands receive directly and login shells pick up from
//! `~/.tilde/sandbox.env`. A loopback-only HTTP API lets anything in the VM call the
//! blueprint's tools; `tools` and `call` are its command-line clients.
mod local;
mod ops;

use crate::api::{base_url, failed, transport};
use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use connectrpc::{
    ConnectError, ErrorCode as Code, client::CallOptions, client::ClientConfig, client::HttpClient,
};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tilde_contracts::proto::tilde::sandbox::v1::{
    self as wire, connect_response::Frame, respond_request::Outcome,
};
use tilde_contracts::services::tilde::sandbox::v1::SandboxServiceClient;

#[derive(Args)]
pub struct Sandbox {
    #[command(subcommand)]
    command: SandboxCommand,
}

#[derive(Subcommand)]
enum SandboxCommand {
    /// Serve this sandbox VM's operations to Tilde. Tilde starts this itself.
    Connect(Connect),
    /// List the tools available to this sandbox.
    Tools(Tools),
    /// Invoke one of this sandbox's tools and print its output JSON.
    Call(Call),
}

#[derive(Args)]
struct Connect {
    /// The gateway's runtime origin.
    #[arg(long, env = "TILDE_URL")]
    url: String,
    /// Single-use enrollment token.
    #[arg(long, env = "TILDE_SANDBOX_TOKEN", hide_env_values = true)]
    token: String,
    /// Loopback address for the local tools API.
    #[arg(long, default_value = local::DEFAULT_LISTEN)]
    listen: SocketAddr,
    /// Where relative paths and commands start. Defaults to the current directory.
    #[arg(long)]
    workdir: Option<PathBuf>,
}

#[derive(Args)]
struct Tools {
    #[arg(long, default_value = local::DEFAULT_LISTEN)]
    listen: SocketAddr,
    /// Print the raw JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct Call {
    /// The tool, as `{source}.{tool}`.
    name: String,
    /// The tool's input JSON. Read from stdin when omitted and stdin is not a terminal.
    input: Option<String>,
    #[arg(long, default_value = local::DEFAULT_LISTEN)]
    listen: SocketAddr,
}

pub async fn run(args: Sandbox) -> Result<()> {
    match args.command {
        SandboxCommand::Connect(args) => connect(args).await,
        SandboxCommand::Tools(args) => local::tools(args.listen, args.json).await,
        SandboxCommand::Call(args) => local::call(args.listen, &args.name, args.input).await,
    }
}

/// Liveness: the gateway pings every ~10s, so this much silence means the stream is dead.
const IDLE: Duration = Duration::from_secs(45);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// The connected process's state, shared by the stream loop, operations and the local API.
struct Daemon {
    client: SandboxServiceClient<HttpClient>,
    /// The enrollment token until the first registration, then the session token.
    token: Mutex<String>,
    sandbox_id: Mutex<Option<String>>,
    connected: AtomicBool,
    workspace: Arc<ops::Workspace>,
    /// Where `.tilde/sandbox.env` and the shell profiles live; None skips writing them.
    home: Option<PathBuf>,
}

async fn connect(args: Connect) -> Result<()> {
    // SAFETY: nothing else reads the environment yet; children must never inherit the token.
    unsafe { std::env::remove_var("TILDE_SANDBOX_TOKEN") };
    if !args.listen.ip().is_loopback() {
        bail!("--listen must be a loopback address, not {}", args.listen);
    }
    let workdir = match args.workdir {
        Some(dir) => dir,
        None => std::env::current_dir().context("Could not read the current directory")?,
    };
    let daemon = Arc::new(Daemon::new(
        &args.url,
        args.token,
        workdir,
        std::env::var_os("HOME").map(PathBuf::from),
    )?);
    let (bound, _server) = local::serve(daemon.clone(), args.listen).await?;
    eprintln!("tilde sandbox: tools API on http://{bound}");
    daemon.stream().await
}

impl Daemon {
    fn new(url: &str, token: String, workdir: PathBuf, home: Option<PathBuf>) -> Result<Self> {
        let base = base_url(url)?;
        let config = ClientConfig::new(
            base.parse()
                .with_context(|| format!("{base} is not a usable endpoint"))?,
        );
        Ok(Self {
            client: SandboxServiceClient::new(transport(&base)?, config),
            token: Mutex::new(token),
            sandbox_id: Mutex::new(None),
            connected: AtomicBool::new(false),
            workspace: Arc::new(ops::Workspace::new(workdir)),
            home,
        })
    }

    fn options(&self) -> CallOptions {
        let mut value =
            http::HeaderValue::from_str(&format!("Bearer {}", self.token.lock().unwrap()))
                .unwrap_or_else(|_| http::HeaderValue::from_static(""));
        value.set_sensitive(true);
        CallOptions::default().with_header(http::header::AUTHORIZATION, value)
    }

    /// Keep a Connect stream open for as long as the gateway accepts this process.
    async fn stream(self: &Arc<Self>) -> Result<()> {
        let mut backoff = Duration::from_secs(1);
        loop {
            let (registered, error) = self.session().await;
            self.connected.store(false, Ordering::Relaxed);
            if registered {
                backoff = Duration::from_secs(1);
            }
            match error {
                Some(error) if error.code == Code::Unauthenticated => bail!(
                    "The gateway no longer accepts this sandbox's credentials ({}); Tilde \
                     relaunches it with a fresh enrollment",
                    error.message.unwrap_or_default()
                ),
                Some(error) => eprintln!(
                    "tilde sandbox: {}; reconnecting in {}s",
                    failed("Connect", error),
                    backoff.as_secs()
                ),
                None => eprintln!(
                    "tilde sandbox: the stream went quiet or ended; reconnecting in {}s",
                    backoff.as_secs()
                ),
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }

    /// One Connect stream until it fails, ends or goes quiet. Reports whether it registered.
    async fn session(self: &Arc<Self>) -> (bool, Option<ConnectError>) {
        let mut stream = match self
            .client
            .connect_with_options(wire::ConnectRequest::default(), self.options())
            .await
        {
            Ok(stream) => stream,
            Err(error) => return (false, Some(error)),
        };
        let mut registered = false;
        loop {
            let message = match tokio::time::timeout(IDLE, stream.message()).await {
                Err(_) | Ok(Ok(None)) => return (registered, None),
                Ok(Err(error)) => return (registered, Some(error)),
                Ok(Ok(Some(message))) => message.to_owned_message(),
            };
            match message.frame {
                Some(Frame::Registered(registration)) => {
                    registered = true;
                    self.register(*registration);
                }
                Some(Frame::Operation(operation)) => {
                    let daemon = self.clone();
                    tokio::spawn(async move { daemon.operate(*operation).await });
                }
                Some(Frame::Ping(_)) | None => {}
            }
        }
    }

    fn register(&self, registration: wire::SandboxRegistered) {
        if let Some(session) = registration.session_token {
            *self.token.lock().unwrap() = session;
        }
        let env: Vec<(String, String)> = registration
            .env
            .into_iter()
            .map(|var| (var.name, var.value))
            .collect();
        if let Some(home) = &self.home
            && let Err(error) = write_env(home, &env)
        {
            eprintln!("tilde sandbox: could not write the shell environment ({error:#})");
        }
        *self.workspace.env.lock().unwrap() = env;
        eprintln!("tilde sandbox: connected as {}", registration.sandbox_id);
        *self.sandbox_id.lock().unwrap() = Some(registration.sandbox_id);
        self.connected.store(true, Ordering::Relaxed);
    }

    async fn operate(&self, operation: wire::SandboxOperation) {
        /// The gateway refuses larger outputs; say so rather than letting Respond fail.
        const MAX_OUTPUT: usize = 1024 * 1024;
        let outcome = match self
            .workspace
            .run(&operation.name, &operation.input_json, &operation.id)
            .await
        {
            Ok(output) => match serde_json::to_string(&output) {
                Ok(json) if json.len() <= MAX_OUTPUT => Outcome::OutputJson(json),
                _ => Outcome::Error(format!("The {} output exceeds 1 MiB", operation.name)),
            },
            Err(error) => Outcome::Error(format!("{error:#}")),
        };
        // A reconnect in flight should not lose an answer; Respond is retried briefly.
        for attempt in 1..=3 {
            let request = wire::RespondRequest {
                id: operation.id.clone(),
                outcome: Some(outcome.clone()),
                ..Default::default()
            };
            match self
                .client
                .respond_with_options(request, self.options())
                .await
            {
                Ok(_) => return,
                Err(error) if attempt < 3 && error.code == Code::Unavailable => {
                    tokio::time::sleep(Duration::from_secs(attempt)).await;
                }
                Err(error) => {
                    eprintln!(
                        "tilde sandbox: {} for {}",
                        failed("Respond", error),
                        operation.name
                    );
                    return;
                }
            }
        }
    }
}

/// Rewrite `~/.tilde/sandbox.env` with exactly `env`, readable only by this user, and make sure
/// login and interactive shells source it.
fn write_env(home: &Path, env: &[(String, String)]) -> Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    const SOURCE: &str =
        "[ -f \"$HOME/.tilde/sandbox.env\" ] && . \"$HOME/.tilde/sandbox.env\" # tilde sandbox";

    let dir = home.join(".tilde");
    std::fs::create_dir_all(&dir)?;
    let mut contents = String::new();
    for (name, value) in env {
        let valid = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            eprintln!("tilde sandbox: skipping {name:?}, not a shell variable name");
            continue;
        }
        contents.push_str(&format!(
            "export {name}='{}'\n",
            value.replace('\'', "'\\''")
        ));
    }
    // Written aside and renamed, so a shell starting right now never sources half a file.
    let staged = dir.join("sandbox.env.tmp");
    let _ = std::fs::remove_file(&staged);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staged)?
        .write_all(contents.as_bytes())?;
    std::fs::rename(&staged, dir.join("sandbox.env"))?;

    for profile in [".profile", ".bashrc"] {
        let path = home.join(profile);
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if existing.lines().any(|line| line == SOURCE) {
            continue;
        }
        let separator = if existing.is_empty() || existing.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?
            .write_all(format!("{separator}{SOURCE}\n").as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream};
    use serde_json::{Value, json};
    use tilde_contracts::proto::tilde::types::v1 as types;
    use tilde_contracts::services::tilde::sandbox::v1::SandboxService;
    use tokio::sync::mpsc;
    use tokio_stream::StreamExt as _;

    /// A stand-in gateway. Each Connect takes the next scripted frame channel, so the test
    /// decides what every stream says and when it ends; the third Connect is refused.
    struct Gateway {
        streams: Mutex<Vec<mpsc::UnboundedReceiver<wire::ConnectResponse>>>,
        responses: mpsc::UnboundedSender<wire::RespondRequest>,
        /// The Authorization header of every call, as "Method Bearer token".
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl Gateway {
        fn saw(&self, method: &str, ctx: &RequestContext) {
            let auth = ctx
                .header(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default();
            self.seen.lock().unwrap().push(format!("{method} {auth}"));
        }
    }

    impl SandboxService for Gateway {
        async fn connect(
            &self,
            ctx: RequestContext,
            _: ServiceRequest<'_, wire::ConnectRequest>,
        ) -> ServiceResult<
            ServiceStream<impl connectrpc::Encodable<wire::ConnectResponse> + Send + use<>>,
        > {
            self.saw("Connect", &ctx);
            let mut streams = self.streams.lock().unwrap();
            if streams.is_empty() {
                return Err(ConnectError::unauthenticated("sandbox session revoked"));
            }
            let frames = streams.remove(0);
            Response::stream_ok(
                tokio_stream::wrappers::UnboundedReceiverStream::new(frames).map(Ok),
            )
        }

        async fn respond<'a>(
            &'a self,
            ctx: RequestContext,
            request: ServiceRequest<'_, wire::RespondRequest>,
        ) -> ServiceResult<impl connectrpc::Encodable<wire::RespondResponse> + Send + use<'a>>
        {
            self.saw("Respond", &ctx);
            let _ = self.responses.send(request.to_owned_message());
            Response::ok(wire::RespondResponse::default())
        }

        async fn list_tools<'a>(
            &'a self,
            ctx: RequestContext,
            _: ServiceRequest<'_, wire::ListToolsRequest>,
        ) -> ServiceResult<impl connectrpc::Encodable<wire::ListToolsResponse> + Send + use<'a>>
        {
            self.saw("ListTools", &ctx);
            Response::ok(wire::ListToolsResponse {
                tools: vec![types::ToolDefinition {
                    name: "git.token".into(),
                    input_schema_json: r#"{"type":"object"}"#.into(),
                    ..Default::default()
                }],
                ..Default::default()
            })
        }

        async fn invoke_tool<'a>(
            &'a self,
            ctx: RequestContext,
            request: ServiceRequest<'_, wire::InvokeToolRequest>,
        ) -> ServiceResult<impl connectrpc::Encodable<wire::InvokeToolResponse> + Send + use<'a>>
        {
            self.saw("InvokeTool", &ctx);
            let request = request.to_owned_message();
            if request.name != "git.token" {
                return Err(ConnectError::not_found("no such tool"));
            }
            let input: Value = serde_json::from_str(&request.input_json).unwrap();
            Response::ok(wire::InvokeToolResponse {
                output_json: json!({ "token": "ghs_x", "repo": input["repo"] }).to_string(),
                ..Default::default()
            })
        }
    }

    fn registered(session: Option<&str>, env: &[(&str, &str)]) -> wire::ConnectResponse {
        wire::ConnectResponse {
            frame: Some(Frame::Registered(Box::new(wire::SandboxRegistered {
                sandbox_id: "sbx-1".into(),
                session_token: session.map(Into::into),
                env: env
                    .iter()
                    .map(|(name, value)| wire::EnvVar {
                        name: (*name).into(),
                        value: (*value).into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }))),
            ..Default::default()
        }
    }

    /// Send one operation and wait for its answer: Ok(output) or Err(message).
    async fn operate(
        frames: &mpsc::UnboundedSender<wire::ConnectResponse>,
        responses: &mut mpsc::UnboundedReceiver<wire::RespondRequest>,
        name: &str,
        input: Value,
    ) -> Result<Value, String> {
        let id = format!("op-{name}-{}", input);
        frames
            .send(wire::ConnectResponse {
                frame: Some(Frame::Operation(Box::new(wire::SandboxOperation {
                    id: id.clone(),
                    name: name.into(),
                    input_json: input.to_string(),
                    ..Default::default()
                }))),
                ..Default::default()
            })
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(20), responses.recv())
            .await
            .expect("an answer in time")
            .expect("the gateway is still up");
        assert_eq!(response.id, id);
        match response.outcome {
            Some(Outcome::OutputJson(json)) => Ok(serde_json::from_str(&json).unwrap()),
            Some(Outcome::Error(message)) => Err(message),
            None => panic!("an answer carries an outcome"),
        }
    }

    /// The whole life of a sandbox process against the gateway's wire format: enrollment swaps
    /// the single-use token for a session token, the blueprint's env reaches both commands and
    /// login shells (via the env file), file operations compose, a dropped stream is redialled
    /// with the session token and its new env replaces the old, the local API forwards tool
    /// calls with the session token while refusing browsers, and a revoked session ends the
    /// process.
    #[tokio::test]
    async fn enrolls_serves_operations_reconnects_and_forwards_tool_calls() {
        let (first, first_frames) = mpsc::unbounded_channel();
        let (second, second_frames) = mpsc::unbounded_channel();
        let (responded, mut responses) = mpsc::unbounded_channel();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let gateway = Gateway {
            streams: Mutex::new(vec![first_frames, second_frames]),
            responses: responded,
            seen: seen.clone(),
        };
        let app = axum::Router::new().fallback_service(
            connectrpc::Router::new()
                .add_service(Arc::new(gateway))
                .into_axum_service(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let home = tempfile::tempdir().unwrap();
        let workdir = tempfile::tempdir().unwrap();
        let daemon = Arc::new(
            Daemon::new(
                &format!("http://{address}"),
                "enroll-1".into(),
                workdir.path().to_path_buf(),
                Some(home.path().to_path_buf()),
            )
            .unwrap(),
        );
        let (local, _server) = local::serve(daemon.clone(), "127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let running = tokio::spawn({
            let daemon = daemon.clone();
            async move { daemon.stream().await }
        });

        let home_dir = home.path().to_str().unwrap();
        first
            .send(registered(
                Some("session-1"),
                &[
                    ("GREETING", "it's here"),
                    ("HOME", home_dir),
                    ("bad-name", "x"),
                ],
            ))
            .unwrap();
        // The in-memory env first, then the same variable as login shells read it from the file.
        let exec = operate(
            &first,
            &mut responses,
            "exec",
            json!({ "command": "printf '%s|' \"$GREETING\"; unset GREETING; \
                     . \"$HOME/.tilde/sandbox.env\"; printf '%s' \"$GREETING\"; \
                     echo oops >&2; exit 3" }),
        )
        .await
        .unwrap();
        assert_eq!(exec["stdout"], "it's here|it's here");
        assert_eq!(exec["stderr"], "oops\n");
        assert_eq!(exec["exit_code"], 3);
        assert_eq!(exec["timed_out"], false);

        let timed_out = operate(
            &first,
            &mut responses,
            "exec",
            json!({ "command": "echo started; sleep 30", "timeout_ms": 300 }),
        )
        .await
        .unwrap();
        assert_eq!(timed_out["timed_out"], true);
        assert_eq!(timed_out["exit_code"], Value::Null);
        assert_eq!(timed_out["stdout"], "started\n");

        let written = operate(
            &first,
            &mut responses,
            "write_file",
            json!({ "path": "notes/a.txt", "content": "one\ntwo\ntwo\n" }),
        )
        .await
        .unwrap();
        assert_eq!(written["bytes_written"], 12);
        let ambiguous = operate(
            &first,
            &mut responses,
            "edit_file",
            json!({ "path": "notes/a.txt", "old_string": "two", "new_string": "2" }),
        )
        .await
        .unwrap_err();
        assert!(ambiguous.contains("occurs 2 times"), "{ambiguous}");
        let edited = operate(
            &first,
            &mut responses,
            "edit_file",
            json!({ "path": "notes/a.txt", "old_string": "two", "new_string": "2", "replace_all": true }),
        )
        .await
        .unwrap();
        assert_eq!(edited["replacements"], 2);
        let read = operate(
            &first,
            &mut responses,
            "read_file",
            json!({ "path": workdir.path().join("notes/a.txt"), "offset": 2, "limit": 1 }),
        )
        .await
        .unwrap();
        assert_eq!(
            read,
            json!({ "content": "2\n", "total_lines": 3, "truncated": false })
        );
        // grep streams files and keeps physical line numbers: a 3 MiB line, cut at 1 MiB in the
        // middle of a character, is still one line, and a match far below it keeps its number.
        let big = format!(
            "x{}\n{}needle\n",
            "é".repeat(3 << 19),
            "filler line\n".repeat(100_000)
        );
        std::fs::write(workdir.path().join("notes/big.log"), big).unwrap();
        let found = operate(
            &first,
            &mut responses,
            "grep",
            json!({ "pattern": "needle", "max_results": 1 }),
        )
        .await
        .unwrap();
        assert_eq!(
            found["matches"],
            json!([{ "path": "notes/big.log", "line": 100_002, "text": "needle" }])
        );
        // apply_patch's file deletion refuses a directory rather than emptying it.
        let refused = operate(
            &first,
            &mut responses,
            "delete_file",
            json!({ "path": "notes" }),
        )
        .await
        .unwrap_err();
        assert!(refused.contains("is a directory"), "{refused}");
        assert!(workdir.path().join("notes/a.txt").exists());

        let env_file = home.path().join(".tilde/sandbox.env");
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&env_file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // The local API forwards with the session token, never the spent enrollment token.
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let called = http
            .post(format!("http://{local}/v1/tools/git.token"))
            .body(r#"{"repo":"acme/app"}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(called.status(), 200);
        assert_eq!(
            called.json::<Value>().await.unwrap(),
            json!({ "token": "ghs_x", "repo": "acme/app" })
        );
        let missing = http
            .post(format!("http://{local}/v1/tools/nope.tool"))
            .send()
            .await
            .unwrap();
        assert_eq!(missing.status(), 404);
        assert_eq!(
            missing.json::<Value>().await.unwrap()["error"],
            "no such tool"
        );
        let browser = http
            .post(format!("http://{local}/v1/tools/git.token"))
            .header("origin", "http://evil.example")
            .send()
            .await
            .unwrap();
        assert_eq!(browser.status(), 403);

        // Dropping the stream makes the process redial; the new registration's env replaces the
        // old one, so a variable the blueprint no longer has is gone from the file.
        drop(first);
        second
            .send(registered(None, &[("HOME", home_dir), ("REGION", "eu")]))
            .unwrap();
        let region = operate(
            &second,
            &mut responses,
            "exec",
            json!({ "command": "printf '%s/%s' \"$REGION\" \"${GREETING:-unset}\"" }),
        )
        .await
        .unwrap();
        assert_eq!(region["stdout"], "eu/unset");
        let env = std::fs::read_to_string(&env_file).unwrap();
        assert!(
            env.contains("export REGION='eu'\n") && !env.contains("GREETING"),
            "{env}"
        );
        let profile = std::fs::read_to_string(home.path().join(".profile")).unwrap();
        assert_eq!(
            profile.matches(".tilde/sandbox.env").count(),
            2,
            "sourced exactly once"
        );

        // A third Connect is refused as Unauthenticated: the process gives up.
        drop(second);
        let ended = tokio::time::timeout(Duration::from_secs(20), running)
            .await
            .expect("the process stops")
            .unwrap();
        assert!(ended.unwrap_err().to_string().contains("fresh enrollment"));

        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen[0], "Connect Bearer enroll-1");
        assert!(
            seen[1..]
                .iter()
                .all(|call| call.ends_with("Bearer session-1")),
            "{seen:?}"
        );
        assert!(seen.contains(&"InvokeTool Bearer session-1".to_string()));
        assert_eq!(
            seen.iter()
                .filter(|call| call.starts_with("Connect"))
                .count(),
            3
        );
    }
}
