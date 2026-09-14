//! Per-process durable delivery queues. Mount the directory on persistent storage.
//! A process lock prevents two gateways consuming the same directory.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tokio::sync::Notify;

const MAX_FILES: usize = 4096;
const MAX_AGENT_BYTES: u64 = 32 * 1024 * 1024;
const RETENTION: Duration = Duration::from_secs(86400);
struct Entry {
    bytes: u64,
    created: SystemTime,
    agent: String,
}
struct State {
    entries: BTreeMap<String, Entry>,
    bytes: u64,
}
#[derive(Clone)]
pub struct Spool {
    path: PathBuf,
    state: Arc<Mutex<State>>,
    limit: u64,
    pub changed: Arc<Notify>,
}
pub fn lock(path: &Path) -> io::Result<File> {
    fs::create_dir_all(path)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.join("lock"))?;
    file.try_lock().map_err(io::Error::other)?;
    Ok(file)
}
impl Spool {
    pub fn open(root: &Path, name: &str, destination: &str, limit: u64) -> io::Result<Self> {
        let path = root.join(format!(
            "{name}-{}",
            hex::encode(Sha256::digest(destination.as_bytes()))
        ));
        // Explicit configuration changes discard pending data for the previous destination.
        Self::discard_other(root, name, Some(&path))?;
        fs::create_dir_all(&path)?;
        let mut state = State {
            entries: BTreeMap::new(),
            bytes: 0,
        };
        for file in fs::read_dir(&path)? {
            let file = file?;
            let name = file.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".otlp") {
                fs::remove_file(file.path())?;
                continue;
            }
            let meta = file.metadata()?;
            let agent = name.split('_').next().unwrap_or("").to_owned();
            state.bytes += meta.len();
            state.entries.insert(
                name,
                Entry {
                    bytes: meta.len(),
                    created: meta.modified()?,
                    agent,
                },
            );
        }
        Ok(Self {
            path,
            state: Arc::new(Mutex::new(state)),
            limit,
            changed: Arc::new(Notify::new()),
        })
    }
    pub fn discard_other(root: &Path, name: &str, keep: Option<&Path>) -> io::Result<()> {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{name}-"))
                && keep != Some(entry.path().as_path())
            {
                fs::remove_dir_all(entry.path())?;
                tracing::warn!(
                    destination = name,
                    "Discarded log backlog after configuration change"
                );
            }
        }
        Ok(())
    }
    pub async fn accept(&self, agent: String, bytes: Vec<u8>) -> io::Result<()> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            if bytes.len() > 8 * 1024 * 1024 {
                return Err(io::Error::other("Log batch too large"));
            }
            uuid::Uuid::parse_str(&agent).map_err(io::Error::other)?;
            let key = format!("{agent}_{}.otlp", hex::encode(Sha256::digest(&bytes)));
            let mut state = this.state.lock().unwrap();
            if state.entries.contains_key(&key) {
                return Ok(());
            }
            let agent_bytes: u64 = state
                .entries
                .values()
                .filter(|v| v.agent == agent)
                .map(|v| v.bytes)
                .sum();
            if state.bytes + bytes.len() as u64 > this.limit
                || state.entries.len() >= MAX_FILES
                || agent_bytes + bytes.len() as u64 > MAX_AGENT_BYTES
            {
                return Err(io::Error::other("Log queue capacity exceeded"));
            }
            let target = this.path.join(&key);
            let pending = target.with_extension("tmp");
            let mut file = File::create(&pending)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&pending, &target)?;
            File::open(&this.path)?.sync_all()?;
            state.bytes += bytes.len() as u64;
            state.entries.insert(
                key,
                Entry {
                    bytes: bytes.len() as u64,
                    created: SystemTime::now(),
                    agent,
                },
            );
            this.changed.notify_one();
            Ok(())
        })
        .await
        .map_err(io::Error::other)?
    }
    pub async fn next(&self) -> io::Result<Option<(String, Vec<u8>)>> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut state = this.state.lock().unwrap();
            let expired: Vec<_> = state
                .entries
                .iter()
                .filter(|(_, e)| e.created.elapsed().unwrap_or_default() > RETENTION)
                .map(|(k, _)| k.clone())
                .collect();
            for key in expired {
                fs::remove_file(this.path.join(&key))?;
                if let Some(e) = state.entries.remove(&key) {
                    state.bytes -= e.bytes;
                    tracing::warn!(
                        agent_id = e.agent,
                        bytes = e.bytes,
                        "Expired undelivered logs after 24 hours"
                    );
                }
            }
            let next = state
                .entries
                .iter()
                .min_by_key(|(_, e)| e.created)
                .map(|(k, _)| k.clone());
            next.map(|key| fs::read(this.path.join(&key)).map(|bytes| (key, bytes)))
                .transpose()
        })
        .await
        .map_err(io::Error::other)?
    }
    pub async fn complete(&self, key: String) -> io::Result<()> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut state = this.state.lock().unwrap();
            fs::remove_file(this.path.join(&key))?;
            File::open(&this.path)?.sync_all()?;
            if let Some(e) = state.entries.remove(&key) {
                state.bytes -= e.bytes;
            }
            Ok(())
        })
        .await
        .map_err(io::Error::other)?
    }
}
