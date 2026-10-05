//! The operations an agent runs in the sandbox: shell commands and file access. Each takes the
//! matching agent tool's JSON input and returns a JSON object; an `Err` travels back as the
//! operation's error message.
use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command, sync::watch};

/// Names the operation a command runs under; `tilde sandbox call` passes it on.
pub const OPERATION_ENV: &str = "TILDE_SANDBOX_OPERATION";
/// Each of a command's stdout and stderr keeps at most its last 256 KiB.
const TAIL: usize = 256 * 1024;
const READ_LIMIT: u64 = 900 * 1024;
const MAX_ENTRIES: usize = 1000;
const MAX_LINE_TEXT: usize = 500;
/// Finished background jobs kept for `exec_output` beyond this many are forgotten, oldest first.
const KEPT_JOBS: usize = 64;

/// What every operation shares: where relative paths start, the blueprint's environment for
/// commands, and the background jobs still answerable through `exec_output`.
pub struct Workspace {
    pub workdir: PathBuf,
    pub env: Mutex<Vec<(String, String)>>,
    jobs: Mutex<BTreeMap<u64, Arc<Job>>>,
    next_job: AtomicU64,
}

impl Workspace {
    pub fn new(workdir: PathBuf) -> Self {
        Self {
            workdir,
            env: Mutex::new(Vec::new()),
            jobs: Mutex::new(BTreeMap::new()),
            next_job: AtomicU64::new(1),
        }
    }

    /// `operation` is the gateway's ID for this operation; commands see it as
    /// TILDE_SANDBOX_OPERATION so the tool calls they make are traced inside it.
    pub async fn run(self: &Arc<Self>, name: &str, input: &str, operation: &str) -> Result<Value> {
        let input: Value = serde_json::from_str(if input.trim().is_empty() { "{}" } else { input })
            .context("The input is not JSON")?;
        match name {
            "exec" => self.exec(parse(name, input)?, operation).await,
            "exec_output" => self.exec_output(parse(name, input)?).await,
            "read_file" | "write_file" | "edit_file" | "delete_file" | "list_dir" | "glob"
            | "grep" => {
                // File access is blocking and a grep can walk a large tree.
                let (workspace, name) = (self.clone(), name.to_string());
                tokio::task::spawn_blocking(move || workspace.file_operation(&name, input))
                    .await
                    .context("The operation panicked")?
            }
            _ => bail!("Unknown sandbox operation {name}"),
        }
    }

    fn file_operation(&self, name: &str, input: Value) -> Result<Value> {
        match name {
            "read_file" => self.read_file(parse(name, input)?),
            "write_file" => self.write_file(parse(name, input)?),
            "edit_file" => self.edit_file(parse(name, input)?),
            "delete_file" => self.delete_file(parse(name, input)?),
            "list_dir" => self.list_dir(parse(name, input)?),
            "glob" => self.glob(parse(name, input)?),
            _ => self.grep(parse(name, input)?),
        }
    }

    fn resolve(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workdir.join(path)
        }
    }

    async fn exec(&self, input: Exec, operation: &str) -> Result<Value> {
        let cwd = input
            .cwd
            .as_deref()
            .map_or_else(|| self.workdir.clone(), |cwd| self.resolve(cwd));
        let clamp = |ms: u64| Duration::from_millis(ms.min(600_000));
        // A background job runs until it exits unless the caller asked for a limit.
        let timeout = match (input.background, input.timeout_ms) {
            (_, Some(ms)) => Some(clamp(ms)),
            (false, None) => Some(clamp(120_000)),
            (true, None) => None,
        };
        let env = self.env.lock().unwrap().clone();
        let job = Job::start(&input.command, &cwd, &env, operation, timeout)?;
        if !input.background {
            let exit = job.finished().await;
            let (stdout, stderr, truncated) = job.output();
            return Ok(json!({
                "exit_code": exit.code,
                "stdout": stdout,
                "stderr": stderr,
                "timed_out": exit.timed_out,
                "truncated": truncated,
            }));
        }
        let id = self.next_job.fetch_add(1, Ordering::Relaxed);
        let mut jobs = self.jobs.lock().unwrap();
        jobs.insert(id, job);
        let finished: Vec<u64> = jobs
            .iter()
            .filter(|(_, job)| job.exit.borrow().is_some())
            .map(|(id, _)| *id)
            .collect();
        for old in finished.iter().take(jobs.len().saturating_sub(KEPT_JOBS)) {
            jobs.remove(old);
        }
        Ok(json!({ "job_id": format!("job-{id}") }))
    }

    async fn exec_output(&self, input: ExecOutput) -> Result<Value> {
        let job = input
            .job_id
            .strip_prefix("job-")
            .and_then(|id| id.parse().ok())
            .and_then(|id| self.jobs.lock().unwrap().get(&id).cloned())
            .ok_or_else(|| anyhow!("Unknown job {}", input.job_id))?;
        let wait = Duration::from_millis(input.wait_ms.min(600_000));
        let exit = tokio::time::timeout(wait, job.finished()).await.ok();
        let (stdout, stderr, truncated) = job.output();
        Ok(json!({
            "running": exit.is_none(),
            "exit_code": exit.and_then(|exit| exit.code),
            "stdout": stdout,
            "stderr": stderr,
            "truncated": truncated,
        }))
    }

    fn read_file(&self, input: ReadFile) -> Result<Value> {
        let path = self.resolve(&input.path);
        let file =
            std::fs::File::open(&path).with_context(|| format!("Could not open {}", input.path))?;
        if input.limit.is_none() && file.metadata()?.len() > READ_LIMIT {
            bail!(
                "{} is larger than 900 KiB; read it in parts with offset and limit",
                input.path
            );
        }
        let first = input.offset.unwrap_or(1).max(1);
        let last = input.limit.map(|limit| first.saturating_add(limit));
        let mut reader = BufReader::new(file);
        let (mut content, mut total, mut truncated) = (String::new(), 0, false);
        let mut line = Vec::new();
        loop {
            line.clear();
            if reader.read_until(b'\n', &mut line)? == 0 {
                break;
            }
            total += 1;
            let text = std::str::from_utf8(&line)
                .map_err(|_| anyhow!("{} is not a UTF-8 text file", input.path))?;
            if total < first || last.is_some_and(|last| total >= last) || truncated {
                continue;
            }
            if content.len() + text.len() > READ_LIMIT as usize {
                truncated = true;
            } else {
                content.push_str(text);
            }
        }
        Ok(json!({ "content": content, "total_lines": total, "truncated": truncated }))
    }

    fn write_file(&self, input: WriteFile) -> Result<Value> {
        let path = self.resolve(&input.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Could not create the directory for {}", input.path))?;
        }
        std::fs::write(&path, &input.content)
            .with_context(|| format!("Could not write {}", input.path))?;
        Ok(json!({ "bytes_written": input.content.len() }))
    }

    fn edit_file(&self, input: EditFile) -> Result<Value> {
        if input.old_string.is_empty() {
            bail!("old_string must not be empty");
        }
        let path = self.resolve(&input.path);
        let bytes =
            std::fs::read(&path).with_context(|| format!("Could not read {}", input.path))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| anyhow!("{} is not a UTF-8 text file", input.path))?;
        let found = text.matches(&input.old_string).count();
        match found {
            0 => bail!("old_string was not found in {}", input.path),
            1 => {}
            _ if input.replace_all => {}
            _ => bail!(
                "old_string occurs {found} times in {}; add surrounding context to make it \
                 unique or set replace_all",
                input.path
            ),
        }
        std::fs::write(&path, text.replace(&input.old_string, &input.new_string))
            .with_context(|| format!("Could not write {}", input.path))?;
        Ok(json!({ "replacements": found }))
    }

    fn delete_file(&self, input: PathInput) -> Result<Value> {
        let path = self.resolve(&input.path);
        let removed = match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(json!({ "deleted": false }));
            }
            Err(error) => Err(error),
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&path),
            Ok(_) => std::fs::remove_file(&path),
        };
        removed.with_context(|| format!("Could not delete {}", input.path))?;
        Ok(json!({ "deleted": true }))
    }

    fn list_dir(&self, input: OptionalPath) -> Result<Value> {
        let path = self.resolve(input.path.as_deref().unwrap_or("."));
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&path)
            .with_context(|| format!("Could not list {}", path.display()))?
        {
            let entry = entry?;
            let kind = entry.file_type()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            entries.push(if kind.is_file() {
                json!({ "name": name, "kind": "file", "size": entry.metadata()?.len() })
            } else {
                let kind = if kind.is_dir() {
                    "dir"
                } else if kind.is_symlink() {
                    "symlink"
                } else {
                    "other"
                };
                json!({ "name": name, "kind": kind })
            });
        }
        entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        let truncated = entries.len() > MAX_ENTRIES;
        entries.truncate(MAX_ENTRIES);
        Ok(json!({ "entries": entries, "truncated": truncated }))
    }

    fn glob(&self, input: Glob) -> Result<Value> {
        let root = self.resolve(input.path.as_deref().unwrap_or("."));
        let matcher = globset::GlobBuilder::new(&input.pattern)
            .literal_separator(true)
            .build()
            .context("Invalid glob pattern")?
            .compile_matcher();
        let (mut paths, mut truncated) = (Vec::new(), false);
        for (relative, _) in walk(&root) {
            if matcher.is_match(&relative) {
                if paths.len() == MAX_ENTRIES {
                    truncated = true;
                    break;
                }
                paths.push(relative);
            }
        }
        Ok(json!({ "paths": paths, "truncated": truncated }))
    }

    fn grep(&self, input: Grep) -> Result<Value> {
        let root = self.resolve(input.path.as_deref().unwrap_or("."));
        let pattern = regex::RegexBuilder::new(&input.pattern)
            .case_insensitive(input.ignore_case)
            .build()
            .context("Invalid regular expression")?;
        // Like ripgrep's --glob: a pattern without a slash matches file names at any depth.
        let filter = match &input.glob {
            Some(glob) => Some((
                globset::Glob::new(glob)
                    .context("Invalid glob pattern")?
                    .compile_matcher(),
                !glob.contains('/'),
            )),
            None => None,
        };
        let max = input.max_results.clamp(1, MAX_ENTRIES);
        let (mut matches, mut truncated) = (Vec::new(), false);
        'files: for (relative, path) in walk(&root) {
            if let Some((matcher, by_name)) = &filter {
                let candidate = if *by_name {
                    relative.rsplit('/').next().unwrap_or(&relative)
                } else {
                    &relative
                };
                if !matcher.is_match(candidate) {
                    continue;
                }
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            // Binary files: a NUL early on, or not UTF-8 at all.
            if bytes[..bytes.len().min(8192)].contains(&0) {
                continue;
            }
            let Ok(text) = std::str::from_utf8(&bytes) else {
                continue;
            };
            for (number, line) in text.lines().enumerate() {
                if !pattern.is_match(line) {
                    continue;
                }
                if matches.len() == max {
                    truncated = true;
                    break 'files;
                }
                let text: String = line.chars().take(MAX_LINE_TEXT).collect();
                matches.push(json!({ "path": relative, "line": number + 1, "text": text }));
            }
        }
        Ok(json!({ "matches": matches, "truncated": truncated }))
    }
}

/// Files under `root` (or `root` itself when it is a file) as (path relative to root, path),
/// honouring .gitignore even outside a git checkout. Hidden files are included; `.git` is not.
fn walk(root: &Path) -> impl Iterator<Item = (String, PathBuf)> {
    let root = root.to_path_buf();
    ignore::WalkBuilder::new(&root)
        .hidden(false)
        .require_git(false)
        .sort_by_file_name(|a, b| a.cmp(b))
        .filter_entry(|entry| entry.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|kind| !kind.is_dir()))
        .map(move |entry| {
            let path = entry.into_path();
            let relative = match path.strip_prefix(&root) {
                Ok(relative) if !relative.as_os_str().is_empty() => relative,
                _ => Path::new(path.file_name().unwrap_or_default()),
            };
            (relative.to_string_lossy().into_owned(), path.clone())
        })
}

fn parse<T: serde::de::DeserializeOwned>(name: &str, input: Value) -> Result<T> {
    serde_json::from_value(input).with_context(|| format!("Invalid input for {name}"))
}

#[derive(Deserialize)]
struct Exec {
    command: String,
    cwd: Option<String>,
    timeout_ms: Option<u64>,
    #[serde(default)]
    background: bool,
}

#[derive(Deserialize)]
struct ExecOutput {
    job_id: String,
    #[serde(default)]
    wait_ms: u64,
}

#[derive(Deserialize)]
struct ReadFile {
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct WriteFile {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct EditFile {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[derive(Deserialize)]
struct PathInput {
    path: String,
}

#[derive(Deserialize)]
struct OptionalPath {
    path: Option<String>,
}

#[derive(Deserialize)]
struct Glob {
    pattern: String,
    path: Option<String>,
}

#[derive(Deserialize)]
struct Grep {
    pattern: String,
    path: Option<String>,
    glob: Option<String>,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default = "default_max_results")]
    max_results: usize,
}

fn default_max_results() -> usize {
    200
}

#[derive(Clone, Copy)]
struct Exit {
    /// None when a signal ended the command.
    code: Option<i32>,
    timed_out: bool,
}

/// A running or finished command and the tail of its output.
struct Job {
    stdout: Arc<Mutex<Tail>>,
    stderr: Arc<Mutex<Tail>>,
    exit: watch::Receiver<Option<Exit>>,
}

#[derive(Default)]
struct Tail {
    bytes: Vec<u8>,
    truncated: bool,
}

impl Job {
    /// `bash -lc` so the login profile (and with it the blueprint's env file) applies, in its own
    /// process group so a timeout takes down everything the command started.
    fn start(
        command: &str,
        cwd: &Path,
        env: &[(String, String)],
        operation: &str,
        timeout: Option<Duration>,
    ) -> Result<Arc<Job>> {
        let mut process = if which("bash") {
            let mut process = Command::new("bash");
            process.arg("-lc");
            process
        } else {
            let mut process = Command::new("sh");
            process.arg("-c");
            process
        };
        process
            .arg(command)
            .current_dir(cwd)
            .envs(env.iter().map(|(name, value)| (name, value)))
            .env_remove("TILDE_SANDBOX_TOKEN")
            .env(OPERATION_ENV, operation)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = process
            .spawn()
            .with_context(|| format!("Could not run the command in {}", cwd.display()))?;
        let stdout = Arc::new(Mutex::new(Tail::default()));
        let stderr = Arc::new(Mutex::new(Tail::default()));
        let readers = [
            tokio::spawn(collect(child.stdout.take(), stdout.clone())),
            tokio::spawn(collect(child.stderr.take(), stderr.clone())),
        ];
        let (done, exit) = watch::channel(None);
        let group = child.id();
        tokio::spawn(async move {
            let waited = match timeout {
                Some(timeout) => tokio::time::timeout(timeout, child.wait()).await.ok(),
                None => Some(child.wait().await),
            };
            let exit = match waited {
                Some(status) => Exit {
                    code: status.ok().and_then(|status| status.code()),
                    timed_out: false,
                },
                None => {
                    if let Some(group) = group {
                        // SAFETY: kill(2) on our own child's process group; no memory involved.
                        unsafe { libc::kill(-(group as i32), libc::SIGKILL) };
                    }
                    let _ = child.wait().await;
                    Exit {
                        code: None,
                        timed_out: true,
                    }
                }
            };
            // Something the command left running in the background can hold its pipes open;
            // give the readers a moment to drain and then report anyway.
            let drained = async {
                for reader in readers {
                    let _ = reader.await;
                }
            };
            let _ = tokio::time::timeout(Duration::from_millis(500), drained).await;
            let _ = done.send(Some(exit));
        });
        Ok(Arc::new(Job {
            stdout,
            stderr,
            exit,
        }))
    }

    async fn finished(&self) -> Exit {
        let mut exit = self.exit.clone();
        let result = exit.wait_for(Option::is_some).await;
        match result {
            Ok(exit) => exit.expect("waited for Some"),
            // The waiting task cannot drop the sender without sending; treat it as killed.
            Err(_) => Exit {
                code: None,
                timed_out: false,
            },
        }
    }

    fn output(&self) -> (String, String, bool) {
        let stdout = self.stdout.lock().unwrap();
        let stderr = self.stderr.lock().unwrap();
        (
            String::from_utf8_lossy(&stdout.bytes).into_owned(),
            String::from_utf8_lossy(&stderr.bytes).into_owned(),
            stdout.truncated || stderr.truncated,
        )
    }
}

async fn collect(pipe: Option<impl tokio::io::AsyncRead + Unpin>, tail: Arc<Mutex<Tail>>) {
    let Some(mut pipe) = pipe else { return };
    let mut buffer = vec![0; 16 * 1024];
    while let Ok(read) = pipe.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        let mut tail = tail.lock().unwrap();
        tail.bytes.extend_from_slice(&buffer[..read]);
        if tail.bytes.len() > TAIL {
            let excess = tail.bytes.len() - TAIL;
            tail.bytes.drain(..excess);
            tail.truncated = true;
        }
    }
}

fn which(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}
