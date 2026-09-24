//! Agent deployment settings and the gateway half of the sidecar protocol.
//! Postgres is the record and the gateway its only writer. A replica caches the
//! threads it holds a lease on, executes the agent for them, and ships typed
//! events back asynchronously. Replicas dial in; the gateway never dials out.
pub mod attachments;
pub mod db;
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
use crate::database::{Pool, Transaction};
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
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Owners that miss this window are treated as gone. Every query that judges liveness
/// takes this as its trailing parameter, so the window lives here alone.
pub const LIVENESS: Duration = Duration::from_secs(15);
pub(crate) fn liveness_secs() -> f64 {
    LIVENESS.as_secs_f64()
}
#[derive(Default)]
pub(crate) struct Channels {
    pub directives: Notifications,
    pub leases: Notifications,
    pub configuration: Notifications,
    pub recovery: Notifications,
}
#[derive(Clone)]
pub struct Deployments {
    pub(crate) pool: Pool,
    pub(crate) encryption: Arc<Encryption>,
    pub(crate) agents: Agents,
    pub(crate) connections: Connections,
    pub(crate) logs: Option<crate::logs::Delivery>,
    pub(crate) telemetry: Option<crate::telemetry::delivery::Queue>,
    pub(crate) channels: Arc<Channels>,
}
impl Deployments {
    pub fn new(
        pool: Pool,
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
        let row = crate::deployment::db::get_opt(&self.pool.get().await?, agent)
            .await?
            .ok_or(Error::NotFound)?;
        let serving = if row.routing == "latest" {
            db::choose_opt(&self.pool.get().await?, agent)
                .await?
                .map(|r| r.id)
        } else {
            row.serving_deployment_id
        };
        Ok(types::Deployment {
            agent_id: agent.to_string(),
            failure_mode: if row.failure_mode == "stop" {
                types::SidecarFailureMode::Stop
            } else {
                types::SidecarFailureMode::Reassign
            }
            .into(),
            routing: if row.routing == "weighted" {
                types::DeploymentRouting::Weighted
            } else {
                types::DeploymentRouting::Latest
            }
            .into(),
            serving_deployment_id: serving.map(|v| v.to_string()).unwrap_or_default(),
            ..Default::default()
        })
    }
    /// Resolve once under the conversation routing lock. Existing pins outlive policy changes.
    pub(crate) async fn selected(
        &self,
        agent: Uuid,
        thread: Option<Uuid>,
    ) -> Result<Option<types::AgentDeployment>, Error> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let chosen = if let Some(thread) = thread {
            crate::chat::access::lock_thread_route(&tx, thread).await?;
            let pinned = crate::chat::pin_deployment(&tx, thread, agent).await?;
            match pinned {
                Some(id) => Some(id),
                None => match crate::deployment::db::lease_holder_opt(
                    &tx,
                    thread,
                    agent,
                    liveness_secs(),
                )
                .await?
                .and_then(|r| r.deployment_id)
                {
                    Some(id) => Some(id),
                    None => crate::deployment::db::choose_opt(&tx, agent)
                        .await?
                        .map(|r| r.id),
                },
            }
        } else {
            crate::deployment::db::choose_opt(&tx, agent)
                .await?
                .map(|r| r.id)
        };
        tx.commit().await?;
        drop(client);
        match chosen {
            Some(id) => self.deployment(agent, id).await.map(Some),
            None => Ok(None),
        }
    }
    pub(crate) async fn is_sidecar(&self, agent: Uuid, thread: Uuid) -> Result<bool, Error> {
        Ok(self
            .selected(agent, Some(thread))
            .await?
            .is_some_and(|d| d.target.as_known() == Some(types::DeploymentTarget::Sidecar)))
    }
    /// Routing and sidecar failure policy are agent-wide. Execution type belongs to each deployment.
    pub async fn set(
        &self,
        agent: Uuid,
        failure: types::SidecarFailureMode,
        routing: types::DeploymentRouting,
    ) -> Result<types::Deployment, Error> {
        let failure = match failure {
            types::SidecarFailureMode::Stop => "stop",
            types::SidecarFailureMode::Reassign => "reassign",
            _ => return Err(Error::Invalid("Select a failure policy".into())),
        };
        let routing = match routing {
            types::DeploymentRouting::Weighted => "weighted",
            _ => "latest",
        };
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let current = crate::deployment::db::lock_opt(&tx, agent)
            .await?
            .ok_or(Error::NotFound)?;
        if current.routing != routing {
            let next = if routing == "latest" {
                crate::deployment::db::deployment_latest_opt(&tx, agent, &all_targets())
                    .await?
                    .map(|r| r.id)
            } else {
                db::choose_opt(&tx, agent).await?.map(|r| r.id)
            };
            if let Some(next) = next {
                self.promote_in(&tx, agent, next).await?;
            }
        }
        crate::deployment::db::settings_execute(&tx, agent, failure, routing).await?;
        tx.commit().await?;
        drop(client);
        self.get(agent).await
    }
    /// Replace the entire active distribution atomically. Existing conversation pins do not move.
    pub async fn set_weights(&self, agent: Uuid, weights: Vec<(Uuid, u32)>) -> Result<(), Error> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let current = crate::deployment::db::lock_opt(&tx, agent)
            .await?
            .ok_or(Error::NotFound)?;
        if current.routing != "weighted" {
            return Err(Error::Invalid(
                "Select weighted routing before setting traffic weights".into(),
            ));
        }
        let rows = crate::deployment::db::deployments_list_all(&tx, agent).await?;
        let active: std::collections::BTreeSet<_> = rows
            .iter()
            .filter(|r| r.status == "registered")
            .map(|r| r.id)
            .collect();
        let supplied: std::collections::BTreeSet<_> = weights.iter().map(|(id, _)| *id).collect();
        if supplied.len() != weights.len() || supplied != active {
            return Err(Error::Invalid(
                "Supply each active deployment exactly once; reload deployments and try again"
                    .into(),
            ));
        }
        if weights.iter().any(|(_, w)| *w > 100)
            || weights.iter().map(|(_, w)| u64::from(*w)).sum::<u64>() != 100
        {
            return Err(Error::Invalid(
                "Traffic weights must be whole percentages from 0 to 100 and total 100%".into(),
            ));
        }
        let available: std::collections::BTreeSet<_> = db::available_all(&tx, agent)
            .await?
            .into_iter()
            .map(|r| r.id)
            .collect();
        if weights
            .iter()
            .any(|(id, weight)| *weight > 0 && !available.contains(id))
        {
            return Err(Error::Invalid("Traffic can only be allocated to deployments with a live, ready connection (or Lambda). Refresh and try again.".into()));
        }
        // Keep a representative endpoint for health checks and legacy agent listings.
        let representative = weights
            .iter()
            .find(|(id, w)| Some(*id) == current.serving_deployment_id && *w > 0)
            .or_else(|| weights.iter().find(|(_, w)| *w > 0))
            .expect("validated nonzero distribution")
            .0;
        self.promote_in(&tx, agent, representative).await?;
        for (deployment, weight) in weights {
            crate::deployment::db::weight_set_execute(&tx, agent, deployment, weight as i32)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// The deployment a token belongs to. Retired deployments no longer authenticate.
    pub async fn authenticate(&self, token: &str) -> Result<Authenticated, Error> {
        authenticate(&self.pool, token).await
    }
    /// Rotate one deployment's token. Instances holding the old token lose access
    /// at their next call; roll them with the new one.
    pub async fn issue_token(&self, agent: Uuid, deployment: Uuid) -> Result<SecretString, Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::deployment::db::lock_opt(&tx, agent)
            .await?
            .ok_or(Error::NotFound)?;
        let token = secrets::random_secret();
        let hash = Sha256::digest(token.expose_secret().as_bytes()).to_vec();
        crate::deployment::db::issue_token_opt(&tx, deployment, &(hash), agent)
            .await?
            .ok_or(Error::NotFound)?;
        self.ensure_secrets(&tx, agent).await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(token)
    }
    /// The agent's token signing material exists from its first deployment token on.
    async fn ensure_secrets(&self, tx: &Transaction<'_>, agent: Uuid) -> Result<(), Error> {
        let sealed = self
            .encryption
            .seal(
                secret_binding(agent),
                &secrets::Secrets::generate().encode()?,
            )?
            .into_bytes();
        crate::deployment::db::secrets_init_execute(tx, agent, &(sealed)).await?;
        Ok(())
    }
    /// Register a deployment. Idempotent on `external_id`: a repeat returns the
    /// existing record and no token. Latest prefers the new deployment once it is
    /// available: Lambda immediately, Gateway/Sidecar after a ready connection arrives.
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
            types::DeploymentTarget::Gateway => "gateway",
            types::DeploymentTarget::Sidecar => "sidecar",
            types::DeploymentTarget::Lambda => "lambda",
            _ => return Err(Error::Invalid("Select a deployment target".into())),
        };
        let reference = match (target, r.target_reference.filter(|v| !v.trim().is_empty())) {
            ("lambda", Some(v))
                if v.len() <= 2048
                    && v.starts_with("arn:aws:lambda:")
                    && v.split(":function:")
                        .nth(1)
                        .is_some_and(|name| !name.is_empty()) =>
            {
                Some(v)
            }
            ("lambda", _) => {
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
        let commit_message = short(r.commit_message, "Commit message")?;
        let branch = short(r.branch, "Branch")?;
        let commit_author = short(r.commit_author, "Commit author")?;
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        let current = crate::deployment::db::lock_opt(&tx, agent)
            .await?
            .ok_or(Error::NotFound)?;
        if let Some(external) = &external
            && let Some(existing) =
                crate::deployment::db::deployment_external_opt(&tx, agent, external).await?
        {
            tx.commit().await?;
            drop(tx_client);
            return Ok((self.deployment(agent, existing.id).await?, None, false));
        }
        let id = Uuid::new_v4();
        let token = secrets::random_secret();
        let hash = Sha256::digest(token.expose_secret().as_bytes()).to_vec();
        crate::deployment::db::deployment_create_one(
            &tx,
            id,
            agent,
            source,
            target,
            reference.as_deref(),
            repository.as_deref(),
            commit.as_deref(),
            external.as_deref(),
            &(label),
            &(hash),
            commit_message.as_deref(),
            branch.as_deref(),
            commit_author.as_deref(),
        )
        .await?;
        self.ensure_secrets(&tx, agent).await?;
        if current.routing == "latest" || current.serving_deployment_id.is_none() {
            self.promote_in(&tx, agent, id).await?;
        }
        tx.commit().await?;
        drop(tx_client);
        Ok((self.deployment(agent, id).await?, Some(token), true))
    }
    pub async fn promote(&self, agent: Uuid, deployment: Uuid) -> Result<types::Deployment, Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        crate::deployment::db::lock_opt(&tx, agent)
            .await?
            .ok_or(Error::NotFound)?;
        if !db::available_all(&tx, agent)
            .await?
            .iter()
            .any(|row| row.id == deployment)
        {
            return Err(Error::Invalid(
                "Only connected, ready deployments or Lambda can be promoted".into(),
            ));
        }
        self.promote_in(&tx, agent, deployment).await?;
        tx.commit().await?;
        drop(tx_client);
        self.get(agent).await
    }
    /// Make `deployment` the one new conversations route to.
    async fn promote_in(
        &self,
        tx: &Transaction<'_>,
        agent: Uuid,
        deployment: Uuid,
    ) -> Result<(), Error> {
        crate::deployment::db::deployment_promote_opt(tx, agent, deployment)
            .await?
            .ok_or(Error::NotFound)?;
        crate::deployment::db::weights_seed_execute(tx, agent, Some(deployment)).await?;
        Ok(())
    }
    /// Retire a deployment: its token stops authenticating and it leaves routing.
    /// Under latest routing, retiring the serving deployment hands serving to the newest
    /// other one; under weighted routing its weight must first be set to zero.
    pub async fn retire(
        &self,
        agent: Uuid,
        deployment: Uuid,
    ) -> Result<types::AgentDeployment, Error> {
        let mut tx_client = self.pool.get().await?;
        let tx = tx_client.transaction().await?;
        let current = crate::deployment::db::lock_opt(&tx, agent)
            .await?
            .ok_or(Error::NotFound)?;
        let retiring = crate::deployment::db::deployment_get_opt(&tx, deployment, agent)
            .await?
            .ok_or(Error::NotFound)?;
        if current.routing == "weighted" && retiring.traffic_weight > 0 {
            return Err(Error::Invalid(
                "Set this deployment's traffic weight to 0% before retiring it".into(),
            ));
        }
        if current.serving_deployment_id == Some(deployment) {
            let next = crate::deployment::db::deployment_latest_except_opt(
                &tx,
                agent,
                &all_targets(),
                deployment,
            )
            .await?
            .map(|r| r.id)
            .ok_or_else(|| {
                Error::Invalid("Register another deployment before retiring the serving one".into())
            })?;
            self.promote_in(&tx, agent, next).await?;
        }
        crate::deployment::db::deployment_retire_opt(&tx, deployment, agent)
            .await?
            .ok_or(Error::NotFound)?;
        tx.commit().await?;
        drop(tx_client);
        self.deployment(agent, deployment).await
    }
    pub async fn deployment(
        &self,
        agent: Uuid,
        deployment: Uuid,
    ) -> Result<types::AgentDeployment, Error> {
        let r =
            crate::deployment::db::deployment_get_opt(&self.pool.get().await?, deployment, agent)
                .await?
                .ok_or(Error::NotFound)?;
        let available = db::available_all(&self.pool.get().await?, agent)
            .await?
            .iter()
            .any(|row| row.id == deployment);
        let settings = self.get(agent).await?;
        Ok(DeploymentRow {
            id: r.id,
            agent_id: r.agent_id,
            source: r.source,
            target: r.target,
            target_reference: r.target_reference,
            repository: r.repository,
            commit_sha: r.commit_sha,
            external_id: r.external_id,
            label: r.label,
            status: r.status,
            token_issued: r.token_issued,
            serving: available
                && (if settings.routing.as_known() == Some(types::DeploymentRouting::Weighted) {
                    r.traffic_weight > 0
                } else {
                    settings.serving_deployment_id == r.id.to_string()
                }),
            routable: available,
            traffic_weight: r.traffic_weight,
            created_at: r.created_at,
            retired_at: r.retired_at,
            commit_message: r.commit_message,
            branch: r.branch,
            commit_author: r.commit_author,
        }
        .into())
    }
    pub async fn deployments(&self, agent: Uuid) -> Result<Vec<types::AgentDeployment>, Error> {
        let available: std::collections::BTreeSet<_> =
            db::available_all(&self.pool.get().await?, agent)
                .await?
                .into_iter()
                .map(|r| r.id)
                .collect();
        let settings = self.get(agent).await?;
        Ok(
            crate::deployment::db::deployments_list_all(&self.pool.get().await?, agent)
                .await?
                .into_iter()
                .map(|r| {
                    DeploymentRow {
                        id: r.id,
                        agent_id: r.agent_id,
                        source: r.source,
                        target: r.target,
                        target_reference: r.target_reference,
                        repository: r.repository,
                        commit_sha: r.commit_sha,
                        external_id: r.external_id,
                        label: r.label,
                        status: r.status,
                        token_issued: r.token_issued,
                        serving: available.contains(&r.id)
                            && (if settings.routing.as_known()
                                == Some(types::DeploymentRouting::Weighted)
                            {
                                r.traffic_weight > 0
                            } else {
                                settings.serving_deployment_id == r.id.to_string()
                            }),
                        routable: available.contains(&r.id),
                        traffic_weight: r.traffic_weight,
                        created_at: r.created_at,
                        retired_at: r.retired_at,
                        commit_message: r.commit_message,
                        branch: r.branch,
                        commit_author: r.commit_author,
                    }
                    .into()
                })
                .collect(),
        )
    }
    /// The deployment a (thread, agent) pair is pinned to, if any and still registered.
    /// The deployment a (thread, agent) pair is pinned to, if it is still registered.
    pub async fn pinned(&self, agent: Uuid, thread: Uuid) -> Result<Option<Uuid>, Error> {
        Ok(
            crate::deployment::db::deployment_pin_opt(&self.pool.get().await?, thread, agent)
                .await?
                .and_then(|r| r.deployment_id),
        )
    }
    pub async fn instances(&self, agent: Uuid) -> Result<Vec<types::AgentInstance>, Error> {
        Ok(
            crate::deployment::db::nodes_all(&self.pool.get().await?, agent)
                .await?
                .into_iter()
                .map(|r| types::AgentInstance {
                    instance_id: r.instance_id.to_string(),
                    agent_id: agent.to_string(),
                    deployment_id: r.deployment_id.to_string(),
                    public_url: r.public_url,
                    runtime_url: r.runtime_url,
                    ready: r.connection_id.is_some()
                        && r.agent_connected
                        && r.ready
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
        // Both host kinds can dial in without a publicly reachable address.
        for endpoint in [&r.public_url, &r.runtime_url] {
            if !endpoint.is_empty() {
                crate::agent::validate_endpoint(endpoint.clone())?;
            }
        }
        crate::deployment::db::register_execute(
            &self.pool.get().await?,
            agent,
            instance,
            deployment,
            &(r.public_url),
            &(r.runtime_url),
        )
        .await?;
        Ok(instance)
    }
    pub(crate) async fn open_watch(
        &self,
        agent: Uuid,
        instance: Uuid,
    ) -> Result<WatchConnection, Error> {
        let connection = Uuid::new_v4();
        db::watch_open_execute(&self.pool.get().await?, agent, instance, connection).await?;
        Ok(WatchConnection {
            pool: self.pool.clone(),
            agent,
            instance,
            connection,
        })
    }
    pub async fn heartbeat(
        &self,
        agent: Uuid,
        instance: Uuid,
        r: &wire::Heartbeat,
    ) -> Result<(), Error> {
        if crate::deployment::db::heartbeat_execute(
            &self.pool.get().await?,
            agent,
            instance,
            r.ready,
            r.agent_ready,
            r.agent_connected,
        )
        .await?
            == 0
        {
            tracing::warn!(agent_id=%agent, instance_id=%instance, "Heartbeat from an unregistered replica");
            return Ok(());
        }
        if let Ok(sample) = id(&r.sample_id) {
            crate::deployment::db::telemetry_health_execute(
                &self.pool.get().await?,
                sample,
                agent,
                &(instance.to_string()),
                Utc::now(),
                r.ready && r.agent_ready,
                r.latency_ms,
            )
            .await?;
        }
        Ok(())
    }
    pub async fn instance_live(&self, agent: Uuid, instance: Uuid) -> Result<bool, Error> {
        Ok(crate::deployment::db::instance_live_one(
            &self.pool.get().await?,
            agent,
            instance,
            liveness_secs(),
        )
        .await?
        .live)
    }
    pub async fn snapshot(
        &self,
        agent: Uuid,
        deployment: Uuid,
        instance: Uuid,
    ) -> Result<wire::Snapshot, Error> {
        let leases =
            crate::deployment::db::leases_held_all(&self.pool.get().await?, agent, instance)
                .await?
                .into_iter()
                .map(|r| lease_frame(agent, r.thread_id, Some(instance), r.updated_at))
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
            crate::deployment::db::leases_since_all(&self.pool.get().await?, agent, since)
                .await?
                .into_iter()
                .map(|r| {
                    (
                        lease_frame(agent, r.thread_id, Some(r.instance_id), r.updated_at),
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
        let row = crate::deployment::db::agent_secrets_one(&self.pool.get().await?, agent).await?;
        let blocked = crate::inference::budgets::blocked(&self.pool).await?;
        let mut response = wire::GetConfigurationResponse {
            inference_blocked: blocked.agents.contains(&agent),
            inference_blocked_identities: blocked
                .identities
                .iter()
                .map(|id| id.to_string())
                .collect(),
            logs_enabled: self.logs.as_ref().is_some_and(|logs| logs.enabled()),
            tracing_enabled: self.telemetry.as_ref().is_some_and(|queue| queue.enabled()),
            agent: crate::agent::rpc::project(&self.agents, self.agents.get(agent).await?)
                .await?
                .into(),
            agent_generation: row.generation,
            ..Default::default()
        };
        for assigned in
            crate::deployment::db::assigned_connections_all(&self.pool.get().await?, agent).await?
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
            let capability = if assigned.capability == "inference" {
                types::Capability::Inference
            } else {
                types::Capability::Channel
            };
            let identities = if capability == types::Capability::Channel {
                crate::deployment::db::identity_grants_all(
                    &self.pool.get().await?,
                    assigned.id,
                    agent,
                )
                .await?
            } else {
                vec![]
            }
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
                capability: capability.into(),
                alias: assigned.alias.clone().unwrap_or_default(),
                inference_blocked: blocked.assignments.contains(&(agent, assigned.id)),
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
        let row = crate::deployment::db::get_opt(&self.pool.get().await?, agent)
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
        Ok(crate::deployment::db::lease_holder_opt(
            &self.pool.get().await?,
            thread,
            agent,
            liveness_secs(),
        )
        .await?
        .filter(|r| r.live)
        .map(|r| Holder {
            instance: r.instance_id,
            deployment: r.deployment_id,
            version: r.updated_at,
        }))
    }
    /// Take or confirm the lease for `instance` in one transaction. A live holder
    /// keeps it; a dead holder loses it, and its interrupted work is ended here. The
    /// pair pins to the claimant's deployment on first lease; a pair pinned to another
    /// deployment is refused when nobody holds it, so routing decides where it runs.
    pub(crate) async fn lease_in(
        &self,
        tx: &Transaction<'_>,
        agent: Uuid,
        deployment: Uuid,
        instance: Uuid,
        thread: Uuid,
    ) -> Result<wire::ThreadLease, Error> {
        crate::chat::access::lock_thread_route(tx, thread).await?;
        // An existing conversation can only be executed by an agent already in it.
        if crate::deployment::db::project_has_thread_one(tx, thread)
            .await?
            .exists
            && !crate::deployment::db::thread_agent_one(tx, thread, agent)
                .await?
                .allowed
        {
            return Err(Error::Denied);
        }
        let mine = |version: DateTime<Utc>| lease_frame(agent, thread, Some(instance), version);
        let mut current = crate::deployment::db::lease_lock_opt(tx, thread, agent)
            .await?
            .map(|r| (r.instance_id, r.updated_at));
        if let Some((holder, version)) = current
            && holder != instance
            && crate::deployment::db::instance_live_one(tx, agent, holder, liveness_secs())
                .await?
                .live
        {
            return Ok(lease_frame(agent, thread, Some(holder), version));
        }
        // Known conversations first obey routing policy; directly-created sidecar
        // conversations without a projected roster acquire their pin on projection.
        let _ = crate::chat::pin_deployment(tx, thread, agent).await?;
        let pinned = crate::deployment::db::lease_pin_opt(tx, thread, agent, deployment)
            .await?
            .and_then(|r| r.deployment_id);
        if pinned.is_some_and(|p| p != deployment) {
            return Err(Error::Denied);
        }
        if current.is_none() {
            // Two replicas may reach here for a brand-new thread; the insert decides.
            if let Some(row) =
                crate::deployment::db::lease_insert_opt(tx, thread, agent, instance).await?
            {
                return Ok(mine(row.updated_at));
            }
            current = crate::deployment::db::lease_lock_opt(tx, thread, agent)
                .await?
                .map(|r| (r.instance_id, r.updated_at));
        }
        let Some((holder, version)) = current else {
            return Err(Error::Conflict);
        };
        if holder == instance {
            return Ok(mine(version));
        }
        // The holder is gone: the claimant takes over and the interrupted run follows the
        // agent's failure policy (restarted by the new holder, or failed here).
        let stop = crate::deployment::db::get_opt(tx, agent)
            .await?
            .is_some_and(|d| d.failure_mode == "stop");
        self.take_over(tx, agent, thread, holder, Some(instance), stop)
            .await?;
        let version = crate::deployment::db::lease_lock_opt(tx, thread, agent)
            .await?
            .map(|r| r.updated_at)
            .unwrap_or_else(Utc::now);
        tracing::debug!(agent_id=%agent, instance_id=%instance, thread_id=%thread, "Lease taken over from a dead holder");
        Ok(mine(version))
    }
    /// Drop this instance's lease on an evicted thread, in the batch's transaction.
    pub(crate) async fn release_in(
        &self,
        tx: &Transaction<'_>,
        agent: Uuid,
        instance: Uuid,
        thread: Uuid,
    ) -> Result<(), Error> {
        crate::deployment::db::lease_release_execute(tx, thread, agent, instance).await?;
        Ok(())
    }
    /// Which live replica should receive gateway-originated work for a thread: the live
    /// holder, else a live instance of the thread's pinned deployment, else one of the
    /// serving deployment. Never a replica of another deployment.
    pub(crate) async fn target_for(
        &self,
        agent: Uuid,
        thread: Option<Uuid>,
    ) -> Result<Option<Uuid>, Error> {
        let Some(selected) = self.selected(agent, thread).await? else {
            return Ok(None);
        };
        if !selected.routable
            || selected.target.as_known() != Some(types::DeploymentTarget::Sidecar)
        {
            return Ok(None);
        }
        let deployment = id(&selected.id)?;
        if let Some(thread) = thread
            && let Some(holder) = self.holder(agent, thread).await?
            && holder.deployment == Some(deployment)
        {
            return Ok(Some(holder.instance));
        }
        Ok(crate::deployment::db::live_node_opt(
            &self.pool.get().await?,
            agent,
            None::<Uuid>,
            Some(deployment),
            liveness_secs(),
        )
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
        crate::deployment::db::directive_insert_execute(
            &self.pool.get().await?,
            key,
            agent,
            instance,
            thread,
            &(payload),
        )
        .await?;
        Ok(key)
    }
    pub(crate) async fn pending_directives(
        &self,
        agent: Uuid,
        instance: Uuid,
    ) -> Result<Vec<wire::Directive>, Error> {
        crate::deployment::db::directives_pending_all(&self.pool.get().await?, agent, instance)
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
        crate::deployment::db::directive_result_opt(
            &self.pool.get().await?,
            key,
            agent,
            instance,
            &(payload),
        )
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
            let row = crate::deployment::db::directive_get_opt(&self.pool.get().await?, key)
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
            "CreateUser"
                | "CreateThread"
                | "GetThread"
                | "ListThreads"
                | "ListMessages"
                | "ListActivity"
                | "GetRun"
                | "SearchMessages"
                | "ListSessions"
                | "GetIdentity"
                | "ListAgents"
                | "GetSession"
                | "SetReadState"
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
        routing::check_creation_scope(agent, &call.method, &call.body, &call.content_type)?;
        let claims =
            tokens::verify_ingress(&call.caller_token, agent, &self.signing_key(agent).await?)?;
        if let Some(thread) = thread {
            if claims.thread_id.is_some_and(|scope| scope != thread)
                || !crate::deployment::db::thread_agent_one(&self.pool.get().await?, thread, agent)
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
            .uri(format!(
                "/tilde.provider.tilde.v1.ChatService/{}",
                call.method
            ));
        if let Ok(value) = http::HeaderValue::from_str(&call.content_type) {
            request = request.header(http::header::CONTENT_TYPE, value);
        }
        let mut request = request
            .body(axum::body::Body::from(call.body))
            .map_err(|_| Error::Denied)?;
        request.extensions_mut().insert(claims);
        let response = match tower::ServiceExt::oneshot(
            crate::chat::providers::tilde::rpc::router(self.chat()),
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
                let row = crate::chat::db::user_get_opt(&self.pool.get().await?, user)
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
    pub deployment: Option<Uuid>,
    pub version: DateTime<Utc>,
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
    pub target_reference: Option<String>,
    pub repository: Option<String>,
    pub commit_sha: Option<String>,
    pub external_id: Option<String>,
    pub label: Option<String>,
    /// What CI knows about the commit; the console shows these in place of the id.
    pub commit_message: Option<String>,
    pub branch: Option<String>,
    pub commit_author: Option<String>,
}
struct DeploymentRow {
    id: Uuid,
    agent_id: Uuid,
    source: String,
    target: String,
    target_reference: Option<String>,
    repository: Option<String>,
    commit_sha: Option<String>,
    external_id: Option<String>,
    label: String,
    status: String,
    token_issued: bool,
    serving: bool,
    routable: bool,
    traffic_weight: i32,
    created_at: chrono::DateTime<Utc>,
    retired_at: Option<chrono::DateTime<Utc>>,
    commit_message: Option<String>,
    branch: Option<String>,
    commit_author: Option<String>,
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
            routable: r.routable,
            traffic_weight: r.traffic_weight as u32,
            created_at: crate::chat::audit::timestamp(r.created_at).into(),
            retired_at: r
                .retired_at
                .map(|t| crate::chat::audit::timestamp(t).into())
                .unwrap_or_default(),
            commit_message: r.commit_message.unwrap_or_default(),
            branch: r.branch.unwrap_or_default(),
            commit_author: r.commit_author.unwrap_or_default(),
            ..Default::default()
        }
    }
}
fn target_wire(target: &str) -> types::DeploymentTarget {
    match target {
        "sidecar" => types::DeploymentTarget::Sidecar,
        "lambda" => types::DeploymentTarget::Lambda,
        _ => types::DeploymentTarget::Gateway,
    }
}
/// Latest considers every execution type; conversation pins keep existing work in place.
fn all_targets() -> Vec<String> {
    vec!["gateway".into(), "sidecar".into(), "lambda".into()]
}
fn short(value: Option<String>, name: &str) -> Result<Option<String>, Error> {
    match value {
        Some(v) if v.chars().count() > 512 => Err(Error::Invalid(format!("{name} is too long"))),
        Some(v) if v.trim().is_empty() => Ok(None),
        other => Ok(other),
    }
}
/// A lease as replicas see it. `version` is the row's `updated_at` in milliseconds, so a
/// replica can order a Watch frame against the lease its own Hydrate returned.
pub(crate) fn lease_frame(
    agent: Uuid,
    thread: Uuid,
    holder: Option<Uuid>,
    version: DateTime<Utc>,
) -> wire::ThreadLease {
    wire::ThreadLease {
        thread_id: thread.to_string(),
        agent_id: agent.to_string(),
        holder_instance_id: holder
            .map(|instance| instance.to_string())
            .unwrap_or_default(),
        held: holder.is_some(),
        version: version.timestamp_millis(),
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

/// Shared credential lookup for deployment RPCs and log ingestion only.
pub(crate) async fn authenticate(pool: &Pool, token: &str) -> Result<Authenticated, Error> {
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    crate::deployment::db::authenticate_opt(&pool.get().await?, &(hash))
        .await?
        .map(|r| Authenticated {
            agent: r.agent_id,
            deployment: r.deployment_id,
            target: target_wire(&r.target),
        })
        .ok_or(Error::Denied)
}

/// Drop also runs when the client cancels a stream, not only when its loop ends.
pub(crate) struct WatchConnection {
    pool: Pool,
    agent: Uuid,
    instance: Uuid,
    connection: Uuid,
}
impl Drop for WatchConnection {
    fn drop(&mut self) {
        let (pool, agent, instance, connection) = (
            self.pool.clone(),
            self.agent,
            self.instance,
            self.connection,
        );
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Ok(client) = pool.get().await {
                    let _ = db::watch_close_execute(&client, agent, instance, connection).await;
                }
            });
        }
    }
}
