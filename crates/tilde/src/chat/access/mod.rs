//! Provider-route IAM. All policy/identity changes serialize on the connection lock.
//! Verification tokens are possession proofs delivered only to the recipient, never to management clients.
pub mod identity;
pub mod rpc;
use super::{
    Chat, ChatError,
    providers::{Channels, adapter},
};
use crate::{connections::service::Connections, proto::tilde::types::v1 as types};
use connectrpc::ConnectError;
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use subtle::ConstantTimeEq;
use uuid::Uuid;
type Result<T> = std::result::Result<T, ConnectError>;
#[derive(Clone)]
pub struct AgentAccess {
    pub(crate) connections: Connections,
    chat: Chat,
}
#[derive(sqlx::FromRow)]
struct Route {
    connection_id: Uuid,
    agent_id: Uuid,
    access_mode: String,
    name: String,
    provider_id: String,
    type_id: String,
    status: String,
    provider_name: String,
    icon_url: Option<String>,
    agent_name: String,
    agent_identity_type: Option<String>,
    agent_identity_value: Option<String>,
}
impl Route {
    fn wire(self) -> types::ChannelAccess {
        let provider = adapter(&self.provider_id, &self.type_id);
        let template = provider.is_some_and(|p| p.supports_verification_template());
        let agent_identity = self
            .agent_identity_value
            .map(|value| types::AgentChannelIdentity {
                connection_id: self.connection_id.to_string(),
                agent_id: self.agent_id.to_string(),
                value,
                identity_type: identity::kind_value(
                    self.agent_identity_type.as_deref().unwrap_or(""),
                )
                .into(),
                ..Default::default()
            });
        types::ChannelAccess { agent_identity: agent_identity.into(), connection_id:self.connection_id.to_string(),agent_id:self.agent_id.to_string(),account_name:self.name,
   provider_id:self.provider_id,provider_name:self.provider_name,icon_url:self.icon_url,connection_status:self.status,
   mode:match self.access_mode.as_str(){"private"=>types::ChannelAccessMode::Private,"public"=>types::ChannelAccessMode::Public,_=>types::ChannelAccessMode::Disabled}.into(),
   identity_types:provider.map(|p|p.identity_types().iter().copied().map(Into::into).collect()).unwrap_or_default(),verification_supported:provider.is_some_and(|p|p.verification_supported()),supports_template:template,
   verification_instructions:if template {"Free-form delivery requires an open WhatsApp customer-service window. Otherwise use an approved template with one body parameter containing the verification link."}else{"A private message with an expiring approval link will be sent to this identity."}.into(), ..Default::default() }
    }
}
impl AgentAccess {
    pub fn new(connections: Connections, chat: Chat) -> Self {
        Self { connections, chat }
    }
    async fn locked_route(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        connection: Uuid,
        agent: Uuid,
    ) -> Result<Route> {
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            connection.to_string()
        )
        .execute(&mut **tx)
        .await
        .map_err(ChatError::from)?;
        sqlx::query_file_as!(
            Route,
            "../../queries/channel_access/route.sql",
            connection,
            agent
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(ChatError::from)?
        .ok_or_else(|| ConnectError::not_found("Chat provider is not assigned to this agent"))
    }
    pub async fn routes(
        &self,
        agent: Uuid,
        after: Option<Uuid>,
        size: u32,
    ) -> Result<(Vec<types::ChannelAccess>, String)> {
        let size = size.clamp(1, 100) as usize;
        let mut rows = sqlx::query_file_as!(
            Route,
            "../../queries/channel_access/routes.sql",
            agent,
            after,
            (size + 1) as i64
        )
        .fetch_all(&self.connections.pool)
        .await
        .map_err(ChatError::from)?;
        let more = rows.len() > size;
        if more {
            rows.pop();
        }
        let next = if more {
            rows.last().unwrap().connection_id.to_string()
        } else {
            String::new()
        };
        Ok((rows.into_iter().map(Route::wire).collect(), next))
    }
    pub async fn identities(
        &self,
        agent: Uuid,
        connection: Option<Uuid>,
        after: Option<Uuid>,
        size: u32,
    ) -> Result<(Vec<types::ChannelIdentity>, String)> {
        let size = size.clamp(1, 100) as usize;
        let mut rows = sqlx::query_file!(
            "../../queries/channel_access/identities.sql",
            agent,
            connection,
            after,
            (size + 1) as i64
        )
        .fetch_all(&self.connections.pool)
        .await
        .map_err(ChatError::from)?;
        let more = rows.len() > size;
        if more {
            rows.pop();
        }
        let next = if more {
            rows.last().unwrap().id.to_string()
        } else {
            String::new()
        };
        Ok((
            rows.into_iter()
                .map(|r| types::ChannelIdentity {
                    id: r.id.to_string(),
                    connection_id: r.connection_id.to_string(),
                    value: r.value,
                    identity_type: identity::kind_value(&r.identity_type).into(),
                    verified_at: r.verified_at.map(super::audit::timestamp).into(),
                    allowed: r.allowed,
                    verification_id: r.verification_id.map(|id| id.to_string()),
                    verification_status: r.verification_status,
                    ..Default::default()
                })
                .collect(),
            next,
        ))
    }
    pub async fn set_mode(
        &self,
        connection: Uuid,
        agent: Uuid,
        mode: types::ChannelAccessMode,
    ) -> Result<()> {
        let mode = match mode {
            types::ChannelAccessMode::Private => "private",
            types::ChannelAccessMode::Public => "public",
            types::ChannelAccessMode::Disabled => "disabled",
            _ => {
                return Err(ConnectError::invalid_argument(
                    "Choose private, public or disabled",
                ));
            }
        };
        let mut tx = self
            .connections
            .pool
            .begin()
            .await
            .map_err(ChatError::from)?;
        let route = self.locked_route(&mut tx, connection, agent).await?;
        if mode == "private"
            && !adapter(&route.provider_id, &route.type_id)
                .is_some_and(|p| p.verification_supported())
        {
            return Err(ConnectError::failed_precondition(
                "This provider supports public or disabled access only",
            ));
        }
        sqlx::query_file!(
            "../../queries/channel_access/policy.sql",
            connection,
            agent,
            mode
        )
        .execute(&mut *tx)
        .await
        .map_err(ChatError::from)?;
        tx.commit().await.map_err(ChatError::from)?;
        self.stop_invalid(connection, agent).await
    }
    pub async fn set_allowed(
        &self,
        connection: Uuid,
        agent: Uuid,
        identity: Uuid,
        allowed: bool,
    ) -> Result<()> {
        let mut tx = self
            .connections
            .pool
            .begin()
            .await
            .map_err(ChatError::from)?;
        self.locked_route(&mut tx, connection, agent).await?;
        let row = sqlx::query_file!(
            "../../queries/channel_access/identity_get.sql",
            connection,
            identity
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(ChatError::from)?
        .ok_or_else(|| ConnectError::not_found("Identity not found"))?;
        if allowed && row.verified_at.is_none() {
            return Err(ConnectError::failed_precondition(
                "Verify the identity before allowing access",
            ));
        }
        if !allowed {
            sqlx::query_file!(
                "../../queries/channel_access/verification_supersede.sql",
                identity,
                agent
            )
            .execute(&mut *tx)
            .await
            .map_err(ChatError::from)?;
        }
        sqlx::query_file!(
            "../../queries/channel_access/grant.sql",
            connection,
            agent,
            identity,
            allowed
        )
        .execute(&mut *tx)
        .await
        .map_err(ChatError::from)?;
        tx.commit().await.map_err(ChatError::from)?;
        self.stop_invalid(connection, agent).await
    }
    async fn stop_invalid(&self, connection: Uuid, agent: Uuid) -> Result<()> {
        let rows = sqlx::query_file!(
            "../../queries/channel_access/invalid_invocations.sql",
            connection,
            agent
        )
        .fetch_all(&self.connections.pool)
        .await
        .map_err(ChatError::from)?;
        for row in rows {
            if let Err(error) = self.chat.cancel_invocation(row.id).await {
                tracing::warn!(invocation_id=%row.id,error=%error,"Access revoked; host cancellation was not acknowledged");
            }
        }
        Ok(())
    }
    /// Create or reuse a pending identity, then deliver a secret link directly through its provider.
    pub async fn request_verification(
        &self,
        request: VerificationRequest,
    ) -> Result<(Uuid, String)> {
        let VerificationRequest {
            id,
            connection,
            agent,
            identity_type,
            value,
            template_name,
            template_language,
        } = request;
        let mut tx = self
            .connections
            .pool
            .begin()
            .await
            .map_err(ChatError::from)?;
        let route = self.locked_route(&mut tx, connection, agent).await?;
        if route.status != "ready" {
            return Err(ConnectError::failed_precondition(
                "Configure this connection before verifying identities",
            ));
        }
        if route.agent_identity_value.is_none() {
            return Err(ConnectError::failed_precondition(
                "Reconnect this channel to establish the agent's sending identity before inviting recipients",
            ));
        }
        let provider = adapter(&route.provider_id, &route.type_id)
            .ok_or_else(|| ConnectError::failed_precondition("No identity provider available"))?;
        if !provider.verification_supported() {
            return Err(ConnectError::failed_precondition(
                "This provider does not support private verification",
            ));
        }
        let recipient = provider.verification_recipient(identity_type, &value)?;
        let value = recipient.value;
        let kind = identity::kind_name(recipient.identity_type);
        if template_name.is_some() && !provider.supports_verification_template() {
            return Err(ConnectError::invalid_argument(
                "This provider does not support message templates",
            ));
        }
        if template_name
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 128)
            || template_language
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 32)
        {
            return Err(ConnectError::invalid_argument(
                "Invalid verification template",
            ));
        }
        if let Some(previous) =
            sqlx::query_file!("../../queries/channel_access/verification_get.sql", id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(ChatError::from)?
        {
            if previous.connection_id != connection
                || previous.agent_id != agent
                || previous.value != value
                || previous.identity_type != kind
            {
                return Err(ConnectError::already_exists(
                    "Verification request ID is already used",
                ));
            }
            return Ok((id, previous.status));
        }
        let identity =
            super::providers::ingress::stable(connection, "user", &format!("{kind}:{value}"));
        sqlx::query_file!("../../queries/chat/channel_user.sql", identity, value)
            .execute(&mut *tx)
            .await
            .map_err(ChatError::from)?;
        let identity = sqlx::query_file!(
            "../../queries/channel_access/identity_upsert.sql",
            identity,
            connection,
            kind,
            value
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(ChatError::from)?
        .id;
        if let Some(recent) = sqlx::query_file!(
            "../../queries/channel_access/verification_latest.sql",
            identity,
            agent
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(ChatError::from)?
            && chrono::Utc::now() - recent.created_at < chrono::Duration::seconds(30)
        {
            return Err(ConnectError::resource_exhausted(
                "Wait 30 seconds before resending verification",
            ));
        }
        sqlx::query_file!(
            "../../queries/channel_access/grant.sql",
            connection,
            agent,
            identity,
            false
        )
        .execute(&mut *tx)
        .await
        .map_err(ChatError::from)?;
        sqlx::query_file!(
            "../../queries/channel_access/verification_supersede.sql",
            identity,
            agent
        )
        .execute(&mut *tx)
        .await
        .map_err(ChatError::from)?;
        let token = crate::connections::model::random_secret();
        let hash = Sha256::digest(token.expose_secret().as_bytes()).to_vec();
        sqlx::query_file!(
            "../../queries/channel_access/verification_create.sql",
            id,
            connection,
            agent,
            identity,
            hash
        )
        .execute(&mut *tx)
        .await
        .map_err(ChatError::from)?;
        tx.commit().await.map_err(ChatError::from)?;
        let url = zeroize::Zeroizing::new(format!(
            "{}/identity/verify/{}?identity_verification_token={}",
            self.connections.public_url,
            id,
            token.expose_secret()
        ));
        drop(token);
        let delivery = async {
            let access = Channels::new(self.connections.clone())
                .access(connection)
                .await?;
            provider
                .send_verification(
                    &access,
                    identity::VerificationMessage {
                        value: &value,
                        agent_name: &route.agent_name,
                        url: &url,
                        template_name: template_name.as_deref(),
                        template_language: template_language.as_deref(),
                    },
                )
                .await
        };
        let delivered = matches!(
            tokio::time::timeout(std::time::Duration::from_secs(20), delivery).await,
            Ok(Ok(()))
        );
        let status = if delivered { "delivered" } else { "failed" };
        sqlx::query_file!(
            "../../queries/channel_access/verification_result.sql",
            id,
            status
        )
        .execute(&self.connections.pool)
        .await
        .map_err(ChatError::from)?;
        Ok((id, status.into()))
    }
    pub async fn verification(&self, id: Uuid, token: &str) -> Result<types::IdentityVerification> {
        let row = sqlx::query_file!("../../queries/channel_access/verification_get.sql", id)
            .fetch_optional(&self.connections.pool)
            .await
            .map_err(ChatError::from)?
            .ok_or_else(|| ConnectError::unauthenticated("Invalid or expired verification link"))?;
        let hash = Sha256::digest(token.as_bytes());
        if token.len() > 128
            || !bool::from(hash.as_slice().ct_eq(&row.token_hash))
            || row.expires_at <= chrono::Utc::now()
        {
            return Err(ConnectError::unauthenticated(
                "Invalid or expired verification link",
            ));
        }
        let agent_identity = row
            .agent_identity_value
            .map(|value| types::AgentChannelIdentity {
                connection_id: row.connection_id.to_string(),
                agent_id: row.agent_id.to_string(),
                value,
                identity_type: identity::kind_value(
                    row.agent_identity_type.as_deref().unwrap_or(""),
                )
                .into(),
                ..Default::default()
            });
        Ok(types::IdentityVerification {
            agent_identity: agent_identity.into(),
            id: row.id.to_string(),
            agent_name: row.agent_name,
            account_name: row.account_name,
            provider_name: row.provider_name,
            value: row.value,
            identity_type: identity::kind_value(&row.identity_type).into(),
            icon_url: row.icon_url,
            status: row.status,
            expires_at: super::audit::timestamp(row.expires_at).into(),
            ..Default::default()
        })
    }
    /// GET never grants access: only explicit approval atomically consumes a delivered proof.
    pub async fn approve(&self, id: Uuid, token: &str) -> Result<types::IdentityVerification> {
        let view = self.verification(id, token).await?;
        if view.status == "approved" {
            return Ok(view);
        }
        let info = sqlx::query_file!("../../queries/channel_access/verification_get.sql", id)
            .fetch_one(&self.connections.pool)
            .await
            .map_err(ChatError::from)?;
        let mut tx = self
            .connections
            .pool
            .begin()
            .await
            .map_err(ChatError::from)?;
        self.locked_route(&mut tx, info.connection_id, info.agent_id)
            .await?;
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        let row = sqlx::query_file!(
            "../../queries/channel_access/verification_claim.sql",
            id,
            hash
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(ChatError::from)?
        .ok_or_else(|| {
            ConnectError::failed_precondition("This verification request cannot be approved")
        })?;
        sqlx::query_file!(
            "../../queries/channel_access/identity_verify.sql",
            row.identity_id
        )
        .execute(&mut *tx)
        .await
        .map_err(ChatError::from)?;
        sqlx::query_file!(
            "../../queries/channel_access/grant.sql",
            row.connection_id,
            row.agent_id,
            row.identity_id,
            true
        )
        .execute(&mut *tx)
        .await
        .map_err(ChatError::from)?;
        tx.commit().await.map_err(ChatError::from)?;
        self.verification(id, token).await
    }
}
pub struct VerificationRequest {
    pub id: Uuid,
    pub connection: Uuid,
    pub agent: Uuid,
    pub identity_type: types::IdentityType,
    pub value: String,
    pub template_name: Option<String>,
    pub template_language: Option<String>,
}

/// Policy reads used for dispatch must occur after taking the same lock as policy updates.
pub(crate) async fn lock_thread_route(
    tx: &mut Transaction<'_, Postgres>,
    thread: Uuid,
) -> super::Result<()> {
    if let Some(binding) = sqlx::query_file!("../../queries/chat/channel_binding.sql", thread)
        .fetch_optional(&mut **tx)
        .await?
    {
        sqlx::query_file!(
            "../../queries/connections/connection_lock.sql",
            binding.connection_id.to_string()
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
