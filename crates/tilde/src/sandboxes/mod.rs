//! Sandboxes: VMs Tilde launches for agents through a sandbox provider connection (E2B or Modal).
//!
//! A blueprint configures them: the connection and template to launch, encrypted environment
//! variables for every shell, tools that processes inside may call (the blueprint's tool sources)
//! and how sandboxes are reused. An agent whose sandbox setting names a blueprint runs every
//! invocation with a sandbox: before the agent is woken, `ensure` finds the invocation's sandbox by
//! the blueprint's reuse mode and launches or wakes it until its `tilde sandbox connect` process has
//! registered. The agent gets the fixed sandbox tools (`tools`) as its `sandbox` source.
//!
//! The process is never dialed. It enrolls with the single-use token it was started with, keeps
//! the session token it receives, and holds a Connect stream over which it receives operations;
//! operations cross gateway processes through the transient `sandbox_calls` queue, as tool host
//! calls do. VM transitions (launch, wake, sleep, terminate) are made by whichever gateway process
//! holds the sandbox's lease. The sweeper sleeps idle sandboxes and terminates those idle past
//! retention or no longer wanted (`sweep_due.sql`); rows keep their VM's provider ID until it is
//! terminated, so nothing is cascaded away while it still runs.
pub mod db;
pub mod launch;
pub mod rpc;
pub mod tools;

use crate::chat::{Scope, providers::Access, tools::ToolResult};
use crate::connections::{model::invalid, service::Connections};
use crate::database::notifications::Notifications;
use crate::encryption::{SealedSecret, SecretBinding};
use crate::error::Error;
use crate::tools::{Owner, Target, ToolSettings};
use connectrpc::ConnectError;
use launch::{Enrollment, Provider, Slept};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

/// A sandbox is reachable while its process's stream refreshed `connected_at` this recently.
pub(crate) const LIVENESS_SECS: f64 = 30.0;
/// Covers a launch, an image build included, and the longest wait for the process to register.
const LEASE_SECS: f64 = 1800.0;
/// Thirty days, the longest terminate_after, and a day.
const SNAPSHOT_TTL_SECS: i64 = 31 * 86_400;
/// How long a woken E2B VM's own process gets to reconnect before another is started.
const RECONNECT_WAIT: Duration = Duration::from_secs(20);
const NOTIFY_CHANNEL: &str = "tilde_sandbox_calls";
pub const REUSE_MODES: [&str; 5] = [
    "thread",
    "agent",
    "agent_identity",
    "global",
    "global_identity",
];

#[derive(Clone)]
pub struct Sandboxes {
    connections: Connections,
    /// The runtime origin sandbox processes dial back to.
    runtime_url: String,
    calls: Arc<Notifications>,
}
pub struct Blueprint {
    pub id: Uuid,
    pub name: String,
    pub connection_id: Uuid,
    pub template: String,
    pub reuse: String,
    pub timings: Timings,
    pub env_names: Vec<String>,
    pub agent_ids: Vec<Uuid>,
}
/// A blueprint's timings, in seconds: a running sandbox sleeps after `sleep_after` unused, any is
/// terminated after `terminate_after` unused, and a launch or wake waits `connect_timeout` for the
/// process to connect. In a change, zero keeps the current value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Timings {
    pub sleep_after: u32,
    pub terminate_after: u32,
    pub connect_timeout: u32,
}
impl Default for Timings {
    fn default() -> Self {
        Self {
            sleep_after: 600,
            terminate_after: 7 * 86_400,
            connect_timeout: 120,
        }
    }
}
impl Timings {
    /// `self` with the non-zero fields of `change`, if the result is in range.
    fn with(self, change: Timings) -> Result<Self, Error> {
        let pick = |new: u32, old: u32| if new == 0 { old } else { new };
        let timings = Self {
            sleep_after: pick(change.sleep_after, self.sleep_after),
            terminate_after: pick(change.terminate_after, self.terminate_after),
            connect_timeout: pick(change.connect_timeout, self.connect_timeout),
        };
        let valid = (60..=86_400).contains(&timings.sleep_after)
            && (3600..=30 * 86_400).contains(&timings.terminate_after)
            && timings.terminate_after >= timings.sleep_after
            && (10..=600).contains(&timings.connect_timeout);
        if !valid {
            return Err(invalid(
                "Sleep after is 1 minute to 1 day, terminate after 1 hour to 30 days and no shorter, and the connect timeout 10 seconds to 10 minutes",
            ));
        }
        Ok(timings)
    }
}
/// Which sandbox an invocation uses: the blueprint and reuse mode, and the agent, thread and person
/// that mode keys by.
pub struct Key {
    pub blueprint: Uuid,
    pub reuse: String,
    pub connection: Uuid,
    pub agent: Option<Uuid>,
    pub thread: Option<Uuid>,
    pub identity: Option<Uuid>,
}
pub(crate) fn digest(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}
fn env_binding<'a>(blueprint: Uuid, name: &'a str) -> SecretBinding<'a> {
    SecretBinding {
        resource_kind: "sandbox_blueprint_env",
        resource_id: blueprint,
        name,
    }
}
fn text(value: &str, field: &str, max: usize) -> Result<String, Error> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max {
        return Err(invalid(&format!(
            "A blueprint {field} is 1 to {max} characters"
        )));
    }
    Ok(value.to_owned())
}
fn reuse(value: &str) -> Result<&str, Error> {
    REUSE_MODES
        .contains(&value)
        .then_some(value)
        .ok_or_else(|| invalid("Unknown sandbox reuse mode"))
}

impl Sandboxes {
    pub fn new(connections: Connections, runtime_url: String) -> Self {
        Self {
            connections,
            runtime_url,
            calls: Arc::default(),
        }
    }
    pub async fn blueprints(&self, search: Option<&str>) -> Result<Vec<Blueprint>, Error> {
        self.read(None, search).await
    }
    pub async fn blueprint(&self, id: Uuid) -> Result<Blueprint, Error> {
        self.read(Some(id), None)
            .await?
            .pop()
            .ok_or(Error::NotFound)
    }
    async fn read(&self, id: Option<Uuid>, search: Option<&str>) -> Result<Vec<Blueprint>, Error> {
        Ok(
            db::blueprint_list_all(&self.connections.pool.get().await?, id, search)
                .await?
                .into_iter()
                .map(|r| Blueprint {
                    id: r.id,
                    name: r.name,
                    connection_id: r.connection_id,
                    template: r.template,
                    reuse: r.reuse,
                    timings: Timings {
                        sleep_after: r.sleep_after_secs as u32,
                        terminate_after: r.terminate_after_secs as u32,
                        connect_timeout: r.connect_timeout_secs as u32,
                    },
                    env_names: r.env_names,
                    agent_ids: r.agent_ids,
                })
                .collect(),
        )
    }
    /// Blueprints launch through a ready, installation-owned E2B or Modal connection.
    async fn check_connection(&self, connection: Uuid) -> Result<(), Error> {
        let row = db::launch_connection_opt(&self.connections.pool.get().await?, connection)
            .await?
            .ok_or_else(|| invalid("Connection not found"))?;
        if Provider::of(&row.provider_id).is_none() {
            return Err(invalid(
                "Sandboxes launch through an E2B or Modal connection",
            ));
        }
        if row.status != "ready" {
            return Err(invalid("The connection is not ready"));
        }
        Ok(())
    }
    pub async fn create_blueprint(
        &self,
        name: &str,
        connection: Uuid,
        template: &str,
        reuse_mode: &str,
        timings: Timings,
    ) -> Result<Blueprint, Error> {
        let name = text(name, "name", 128)?;
        let template = text(template, "template", 512)?;
        let timings = Timings::default().with(timings)?;
        self.check_connection(connection).await?;
        let id = Uuid::new_v4();
        if db::blueprint_insert_execute(
            &self.connections.pool.get().await?,
            id,
            &name,
            connection,
            &template,
            reuse(reuse_mode)?,
            timings,
        )
        .await?
            == 0
        {
            return Err(invalid("Another blueprint has this name"));
        }
        self.blueprint(id).await
    }
    /// Sandboxes no longer wanted under the new settings are terminated by the sweep it starts.
    pub async fn update_blueprint(
        &self,
        id: Uuid,
        name: Option<&str>,
        connection: Option<Uuid>,
        template: Option<&str>,
        reuse_mode: Option<&str>,
        timings: Option<Timings>,
    ) -> Result<Blueprint, Error> {
        let name = name.map(|n| text(n, "name", 128)).transpose()?;
        let template = template.map(|t| text(t, "template", 512)).transpose()?;
        let reuse_mode = reuse_mode.map(reuse).transpose()?;
        if let Some(connection) = connection {
            self.check_connection(connection).await?;
        }
        let current = self.blueprint(id).await?;
        let timings = timings.map(|t| current.timings.with(t)).transpose()?;
        if db::blueprint_update_execute(
            &self.connections.pool.get().await?,
            id,
            name.as_deref(),
            connection,
            template.as_deref(),
            reuse_mode,
            timings,
        )
        .await?
            == 0
        {
            return Err(invalid("Another blueprint has this name"));
        }
        if reuse_mode.is_some_and(|r| r != current.reuse)
            || connection.is_some_and(|c| c != current.connection_id)
            || timings.is_some_and(|t| t != current.timings)
        {
            self.sweep_soon();
        }
        self.blueprint(id).await
    }
    /// Refused while an agent uses it; its sandboxes are terminated first.
    pub async fn delete_blueprint(&self, id: Uuid) -> Result<(), Error> {
        let blueprint = self.blueprint(id).await?;
        if !blueprint.agent_ids.is_empty() {
            return Err(invalid(
                "Agents use this blueprint; remove it from their sandbox settings first",
            ));
        }
        for sandbox in self.list(Some(id), None).await? {
            self.terminate(sandbox.id).await?;
        }
        match db::blueprint_delete_execute(&self.connections.pool.get().await?, id).await? {
            0 => Err(invalid(
                "Agents use this blueprint; remove it from their sandbox settings first",
            )),
            _ => Ok(()),
        }
    }
    /// Sandboxes receive the current environment whenever their process connects.
    pub async fn set_env(
        &self,
        blueprint: Uuid,
        name: &str,
        value: &SecretString,
    ) -> Result<Blueprint, Error> {
        let valid = name.len() <= 128
            && name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.to_ascii_uppercase().starts_with("TILDE_");
        if !valid {
            return Err(invalid(
                "Variable names are letters, digits and underscores, not starting with a digit or TILDE_",
            ));
        }
        if value.expose_secret().len() > 32 * 1024 {
            return Err(invalid("A variable's value is at most 32 KiB"));
        }
        self.blueprint(blueprint).await?;
        let sealed = self
            .connections
            .crypto
            .seal(env_binding(blueprint, name), value)?
            .into_bytes();
        db::env_set_execute(
            &self.connections.pool.get().await?,
            blueprint,
            name,
            &sealed,
        )
        .await?;
        self.blueprint(blueprint).await
    }
    pub async fn delete_env(&self, blueprint: Uuid, name: &str) -> Result<Blueprint, Error> {
        db::env_delete_execute(&self.connections.pool.get().await?, blueprint, name).await?;
        self.blueprint(blueprint).await
    }
    /// The blueprint's environment, opened for a connecting sandbox process.
    pub(crate) async fn env(&self, blueprint: Uuid) -> Result<Vec<(String, SecretString)>, Error> {
        db::env_all(&self.connections.pool.get().await?, blueprint)
            .await?
            .into_iter()
            .map(|row| {
                let value = self.connections.crypto.open(
                    env_binding(blueprint, &row.name),
                    SealedSecret::from_bytes(&row.encrypted_value)?,
                )?;
                Ok((row.name, value))
            })
            .collect()
    }
    pub async fn agent_sandbox(&self, agent: Uuid) -> Result<Option<db::AgentSandboxRow>, Error> {
        Ok(db::agent_sandbox_get_opt(&self.connections.pool.get().await?, agent).await?)
    }
    /// Give the agent a sandbox of `blueprint`, with its sandbox tool source (every sandbox tool)
    /// when it has none yet. Its sandboxes of another blueprint are terminated by the sweep.
    pub async fn set_agent_sandbox(
        &self,
        agent: Uuid,
        blueprint: Uuid,
    ) -> Result<db::AgentSandboxRow, Error> {
        self.blueprint(blueprint).await?;
        let mut client = self.connections.pool.get().await?;
        let tx = client.transaction().await?;
        db::agent_sandbox_set_execute(&tx, agent, blueprint)
            .await
            .map_err(|e| match e.is_foreign_key_violation() {
                true => Error::NotFound,
                false => e.into(),
            })?;
        if db::agent_sandbox_get_opt(&tx, agent)
            .await?
            .is_some_and(|row| row.source_id.is_none())
        {
            let source =
                crate::tools::insert_source(&tx, Owner::Agent(agent), Target::Sandbox, "sandbox")
                    .await?;
            for tool in tools::definitions() {
                crate::tools::db::agent_tool_set_execute(
                    &tx,
                    source,
                    &tool.name,
                    &tool.name,
                    &ToolSettings::default(),
                )
                .await?;
            }
        }
        tx.commit().await?;
        drop(client);
        self.sweep_soon();
        self.agent_sandbox(agent).await?.ok_or(Error::NotFound)
    }
    /// Its sandbox tool source goes with it; its sandboxes are terminated by the sweep.
    pub async fn remove_agent_sandbox(&self, agent: Uuid) -> Result<(), Error> {
        db::agent_sandbox_delete_execute(&self.connections.pool.get().await?, agent).await?;
        self.sweep_soon();
        Ok(())
    }
    pub async fn list(
        &self,
        blueprint: Option<Uuid>,
        agent: Option<Uuid>,
    ) -> Result<Vec<db::SandboxListRow>, Error> {
        Ok(db::sandbox_list_all(
            &self.connections.pool.get().await?,
            blueprint,
            agent,
            LIVENESS_SECS,
        )
        .await?)
    }
    /// Terminate the VM at its provider and forget it. Refused while another process is
    /// launching or waking it.
    pub async fn terminate(&self, id: Uuid) -> Result<(), Error> {
        if db::sandbox_lease_opt(&self.connections.pool.get().await?, id, LEASE_SECS)
            .await?
            .is_none()
        {
            return Err(Error::Conflict);
        }
        self.terminate_leased(id).await
    }

    /// The sandbox `invocation` of `agent` uses, launched or woken until its process has
    /// registered; `None` when the agent has no sandbox.
    pub(crate) async fn ensure(
        &self,
        agent: Uuid,
        thread: Uuid,
        run: Uuid,
        invocation: Uuid,
    ) -> Result<Option<Uuid>, Error> {
        let sandbox = self.wake(agent, thread, run).await?;
        if let Some(sandbox) = sandbox {
            db::sandbox_touch_execute(&self.connections.pool.get().await?, sandbox, invocation)
                .await?;
        }
        Ok(sandbox)
    }
    async fn wake(&self, agent: Uuid, thread: Uuid, run: Uuid) -> Result<Option<Uuid>, Error> {
        let pool = &self.connections.pool;
        let Some(setting) = db::agent_sandbox_get_opt(&pool.get().await?, agent).await? else {
            return Ok(None);
        };
        let key = self.key(&setting, agent, thread, run).await?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs_f64(LEASE_SECS);
        loop {
            let client = pool.get().await?;
            let id = match db::sandbox_find_opt(&client, &key).await? {
                Some(id) => id,
                None => {
                    let id = Uuid::new_v4();
                    // Created leased: this process launches it. Another may have just created
                    // one with this key, which the next turn finds.
                    if let Some(since) =
                        db::sandbox_insert_opt(&client, id, &key, LEASE_SECS).await?
                    {
                        drop(client);
                        self.start(id, since).await?;
                        return Ok(Some(id));
                    }
                    continue;
                }
            };
            let row = db::sandbox_get_opt(&client, id, LIVENESS_SECS, chrono::Utc::now())
                .await?
                .ok_or(Error::NotFound)?;
            // A sandbox another process is sleeping or terminating is waited for.
            if row.status == "running" && row.live && !row.leased {
                return Ok(Some(id));
            }
            if let Some(since) = db::sandbox_lease_opt(&client, id, LEASE_SECS).await? {
                drop(client);
                self.start(id, since).await?;
                return Ok(Some(id));
            }
            drop(client);
            if tokio::time::Instant::now() > deadline {
                return Err(invalid("The sandbox is still starting"));
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    /// The person-keyed modes key by the invocation's verified person (as personal tools resolve
    /// them, rolled up to the root identity) and fall back to the thread without one.
    async fn key(
        &self,
        setting: &db::AgentSandboxRow,
        agent: Uuid,
        thread: Uuid,
        run: Uuid,
    ) -> Result<Key, Error> {
        let person = match setting.reuse.as_str() {
            "agent_identity" | "global_identity" => crate::tools::db::invocation_user_opt(
                &self.connections.pool.get().await?,
                run,
                thread,
            )
            .await?
            .map(|user| user.root_identity_id.unwrap_or(user.id)),
            _ => None,
        };
        let (agent, thread, identity) = match (setting.reuse.as_str(), person) {
            ("agent", _) => (Some(agent), None, None),
            ("global", _) => (None, None, None),
            ("agent_identity", Some(person)) => (Some(agent), None, Some(person)),
            ("global_identity", Some(person)) => (None, None, Some(person)),
            _ => (Some(agent), Some(thread), None),
        };
        Ok(Key {
            blueprint: setting.blueprint_id,
            reuse: setting.reuse.clone(),
            connection: setting.connection_id,
            agent,
            thread,
            identity,
        })
    }
    async fn access(&self, connection: Uuid) -> Result<(Provider, Access), Error> {
        let row = db::launch_connection_opt(&self.connections.pool.get().await?, connection)
            .await?
            .ok_or(Error::NotFound)?;
        let provider = Provider::of(&row.provider_id)
            .ok_or_else(|| invalid("Sandboxes launch through an E2B or Modal connection"))?;
        Ok((provider, self.connections.access(connection).await?))
    }
    /// A fresh single-use token for the process about to be started.
    async fn enroll(&self, id: Uuid) -> Result<SecretString, Error> {
        let token = crate::connections::model::random_secret();
        db::sandbox_enrollment_execute(
            &self.connections.pool.get().await?,
            id,
            &digest(token.expose_secret()),
        )
        .await?;
        Ok(token)
    }
    /// Launch or wake the sandbox while holding its lease, releasing it as running or failed.
    async fn start(&self, id: Uuid, since: chrono::DateTime<chrono::Utc>) -> Result<(), Error> {
        let result = self.bring_up(id, since).await;
        let (status, error) = match &result {
            Ok(()) => ("running", String::new()),
            Err(error) => ("failed", error.to_string().chars().take(2048).collect()),
        };
        db::sandbox_settle_execute(&self.connections.pool.get().await?, id, status, &error).await?;
        result
    }
    async fn bring_up(&self, id: Uuid, since: chrono::DateTime<chrono::Utc>) -> Result<(), Error> {
        let row = db::sandbox_get_opt(
            &self.connections.pool.get().await?,
            id,
            LIVENESS_SECS,
            since,
        )
        .await?
        .ok_or(Error::NotFound)?;
        if row.live {
            return Ok(());
        }
        let (provider, access) = self.access(row.connection_id).await?;
        let runtime_url = self.runtime_url.as_str();
        if let Some(vm) = row.provider_sandbox_id.as_deref() {
            // A paused E2B VM resumes with its process, which reconnects by itself; otherwise
            // another process is started in it.
            if launch::resume(provider, &access, vm).await? {
                db::sandbox_renewed_execute(
                    &self.connections.pool.get().await?,
                    id,
                    launch::lifetime(provider),
                )
                .await?;
                if self.registered(id, since, RECONNECT_WAIT).await? {
                    return Ok(());
                }
                let token = self.enroll(id).await?;
                launch::start(
                    provider,
                    &access,
                    vm,
                    &Enrollment {
                        runtime_url,
                        token: &token,
                    },
                )
                .await?;
                return self.wait_registered(id, since, &row).await;
            }
            // A Modal VM whose process is gone is gone with it.
            if provider == Provider::Modal {
                launch::terminate(provider, &access, vm).await?;
            }
        }
        let token = self.enroll(id).await?;
        let image = row.snapshot_id.as_deref().unwrap_or(&row.template);
        let vm = launch::launch(
            provider,
            &access,
            image,
            &Enrollment {
                runtime_url,
                token: &token,
            },
        )
        .await?;
        db::sandbox_launched_execute(
            &self.connections.pool.get().await?,
            id,
            &vm,
            launch::lifetime(provider),
        )
        .await?;
        self.wait_registered(id, since, &row).await
    }
    /// Up to the blueprint's connect timeout.
    async fn wait_registered(
        &self,
        id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
        row: &db::SandboxRow,
    ) -> Result<(), Error> {
        let timeout = Duration::from_secs(row.connect_timeout_secs as u64);
        if self.registered(id, since, timeout).await? {
            return Ok(());
        }
        Err(invalid(&format!(
            "The sandbox's tilde process did not connect within {} seconds. Check that the image has the tilde CLI on its PATH and can reach {}",
            timeout.as_secs(),
            self.runtime_url
        )))
    }
    /// Whether the process connected after `since`, waiting up to `wait`.
    async fn registered(
        &self,
        id: Uuid,
        since: chrono::DateTime<chrono::Utc>,
        wait: Duration,
    ) -> Result<bool, Error> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            let row = db::sandbox_get_opt(
                &self.connections.pool.get().await?,
                id,
                LIVENESS_SECS,
                since,
            )
            .await?
            .ok_or(Error::NotFound)?;
            if row.registered {
                return Ok(true);
            }
            if tokio::time::Instant::now() > deadline {
                return Ok(false);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    /// Sleep a leased, running sandbox. A failure keeps it running and postpones the next try.
    async fn sleep(&self, id: Uuid) -> Result<(), Error> {
        let pool = &self.connections.pool;
        let Some(row) =
            db::sandbox_get_opt(&pool.get().await?, id, LIVENESS_SECS, chrono::Utc::now()).await?
        else {
            return Ok(());
        };
        let slept = async {
            let vm = row
                .provider_sandbox_id
                .as_deref()
                .ok_or_else(|| invalid("The sandbox has no VM"))?;
            let (provider, access) = self.access(row.connection_id).await?;
            // A snapshot outlives the longest retention a blueprint may set, so a sleeping sandbox
            // can always wake, even after its blueprint's retention is raised.
            launch::sleep(provider, &access, vm, SNAPSHOT_TTL_SECS).await
        }
        .await;
        let client = pool.get().await?;
        match slept {
            Ok(Slept::Paused(vm)) => {
                db::sandbox_slept_execute(&client, id, Some(&vm), None).await?;
            }
            Ok(Slept::Snapshot(image)) => {
                db::sandbox_slept_execute(&client, id, None, Some(&image)).await?;
            }
            Err(error) => {
                let error: String = error.to_string().chars().take(2048).collect();
                db::sandbox_settle_execute(&client, id, "running", &error).await?;
            }
        }
        Ok(())
    }
    /// Terminate a leased sandbox's VM and delete it. A VM that cannot be reached keeps its row,
    /// so the sweeper retries instead of losing track of it.
    async fn terminate_leased(&self, id: Uuid) -> Result<(), Error> {
        let pool = &self.connections.pool;
        let Some(row) =
            db::sandbox_get_opt(&pool.get().await?, id, LIVENESS_SECS, chrono::Utc::now()).await?
        else {
            return Ok(());
        };
        if let Some(vm) = row.provider_sandbox_id.as_deref() {
            let terminated = async {
                let (provider, access) = self.access(row.connection_id).await?;
                launch::terminate(provider, &access, vm).await
            }
            .await;
            if let Err(error) = terminated {
                let message: String = error.to_string().chars().take(2048).collect();
                db::sandbox_settle_execute(&pool.get().await?, id, &row.status, &message).await?;
                return Err(error);
            }
        }
        db::sandbox_delete_execute(&pool.get().await?, id).await?;
        Ok(())
    }
    /// Sleep idle sandboxes and terminate those idle past retention or no longer wanted.
    pub async fn sweep(&self) -> Result<(), Error> {
        let pool = &self.connections.pool;
        let due = {
            let mut client = pool.get().await?;
            let tx = client.transaction().await?;
            // One sweeper at a time; each transition still takes the sandbox's own lease.
            if !db::sweep_lock_one(&tx).await? {
                return Ok(());
            }
            db::calls_expire_execute(&tx).await?;
            let due = db::sweep_due_all(&tx, None).await?;
            tx.commit().await?;
            due
        };
        for row in due {
            let client = pool.get().await?;
            if db::sandbox_lease_opt(&client, row.id, LEASE_SECS)
                .await?
                .is_none()
            {
                continue;
            }
            // A call may have used it since the list was read.
            let Some(row) = db::sweep_due_all(&client, Some(row.id)).await?.pop() else {
                db::sandbox_release_execute(&client, row.id).await?;
                continue;
            };
            drop(client);
            let done = match row.action.as_str() {
                "sleep" => self.sleep(row.id).await,
                _ => self.terminate_leased(row.id).await,
            };
            if let Err(error) = done {
                tracing::warn!(sandbox = %row.id, %error, "Sandbox sweep step failed");
            }
        }
        for row in db::renew_due_all(&pool.get().await?, None).await? {
            if let Err(error) = self.renew(row.id).await {
                tracing::warn!(sandbox = %row.id, %error, "Sandbox renewal failed");
            }
        }
        Ok(())
    }
    /// Keep a running VM from the provider's own pause or end: an E2B VM's TTL is renewed; a
    /// Modal VM, which cannot be, is put to sleep before its lifetime ends so its files survive,
    /// once no operation is in flight. Done under the sandbox's lease, so it never resumes a VM
    /// another process has just put to sleep.
    async fn renew(&self, id: Uuid) -> Result<(), Error> {
        let pool = &self.connections.pool;
        let client = pool.get().await?;
        if db::sandbox_lease_opt(&client, id, LEASE_SECS)
            .await?
            .is_none()
        {
            return Ok(());
        }
        let Some(row) = db::renew_due_all(&client, Some(id)).await?.pop() else {
            db::sandbox_release_execute(&client, id).await?;
            return Ok(());
        };
        drop(client);
        let (provider, access) = self.access(row.connection_id).await?;
        if provider == Provider::Modal && !row.busy {
            return self.sleep(id).await;
        }
        let renewed = async {
            if provider == Provider::E2b
                && launch::resume(provider, &access, &row.provider_sandbox_id).await?
            {
                db::sandbox_renewed_execute(&pool.get().await?, id, launch::lifetime(provider))
                    .await?;
            }
            Ok::<_, Error>(())
        }
        .await;
        db::sandbox_release_execute(&pool.get().await?, id).await?;
        renewed
    }
    /// After a settings change, terminate what it made unwanted without waiting for the timer.
    fn sweep_soon(&self) {
        let sandboxes = self.clone();
        tokio::spawn(async move {
            if let Err(error) = sandboxes.sweep().await {
                tracing::warn!(%error, "Sandbox sweep failed");
            }
        });
    }
    /// Sweep every minute until shutdown.
    pub async fn run(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut poll = tokio::time::interval(Duration::from_secs(60));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                _ = poll.tick() => {
                    if let Err(error) = self.sweep().await {
                        tracing::warn!(%error, "Sandbox sweep failed");
                    }
                }
            }
        }
    }

    /// One of the agent's sandbox tools, in the invocation's sandbox (woken if it slept).
    pub(crate) async fn invoke(
        &self,
        scope: &Scope,
        tool: &str,
        input: Value,
    ) -> ToolResult<Value> {
        let sandbox = self
            .ensure(scope.agent_id, scope.thread_id, scope.run_id, scope.id)
            .await?
            .ok_or_else(|| ConnectError::failed_precondition("The agent has no sandbox"))?;
        tools::invoke(self, sandbox, scope.id, tool, input).await
    }
    /// Queue an operation of `invocation` for the sandbox's process and wait for its answer. It
    /// carries the current trace context: the agent tool call it serves.
    pub(crate) async fn operate(
        &self,
        sandbox: Uuid,
        invocation: Uuid,
        operation: &str,
        input: &Value,
        timeout: Duration,
    ) -> ToolResult<Value> {
        let pool = &self.connections.pool;
        let id = Uuid::new_v4();
        let trace = crate::telemetry::tracing::context::capture();
        // Subscribe before the insert so the completion cannot be missed.
        let mut changes = self
            .calls
            .subscribe(pool, NOTIFY_CHANNEL)
            .await
            .map_err(Error::from)?;
        changes.borrow_and_update();
        let client = pool.get().await.map_err(Error::from)?;
        db::call_insert_execute(
            &client,
            id,
            sandbox,
            operation,
            &input.to_string(),
            invocation,
            &trace,
        )
        .await
        .map_err(Error::from)?;
        db::sandbox_touch_execute(&client, sandbox, invocation)
            .await
            .map_err(Error::from)?;
        drop(client);
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let done = db::call_take_opt(&pool.get().await.map_err(Error::from)?, id)
                .await
                .map_err(Error::from)?;
            if let Some(done) = done {
                if done.status == "failed" {
                    return Err(ConnectError::unknown(done.error));
                }
                return serde_json::from_str(&done.output_json)
                    .map_err(|_| ConnectError::unknown("The sandbox returned invalid output"));
            }
            // The slow tick covers a notification lost to a listener reconnect, and keeps a
            // long operation's sandbox counted as in use.
            tokio::select! {
                _ = changes.changed() => { changes.borrow_and_update(); }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    db::sandbox_touch_execute(&pool.get().await.map_err(Error::from)?, sandbox, invocation)
                        .await
                        .map_err(Error::from)?;
                }
                _ = tokio::time::sleep_until(deadline) => {
                    // The process may still run it; without the row its late Respond finds nothing.
                    db::call_abandon_execute(&pool.get().await.map_err(Error::from)?, id)
                        .await
                        .map_err(Error::from)?;
                    return Err(ConnectError::deadline_exceeded("The sandbox did not answer in time"));
                }
            }
        }
    }
}
