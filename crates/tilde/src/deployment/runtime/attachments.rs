//! Attachment bytes stay in bounded replica memory until the gateway stores them.
use super::*;
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
const PENDING_LIMIT: usize = 256 * 1024 * 1024;
impl Runtime {
    pub async fn upload_attachment(&self, r: chat::UploadAttachment) -> Result<types::Attachment> {
        let key = id(&r.id)?;
        let thread = id(&r.thread_id)?;
        if r.content.len() > 128 * 1024 * 1024
            || r.filename.trim().is_empty()
            || r.filename.len() > 1024
            || r.media_type.len() > 255
        {
            return Err(ChatError::Invalid(
                "Invalid attachment or exceeds 128 MiB".into(),
            ));
        }
        let shared = self.load(thread).await?;
        let metadata = types::Attachment {
            id: r.id,
            thread_id: r.thread_id,
            filename: r.filename,
            media_type: r.media_type,
            size_bytes: r.content.len() as i64,
            sha256: hex::encode(Sha256::digest(&r.content)),
            ..Default::default()
        };
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        if let Some(old) = t.attachments.get(&key) {
            if old.metadata.sha256 != metadata.sha256
                || old.metadata.filename != metadata.filename
                || old.metadata.media_type != metadata.media_type
            {
                return Err(ChatError::Conflict);
            }
            return Ok(old.metadata.clone());
        }
        let mut bytes = self.state.attachments.lock().await;
        if bytes.values().map(|v| v.len()).sum::<usize>() + r.content.len() > PENDING_LIMIT {
            return Err(ChatError::Invalid(
                "Pending attachment storage is full".into(),
            ));
        }
        bytes.insert(key, Zeroizing::new(r.content));
        drop(bytes);
        t.attachments.insert(
            key,
            store::AttachmentEntry {
                metadata: metadata.clone(),
                source: None,
            },
        );
        self.emit(&mut t, "attachment.created", metadata.clone().into(), None)?;
        drop(t);
        self.state.outbox_notify.notify_one();
        Ok(metadata)
    }
    pub async fn download_attachment(
        &self,
        thread: Uuid,
        key: Uuid,
    ) -> Result<chat::AttachmentContent> {
        let shared = self.load(thread).await?;
        let (attachment, source) = {
            let t = shared.lock().await;
            let entry = t.attachments.get(&key).ok_or(ChatError::NotFound)?;
            (entry.metadata.clone(), entry.source.clone())
        };
        if let Some(content) = self
            .state
            .attachments
            .lock()
            .await
            .get(&key)
            .map(|v| v.to_vec())
        {
            return Ok(chat::AttachmentContent {
                attachment,
                content,
            });
        }
        if attachment.persisted {
            let content = self.gateway.download_attachment(thread, key).await?;
            return Ok(chat::AttachmentContent {
                attachment,
                content,
            });
        }
        let mut source = source.ok_or(ChatError::Transport)?;
        let (provider, _typ, access) = self
            .connection_access(id(&source.connection_id)?)
            .await
            .map_err(|_| ChatError::Transport)?;
        let url = source.url.take().map(SecretString::from);
        let content = crate::chat::providers::files::download(
            &provider,
            &access,
            source.provider_reference.as_deref(),
            url.as_ref().map(|s| s.expose_secret()),
        )
        .await
        .map_err(|_| ChatError::Transport)?;
        let mut store = self.state.attachments.lock().await;
        if store.values().map(|v| v.len()).sum::<usize>() + content.len() > PENDING_LIMIT {
            return Err(ChatError::Transport);
        }
        store.insert(key, Zeroizing::new(content.clone()));
        drop(store);
        self.state.outbox_notify.notify_one();
        Ok(chat::AttachmentContent {
            attachment,
            content,
        })
    }
    pub(crate) fn record_attachment_source(
        &self,
        t: &mut ThreadState,
        source: types::AttachmentSource,
    ) -> Result<()> {
        let key = id(&source.attachment.id)?;
        let metadata = source
            .attachment
            .clone()
            .into_option()
            .ok_or(ChatError::NotFound)?;
        t.attachments.entry(key).or_insert(store::AttachmentEntry {
            metadata,
            source: Some(source.clone()),
        });
        self.emit(t, "attachment.source", source.into(), None)
    }
    /// Move cached bytes to gateway storage, then release local memory.
    pub async fn transfer_attachments(&self) -> Result<()> {
        let pending: Vec<Uuid> = self
            .state
            .attachments
            .lock()
            .await
            .keys()
            .copied()
            .collect();
        for key in pending {
            let Some(thread) = self.attachment_thread(key).await else {
                continue;
            };
            let shared = self.load(thread).await?;
            let metadata = {
                let t = shared.lock().await;
                match t.attachments.get(&key) {
                    Some(entry) if !entry.metadata.persisted => entry.metadata.clone(),
                    Some(_) => {
                        drop(t);
                        self.state.attachments.lock().await.remove(&key);
                        continue;
                    }
                    None => continue,
                }
            };
            let Some(content) = self
                .state
                .attachments
                .lock()
                .await
                .get(&key)
                .map(|v| v.to_vec())
            else {
                continue;
            };
            let stored = self.gateway.upload_attachment(metadata, content).await?;
            let mut t = shared.lock().await;
            if let Some(entry) = t.attachments.get_mut(&key) {
                entry.metadata = stored;
                entry.metadata.persisted = true;
            }
            drop(t);
            self.state.attachments.lock().await.remove(&key);
        }
        Ok(())
    }
    async fn attachment_thread(&self, key: Uuid) -> Option<Uuid> {
        let threads: Vec<Shared> = self
            .state
            .threads
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        for shared in threads {
            let t = shared.lock().await;
            if t.attachments.contains_key(&key) {
                return id(&t.thread.id).ok();
            }
        }
        None
    }
}
