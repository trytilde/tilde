//! Private S3 avatar objects, using Tilde's existing AWS credential and SigV4 primitives.
//! Browser reads use expiring signed URLs; no bucket public-read policy is required.
use super::{Agent, Agents};
use crate::{error::Error, iam::capabilities::Capabilities};
use client_aws_sigv4::{Credentials, Signer, aws_encode};
use reqwest::Method;
use sqlx::types::Json;
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

pub const MAX_AVATAR_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone)]
pub struct AvatarStore {
    bucket: String,
    region: String,
    endpoint: url::Url,
    public_endpoint: url::Url,
    credentials: Option<Credentials>,
    http: reqwest::Client,
}
impl AvatarStore {
    /// Explicit credentials support MinIO; otherwise refresh the normal AWS chain per operation.
    pub fn new(
        bucket: String,
        region: String,
        endpoint: String,
        public_endpoint: Option<String>,
        credentials: Option<Credentials>,
    ) -> Result<Self, Error> {
        let parse = |value: &str| -> Result<url::Url, Error> {
            let url =
                url::Url::parse(value).map_err(|_| Error::Invalid("Invalid S3 endpoint".into()))?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(Error::Invalid("Invalid S3 endpoint".into()));
            }
            Ok(url)
        };
        if bucket.is_empty() || region.is_empty() {
            return Err(Error::Invalid("S3 bucket and region are required".into()));
        }
        Ok(Self {
            public_endpoint: parse(public_endpoint.as_deref().unwrap_or(&endpoint))?,
            endpoint: parse(&endpoint)?,
            bucket,
            region,
            credentials,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| Error::AvatarStorage)?,
        })
    }
    async fn signer(&self) -> Result<Signer, Error> {
        let credentials = match &self.credentials {
            Some(credentials) => credentials.clone(),
            None => {
                client_aws_config::load(&self.region)
                    .await
                    .map_err(|_| Error::AvatarStorage)?
                    .credentials
            }
        };
        Ok(Signer::new(credentials, &self.region, "s3"))
    }
    fn object_url(&self, endpoint: &url::Url, key: &str) -> url::Url {
        let mut url = endpoint.clone();
        url.set_path(&format!(
            "{}/{}/{}",
            endpoint.path().trim_end_matches('/'),
            aws_encode(self.bucket.as_bytes()),
            key.split('/')
                .map(|s| aws_encode(s.as_bytes()))
                .collect::<Vec<_>>()
                .join("/")
        ));
        url
    }
    async fn write(
        &self,
        method: Method,
        key: &str,
        content_type: &str,
        content: Vec<u8>,
    ) -> Result<(), Error> {
        let mut request = self
            .http
            .request(method, self.object_url(&self.endpoint, key))
            .header("content-type", content_type)
            .body(content.clone())
            .build()
            .map_err(|_| Error::AvatarStorage)?;
        self.signer()
            .await?
            .sign_request_at(&mut request, &content, SystemTime::now())
            .map_err(|_| Error::AvatarStorage)?;
        let response = self
            .http
            .execute(request)
            .await
            .map_err(|_| Error::AvatarStorage)?;
        if !response.status().is_success() {
            return Err(Error::AvatarStorage);
        }
        Ok(())
    }
    async fn url(&self, key: &str) -> Result<String, Error> {
        Ok(self
            .signer()
            .await?
            .presign_at(
                Method::GET,
                self.object_url(&self.public_endpoint, key),
                BTreeMap::new(),
                3600,
                SystemTime::now(),
            )
            .map_err(|_| Error::AvatarStorage)?
            .url
            .to_string())
    }
}
impl Agents {
    pub fn with_avatar_store(mut self, store: AvatarStore) -> Self {
        self.avatars = Some(store);
        self
    }

    pub async fn avatar_url(&self, agent: &Agent) -> Result<Option<String>, Error> {
        match (&self.avatars, &agent.avatar_key) {
            (Some(store), Some(key)) => store.url(key).await.map(Some),
            _ => Ok(None),
        }
    }

    /// The management listener authenticates upload requests. Runtime has no upload method.
    pub async fn upload_avatar(
        &self,
        id: Uuid,
        media_type: &str,
        content: Vec<u8>,
    ) -> Result<Agent, Error> {
        let extension = validate_image(media_type, &content)?;
        let store = self.avatars.as_ref().ok_or(Error::AvatarStorageDisabled)?;
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_file_as!(Agent, "../../queries/iam/agent_lock.sql", id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound)?;
        let key = format!("agents/{id}/avatars/{}.{}", Uuid::new_v4(), extension);
        store.write(Method::PUT, &key, media_type, content).await?;
        let saved = sqlx::query_file_as!(Agent, "../../queries/agent/avatar.sql", id, key)
            .fetch_one(&mut *tx)
            .await;
        let agent = match saved {
            Ok(agent) => agent,
            Err(error) => {
                let _ = store.write(Method::DELETE, &key, media_type, vec![]).await;
                return Err(error.into());
            }
        };
        tx.commit().await?;
        if let Some(old) = current.avatar_key {
            if store
                .write(Method::DELETE, &old, media_type, vec![])
                .await
                .is_err()
            {
                tracing::warn!(agent_id = %id, "Unable to remove previous avatar object");
            }
        }
        Ok(agent)
    }
}

/// Raster signatures prevent HTML/SVG uploads being served from the object-storage origin.
fn validate_image(media_type: &str, bytes: &[u8]) -> Result<&'static str, Error> {
    if bytes.is_empty() || bytes.len() > MAX_AVATAR_BYTES {
        return Err(Error::Invalid(
            "Avatar must be an image of at most 5 MiB".into(),
        ));
    }
    match media_type {
        "image/png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Ok("png"),
        "image/jpeg" if bytes.starts_with(b"\xff\xd8\xff") => Ok("jpg"),
        "image/gif" if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") => Ok("gif"),
        "image/webp" if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") => {
            Ok("webp")
        }
        _ => Err(Error::Invalid(
            "Upload a PNG, JPEG, GIF or WebP image".into(),
        )),
    }
}
