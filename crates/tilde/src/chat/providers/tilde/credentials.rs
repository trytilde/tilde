//! The application API key is the `api_key` value of the agent's Tilde connection. It
//! authenticates a trusted application; the identity that application asserts is recorded as
//! a channel identity of that connection and admitted by the same rule as every other channel.
use crate::{
    chat::{Chat, ChatError},
    connections::{catalog::tilde as catalog, service::Connections},
    deployment::{Deployments, tokens::IngressClaims},
    error::Error,
};
use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;
use uuid::Uuid;
impl Chat {
    fn connections(&self) -> Result<&Connections, Error> {
        self.channels
            .as_ref()
            .map(|channels| &channels.connections)
            .ok_or_else(|| Error::Invalid("Connections are not configured".into()))
    }
    /// Reveal or rotate the agent's application key. Rotation replaces the sealed value and
    /// bumps the credential version, so every cached copy is dropped before the key is returned.
    pub async fn tilde_chat_key(&self, agent: Uuid, rotate: bool) -> Result<SecretString, Error> {
        let connections = self.connections()?;
        let connection =
            crate::connections::db::tilde_connection_opt(&self.pg()?.get().await?, agent)
                .await?
                .ok_or(Error::NotFound)?
                .id;
        if rotate {
            let key = catalog::generate_key();
            connections
                .replace_value(connection, catalog::KEY_FIELD, &key)
                .await?;
            return Ok(key);
        }
        connections
            .resolve(connection)
            .await?
            .remove(catalog::KEY_FIELD)
            .ok_or(Error::NotFound)
    }
}
impl Deployments {
    pub(crate) async fn authenticate_tilde(
        &self,
        agent: Uuid,
        api_key: &str,
        identity: &str,
    ) -> Result<(SecretString, IngressClaims), Error> {
        let identity = crate::chat::access::identity::Identity {
            identity_type: crate::proto::tilde::types::v1::IdentityType::Username,
            value: identity.to_owned(),
        };
        identity.validate().map_err(|_| Error::Denied)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // Serialize identity creation and key rotation with the same agent lock.
        crate::deployment::db::lock_opt(&tx, agent)
            .await?
            .ok_or(Error::Denied)?;
        let connection = crate::connections::db::tilde_connection_opt(&tx, agent)
            .await?
            .ok_or(Error::Denied)?
            .id;
        let expected = self
            .connections
            .resolve(connection)
            .await
            .map_err(|_| Error::Denied)?
            .remove(catalog::KEY_FIELD)
            .ok_or(Error::Denied)?;
        if !bool::from(
            expected
                .expose_secret()
                .as_bytes()
                .ct_eq(api_key.as_bytes()),
        ) {
            return Err(Error::Denied);
        }
        drop(expected);
        // The application vouches for the identity, so it counts as attested. Unknown identities
        // are recorded even when refused so an editor can allow them; the shared policy then
        // decides: disabled admits nobody, private only allowed identities, public everyone.
        let kind = crate::chat::access::identity::kind_name(identity.identity_type);
        let user = super::super::ingress::stable(
            connection,
            "user",
            &format!("{kind}:{}", identity.value),
        );
        crate::chat::db::channel_user_execute(&tx, user, &identity.value).await?;
        crate::chat::access::db::identity_upsert_one(&tx, user, connection, kind, &identity.value)
            .await?;
        db::attest_identity(&tx, user).await?;
        let allowed = crate::chat::access::db::allowed_one(&tx, agent, user)
            .await?
            .allowed;
        tx.commit().await?;
        drop(client);
        if !allowed {
            return Err(Error::Denied);
        }
        let token = self.issue_identity_token(agent, user).await?;
        let claims = crate::deployment::tokens::verify_ingress(
            token.expose_secret(),
            agent,
            &self.signing_key(agent).await?,
        )?;
        Ok((token, claims))
    }
}
use super::db;
pub(crate) fn scope(
    ctx: &connectrpc::RequestContext,
) -> Result<IngressClaims, connectrpc::ConnectError> {
    ctx.extensions()
        .get::<IngressClaims>()
        .cloned()
        .ok_or_else(|| connectrpc::ConnectError::unauthenticated("Provider scope required"))
}
pub(crate) async fn member(
    chat: &Chat,
    claims: &IngressClaims,
    thread: Uuid,
) -> Result<(), ChatError> {
    if let Some(user) = claims.user_id {
        let roster = chat.thread(thread).await?;
        if roster.channel.is_set()
            || !roster
                .participants
                .iter()
                .any(|p| p.active && p.user_id.as_deref() == Some(&user.to_string()))
        {
            return Err(ChatError::Forbidden);
        }
    }
    Ok(())
}
