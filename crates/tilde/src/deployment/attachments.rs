//! Sidecars retain temporary bytes; the gateway encrypts and durably stores them
//! in S3. Retention cannot discard an attachment until this transfer completes.
use super::Deployments;
use crate::{
    encryption::{SealedSecret, SecretBinding},
    error::Error,
    proto::tilde::types::v1 as types,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;
impl Deployments {
    async fn attachment_receipt(&self, agent: Uuid, thread: Uuid, key: Uuid) -> Result<(), Error> {
        let peer = self.peer(agent).await?.ok_or_else(|| {
            Error::Invalid("Attachment receipt awaits a reachable sidecar".into())
        })?;
        let row = sqlx::query_file!("../../queries/chat/attachment_get.sql", key, thread)
            .fetch_one(&self.pool)
            .await?;
        if row.object_key.is_none() {
            return Err(Error::Conflict);
        }
        let metadata = types::Attachment {
            id: key.to_string(),
            thread_id: thread.to_string(),
            filename: row.filename,
            media_type: row.media_type,
            size_bytes: row.size_bytes,
            sha256: row.sha256,
            persisted: true,
            ..Default::default()
        };
        let affected = peer
            .client
            .transaction(vec![super::corrosion::client::statement(
                "UPDATE attachments SET uploaded=1,payload=? WHERE id=? AND thread_id=?",
                vec![
                    serde_json::json!(peer.seal(key, "attachment", &metadata)?),
                    serde_json::json!(key),
                    serde_json::json!(thread),
                ],
            )])
            .await?;
        if affected.first().copied().unwrap_or(0) == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }
    pub async fn transfer_attachment(
        &self,
        agent: Uuid,
        transfer: &types::AttachmentTransfer,
    ) -> Result<(), Error> {
        let key = super::id(&transfer.attachment_id)?;
        let thread = super::id(&transfer.thread_id)?;
        let store = self
            .agents
            .object_store()
            .ok_or(Error::AvatarStorageDisabled)?;
        let row = sqlx::query_file!("../../queries/chat/attachment_get.sql", key, thread)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::NotFound)?;
        let settings = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_one(&self.pool)
            .await?;
        let secrets = self.open_secrets(
            agent,
            settings.encrypted_secrets.as_deref().ok_or(Error::Denied)?,
        )?;
        let nodes = sqlx::query_file!("../../queries/deployment/project/node_available.sql", agent)
            .fetch_all(&self.pool)
            .await?;
        if row.object_key.is_some() {
            return self.attachment_receipt(agent, thread, key).await;
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::AvatarStorage)?;
        for node in nodes {
            let url = format!(
                "{}/attachments/{key}?thread_id={thread}",
                node.agent_ingress_url.trim_end_matches('/')
            );
            let Ok(mut response) = client
                .get(&url)
                .bearer_auth(secrets.api_token.expose_secret())
                .send()
                .await
            else {
                continue;
            };
            if !response.status().is_success() {
                continue;
            }
            let mut bytes = Zeroizing::new(Vec::new());
            while let Some(chunk) = response.chunk().await.map_err(|_| Error::AvatarStorage)? {
                if bytes.len() + chunk.len() > 128 * 1024 * 1024 {
                    return Err(Error::AvatarStorage);
                }
                bytes.extend_from_slice(&chunk);
            }
            let size = bytes.len() as i64;
            let hash = hex::encode(Sha256::digest(bytes.as_slice()));
            if !row.sha256.is_empty() && row.sha256 != hash {
                return Err(Error::Invalid("Attachment checksum mismatch".into()));
            }
            let encoded = SecretString::from(STANDARD.encode(bytes.as_slice()));
            drop(bytes);
            let sealed = self.encryption.seal(binding(key), &encoded)?.into_bytes();
            drop(encoded);
            let object = format!("attachments/{agent}/{key}");
            store
                .write(
                    reqwest::Method::PUT,
                    &object,
                    "application/octet-stream",
                    sealed,
                )
                .await?;
            sqlx::query_file!(
                "../../queries/deployment/attachment_object.sql",
                key,
                object,
                size,
                hash
            )
            .execute(&self.pool)
            .await?;
            self.attachment_receipt(agent, thread, key).await?;
            let _ = client
                .delete(url)
                .bearer_auth(secrets.api_token.expose_secret())
                .send()
                .await;
            return Ok(());
        }
        Err(Error::Invalid(
            "Attachment bytes are not available from a sidecar yet".into(),
        ))
    }
}
pub(crate) fn binding(key: Uuid) -> SecretBinding<'static> {
    SecretBinding {
        resource_kind: "chat_attachment",
        resource_id: key,
        name: "content",
    }
}
pub(crate) async fn download(
    store: &crate::agent::avatar::AvatarStore,
    encryption: &crate::encryption::Encryption,
    key: Uuid,
    object: &str,
) -> Result<Vec<u8>, Error> {
    let sealed = store.read(object).await?;
    let secret = encryption.open(binding(key), SealedSecret::from_bytes(&sealed)?)?;
    STANDARD
        .decode(secret.expose_secret())
        .map_err(|_| Error::Encryption)
}

pub fn router(service: Deployments) -> axum::Router {
    axum::Router::new()
        .route(
            "/agents/{agent}/attachments/{thread}/{key}",
            axum::routing::get(receive),
        )
        .with_state(service)
}
async fn receive(
    axum::extract::State(service): axum::extract::State<Deployments>,
    axum::extract::Path((agent, thread, key)): axum::extract::Path<(Uuid, Uuid, Uuid)>,
    headers: http::HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let result = async {
        let token = headers
            .get(http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or(Error::Denied)?;
        if service.authenticate(token).await? != agent
            || !sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
                .fetch_one(&service.pool)
                .await?
                .allowed
        {
            return Err(Error::Denied);
        }
        let content = service.chat().download_attachment(thread, key).await?;
        Ok::<_, Error>(content.content)
    }
    .await;
    match result {
        Ok(bytes) => bytes.into_response(),
        Err(e) => connectrpc::ConnectError::from(e).into_response(),
    }
}
