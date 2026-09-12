//! Agent deployment configuration and scoped registration. Postgres owns control
//! decisions; the agent-event-ingress listener is the only sidecar control entry.
//! Gateway mode does not start Corrosion or require any peer infrastructure.
pub mod attachments;
pub mod bridge;
pub mod cold;
pub mod corrosion;
pub mod gateway;
pub mod logs;
pub mod project;
pub mod provider_events;
pub mod proxy;
pub mod public;
pub mod recovery;
pub mod retention;
pub mod routing;
pub mod rpc;
pub mod runtime;
mod secrets;
pub mod sidecar;
mod sidecar_commands;
pub mod telemetry;
pub mod tokens;
use crate::proto::tilde::{agent_event_ingress::v1 as wire, types::v1 as types};
use crate::{
    agent::Agents,
    connections::service::Connections,
    encryption::{Encryption, SealedSecret, SecretBinding},
    error::Error,
};
use chrono::Utc;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub const CORROSION_REVISION: &str = "753cfd2408d67edeaf3052a990175356ed4dcbc6";
pub fn schema_revision() -> String {
    hex::encode(Sha256::digest(
        include_str!("../../../../schema/corrosion/runtime.sql").as_bytes(),
    ))
}
#[derive(Clone)]
pub struct Deployments {
    pub(crate) pool: PgPool,
    pub(crate) encryption: Arc<Encryption>,
    pub(crate) agents: Agents,
    pub(crate) connections: Connections,
    pub(crate) logs: Option<crate::logs::Delivery>,
    pub(crate) telemetry: Option<crate::telemetry::delivery::Queue>,
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
            retention_days: row.retention_days as u32,
            token_issued: row.token_hash.is_some(),
            ..Default::default()
        })
    }
    pub async fn set(
        &self,
        agent: Uuid,
        mode: types::DeploymentMode,
        endpoint: Option<String>,
        failure: types::SidecarFailureMode,
        days: u32,
    ) -> Result<types::Deployment, Error> {
        if days == 0 {
            return Err(Error::Invalid(
                "Retention must be a positive number of whole days".into(),
            ));
        }
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
            if current.deployment_mode == "sidecar"
                && sqlx::query_file!("../../queries/deployment/retained.sql", agent)
                    .fetch_one(&mut *tx)
                    .await?
                    .retained
            {
                return Err(Error::Invalid(
                    "Wait for paused conversations to finish archiving before changing deployment"
                        .into(),
                ));
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
            if mode == "sidecar" {
                sqlx::query_file!("../../queries/deployment/old_placements.sql", agent)
                    .execute(&mut *tx)
                    .await?;
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
        sqlx::query_file!(
            "../../queries/deployment/settings.sql",
            agent,
            failure,
            i64::from(days)
        )
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
                        &secrets::Secrets::generate()?.encode()?,
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
                    public_ingress_url: r.public_ingress_url,
                    agent_ingress_url: r.agent_ingress_url,
                    runtime_url: r.runtime_url,
                    gossip_address: r.gossip_address,
                    ready: r.ready
                        && r.agent_ready
                        && r.last_seen_at > Utc::now() - chrono::Duration::seconds(15),
                    last_seen_at: crate::chat::audit::timestamp(r.last_seen_at).into(),
                    ..Default::default()
                })
                .collect(),
        )
    }
    pub async fn register(
        &self,
        agent: Uuid,
        r: wire::RegisterSidecarRequest,
    ) -> Result<wire::RegisterSidecarResponse, Error> {
        let instance = id(&r.instance_id)?;
        for endpoint in [
            &r.public_ingress_url,
            &r.agent_ingress_url,
            &r.runtime_url,
            &r.local_agent_endpoint,
        ] {
            crate::agent::validate_endpoint(endpoint.clone())?;
        }
        let gossip: std::net::SocketAddr = r.gossip_address.parse().map_err(|_| {
            Error::Invalid("Gossip address must be an advertised IP and port".into())
        })?;
        if gossip.ip().is_unspecified() || gossip.port() == 0 {
            return Err(Error::Invalid("Gossip address must be reachable".into()));
        }
        let row = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_one(&self.pool)
            .await?;
        let secret = self.open_secrets(
            agent,
            row.encrypted_secrets.as_deref().ok_or(Error::Denied)?,
        )?;
        let certificate = secret.certificate(gossip.ip())?;
        sqlx::query_file!(
            "../../queries/deployment/register.sql",
            agent,
            instance,
            r.public_ingress_url,
            r.agent_ingress_url,
            r.runtime_url,
            r.gossip_address,
            r.local_agent_endpoint
        )
        .execute(&self.pool)
        .await?;
        Ok(wire::RegisterSidecarResponse {
            corrosion_token: secret.api_token.expose_secret().into(),
            agent_id: agent.to_string(),
            cluster_id: row.cluster_id.to_string(),
            peers: self
                .nodes(agent)
                .await?
                .into_iter()
                .filter(|p| p.instance_id != r.instance_id)
                .collect(),
            encryption_key: secret.encryption_key.expose_secret().into(),
            token_signing_key: secret.signing_key.expose_secret().into(),
            tls_certificate: certificate.0,
            tls_private_key: certificate.1.expose_secret().into(),
            tls_ca: secret.ca_certificate.expose_secret().into(),
            retention_days: row.retention_days as u32,
            schema_revision: schema_revision(),
            ..Default::default()
        })
    }
    pub async fn heartbeat(
        &self,
        agent: Uuid,
        r: wire::HeartbeatRequest,
    ) -> Result<wire::HeartbeatResponse, Error> {
        let instance = id(&r.instance_id)?;
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
        if r.agent_ready {
            sqlx::query_file!("../../queries/deployment/cold_renew.sql", agent, instance)
                .execute(&self.pool)
                .await?;
        }
        let row = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_one(&self.pool)
            .await?;
        Ok(wire::HeartbeatResponse {
            paused: row.paused,
            agent_generation: row.generation,
            retention_days: row.retention_days as u32,
            ..Default::default()
        })
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
    fn open_secrets(&self, agent: Uuid, sealed: &[u8]) -> Result<secrets::Secrets, Error> {
        let value = self
            .encryption
            .open(secret_binding(agent), SealedSecret::from_bytes(sealed)?)?;
        secrets::Secrets::decode(value)
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
