//! Agent deployment settings and the gateway half of the sidecar protocol.
//! Postgres is the record and the gateway its only writer. A replica caches the
//! threads it holds a lease on, executes the agent for them, and ships typed
//! events back asynchronously. Replicas dial in; the gateway never dials out.
pub mod attachments;
pub mod gateway;
mod hydrate;
pub mod local_run;
pub mod logs;
pub mod project;
pub mod provider_events;
pub mod public;
pub mod recovery;
mod relay;
pub mod routing;
pub mod rpc;
pub mod run;
pub mod runtime;
mod secrets;
pub(crate) use secrets::random_secret;
pub mod sidecar;
pub mod telemetry;
pub mod tokens;
pub mod wake;
use crate::proto::tilde::{agent_event_ingress::v1 as wire, types::v1 as types};
use crate::{
    agent::Agents,
    chat::Chat,
    connections::service::Connections,
    database::notifications::Notifications,
    encryption::{Encryption, SealedSecret, SecretBinding},
    error::Error,
};
use buffa::Message;
use chrono::Utc;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Owners that miss this window are treated as gone.
pub const LIVENESS: Duration = Duration::from_secs(15);
#[derive(Default)]
pub(crate) struct Channels {
    pub directives: Notifications,
    pub leases: Notifications,
    pub configuration: Notifications,
    pub recovery: Notifications,
}
#[derive(Clone)]
pub struct Deployments {
    pub(crate) pool: PgPool,
    pub(crate) encryption: Arc<Encryption>,
    pub(crate) agents: Agents,
    pub(crate) connections: Connections,
    pub(crate) logs: Option<crate::logs::Delivery>,
    pub(crate) telemetry: Option<crate::telemetry::delivery::Queue>,
    pub(crate) channels: Arc<Channels>,
}
impl Deployments {
    pub fn new(
        pool: PgPool,
        encryption: Arc<Encryption>,
        agents: Agents,
        connections: Connections,
    ) -> Self {
        Self {
            pool,
            encryption,
            agents,
            connections,
            telemetry: None,
            logs: None,
            channels: Arc::default(),
        }
    }
    pub fn with_logs(mut self, logs: crate::logs::Delivery) -> Self {
        self.logs = Some(logs);
        self
    }
    pub fn with_telemetry(mut self, queue: crate::telemetry::delivery::Queue) -> Self {
        self.telemetry = Some(queue);
        self
    }
    pub(crate) fn chat(&self) -> Chat {
        Chat::new(self.pool.clone(), self.encryption.clone(), String::new())
            .with_connections(self.connections.clone())
            .with_deployments(self.clone())
            .with_objects(self.agents.object_store().cloned())
    }
    pub async fn get(&self, agent: Uuid) -> Result<types::Deployment, Error> {
        let row = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(types::Deployment {
            agent_id: agent.to_string(),
            mode: mode_wire(&row.deployment_mode).into(),
            failure_mode: if row.failure_mode == "stop" {
                types::SidecarFailureMode::Stop
            } else {
                types::SidecarFailureMode::Reassign
            }
            .into(),
            routing: if row.routing == "manual" {
                types::DeploymentRouting::Manual
            } else {
                types::DeploymentRouting::Latest
            }
            .into(),
            serving_deployment_id: row
                .serving_deployment_id
                .map(|v| v.to_string())
                .unwrap_or_default(),
            ..Default::default()
        })
    }
    pub async fn is_sidecar(&self, agent: Uuid) -> Result<bool, Error> {
        Ok(sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_optional(&self.pool)
            .await?
            .is_some_and(|r| r.deployment_mode == "sidecar"))
    }
    /// Agent-level execution settings. Changing the mode needs a paused, idle agent.
    /// Deployments are not retired by a mode change; the newest one of the matching
    /// target starts serving, or nothing serves until one is registered.
    pub async fn set(
        &self,
        agent: Uuid,
        mode: types::DeploymentMode,
        failure: types::SidecarFailureMode,
        routing: types::DeploymentRouting,
    ) -> Result<types::Deployment, Error> {
        let mode = match mode {
            types::DeploymentMode::Gateway => "gateway",
            types::DeploymentMode::Sidecar => "sidecar",
            _ => {
                return Err(Error::Invalid(
                    "Select gateway or sidecar deployment".into(),
                ));
            }
        };
        let failure = match failure {
            types::SidecarFailureMode::Stop => "stop",
            types::SidecarFailureMode::Reassign => "reassign",
            _ => return Err(Error::Invalid("Select a failure policy".into())),
        };
        let routing = match routing {
            types::DeploymentRouting::Manual => "manual",
            _ => "latest",
        };
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_file!("../../queries/deployment/lock.sql", agent)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        if current.deployment_mode != mode {
            if !current.paused {
                return Err(Error::AgentNotPaused);
            }
            if sqlx::query_file!("../../queries/deployment/active_work.sql", agent)
                .fetch_one(&mut *tx)
                .await?
                .active
            {
                return Err(Error::Invalid(
                    "Wait for active work to stop before changing deployment".into(),
                ));
            }
            sqlx::query_file!("../../queries/deployment/leases_clear.sql", agent)
                .execute(&mut *tx)
                .await?;
            mirror_flag(&mut tx).await?;
            sqlx::query_file!(
                "../../queries/deployment/set.sql",
                agent,
                mode,
                None::<String>
            )
            .execute(&mut *tx)
            .await?;
            let latest = sqlx::query_file!(
                "../../queries/deployment/deployment_latest.sql",
                agent,
                target_for_mode(mode)
            )
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| r.id);
            match latest {
                Some(next) => self.promote_in(&mut tx, agent, next).await?,
                None => {
                    sqlx::query_file!("../../queries/deployment/serving_clear.sql", agent)
                        .execute(&mut *tx)
                        .await?;
                }
            }
        }
        sqlx::query_file!(
            "../../queries/deployment/settings.sql",
            agent,
            failure,
            routing
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.get(agent).await
    }
    /// The deployment a token belongs to. Retired deployments no longer authenticate.
    pub async fn authenticate(&self, token: &str) -> Result<Authenticated, Error> {
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        sqlx::query_file!("../../queries/deployment/authenticate.sql", hash)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| Authenticated {
                agent: r.agent_id,
                deployment: r.deployment_id,
                target: target_wire(&r.target),
            })
            .ok_or(Error::Denied)
    }
    /// Rotate one deployment's token. Instances holding the old token lose access
    /// at their next call; roll them with the new one.
    pub async fn issue_token(&self, agent: Uuid, deployment: Uuid) -> Result<SecretString, Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!("../../queries/deployment/lock.sql", agent)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        let token = secrets::random_secret();
        let hash = Sha256::digest(token.expose_secret().as_bytes()).to_vec();
        sqlx::query_file!(
            "../../queries/deployment/issue_token.sql",
            deployment,
            hash,
            agent
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound)?;
        self.ensure_secrets(&mut tx, agent).await?;
        tx.commit().await?;
        Ok(token)
    }
    /// The agent's token signing material exists from its first deployment token on.
    async fn ensure_secrets(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        agent: Uuid,
    ) -> Result<(), Error> {
        let sealed = self
            .encryption
            .seal(
                secret_binding(agent),
                &secrets::Secrets::generate().encode()?,
            )?
            .into_bytes();
        sqlx::query_file!("../../queries/deployment/secrets_init.sql", agent, sealed)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
    /// Register a deployment. Idempotent on `external_id`: a repeat returns the
    /// existing record and no token. Under latest routing a new deployment matching
    /// the agent's mode starts serving new threads at once.
    pub async fn register_deployment(
        &self,
        agent: Uuid,
        r: RegisterDeployment,
    ) -> Result<(types::AgentDeployment, Option<SecretString>, bool), Error> {
        let source = match r.source {
            types::DeploymentSource::Ci => "ci",
            types::DeploymentSource::Manual => "manual",
            _ => return Err(Error::Invalid("Select a deployment source".into())),
        };
        let target = match r.target {
            types::DeploymentTarget::Direct => "direct",
            types::DeploymentTarget::Sidecar => "sidecar",
            types::DeploymentTarget::AwsLambda => "aws_lambda",
            _ => return Err(Error::Invalid("Select a deployment target".into())),
        };
        let endpoint = match (target, r.endpoint_url.filter(|v| !v.trim().is_empty())) {
            ("direct", Some(url)) => Some(crate::agent::validate_endpoint(url)?),
            ("direct", None) => {
                return Err(Error::Invalid(
                    "A direct deployment requires an endpoint URL".into(),
                ));
            }
            (_, Some(_)) => {
                return Err(Error::Invalid(
                    "Only direct deployments take an endpoint URL".into(),
                ));
            }
            _ => None,
        };
        let reference = match (target, r.target_reference.filter(|v| !v.trim().is_empty())) {
            ("aws_lambda", Some(v)) if v.len() <= 2048 => Some(v),
            ("aws_lambda", _) => {
                return Err(Error::Invalid(
                    "A Lambda deployment requires a function reference".into(),
                ));
            }
            (_, Some(_)) => {
                return Err(Error::Invalid(
                    "Only Lambda deployments take a target reference".into(),
                ));
            }
            _ => None,
        };
        let repository = short(r.repository, "Repository")?;
        let commit = short(r.commit_sha, "Commit")?;
        if commit
            .as_deref()
            .is_some_and(|c| !c.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(Error::Invalid("Commit must be a hex SHA".into()));
        }
        let external = short(r.external_id, "External ID")?;
        let label = short(r.label, "Label")?.unwrap_or_default();
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_file!("../../queries/deployment/lock.sql", agent)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        if let Some(external) = &external
            && let Some(existing) = sqlx::query_file!(
                "../../queries/deployment/deployment_external.sql",
                agent,
                external
            )
            .fetch_optional(&mut *tx)
            .await?
        {
            tx.commit().await?;
            return Ok((self.deployment(agent, existing.id).await?, None, false));
        }
        let id = Uuid::new_v4();
        let token = secrets::random_secret();
        let hash = Sha256::digest(token.expose_secret().as_bytes()).to_vec();
        sqlx::query_file!(
            "../../queries/deployment/deployment_create.sql",
            id,
            agent,
            source,
            target,
            endpoint,
            reference,
            repository,
            commit,
            external,
            label,
            hash
        )
        .fetch_one(&mut *tx)
        .await?;
        self.ensure_secrets(&mut tx, agent).await?;
        if current.routing == "latest" && target_for_mode(&current.deployment_mode) == target {
            self.promote_in(&mut tx, agent, id).await?;
        }
        tx.commit().await?;
        Ok((self.deployment(agent, id).await?, Some(token), true))
    }
    pub async fn promote(&self, agent: Uuid, deployment: Uuid) -> Result<types::Deployment, Error> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_file!("../../queries/deployment/lock.sql", agent)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        let row = sqlx::query_file!(
            "../../queries/deployment/deployment_get.sql",
            deployment,
            agent
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound)?;
        if target_for_mode(&current.deployment_mode) != row.target {
            return Err(Error::Invalid(
                "The deployment's target does not match the agent's execution mode".into(),
            ));
        }
        self.promote_in(&mut tx, agent, deployment).await?;
        tx.commit().await?;
        self.get(agent).await
    }
    /// Make `deployment` the one new threads route to, mirroring a direct endpoint
    /// onto the agent for legacy readers (health polling, listings).
    async fn promote_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        agent: Uuid,
        deployment: Uuid,
    ) -> Result<(), Error> {
        let row = sqlx::query_file!(
            "../../queries/deployment/deployment_promote.sql",
            agent,
            deployment
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(Error::NotFound)?;
        if row.target == "direct" {
            mirror_flag(tx).await?;
            sqlx::query_file!(
                "../../queries/deployment/mirror_endpoint.sql",
                agent,
                row.endpoint_url
            )
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }
    /// Retire a deployment: its token stops authenticating and it leaves routing.
    /// Retiring the serving deployment hands serving to the newest other one.
    pub async fn retire(
        &self,
        agent: Uuid,
        deployment: Uuid,
    ) -> Result<types::AgentDeployment, Error> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_file!("../../queries/deployment/lock.sql", agent)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        if current.serving_deployment_id == Some(deployment) {
            let next = sqlx::query_file!(
                "../../queries/deployment/deployment_latest_except.sql",
                agent,
                target_for_mode(&current.deployment_mode),
                deployment
            )
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| r.id)
            .ok_or_else(|| {
                Error::Invalid("Register another deployment before retiring the serving one".into())
            })?;
            self.promote_in(&mut tx, agent, next).await?;
        }
        sqlx::query_file!(
            "../../queries/deployment/deployment_retire.sql",
            deployment,
            agent
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound)?;
        tx.commit().await?;
        self.deployment(agent, deployment).await
    }
    pub async fn deployment(
        &self,
        agent: Uuid,
        deployment: Uuid,
    ) -> Result<types::AgentDeployment, Error> {
        let r = sqlx::query_file!(
            "../../queries/deployment/deployment_get.sql",
            deployment,
            agent
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::NotFound)?;
        Ok(DeploymentRow {
            id: r.id,
            agent_id: r.agent_id,
            source: r.source,
            target: r.target,
            endpoint_url: r.endpoint_url,
            target_reference: r.target_reference,
            repository: r.repository,
            commit_sha: r.commit_sha,
            external_id: r.external_id,
            label: r.label,
            status: r.status,
            token_issued: r.token_issued,
            serving: r.serving,
            created_at: r.created_at,
            retired_at: r.retired_at,
        }
        .into())
    }
    pub async fn deployments(&self, agent: Uuid) -> Result<Vec<types::AgentDeployment>, Error> {
        Ok(
            sqlx::query_file!("../../queries/deployment/deployments_list.sql", agent)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|r| {
                    DeploymentRow {
                        id: r.id,
                        agent_id: r.agent_id,
                        source: r.source,
                        target: r.target,
                        endpoint_url: r.endpoint_url,
                        target_reference: r.target_reference,
                        repository: r.repository,
                        commit_sha: r.commit_sha,
                        external_id: r.external_id,
                        label: r.label,
                        status: r.status,
                        token_issued: r.token_issued,
                        serving: r.serving,
                        created_at: r.created_at,
                        retired_at: r.retired_at,
                    }
                    .into()
                })
                .collect(),
        )
    }
    /// The deployment a (thread, agent) pair is pinned to, if any and still registered.
    pub(crate) async fn pinned(&self, agent: Uuid, thread: Uuid) -> Result<Option<Uuid>, Error> {
        Ok(
            sqlx::query_file!("../../queries/deployment/deployment_pin.sql", thread, agent)
                .fetch_optional(&self.pool)
                .await?
                .and_then(|r| r.deployment_id),
        )
    }
    pub async fn instances(&self, agent: Uuid) -> Result<Vec<types::AgentInstance>, Error> {
        Ok(
            sqlx::query_file!("../../queries/deployment/nodes.sql", agent)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|r| types::AgentInstance {
                    instance_id: r.instance_id.to_string(),
                    agent_id: agent.to_string(),
                    deployment_id: r.deployment_id.to_string(),
                    public_url: r.public_url,
                    runtime_url: r.runtime_url,
                    ready: r.ready
                        && r.agent_ready
                        && r.last_seen_at
                            > Utc::now() - chrono::Duration::from_std(LIVENESS).unwrap_or_default(),
                    last_seen_at: crate::chat::audit::timestamp(r.last_seen_at).into(),
                    ..Default::default()
                })
                .collect(),
        )
    }
    pub async fn register(
        &self,
        agent: Uuid,
        deployment: Uuid,
        r: &wire::WatchRequest,
    ) -> Result<Uuid, Error> {
        let instance = id(&r.instance_id)?;
        // Connected hosts have no address of their own; sidecars always do.
        for endpoint in [&r.public_url, &r.runtime_url] {
            if !endpoint.is_empty() {
                crate::agent::validate_endpoint(endpoint.clone())?;
            }
        }
        sqlx::query_file!(
            "../../queries/deployment/register.sql",
            agent,
            instance,
            deployment,
            r.public_url,
            r.runtime_url
        )
        .execute(&self.pool)
        .await?;
        Ok(instance)
    }
    pub async fn heartbeat(
        &self,
        agent: Uuid,
        instance: Uuid,
        r: &wire::Heartbeat,
    ) -> Result<(), Error> {
        if sqlx::query_file!(
            "../../queries/deployment/heartbeat.sql",
            agent,
            instance,
            r.ready,
            r.agent_ready
        )
        .execute(&self.pool)
        .await?
        .rows_affected()
            == 0
        {
            tracing::warn!(agent_id=%agent, instance_id=%instance, "Heartbeat from an unregistered replica");
            return Ok(());
        }
        if let Ok(sample) = id(&r.sample_id) {
            sqlx::query_file!(
                "../../queries/deployment/telemetry/health.sql",
                sample,
                agent,
                instance.to_string(),
                Utc::now(),
                r.ready && r.agent_ready,
                r.latency_ms
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
    pub async fn instance_live(&self, agent: Uuid, instance: Uuid) -> Result<bool, Error> {
        Ok(sqlx::query_file!(
            "../../queries/deployment/instance_live.sql",
            agent,
            instance
        )
        .fetch_one(&self.pool)
        .await?
        .live)
    }
    pub async fn snapshot(
        &self,
        agent: Uuid,
        deployment: Uuid,
        instance: Uuid,
    ) -> Result<wire::Snapshot, Error> {
        let leases = sqlx::query_file!("../../queries/deployment/leases_held.sql", agent, instance)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| lease_frame(agent, r.thread_id, Some((instance, String::new()))))
            .collect();
        Ok(wire::Snapshot {
            configuration: self.configuration(agent).await?.into(),
            leases,
            token_signing_key: self.signing_key(agent).await?.expose_secret().into(),
            deployment_id: deployment.to_string(),
            ..Default::default()
        })
    }
    /// Lease changes for one agent since a point in time; replicas ignore ones that do not concern them.
    pub async fn leases_since(
        &self,
        agent: Uuid,
        since: chrono::DateTime<Utc>,
    ) -> Result<Vec<(wire::ThreadLease, chrono::DateTime<Utc>)>, Error> {
        Ok(
            sqlx::query_file!("../../queries/deployment/leases_since.sql", agent, since)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|r| {
                    (
                        lease_frame(
                            agent,
                            r.thread_id,
                            Some((r.instance_id, r.public_url.unwrap_or_default())),
                        ),
                        r.updated_at,
                    )
                })
                .collect(),
        )
    }
    pub async fn configuration(
        &self,
        agent: Uuid,
    ) -> Result<wire::GetConfigurationResponse, Error> {
        let row = sqlx::query_file!("../../queries/deployment/agent_secrets.sql", agent)
            .fetch_one(&self.pool)
            .await?;
        let signing = self.encryption.open(
            SecretBinding {
                resource_kind: "agent",
                resource_id: agent,
                name: "webhook_signing_key",
            },
            SealedSecret::from_bytes(&row.webhook_signing_key)?,
        )?;
        let mut response = wire::GetConfigurationResponse {
            logs_enabled: self.logs.as_ref().is_some_and(|logs| logs.enabled()),
            tracing_enabled: self.telemetry.as_ref().is_some_and(|queue| queue.enabled()),
            agent: crate::agent::rpc::project(&self.agents, self.agents.get(agent).await?)
                .await?
                .into(),
            agent_generation: row.generation,
            webhook_signing_key: signing.expose_secret().into(),
            ..Default::default()
        };
        drop(signing);
        for assigned in
            sqlx::query_file!("../../queries/deployment/assigned_connections.sql", agent)
                .fetch_all(&self.pool)
                .await?
        {
            let connection = self.connections.get(assigned.id).await?;
            let credentials = if connection.status == "ready" {
                let values = self.connections.resolve(assigned.id).await?;
                values
                    .iter()
                    .map(|(name, value)| wire::CredentialField {
                        name: name.clone(),
                        value: value.expose_secret().into(),
                        ..Default::default()
                    })
                    .collect()
            } else {
                vec![]
            };
            let identities = sqlx::query_file!(
                "../../queries/deployment/identity_grants.sql",
                assigned.id,
                agent
            )
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| wire::IdentityGrant {
                id: r.id.to_string(),
                identity_type: crate::chat::access::identity::kind_value(&r.identity_type).into(),
                value: r.value,
                verified: r.verified,
                allowed: r.allowed,
                ..Default::default()
            })
            .collect();
            response.connections.push(wire::ConnectionConfiguration {
                identities,
                expires_at: connection
                    .token_expires_at
                    .map(|t| t.timestamp_millis())
                    .unwrap_or(0),
                access_mode: match assigned.access_mode.as_str() {
                    "public" => types::ChannelAccessMode::Public,
                    "disabled" => types::ChannelAccessMode::Disabled,
                    _ => types::ChannelAccessMode::Private,
                }
                .into(),
                id: assigned.id.to_string(),
                provider_id: connection.provider_id,
                type_id: connection.type_id,
                name: connection.name,
                status: connection.status,
                account_label: connection.account_label.unwrap_or_default(),
                credential_version: connection.credential_version,
                credentials,
                ..Default::default()
            });
        }
        Ok(response)
    }
    pub(crate) async fn signing_key(&self, agent: Uuid) -> Result<SecretString, Error> {
        let row = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(self
            .open_secrets(
                agent,
                row.encrypted_secrets.as_deref().ok_or(Error::Denied)?,
            )?
            .signing_key)
    }
    fn open_secrets(&self, agent: Uuid, sealed: &[u8]) -> Result<secrets::Secrets, Error> {
        let value = self
            .encryption
            .open(secret_binding(agent), SealedSecret::from_bytes(sealed)?)?;
        secrets::Secrets::decode(value)
    }
    /// Who executes a thread for this agent right now, if the holder is alive.
    pub(crate) async fn holder(&self, agent: Uuid, thread: Uuid) -> Result<Option<Holder>, Error> {
        Ok(
            sqlx::query_file!("../../queries/deployment/lease_holder.sql", thread, agent)
                .fetch_optional(&self.pool)
                .await?
                .filter(|r| r.live)
                .map(|r| Holder {
                    instance: r.instance_id,
                    public_url: r.public_url.unwrap_or_default(),
                }),
        )
    }
    /// Take or confirm the lease for `instance` in one transaction. A live holder
    /// keeps it; a dead holder loses it, and its interrupted work is ended here.
    pub(crate) async fn lease_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        agent: Uuid,
        instance: Uuid,
        thread: Uuid,
    ) -> Result<wire::ThreadLease, Error> {
        crate::chat::access::lock_thread_route(tx, thread).await?;
        // An existing conversation can only be executed by an agent already in it.
        if sqlx::query_file!("../../queries/deployment/project/has_thread.sql", thread)
            .fetch_one(&mut **tx)
            .await?
            .exists
            && !sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
                .fetch_one(&mut **tx)
                .await?
                .allowed
        {
            return Err(Error::Denied);
        }
        let mut current =
            sqlx::query_file!("../../queries/deployment/lease_lock.sql", thread, agent)
                .fetch_optional(&mut **tx)
                .await?
                .map(|r| r.instance_id);
        if current.is_none() {
            // Two replicas may reach here for a brand-new thread; the insert decides.
            if sqlx::query_file!(
                "../../queries/deployment/lease_insert.sql",
                thread,
                agent,
                instance
            )
            .fetch_optional(&mut **tx)
            .await?
            .is_some()
            {
                return Ok(lease_frame(agent, thread, Some((instance, String::new()))));
            }
            current = sqlx::query_file!("../../queries/deployment/lease_lock.sql", thread, agent)
                .fetch_optional(&mut **tx)
                .await?
                .map(|r| r.instance_id);
        }
        let Some(holder) = current else {
            return Err(Error::Conflict);
        };
        if holder == instance {
            return Ok(lease_frame(agent, thread, Some((instance, String::new()))));
        }
        let node = sqlx::query_file!("../../queries/deployment/instance_live.sql", agent, holder)
            .fetch_one(&mut **tx)
            .await?;
        if node.live {
            let url = sqlx::query_file!("../../queries/deployment/node_url.sql", agent, holder)
                .fetch_optional(&mut **tx)
                .await?
                .map(|r| r.public_url)
                .unwrap_or_default();
            return Ok(lease_frame(agent, thread, Some((holder, url))));
        }
        // The holder is gone: the claimant takes over and the interrupted run follows the
        // agent's failure policy (restarted by the new holder, or failed here).
        let stop = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_optional(&mut **tx)
            .await?
            .is_some_and(|d| d.failure_mode == "stop");
        self.take_over(tx, agent, thread, holder, Some(instance), stop)
            .await?;
        tracing::debug!(agent_id=%agent, instance_id=%instance, thread_id=%thread, "Lease taken over from a dead holder");
        Ok(lease_frame(agent, thread, Some((instance, String::new()))))
    }
    /// Drop this instance's lease on an evicted thread.
    pub(crate) async fn release(
        &self,
        agent: Uuid,
        instance: Uuid,
        thread: Uuid,
    ) -> Result<(), Error> {
        sqlx::query_file!(
            "../../queries/deployment/lease_release.sql",
            thread,
            agent,
            instance
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    /// Which live replica should receive gateway-originated work for a thread.
    pub(crate) async fn target_for(
        &self,
        agent: Uuid,
        thread: Option<Uuid>,
    ) -> Result<Option<Uuid>, Error> {
        if let Some(thread) = thread
            && let Some(holder) = self.holder(agent, thread).await?
        {
            return Ok(Some(holder.instance));
        }
        let pinned = match thread {
            Some(thread) => self.pinned(agent, thread).await?,
            None => None,
        };
        Ok(sqlx::query_file!(
            "../../queries/deployment/live_node.sql",
            agent,
            None::<Uuid>,
            pinned
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|r| r.instance_id))
    }
    pub(crate) fn seal_record<M: Message>(
        &self,
        key: Uuid,
        kind: &str,
        value: &M,
    ) -> Result<Vec<u8>, Error> {
        let bytes = Zeroizing::new(value.encode_to_vec());
        let text = SecretString::from(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes.as_slice(),
        ));
        Ok(self
            .encryption
            .seal(
                SecretBinding {
                    resource_kind: "sidecar_control",
                    resource_id: key,
                    name: kind,
                },
                &text,
            )?
            .into_bytes())
    }
    pub(crate) fn open_record<M: Message>(
        &self,
        key: Uuid,
        kind: &str,
        sealed: &[u8],
    ) -> Result<M, Error> {
        let text = self.encryption.open(
            SecretBinding {
                resource_kind: "sidecar_control",
                resource_id: key,
                name: kind,
            },
            SealedSecret::from_bytes(sealed)?,
        )?;
        let bytes = Zeroizing::new(
            base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                text.expose_secret(),
            )
            .map_err(|_| Error::Encryption)?,
        );
        M::decode_from_slice(bytes.as_slice()).map_err(|_| Error::Encryption)
    }
    /// Queue durable gateway-originated work for one replica.
    pub(crate) async fn direct(
        &self,
        agent: Uuid,
        instance: Uuid,
        thread: Option<Uuid>,
        action: wire::directive::Action,
    ) -> Result<Uuid, Error> {
        self.direct_as(Uuid::new_v4(), agent, instance, thread, action)
            .await
    }
    /// Queue a directive under a caller-chosen key, so the payload can carry it.
    pub(crate) async fn direct_as(
        &self,
        key: Uuid,
        agent: Uuid,
        instance: Uuid,
        thread: Option<Uuid>,
        action: wire::directive::Action,
    ) -> Result<Uuid, Error> {
        let directive = wire::Directive {
            id: key.to_string(),
            thread_id: thread.map(|t| t.to_string()).unwrap_or_default(),
            action: Some(action),
            ..Default::default()
        };
        let payload = self.seal_record(key, "directive", &directive)?;
        sqlx::query_file!(
            "../../queries/deployment/directive_insert.sql",
            key,
            agent,
            instance,
            thread,
            payload
        )
        .execute(&self.pool)
        .await?;
        Ok(key)
    }
    pub(crate) async fn pending_directives(
        &self,
        agent: Uuid,
        instance: Uuid,
    ) -> Result<Vec<wire::Directive>, Error> {
        sqlx::query_file!(
            "../../queries/deployment/directives_pending.sql",
            agent,
            instance
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| self.open_record(r.id, "directive", &r.payload))
        .collect()
    }
    pub(crate) async fn directive_result(
        &self,
        agent: Uuid,
        instance: Uuid,
        key: Uuid,
        result: &wire::CallResult,
    ) -> Result<(), Error> {
        let payload = self.seal_record(key, "result", result)?;
        sqlx::query_file!(
            "../../queries/deployment/directive_result.sql",
            key,
            agent,
            instance,
            payload
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(())
    }
    pub(crate) async fn wait_directive(
        &self,
        key: Uuid,
        timeout: Duration,
    ) -> Result<wire::CallResult, Error> {
        let mut changed = self
            .channels
            .directives
            .subscribe(&self.pool, "tilde_sidecar_directives")
            .await?;
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            changed.borrow_and_update();
            let row = sqlx::query_file!("../../queries/deployment/directive_get.sql", key)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(Error::NotFound)?;
            if row.acked_at.is_some() {
                // A wake is acknowledged without a result; calls carry one.
                return match row.result {
                    Some(result) => self.open_record(key, "result", &result),
                    None => Ok(wire::CallResult::default()),
                };
            }
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => return Err(Error::Invalid("Sidecar did not answer in time".into())),
                changed = changed.changed() => { if changed.is_err() { return Err(Error::Invalid("Directive notifications ended".into())); } }
            }
        }
    }
    /// Run one ingress call on the replica that owns a thread and relay its answer.
    pub async fn forward(
        &self,
        agent: Uuid,
        thread: Option<Uuid>,
        call: wire::IngressCall,
    ) -> Result<wire::CallResult, Error> {
        let thread = match thread {
            Some(thread) => Some(thread),
            None => routing::thread(&self.chat(), &call.method, &call.body, &call.content_type)
                .await
                .ok()
                .flatten(),
        };
        if matches!(
            call.method.as_str(),
            "GetThread" | "ListThreads" | "ListMessages" | "ListActivity" | "GetRun"
        ) {
            return self.serve_read(agent, thread, call).await;
        }
        let instance = self.target_for(agent, thread).await?.ok_or_else(|| {
            Error::Invalid("No sidecar replica is available for this agent".into())
        })?;
        let key = self.direct(agent, instance, thread, call.into()).await?;
        self.wait_directive(key, Duration::from_secs(30)).await
    }
    /// Reads are agent-wide, so the projection answers them for every replica.
    async fn serve_read(
        &self,
        agent: Uuid,
        thread: Option<Uuid>,
        call: wire::IngressCall,
    ) -> Result<wire::CallResult, Error> {
        let claims =
            tokens::verify_ingress(&call.caller_token, agent, &self.signing_key(agent).await?)?;
        if let Some(thread) = thread {
            if claims.thread_id.is_some_and(|scope| scope != thread)
                || !sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
                    .fetch_one(&self.pool)
                    .await?
                    .allowed
            {
                return Err(Error::Denied);
            }
        } else if claims.thread_id.is_some() {
            return Err(Error::Denied);
        }
        let mut request = http::Request::builder()
            .method(http::Method::POST)
            .uri(format!("/tilde.ingress.v1.ChatService/{}", call.method));
        if let Ok(value) = http::HeaderValue::from_str(&call.content_type) {
            request = request.header(http::header::CONTENT_TYPE, value);
        }
        let mut request = request
            .body(axum::body::Body::from(call.body))
            .map_err(|_| Error::Denied)?;
        request.extensions_mut().insert(claims);
        let response = match tower::ServiceExt::oneshot(
            crate::chat::rpc::ingress::router(self.chat()),
            request,
        )
        .await
        {
            Ok(response) => response,
            Err(never) => match never {},
        };
        let status = response.status().as_u16() as i32;
        let content_type = response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let body = axum::body::to_bytes(response.into_body(), 192 * 1024 * 1024)
            .await
            .map(|b| b.to_vec())
            .unwrap_or_default();
        Ok(wire::CallResult {
            status,
            content_type,
            body,
            ..Default::default()
        })
    }
    pub async fn resolve_participant(
        &self,
        r: wire::ResolveParticipantRequest,
    ) -> Result<wire::ResolveParticipantResponse, Error> {
        match r.key {
            Some(wire::resolve_participant_request::Key::AgentId(agent)) => {
                let agent = self.agents.get(id(&agent)?).await?;
                Ok(wire::ResolveParticipantResponse {
                    id: agent.id.to_string(),
                    name: agent.name,
                    ..Default::default()
                })
            }
            Some(wire::resolve_participant_request::Key::UserId(user)) => {
                let user = id(&user)?;
                let row = sqlx::query_file!("../../queries/chat/user_get.sql", user)
                    .fetch_optional(&self.pool)
                    .await?
                    .ok_or(Error::NotFound)?;
                Ok(wire::ResolveParticipantResponse {
                    id: user.to_string(),
                    name: row.name,
                    ..Default::default()
                })
            }
            None => Err(Error::Invalid("Resolve requires an agent or user".into())),
        }
    }
}
/// A live replica executing a thread.
pub(crate) struct Holder {
    pub instance: Uuid,
    pub public_url: String,
}
/// What a deployment token proves.
#[derive(Clone, Copy, Debug)]
pub struct Authenticated {
    pub agent: Uuid,
    pub deployment: Uuid,
    pub target: types::DeploymentTarget,
}
pub struct RegisterDeployment {
    pub source: types::DeploymentSource,
    pub target: types::DeploymentTarget,
    pub endpoint_url: Option<String>,
    pub target_reference: Option<String>,
    pub repository: Option<String>,
    pub commit_sha: Option<String>,
    pub external_id: Option<String>,
    pub label: Option<String>,
}
struct DeploymentRow {
    id: Uuid,
    agent_id: Uuid,
    source: String,
    target: String,
    endpoint_url: Option<String>,
    target_reference: Option<String>,
    repository: Option<String>,
    commit_sha: Option<String>,
    external_id: Option<String>,
    label: String,
    status: String,
    token_issued: bool,
    serving: bool,
    created_at: chrono::DateTime<Utc>,
    retired_at: Option<chrono::DateTime<Utc>>,
}
impl From<DeploymentRow> for types::AgentDeployment {
    fn from(r: DeploymentRow) -> Self {
        types::AgentDeployment {
            id: r.id.to_string(),
            agent_id: r.agent_id.to_string(),
            source: if r.source == "ci" {
                types::DeploymentSource::Ci
            } else {
                types::DeploymentSource::Manual
            }
            .into(),
            target: target_wire(&r.target).into(),
            endpoint_url: r.endpoint_url.unwrap_or_default(),
            target_reference: r.target_reference.unwrap_or_default(),
            repository: r.repository.unwrap_or_default(),
            commit_sha: r.commit_sha.unwrap_or_default(),
            external_id: r.external_id.unwrap_or_default(),
            label: r.label,
            status: if r.status == "retired" {
                types::DeploymentStatus::Retired
            } else {
                types::DeploymentStatus::Registered
            }
            .into(),
            token_issued: r.token_issued,
            serving: r.serving,
            created_at: crate::chat::audit::timestamp(r.created_at).into(),
            retired_at: r
                .retired_at
                .map(|t| crate::chat::audit::timestamp(t).into())
                .unwrap_or_default(),
            ..Default::default()
        }
    }
}
fn mode_wire(mode: &str) -> types::DeploymentMode {
    if mode == "sidecar" {
        types::DeploymentMode::Sidecar
    } else {
        types::DeploymentMode::Gateway
    }
}
fn target_wire(target: &str) -> types::DeploymentTarget {
    match target {
        "sidecar" => types::DeploymentTarget::Sidecar,
        "aws_lambda" => types::DeploymentTarget::AwsLambda,
        _ => types::DeploymentTarget::Direct,
    }
}
/// The deployment target new threads of an agent in `mode` route to.
fn target_for_mode(mode: &str) -> &'static str {
    if mode == "sidecar" {
        "sidecar"
    } else {
        "direct"
    }
}
fn short(value: Option<String>, name: &str) -> Result<Option<String>, Error> {
    match value {
        Some(v) if v.chars().count() > 512 => Err(Error::Invalid(format!("{name} is too long"))),
        Some(v) if v.trim().is_empty() => Ok(None),
        other => Ok(other),
    }
}
/// Writes to `agents.endpoint_url` inside this transaction mirror a promotion; the
/// endpoint trigger must not mint another deployment for them.
async fn mirror_flag(tx: &mut Transaction<'_, Postgres>) -> Result<(), Error> {
    sqlx::query("SELECT set_config('tilde.deployment_mirror','on',true)")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub(crate) fn lease_frame(
    agent: Uuid,
    thread: Uuid,
    holder: Option<(Uuid, String)>,
) -> wire::ThreadLease {
    let (instance, url) = holder.clone().unwrap_or_default();
    wire::ThreadLease {
        thread_id: thread.to_string(),
        agent_id: agent.to_string(),
        holder_instance_id: if holder.is_some() {
            instance.to_string()
        } else {
            String::new()
        },
        holder_public_url: url,
        held: holder.is_some(),
        ..Default::default()
    }
}
pub(crate) fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Invalid UUID".into()))
}
fn secret_binding(agent: Uuid) -> SecretBinding<'static> {
    SecretBinding {
        resource_kind: "agent_deployment",
        resource_id: agent,
        name: "secrets",
    }
}
