mod common;
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tilde::{
    encryption::{Encryption, KeyProtection},
    error::Error,
};
use uuid::Uuid;

const KEY_ARN: &str = "arn:aws:kms:us-east-1:111111111111:key/11111111-1111-1111-1111-111111111111";
#[derive(Clone, Default)]
struct FakeKms {
    calls: Arc<Mutex<Vec<Value>>>,
    response_override: Arc<Mutex<Option<Value>>>,
}
async fn kms(
    State(state): State<FakeKms>,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        headers.get("content-type").unwrap(),
        "application/x-amz-json-1.1"
    );
    assert!(
        headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("AWS4-HMAC-SHA256")
    );
    assert_eq!(headers.get("x-amz-security-token").unwrap(), "test-session");
    let target = headers.get("x-amz-target").unwrap().to_str().unwrap();
    let mut calls = state.calls.lock().unwrap();
    let valid =
        body["KeyId"] == "alias/test" && body["EncryptionContext"]["application"] == "agent-engine";
    if !valid {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"__type":"InvalidCiphertextException","message":"denied"})),
        ));
    }
    if let Some(response) = state.response_override.lock().unwrap().clone() {
        return Ok(Json(response));
    }
    if target.ends_with("GenerateDataKey") {
        assert_eq!(body["KeySpec"], "AES_256");
        assert!(
            Uuid::parse_str(body["EncryptionContext"]["data_key_id"].as_str().unwrap()).is_ok()
        );
        calls.push(body);
        Ok(Json(
            json!({"KeyId":KEY_ARN,"Plaintext":STANDARD.encode([9u8;32]),"CiphertextBlob":STANDARD.encode(b"opaque-kms-wrapped-key")}),
        ))
    } else {
        assert!(target.ends_with("Decrypt"));
        assert_eq!(
            body["CiphertextBlob"],
            STANDARD.encode(b"opaque-kms-wrapped-key")
        );
        assert_eq!(body["EncryptionContext"], calls[0]["EncryptionContext"]);
        calls.push(body);
        Ok(Json(
            json!({"KeyId":KEY_ARN,"Plaintext":STANDARD.encode([9u8;32])}),
        ))
    }
}

#[tokio::test]
async fn kms_backend_uses_signed_http_calls_and_reopens_the_persisted_wrapped_key() {
    let db = common::Database::new().await;
    let state = FakeKms::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new()
        .route("/", post(kms))
        .with_state(state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = client_aws_kms::Client::with_config(
        client_aws_config::Config {
            region: "us-east-1".into(),
            credentials: client_aws_sigv4::Credentials {
                access_key_id: "test-access".into(),
                secret_access_key: "test-secret".into(),
                session_token: Some("test-session".into()),
            },
        },
        &format!("http://{address}"),
    )
    .unwrap();
    let first = Encryption::initialize(
        &db.pool,
        KeyProtection::kms(client.clone(), "alias/test".into()).unwrap(),
    )
    .await
    .unwrap();
    let value = first
        .seal(
            common::binding(Uuid::nil(), "token"),
            &SecretString::from("private-value"),
        )
        .unwrap();
    let second = Encryption::initialize(
        &db.pool,
        KeyProtection::kms(client.clone(), "alias/test".into()).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        second
            .open(common::binding(Uuid::nil(), "token"), value)
            .unwrap()
            .expose_secret(),
        "private-value"
    );
    assert_eq!(
        state.calls.lock().unwrap().len(),
        2,
        "hot-path encryption/decryption must not call KMS"
    );
    assert!(matches!(
        Encryption::initialize(&db.pool, common::seed(9)).await,
        Err(Error::BackendMismatch)
    ));
    assert!(matches!(
        Encryption::initialize(
            &db.pool,
            KeyProtection::kms(client.clone(), "alias/wrong".into()).unwrap()
        )
        .await,
        Err(Error::Kms)
    ));
    for response in [
        json!({"KeyId":"wrong-key-arn","Plaintext":STANDARD.encode([9u8;32])}),
        json!({"KeyId":KEY_ARN,"Plaintext":STANDARD.encode([9u8;16])}),
        json!({"KeyId":KEY_ARN,"Plaintext":"not base64!"}),
        json!({"KeyId":KEY_ARN}),
    ] {
        *state.response_override.lock().unwrap() = Some(response);
        assert!(matches!(
            Encryption::initialize(
                &db.pool,
                KeyProtection::kms(client.clone(), "alias/test".into()).unwrap()
            )
            .await,
            Err(Error::Kms)
        ));
    }
    *state.response_override.lock().unwrap() = None;
    let persisted: Vec<u8> = sqlx::query_scalar("SELECT wrapped_key FROM encryption_keys")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(persisted, b"opaque-kms-wrapped-key");
    server.abort();
    db.close().await;
}
