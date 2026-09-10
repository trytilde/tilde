//! Focused AWS configuration and credential resolution.

use client_aws_sigv4::Credentials;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Clone, Debug)]
pub struct Config {
    pub region: String,
    pub credentials: Credentials,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "AWS credentials are unavailable; configure environment, ECS task-role, or a shared credentials profile"
    )]
    CredentialsUnavailable,
    #[error("failed to fetch ECS credentials: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid ECS credentials response: {0}")]
    InvalidEcsResponse(String),
}

/// Resolve environment, ECS role, or shared-file credentials. No placeholder credentials.
pub async fn load(region: impl Into<String>) -> Result<Config, Error> {
    Ok(Config {
        region: region.into(),
        credentials: resolve_credentials().await?,
    })
}

async fn resolve_credentials() -> Result<Credentials, Error> {
    if let (Ok(access_key_id), Ok(mut secret_access_key)) = (
        std::env::var("AWS_ACCESS_KEY_ID"),
        std::env::var("AWS_SECRET_ACCESS_KEY").map(Zeroizing::new),
    ) {
        if access_key_id.is_empty() || secret_access_key.is_empty() {
            return Err(Error::CredentialsUnavailable);
        }
        return Ok(Credentials {
            access_key_id,
            secret_access_key: std::mem::take(&mut *secret_access_key),
            session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
        });
    }
    if let Some(uri) = ecs_uri() {
        return ecs_credentials(&uri).await;
    }
    shared_credentials().ok_or(Error::CredentialsUnavailable)
}

fn ecs_uri() -> Option<String> {
    std::env::var("AWS_CONTAINER_CREDENTIALS_FULL_URI")
        .ok()
        .or_else(|| {
            std::env::var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI")
                .ok()
                .map(|path| format!("http://169.254.170.2{path}"))
        })
}

#[derive(serde::Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "PascalCase")]
struct EcsCredentials {
    access_key_id: String,
    secret_access_key: String,
    token: Option<String>,
}

async fn ecs_credentials(uri: &str) -> Result<Credentials, Error> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut request = client.get(uri);
    if let Ok(token) = std::env::var("AWS_CONTAINER_AUTHORIZATION_TOKEN") {
        let token = Zeroizing::new(token);
        request = request.header("authorization", token.as_str());
    }
    let response = request.send().await?.error_for_status()?;
    let text = Zeroizing::new(response.text().await?);
    let mut value: EcsCredentials = serde_json::from_str(&text)
        .map_err(|_| Error::InvalidEcsResponse("invalid JSON".into()))?;
    if value.access_key_id.is_empty() || value.secret_access_key.is_empty() {
        return Err(Error::InvalidEcsResponse(
            "empty access key or secret".into(),
        ));
    }
    Ok(Credentials {
        access_key_id: std::mem::take(&mut value.access_key_id),
        secret_access_key: std::mem::take(&mut value.secret_access_key),
        session_token: value.token.take(),
    })
}

fn shared_credentials() -> Option<Credentials> {
    let path = std::env::var("AWS_SHARED_CREDENTIALS_FILE")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| std::path::PathBuf::from(home).join(".aws/credentials"))
        })?;
    let profile = std::env::var("AWS_PROFILE").unwrap_or_else(|_| "default".into());
    let text = Zeroizing::new(std::fs::read_to_string(path).ok()?);
    let mut current = "";
    let mut access = None;
    let mut secret = None;
    let mut token = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            current = &line[1..line.len() - 1];
            continue;
        }
        if current != profile {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "aws_access_key_id" => access = Some(value.trim().to_string()),
            "aws_secret_access_key" => secret = Some(Zeroizing::new(value.trim().to_string())),
            "aws_session_token" => token = Some(Zeroizing::new(value.trim().to_string())),
            _ => {}
        }
    }
    let access_key_id: String = access?;
    let mut secret = secret?;
    if access_key_id.is_empty() || secret.is_empty() {
        return None;
    }
    Some(Credentials {
        access_key_id,
        secret_access_key: std::mem::take(&mut *secret),
        session_token: token.map(|mut token| std::mem::take(&mut *token)),
    })
}
