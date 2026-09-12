use super::*;
use sha2::{Digest, Sha256};
impl Runtime {
    pub async fn upload_attachment(&self, r: chat::UploadAttachment) -> Result<types::Attachment> {
        let key = id(&r.id)?;
        let thread = id(&r.thread_id)?;
        self.thread(thread).await?;
        if r.content.len() > 128 * 1024 * 1024
            || r.filename.trim().is_empty()
            || r.filename.len() > 1024
            || r.media_type.len() > 255
        {
            return Err(ChatError::Invalid(
                "Invalid attachment or exceeds 128 MiB".into(),
            ));
        }
        let metadata = types::Attachment {
            id: r.id,
            thread_id: r.thread_id,
            filename: r.filename,
            media_type: r.media_type,
            size_bytes: r.content.len() as i64,
            sha256: hex::encode(Sha256::digest(&r.content)),
            ..Default::default()
        };
        let _guard = self.mutation.lock().await;
        let existing = match self.attachment_metadata(thread, key).await {
            Ok(old) => {
                if old.sha256 != metadata.sha256
                    || old.filename != metadata.filename
                    || old.media_type != metadata.media_type
                {
                    return Err(ChatError::Conflict);
                }
                if old.persisted {
                    return Ok(old);
                }
                true
            }
            Err(ChatError::NotFound) => false,
            Err(e) => return Err(e),
        };
        let mut bytes = self.attachments.lock().await;
        if bytes.values().map(|v| v.len()).sum::<usize>() + r.content.len() > 256 * 1024 * 1024 {
            return Err(ChatError::Invalid(
                "Pending attachment storage is full".into(),
            ));
        }
        if let Some(old) = bytes.get(&key) {
            if old.as_slice() != r.content.as_slice() {
                return Err(ChatError::Conflict);
            }
            return self.attachment_metadata(thread, key).await;
        }
        bytes.insert(key, Zeroizing::new(r.content));
        drop(bytes);
        if existing {
            self.client
                .transaction(vec![statement(
                    "UPDATE attachments SET holder_instance_id=? WHERE id=?",
                    vec![json!(self.instance_id), json!(key)],
                )])
                .await?;
            return Ok(metadata);
        }
        if let Err(e)=self.commit(thread,vec![statement("INSERT INTO attachments(id,thread_id,holder_instance_id,payload) VALUES(?,?,?,?)",vec![json!(key),json!(thread),json!(self.instance_id),json!(self.seal(key,"attachment",&metadata)?)]),self.event(thread,"attachment.created",metadata.clone().into())?]).await{self.attachments.lock().await.remove(&key);return Err(e);}
        Ok(metadata)
    }
    pub async fn attachment_metadata(&self, thread: Uuid, key: Uuid) -> Result<types::Attachment> {
        #[derive(Deserialize)]
        struct Metadata {
            payload: String,
            uploaded: i64,
        }
        let row = self
            .client
            .query::<Metadata>(
                "SELECT payload,uploaded FROM attachments WHERE id=? AND thread_id=?",
                vec![json!(key), json!(thread)],
            )
            .await?
            .pop()
            .ok_or(ChatError::NotFound)?;
        let mut value: types::Attachment = self.open(key, "attachment", &row.payload)?;
        value.persisted = row.uploaded != 0;
        Ok(value)
    }
    pub async fn download_attachment(
        &self,
        thread: Uuid,
        key: Uuid,
    ) -> Result<chat::AttachmentContent> {
        let attachment = self.attachment_metadata(thread, key).await?;
        if let Some(content) = self.attachments.lock().await.get(&key).map(|v| v.to_vec()) {
            return Ok(chat::AttachmentContent {
                attachment,
                content,
            });
        }
        if attachment.persisted {
            let archive = self.archive.as_ref().ok_or(ChatError::Transport)?;
            let content = archive.attachment(self.agent_id, thread, key).await?;
            return Ok(chat::AttachmentContent {
                attachment,
                content,
            });
        }
        #[derive(Deserialize)]
        struct Source {
            source: String,
        }
        let source = self
            .client
            .query::<Source>(
                "SELECT source FROM attachments WHERE id=? AND thread_id=?",
                vec![json!(key), json!(thread)],
            )
            .await?
            .pop()
            .ok_or(ChatError::NotFound)?;
        if source.source.is_empty() {
            return Err(ChatError::Transport);
        }
        let mut source: types::AttachmentSource =
            self.open(key, "attachment_source", &source.source)?;
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
        let mut store = self.attachments.lock().await;
        if store.values().map(|v| v.len()).sum::<usize>() + content.len() > 256 * 1024 * 1024 {
            return Err(ChatError::Transport);
        }
        store.insert(key, Zeroizing::new(content.clone()));
        Ok(chat::AttachmentContent {
            attachment,
            content,
        })
    }
}

impl Runtime {
    pub async fn record_attachment_source(&self, source: types::AttachmentSource) -> Result<()> {
        let key = id(&source.attachment.id)?;
        let thread = id(&source.attachment.thread_id)?;
        self.commit(thread,vec![statement("INSERT INTO attachments(id,thread_id,holder_instance_id,payload,source) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(key),json!(thread),json!(self.instance_id),json!(self.seal(key,"attachment",&*source.attachment)?),json!(self.seal(key,"attachment_source",&source)?)]),self.event(thread,"attachment.source",source.into())?]).await
    }
    pub async fn release_attachment(&self, key: Uuid) {
        self.attachments.lock().await.remove(&key);
    }
}

impl Runtime {
    pub async fn attachment_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        use futures::StreamExt;
        loop {
            if let Ok(mut changes) = self
                .client
                .subscribe::<IdRow>(
                    "SELECT id,uploaded FROM attachments WHERE uploaded=1",
                    vec![],
                )
                .await
            {
                loop {
                    tokio::select! { _=shutdown.changed()=>return, row=changes.next()=>match row {
                        Some(Ok(row)) if !row.deleted=>{ if let Ok(key)=id(&row.value.id) {self.release_attachment(key).await;} },Some(Ok(_))=>{},_=>break,
                    } }
                }
            }
            tokio::select! {_=shutdown.changed()=>return,_=tokio::time::sleep(std::time::Duration::from_secs(1))=>{}}
        }
    }
}
