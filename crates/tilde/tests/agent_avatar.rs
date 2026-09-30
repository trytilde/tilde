mod common;
use base64::Engine;
use tilde::{
    agent::{Agents, CreateAgent, avatar::AvatarStore},
    error::Error,
};
use uuid::Uuid;

/// Exercises real private-object PUT, signed GET, replacement and database association.
#[tokio::test]
async fn avatars_round_trip_through_private_s3_and_keep_the_generated_identity() {
    let db = common::Database::new().await;
    let store: AvatarStore = common::bucket(&common::Storage::load());
    let encryption = tilde::encryption::Encryption::initialize(&db.pool, common::seed(9))
        .await
        .unwrap();
    let agents =
        Agents::new(db.pool.clone(), std::sync::Arc::new(encryption)).with_avatar_store(store);
    let id = Uuid::new_v4();
    let created = agents
        .create(CreateAgent {
            description: String::new(),
            concurrency_policy: Default::default(),
            id,
            name: "Avatar fixture".into(),
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
    db.close().await;
}
