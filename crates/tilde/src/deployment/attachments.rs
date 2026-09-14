//! Replicas hold attachment bytes in memory; the gateway encrypts them into S3.
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
    pub async fn store_attachment(
        &self,
        agent: Uuid,
        attachment: types::Attachment,
        content: Vec<u8>,
    ) -> Result<types::Attachment, Error> {
        let key = super::id(&attachment.id)?;
        let thread = super::id(&attachment.thread_id)?;
        if !sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
            .fetch_one(&self.pool)
            .await?
            .allowed
        {
            return Err(Error::Denied);
        }
        let store = self
            .agents
            .object_store()
            .ok_or(Error::AvatarStorageDisabled)?;
        let content = Zeroizing::new(content);
        if content.len() > 128 * 1024 * 1024 {
            return Err(Error::Invalid("Attachment exceeds 128 MiB".into()));
        }
        let size = content.len() as i64;
        let hash = hex::encode(Sha256::digest(content.as_slice()));
        if !attachment.sha256.is_empty() && attachment.sha256 != hash {
            return Err(Error::Invalid("Attachment checksum mismatch".into()));
        }
        sqlx::query_file!(
            "../../queries/deployment/project/attachment.sql",
            key,
            thread,
            attachment.filename,
            attachment.media_type,
            size,
            hash
        )
        .execute(&self.pool)
        .await?;
        let existing = sqlx::query_file!("../../queries/chat/attachment_get.sql", key, thread)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::NotFound)?;
        if existing.object_key.is_none() {
            let encoded = SecretString::from(STANDARD.encode(content.as_slice()));
            drop(content);
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
        }
        Ok(types::Attachment {
            id: key.to_string(),
            thread_id: thread.to_string(),
            filename: existing.filename,
            media_type: existing.media_type,
            size_bytes: size,
            sha256: hash,
            persisted: true,
            ..Default::default()
        })
    }
    pub async fn fetch_attachment(
        &self,
        agent: Uuid,
        thread: Uuid,
        key: Uuid,
    ) -> Result<Vec<u8>, Error> {
        if !sqlx::query_file!("../../queries/deployment/thread_agent.sql", thread, agent)
            .fetch_one(&self.pool)
            .await?
            .allowed
        {
            return Err(Error::Denied);
        }
        Ok(self.chat().download_attachment(thread, key).await?.content)
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
