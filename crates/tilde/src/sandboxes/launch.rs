//! A blueprint's sandbox provider: how its VMs are launched, slept, woken and terminated, and how
//! the `tilde sandbox connect` process is started in them. E2B pauses a VM with its memory, so a
//! woken VM's process reconnects by itself; Modal keeps only a filesystem snapshot, so a woken
//! Modal sandbox is a new VM launched from it.
use crate::chat::providers::Access;
use crate::error::Error;
use crate::tools::providers::{e2b, modal};
use connectrpc::ConnectError;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Map, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    E2b,
    Modal,
}
impl Provider {
    pub fn of(provider_id: &str) -> Option<Self> {
        match provider_id {
            "e2b" => Some(Self::E2b),
            "modal" => Some(Self::Modal),
            _ => None,
        }
    }
}
/// How long the provider keeps a VM from launch or renewal before pausing (E2B) or ending
/// (Modal) it by itself.
pub fn lifetime(provider: Provider) -> f64 {
    match provider {
        Provider::E2b => e2b::LIFECYCLE_TTL_SECS as f64,
        Provider::Modal => f64::from(modal::SANDBOX_LIFETIME_SECS),
    }
}
/// What the started process needs to dial back in.
pub struct Enrollment<'a> {
    pub runtime_url: &'a str,
    pub token: &'a SecretString,
}
type Result<T> = std::result::Result<T, Error>;
/// A provider's refusal, as the reason a sandbox could not be launched, woken, slept or
/// terminated.
fn failed(error: ConnectError) -> Error {
    Error::Invalid(match error.message {
        Some(message) => format!("The sandbox provider: {message}"),
        None => "The sandbox provider failed".into(),
    })
}
/// Where a sleeping sandbox wakes from: the paused VM, or a filesystem snapshot.
pub enum Slept {
    Paused(String),
    Snapshot(String),
}

/// A new VM from `image` (the template, or a Modal snapshot) with its process started. Returns
/// the provider's ID for it.
pub async fn launch(
    provider: Provider,
    access: &Access,
    image: &str,
    enrollment: &Enrollment<'_>,
) -> Result<String> {
    match provider {
        Provider::E2b => {
            let id = e2b::launch(access, image).await.map_err(failed)?;
            if let Err(error) = start(provider, access, &id, enrollment).await {
                // Never leave a VM running without a process to reach it.
                let _ = e2b::kill(access, &id).await;
                return Err(error);
            }
            Ok(id)
        }
        // The process is the VM's entrypoint. The token is single-use, so its copy in Modal's
        // sandbox definition is worthless once the process has enrolled.
        Provider::Modal => {
            let command = [
                "env".to_owned(),
                format!("TILDE_URL={}", enrollment.runtime_url),
                format!("TILDE_SANDBOX_TOKEN={}", enrollment.token.expose_secret()),
                "tilde".to_owned(),
                "sandbox".to_owned(),
                "connect".to_owned(),
            ];
            modal::launch(access, image, command.into())
                .await
                .map_err(failed)
        }
    }
}
/// Start the process again in a running VM whose process is gone. Modal sandboxes cannot: their
/// process is the VM.
pub async fn start(
    provider: Provider,
    access: &Access,
    id: &str,
    enrollment: &Enrollment<'_>,
) -> Result<()> {
    if provider == Provider::Modal {
        return Err(Error::Invalid(
            "Modal sandboxes are relaunched rather than restarted".into(),
        ));
    }
    let mut envs = Map::new();
    envs.insert("TILDE_URL".into(), Value::from(enrollment.runtime_url));
    envs.insert(
        "TILDE_SANDBOX_TOKEN".into(),
        Value::from(enrollment.token.expose_secret()),
    );
    e2b::spawn(
        access,
        id,
        "mkdir -p ~/.tilde && exec tilde sandbox connect >> ~/.tilde/sandbox.log 2>&1 < /dev/null",
        &envs,
    )
    .await
    .map_err(failed)
}
/// Wake a paused VM, or renew a running one's lifetime. False when it is gone, or the provider
/// cannot do either (Modal).
pub async fn resume(provider: Provider, access: &Access, id: &str) -> Result<bool> {
    match provider {
        Provider::E2b => e2b::resume(access, id).await.map_err(failed),
        Provider::Modal => Ok(false),
    }
}
/// A Modal snapshot expires after `snapshot_ttl` seconds.
pub async fn sleep(
    provider: Provider,
    access: &Access,
    id: &str,
    snapshot_ttl: i64,
) -> Result<Slept> {
    match provider {
        Provider::E2b => {
            e2b::pause(access, id).await.map_err(failed)?;
            Ok(Slept::Paused(id.to_owned()))
        }
        Provider::Modal => {
            let image = modal::snapshot(access, id, snapshot_ttl)
                .await
                .map_err(failed)?;
            modal::terminate(access, id).await.map_err(failed)?;
            Ok(Slept::Snapshot(image))
        }
    }
}
pub async fn terminate(provider: Provider, access: &Access, id: &str) -> Result<()> {
    match provider {
        Provider::E2b => e2b::kill(access, id).await.map_err(failed),
        Provider::Modal => modal::terminate(access, id).await.map_err(failed),
    }
}
