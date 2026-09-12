//! Corrosion is a supervised child, not a second database service the user must
//! configure. Each agent has its own database, cluster CA, API token and peer set.
use super::Client;
use crate::{
    chat::{ChatError, Result},
    proto::tilde::agent_event_ingress::v1::RegisterSidecarResponse,
};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::{Child, Command};
use zeroize::{ZeroizeOnDrop, Zeroizing};
pub struct Process {
    child: Child,
    pub client: Client,
    config: PathBuf,
    binary: PathBuf,
}
#[derive(Serialize)]
struct Config {
    db: Db,
    gossip: Gossip,
    api: Api,
    admin: Admin,
    reaper: Reaper,
}
#[derive(Serialize)]
struct Db {
    path: String,
    schema_paths: Vec<String>,
    cache_size_kib: i64,
}
#[derive(Serialize)]
struct Gossip {
    addr: String,
    bootstrap: Vec<String>,
    tls: Tls,
}
#[derive(Serialize)]
struct Tls {
    cert_file: String,
    key_file: String,
    ca_file: String,
    client: TlsClient,
}
#[derive(Serialize)]
struct TlsClient {
    cert_file: String,
    key_file: String,
}
#[derive(Serialize)]
struct Api {
    addr: String,
    authz: Authz,
}
#[derive(Serialize, ZeroizeOnDrop)]
struct Authz {
    #[serde(rename = "bearer-token")]
    bearer_token: String,
}
#[derive(Serialize)]
struct Admin {
    path: String,
}
#[derive(Serialize)]
struct Reaper {
    check_interval: u64,
    tables: BTreeMap<String, Retention>,
}
#[derive(Serialize)]
struct Retention {
    retention: String,
}
impl Process {
    pub async fn start(
        binary: PathBuf,
        directory: &Path,
        api: SocketAddr,
        gossip: SocketAddr,
        token: &SecretString,
        registration: &RegisterSidecarResponse,
    ) -> Result<Self> {
        tokio::fs::create_dir_all(directory)
            .await
            .map_err(|_| ChatError::Transport)?;
        let directory = tokio::fs::canonicalize(directory)
            .await
            .map_err(|_| ChatError::Transport)?;
        let file = |name: &str| directory.join(name).to_string_lossy().into_owned();
        let files = [
            (
                "runtime.sql",
                include_str!("../../../../../schema/corrosion/runtime.sql"),
            ),
            ("certificate.pem", &registration.tls_certificate),
            ("key.pem", &registration.tls_private_key),
            ("ca.pem", &registration.tls_ca),
        ];
        for (name, contents) in files {
            private_write(&directory.join(name), contents.as_bytes()).await?;
        }
        let config = Config {
            db: Db {
                path: file("state.sqlite"),
                schema_paths: vec![file("runtime.sql")],
                cache_size_kib: -131072,
            },
            gossip: Gossip {
                addr: gossip.to_string(),
                bootstrap: registration
                    .peers
                    .iter()
                    .map(|p| p.gossip_address.clone())
                    .collect(),
                tls: Tls {
                    cert_file: file("certificate.pem"),
                    key_file: file("key.pem"),
                    ca_file: file("ca.pem"),
                    client: TlsClient {
                        cert_file: file("certificate.pem"),
                        key_file: file("key.pem"),
                    },
                },
            },
            api: Api {
                addr: api.to_string(),
                authz: Authz {
                    bearer_token: token.expose_secret().into(),
                },
            },
            admin: Admin {
                path: file("admin.sock"),
            },
            reaper: Reaper {
                check_interval: 3600,
                tables: [
                    "health",
                    "threads",
                    "participants",
                    "assignments",
                    "retirements",
                    "retirement_acks",
                    "typing",
                    "channel_threads",
                    "message_dispatch",
                    "bridge_message_versions",
                    "events",
                    "commands",
                    "control_receipts",
                    "traces",
                    "messages",
                    "runs",
                    "invocations",
                    "goals",
                    "tasks",
                    "tool_calls",
                    "attachments",
                    "converted_messages",
                    "channel_receipts",
                ]
                .into_iter()
                .map(|name| {
                    (
                        name.into(),
                        Retention {
                            retention: "7d".into(),
                        },
                    )
                })
                .collect(),
            },
        };
        let config_path = directory.join("config.toml");
        let encoded = Zeroizing::new(toml::to_string(&config).map_err(|_| ChatError::Transport)?);
        private_write(&config_path, encoded.as_bytes()).await?;
        let child = spawn(&binary, &config_path)?;
        let client = Client::new(&format!("http://{api}"), token.clone())?;
        let mut process = Self {
            child,
            client,
            config: config_path,
            binary,
        };
        process.ready().await?;
        Ok(process)
    }
    pub async fn ready(&mut self) -> Result<()> {
        for _ in 0..100 {
            if self
                .child
                .try_wait()
                .map_err(|_| ChatError::Transport)?
                .is_some()
            {
                return Err(ChatError::Transport);
            }
            if self
                .client
                .query::<serde_json::Value>("SELECT 1 AS ready", vec![])
                .await
                .is_ok()
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(ChatError::Transport)
    }
    pub async fn supervise(&mut self) -> Result<bool> {
        if self
            .child
            .try_wait()
            .map_err(|_| ChatError::Transport)?
            .is_some()
        {
            self.child = spawn(&self.binary, &self.config)?;
            self.ready().await?;
            return Ok(true);
        }
        Ok(false)
    }
    pub async fn stop(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}
fn spawn(binary: &Path, config: &Path) -> Result<Child> {
    Command::new(binary)
        .arg("--config")
        .arg(config)
        .arg("agent")
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|_| ChatError::Transport)
}
async fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path).await.map_err(|_| ChatError::Transport)?;
    file.write_all(bytes)
        .await
        .map_err(|_| ChatError::Transport)?;
    Ok(())
}
