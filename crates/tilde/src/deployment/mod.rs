//! Agent deployment settings and the gateway half of the sidecar protocol.
//! Postgres owns every control decision. Replicas dial in over one connection;
//! the gateway never opens a connection to a replica.
pub mod attachments;
pub mod gateway;
mod hydrate;
pub mod logs;
pub mod project;
pub mod provider_events;
pub mod public;
pub mod recovery;
mod relay;
pub mod routing;
pub mod rpc;
pub mod runtime;
mod secrets;
pub(crate) use secrets::random_secret;
pub mod sidecar;
pub mod telemetry;
pub mod tokens;
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
    pub assignments: Notifications,
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
            mode: if row.deployment_mode == "sidecar" {
                types::DeploymentMode::Sidecar
            } else {
                types::DeploymentMode::Gateway
            }
            .into(),
            endpoint_url: row.endpoint_url,
            failure_mode: if row.failure_mode == "stop" {
                types::SidecarFailureMode::Stop
            } else {
                types::SidecarFailureMode::Reassign
            }
            .into(),
            token_issued: row.token_hash.is_some(),
            ..Default::default()
        })
    }
    pub async fn is_sidecar(&self, agent: Uuid) -> Result<bool, Error> {
        Ok(sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_optional(&self.pool)
            .await?
            .is_some_and(|r| r.deployment_mode == "sidecar"))
    }
    pub async fn set(
        &self,
        agent: Uuid,
        mode: types::DeploymentMode,
        endpoint: Option<String>,
        failure: types::SidecarFailureMode,
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
            sqlx::query_file!("../../queries/deployment/migrate_assignments.sql", agent)
                .execute(&mut *tx)
                .await?;
        }
        let endpoint = if mode == "gateway" {
            Some(crate::agent::validate_endpoint(endpoint.ok_or_else(
                || Error::Invalid("Gateway deployment requires an endpoint URL".into()),
            )?)?)
        } else {
            None
        };
        sqlx::query_file!("../../queries/deployment/set.sql", agent, mode, endpoint)
            .execute(&mut *tx)
            .await?;
        sqlx::query_file!("../../queries/deployment/settings.sql", agent, failure)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.get(agent).await
    }
    pub async fn authenticate(&self, token: &str) -> Result<Uuid, Error> {
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        sqlx::query_file!("../../queries/deployment/authenticate.sql", hash)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| r.id)
            .ok_or(Error::Denied)
    }
    pub async fn issue_token(&self, agent: Uuid) -> Result<SecretString, Error> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_file!("../../queries/deployment/lock.sql", agent)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        if row.deployment_mode != "sidecar" {
            return Err(Error::Invalid(
                "Select sidecar mode before issuing a deployment token".into(),
            ));
        }
        let token = secrets::random_secret();
        let hash = Sha256::digest(token.expose_secret().as_bytes()).to_vec();
        let sealed = if row.encrypted_secrets.is_none() {
            Some(
                self.encryption
                    .seal(
                        secret_binding(agent),
                        &secrets::Secrets::generate().encode()?,
                    )?
                    .into_bytes(),
            )
        } else {
            None
        };
        sqlx::query_file!(
            "../../queries/deployment/issue_token.sql",
            agent,
            hash,
            sealed
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(token)
    }
    pub async fn nodes(&self, agent: Uuid) -> Result<Vec<types::SidecarNode>, Error> {
        Ok(
            sqlx::query_file!("../../queries/deployment/nodes.sql", agent)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|r| types::SidecarNode {
                    instance_id: r.instance_id.to_string(),
                    agent_id: agent.to_string(),
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
    pub async fn register(&self, agent: Uuid, r: &wire::WatchRequest) -> Result<Uuid, Error> {
        let instance = id(&r.instance_id)?;
        for endpoint in [&r.public_url, &r.runtime_url] {
            crate::agent::validate_endpoint(endpoint.clone())?;
        }
        sqlx::query_file!(
            "../../queries/deployment/register.sql",
            agent,
            instance,
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
            return Err(Error::NotFound);
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
    pub async fn snapshot(&self, agent: Uuid) -> Result<wire::Snapshot, Error> {
        let since = chrono::DateTime::<Utc>::from_timestamp(0, 0).unwrap_or_default();
        Ok(wire::Snapshot {
            configuration: self.configuration(agent).await?.into(),
            assignments: self
                .assignments_since(agent, since)
                .await?
                .into_iter()
                .map(|(a, _)| a)
                .collect(),
            token_signing_key: self.signing_key(agent).await?.expose_secret().into(),
            ..Default::default()
        })
    }
    pub async fn assignments_since(
        &self,
        agent: Uuid,
        since: chrono::DateTime<Utc>,
    ) -> Result<Vec<(types::ParticipantAssignment, chrono::DateTime<Utc>)>, Error> {
        Ok(sqlx::query_file!(
            "../../queries/deployment/assignments_since.sql",
            agent,
            since
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| {
            (
                types::ParticipantAssignment {
                    thread_id: r.thread_id.to_string(),
                    participant_id: r.participant_id.to_string(),
                    agent_id: agent.to_string(),
                    owner_instance_id: r.owner_instance_id.to_string(),
                    generation: r.generation as u64,
                    stopped: r.stopped,
                    ..Default::default()
                },
                r.updated_at,
            )
        })
        .collect())
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
    /// Grant or refuse ownership of one thread participant under the Postgres lock.
    pub async fn claim(
        &self,
        agent: Uuid,
        instance: Uuid,
        thread: Uuid,
        participant: Uuid,
    ) -> Result<wire::ClaimResult, Error> {
        let mut tx = self.pool.begin().await?;
        let result = self
            .claim_in(&mut tx, agent, instance, thread, participant)
            .await?;
        tx.commit().await?;
        Ok(result)
    }
    pub(crate) async fn claim_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        agent: Uuid,
        instance: Uuid,
        thread: Uuid,
        participant: Uuid,
    ) -> Result<wire::ClaimResult, Error> {
        crate::chat::access::lock_thread_route(tx, thread).await?;
        // An existing conversation can only be claimed by an agent already in it.
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
        let current = sqlx::query_file!(
            "../../queries/deployment/assignment_lock.sql",
            thread,
            participant
        )
        .fetch_optional(&mut **tx)
        .await?;
        let result = |granted: bool, generation: i64, owner: Uuid| wire::ClaimResult {
            thread_id: thread.to_string(),
            participant_id: participant.to_string(),
            granted,
            generation: generation as u64,
            owner_instance_id: owner.to_string(),
            ..Default::default()
        };
        let generation = match current {
            None => 1,
            Some(c) if c.owner_instance_id == instance && !c.stopped => {
                return Ok(result(true, c.generation, instance));
            }
            Some(c) => {
                let live = !c.stopped
                    && sqlx::query_file!(
                        "../../queries/deployment/instance_live.sql",
                        agent,
                        c.owner_instance_id
                    )
                    .fetch_one(&mut **tx)
                    .await?
                    .live;
                if live {
                    return Ok(result(false, c.generation, c.owner_instance_id));
                }
                // The owner is gone: this is a recovery with the claimant as the
                // replacement, so the interrupted run follows the same policy.
                let generation = c.generation.checked_add(1).ok_or(Error::Conflict)?;
                let restart_run =
                    sqlx::query_file!("../../queries/deployment/failure_mode.sql", agent)
                        .fetch_optional(&mut **tx)
                        .await?
                        .is_none_or(|d| d.failure_mode != "stop");
                self.take_over(
                    tx,
                    recovery::TakeOver {
                        thread,
                        participant,
                        agent,
                        previous: c.owner_instance_id,
                        owner: instance,
                        generation,
                        mark_stopped: false,
                        restart_run,
                    },
                )
                .await?;
                tracing::debug!(agent_id=%agent, instance_id=%instance, thread_id=%thread, generation, "Claim took over a dead owner");
                return Ok(result(true, generation, instance));
            }
        };
        sqlx::query_file!(
            "../../queries/deployment/assign.sql",
            thread,
            participant,
            agent,
            instance,
            generation,
            false
        )
        .execute(&mut **tx)
        .await?;
        tracing::debug!(agent_id=%agent, instance_id=%instance, thread_id=%thread, generation, "Claim granted");
        Ok(result(true, generation, instance))
    }
    /// Which live replica should receive work for a thread, assigning one when needed.
    pub(crate) async fn owner_for(
        &self,
        agent: Uuid,
        thread: Option<Uuid>,
    ) -> Result<Option<(Uuid, u64)>, Error> {
        if let Some(thread) = thread
            && let Some(current) =
                sqlx::query_file!("../../queries/deployment/current_owner.sql", thread, agent)
                    .fetch_optional(&self.pool)
                    .await?
            && !current.stopped
            && self.instance_live(agent, current.owner_instance_id).await?
        {
            return Ok(Some((current.owner_instance_id, current.generation as u64)));
        }
        let Some(node) = sqlx::query_file!("../../queries/deployment/choose_owner.sql", agent)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        if let Some(thread) = thread
            && let Some(current) =
                sqlx::query_file!("../../queries/deployment/current_owner.sql", thread, agent)
                    .fetch_optional(&self.pool)
                    .await?
        {
            let claim = self
                .claim(agent, node.instance_id, thread, current.participant_id)
                .await?;
            return Ok(Some((node.instance_id, claim.generation)));
        }
        Ok(Some((node.instance_id, 0)))
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
        generation: u64,
        action: wire::directive::Action,
    ) -> Result<Uuid, Error> {
        let key = Uuid::new_v4();
        let directive = wire::Directive {
            id: key.to_string(),
            thread_id: thread.map(|t| t.to_string()).unwrap_or_default(),
            generation,
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
            generation as i64,
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
    async fn wait_directive(
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
                let result = row.result.ok_or(Error::NotFound)?;
                return self.open_record(key, "result", &result);
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
        let (instance, generation) = self.owner_for(agent, thread).await?.ok_or_else(|| {
            Error::Invalid("No sidecar replica is available for this agent".into())
        })?;
        let key = self
            .direct(agent, instance, thread, generation, call.into())
            .await?;
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
