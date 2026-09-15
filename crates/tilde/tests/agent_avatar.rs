#[allow(dead_code)]
mod common;
use base64::Engine;
use client_aws_sigv4::Credentials;
use secrecy::SecretString;
use std::sync::Arc;
use tilde::{
    agent::{Agents, CreateAgent, avatar::AvatarStore},
    encryption::Encryption,
    error::Error,
};
use uuid::Uuid;

/// Exercises real private-object PUT, signed GET, replacement cleanup and database association.
/// Run task test:avatars to start MinIO with the .env configuration.
#[tokio::test]
async fn avatars_round_trip_through_private_s3_and_keep_the_generated_identity() {
    let db = common::Database::new().await;
    let env = |name| {
        std::env::var(name).unwrap_or_else(|_| panic!("{name} required; run task test:avatars"))
    };
    let store = AvatarStore::new(
        env("ENGINE_S3_BUCKET"),
        env("ENGINE_S3_REGION"),
        env("ENGINE_S3_ENDPOINT"),
        None,
        Some(Credentials {
            access_key_id: env("ENGINE_S3_ACCESS_KEY_ID"),
            secret_access_key: env("ENGINE_S3_SECRET_ACCESS_KEY"),
            session_token: None,
        }),
    )
    .unwrap();
    let agents = Agents::new(
        db.pool.clone(),
        Arc::new(
            Encryption::initialize(&db.pool, common::seed(62))
                .await
                .unwrap(),
        ),
    )
    .with_avatar_store(store);
    let id = Uuid::new_v4();
    let created = agents
        .create(CreateAgent {
            concurrency_policy: Default::default(),
            id,
            name: "Avatar fixture".into(),
            endpoint_url: "https://avatar.example.com".into(),
            webhook_signing_key: SecretString::from("avatar-fixture-signing-key-32-bytes-long"),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    assert!(agents.avatar_url(&created).await.unwrap().is_none());
    assert!(!created.avatar_seed.is_nil());
    let png = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1cAAAAASUVORK5CYII=").unwrap();
    let first = agents
        .upload_avatar(id, "image/png", png.clone())
        .await
        .unwrap();
    let first_url = agents.avatar_url(&first).await.unwrap().unwrap();
    let http = reqwest::Client::new();
    let response = http
        .get(&first_url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.bytes().await.unwrap().as_ref(), png);
    let mut unsigned = url::Url::parse(&first_url).unwrap();
    unsigned.set_query(None);
    assert_eq!(http.get(unsigned).send().await.unwrap().status(), 403);
    assert!(matches!(
        agents
            .upload_avatar(id, "image/png", b"<html>not an image</html>".to_vec())
            .await,
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        agents
            .upload_avatar(id, "image/svg+xml", b"<svg/>".to_vec())
            .await,
        Err(Error::Invalid(_))
    ));
    let second = agents.upload_avatar(id, "image/png", png).await.unwrap();
    assert_ne!(first.avatar_key, second.avatar_key);
    assert_eq!(second.avatar_seed, created.avatar_seed);
    assert_eq!(agents.get(id).await.unwrap().avatar_key, second.avatar_key);
    assert_eq!(http.get(first_url).send().await.unwrap().status(), 404);
    // Clean only this test's randomly named object, using a signed delete.
    let mut url = url::Url::parse(&agents.avatar_url(&second).await.unwrap().unwrap()).unwrap();
    url.set_query(None);
    let mut request = http.delete(url).build().unwrap();
    client_aws_sigv4::Signer::new(
        Credentials {
            access_key_id: env("ENGINE_S3_ACCESS_KEY_ID"),
            secret_access_key: env("ENGINE_S3_SECRET_ACCESS_KEY"),
            session_token: None,
        },
        env("ENGINE_S3_REGION"),
        "s3",
    )
    .sign_request_at(&mut request, &[], std::time::SystemTime::now())
    .unwrap();
    http.execute(request)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    db.close().await;
}
