mod common;
use axum::{
    Json, Router,
    extract::{Form, State},
    routing::{get, post},
};
use base64::Engine;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::{
    connections::{catalog::Endpoints, model::*, service::Connections},
    encryption::Encryption,
};
use uuid::Uuid;

const PEM: &str = include_str!("fixtures/connection-test-key.pem");
fn values(fields: &[(&str, &str)]) -> Values {
    fields
        .iter()
        .map(|(k, v)| (k.to_string(), SecretString::from(*v)))
        .collect()
}
fn static_schema() -> serde_json::Value {
    json!({"type":"object","additionalProperties":false,"properties":{"strange_secret-key":{"type":"string","title":"API key","writeOnly":true,"minLength":1},"workspace":{"type":"string","title":"Workspace","minLength":1}},"required":["strange_secret-key","workspace"]})
}
fn custom(id: &str, driver: Driver, oauth: Option<OAuth>) -> Provider {
    let source = match driver {
        Driver::Static => CredentialSource::Static {
            schema: static_schema(),
        },
        Driver::OAuthCode => CredentialSource::OAuth {
            grant: OAuthGrant::AuthorizationCode,
            configuration: oauth.unwrap().into(),
            additional_schema: None,
        },
        Driver::OAuthClientCredentials => CredentialSource::OAuth {
            grant: OAuthGrant::ClientCredentials,
            configuration: oauth.unwrap().into(),
            additional_schema: None,
        },
        Driver::OAuthJwtBearer => CredentialSource::OAuth {
            grant: OAuthGrant::JwtBearer,
            configuration: oauth.unwrap().into(),
            additional_schema: None,
        },
        Driver::Custom => CredentialSource::Custom,
    };
    Provider {
        account_name_label: None,
        icon_url: None,
        instructions: None,
        id: id.into(),
        name: "Custom account".into(),
        kind: ProviderKind::Configured,
        categories: vec!["other".into()],
        connection_types: vec![ConnectionType {
            id: "account".into(),
            name: "Account".into(),
            capabilities: vec![],
            credential_source: source,
        }],
    }
}
fn parse_brokering_url(url: &str) -> (Uuid, String) {
    let url = url::Url::parse(url).unwrap();
    (
        Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap(),
        url.query_pairs()
            .find(|(key, _)| key == "connection_setup_token")
            .unwrap()
            .1
            .into_owned(),
    )
}
async fn app_path(service: &Connections, setup: Uuid, token: &str, path: &str) -> BrokerView {
    let view = service.view(setup, token).await.unwrap();
    service
        .advance(
            setup,
            token,
            view.action_id,
            values(&[("setup_path", path)]),
        )
        .await
        .unwrap()
}
fn callback_state(action: &Action) -> (Uuid, String) {
    let raw = match action {
        Action::Redirect { url } | Action::FormPost { url, .. } => url,
        _ => panic!("expected browser handoff"),
    };
    let url = url::Url::parse(raw).unwrap();
    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    let (id, token) = state.split_once('.').unwrap();
    (Uuid::parse_str(id).unwrap(), token.into())
}
async fn service(db: &Database) -> Connections {
    Connections::new(
        db.pool.clone(),
        Arc::new(Encryption::initialize(&db.pool, seed(7)).await.unwrap()),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
    )
    .unwrap()
}

#[tokio::test]
async fn custom_fields_are_encrypted_durable_and_reconnect_is_atomic() {
    let db = Database::new().await;
    let service = service(&db).await;
    service.seed().await.unwrap();
    service
        .register_provider(custom("custom/acme", Driver::Static, None), None)
        .await
        .unwrap();
    let mut replacement = custom("custom/acme", Driver::Static, None);
    replacement.name = "Updated account".into();
    replacement.icon_url = Some("https://cdn.example.com/acme.svg".into());
    replacement.instructions = Some("Use the API key from your Acme account.".into());
    service.register_provider(replacement, None).await.unwrap();
    assert_eq!(
        service.provider("custom/acme").await.unwrap().name,
        "Updated account"
    );
    assert!(
        service
            .register_provider(tilde::connections::catalog::github::definition(), None)
            .await
            .is_err()
    );
    let id = Uuid::new_v4();
    let start = service
        .start(id, "Account", "custom/acme", "account", &[])
        .await
        .unwrap();
    let (flow, token) = parse_brokering_url(&start.brokering_url);
    let presentation = service.view(flow, &token).await.unwrap();
    assert_eq!(
        presentation.icon_url.as_deref(),
        Some("https://cdn.example.com/acme.svg")
    );
    assert_eq!(
        presentation.instructions.as_deref(),
        Some("Use the API key from your Acme account.")
    );
    let catalog = service.provider("custom/acme").await.unwrap();
    assert_eq!(catalog.icon_url, presentation.icon_url);
    assert_eq!(catalog.instructions, presentation.instructions);
    assert!(presentation.setup_instructions.is_empty());
    assert!(presentation.webhook_url.is_none());

    assert_eq!(
        service
            .start(id, "Account", "custom/acme", "account", &[])
            .await
            .unwrap()
            .brokering_url,
        start.brokering_url
    );
    assert!(service.view(flow, "wrong").await.is_err());
    let view = service.view(flow, &token).await.unwrap();
    assert!(
        service
            .advance(
                flow,
                &token,
                view.action_id,
                values(&[("unknown", "value")])
            )
            .await
            .is_err()
    );
    let complete = service
        .advance(
            flow,
            &token,
            view.action_id,
            values(&[
                ("strange_secret-key", "private-credential-value"),
                ("workspace", "work"),
            ]),
        )
        .await
        .unwrap();
    assert!(matches!(complete.action, Action::Complete));
    assert!(
        service
            .advance(
                flow,
                &token,
                view.action_id,
                values(&[("strange_secret-key", "retry"), ("workspace", "work")])
            )
            .await
            .is_err()
    );
    let encrypted:Vec<u8>=sqlx::query_scalar("SELECT encrypted_value FROM connection_values WHERE connection_id=$1 AND field_key='strange_secret-key'").bind(id).fetch_one(&db.pool).await.unwrap();
    assert!(
        !encrypted
            .windows(24)
            .any(|v| v == b"private-credential-value")
    );
    assert!(!format!("{:?}", service.get(id).await.unwrap()).contains("private-credential"));
    let crypto = Encryption::initialize(&db.pool, seed(7)).await.unwrap();
    let plaintext = crypto
        .open(
            tilde::encryption::SecretBinding {
                resource_kind: "connection",
                ..common::binding(id, "strange_secret-key")
            },
            tilde::encryption::SealedSecret::from_bytes(&encrypted).unwrap(),
        )
        .unwrap();
    assert_eq!(plaintext.expose_secret(), "private-credential-value");
    let restarted = Connections::new(
        db.pool.clone(),
        Arc::new(Encryption::initialize(&db.pool, seed(7)).await.unwrap()),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    assert_eq!(
        restarted.resolve(id).await.unwrap()["strange_secret-key"].expose_secret(),
        "private-credential-value"
    );
    let pending = restarted.reconnect(id).await.unwrap();
    let (flow2, token2) = parse_brokering_url(&pending.brokering_url);
    // A capability for a completed attempt cannot cancel a later attempt.
    restarted.cancel(flow, &token).await.unwrap();
    assert!(matches!(
        restarted.view(flow2, &token2).await.unwrap().action,
        Action::Form
    ));
    assert_eq!(
        restarted.reconnect(id).await.unwrap().brokering_url,
        pending.brokering_url
    );

    assert_eq!(
        restarted.resolve(id).await.unwrap()["strange_secret-key"].expose_secret(),
        "private-credential-value"
    );
    restarted.cancel(flow2, &token2).await.unwrap();
    assert_eq!(restarted.get(id).await.unwrap().status, "ready");
    let pending = restarted.reconnect(id).await.unwrap();
    let (flow3, token3) = parse_brokering_url(&pending.brokering_url);
    let view = restarted.view(flow3, &token3).await.unwrap();
    restarted
        .advance(
            flow3,
            &token3,
            view.action_id,
            values(&[("strange_secret-key", "replacement"), ("workspace", "work")]),
        )
        .await
        .unwrap();
    assert_eq!(
        restarted.resolve(id).await.unwrap()["strange_secret-key"].expose_secret(),
        "replacement"
    );
    restarted.disconnect(id).await.unwrap();
    assert!(restarted.resolve(id).await.is_err());
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM connection_values WHERE connection_id=$1")
            .bind(id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    db.close().await;
}

#[derive(Clone, Default)]
struct Fixture {
    forms: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    calls: Arc<Mutex<Vec<String>>>,
    delay: Arc<std::sync::atomic::AtomicBool>,
    reject_refresh: Arc<std::sync::atomic::AtomicBool>,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}
async fn token(
    State(state): State<Fixture>,
    Form(form): Form<BTreeMap<String, String>>,
) -> Json<Value> {
    let refresh = form.get("grant_type").is_some_and(|v| v == "refresh_token");
    if form
        .get("grant_type")
        .is_some_and(|v| v == "urn:ietf:params:oauth:grant-type:jwt-bearer")
    {
        let key = jsonwebtoken::DecodingKey::from_rsa_pem(include_bytes!(
            "fixtures/connection-test-public.pem"
        ))
        .unwrap();
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.validate_aud = false;
        let claims = jsonwebtoken::decode::<Value>(&form["assertion"], &key, &validation)
            .unwrap()
            .claims;
        assert_eq!(claims["iss"], "fixture@example.com");
    }
    state.forms.lock().unwrap().push(form);
    if refresh
        && state
            .reject_refresh
            .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Json(json!({"error":"invalid_grant"}));
    }

    if state.delay.load(std::sync::atomic::Ordering::SeqCst) {
        state.entered.notify_one();
        state.release.notified().await;
    }

    Json(
        json!({"access_token":if refresh{"refreshed-access"}else{"initial-access"},"refresh_token":if refresh{"rotated-refresh"}else{"initial-refresh"},"expires_in":600,"scope":"read","account":{"workspace_id":"workspace-from-oauth"}}),
    )
}
async fn github_app() -> Json<Value> {
    Json(json!({"id":42,"slug":"fixture-app","client_id":"Iv1.fixture"}))
}
async fn github_convert(State(state): State<Fixture>) -> Json<Value> {
    state.calls.lock().unwrap().push("github-convert".into());
    Json(
        json!({"id":42,"slug":"fixture-app","pem":PEM,"webhook_secret":"webhook-private","client_id":"Iv1.fixture","client_secret":"client-private"}),
    )
}
async fn github_install(
    State(state): State<Fixture>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let token = headers
        .get("authorization")
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap();
    let key = jsonwebtoken::DecodingKey::from_rsa_pem(include_bytes!(
        "fixtures/connection-test-public.pem"
    ))
    .unwrap();
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.validate_aud = false;
    let claims = jsonwebtoken::decode::<Value>(token, &key, &validation)
        .unwrap()
        .claims;
    assert_eq!(claims["iss"], "Iv1.fixture");
    state.calls.lock().unwrap().push("github-install".into());
    Json(json!({"id":73,"account":{"login":"fixture-owner"}}))
}
async fn github_token(State(state): State<Fixture>) -> Json<Value> {
    state.calls.lock().unwrap().push("github-token".into());
    Json(
        json!({"token":"github-private-token","expires_at":(chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339()}),
    )
}
async fn slack_rotate(Form(form): Form<BTreeMap<String, String>>) -> Json<Value> {
    assert_eq!(form["refresh_token"], "configuration-refresh");
    Json(json!({"ok":true,"token":"configuration-access","refresh_token":"configuration-rotated"}))
}
async fn slack_create(State(state): State<Fixture>, Json(body): Json<Value>) -> Json<Value> {
    let manifest: Value = serde_json::from_str(body["manifest"].as_str().unwrap()).unwrap();
    assert_eq!(
        manifest["oauth_config"]["redirect_urls"][0],
        "http://127.0.0.1:18888/connections/callback"
    );
    assert!(
        manifest["oauth_config"]["scopes"]["bot"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "chat:write")
    );
    assert!(
        manifest["settings"]["event_subscriptions"]["request_url"]
            .as_str()
            .unwrap()
            .starts_with("https://ingress.example/connections/webhooks/")
    );
    state.calls.lock().unwrap().push("slack-create".into());
    Json(
        json!({"ok":true,"app_id":"A123","credentials":{"client_id":"slack-client","client_secret":"slack-secret","signing_secret":"slack-signing"}}),
    )
}
async fn slack_token(
    State(state): State<Fixture>,
    Form(form): Form<BTreeMap<String, String>>,
) -> Json<Value> {
    assert_eq!(form["client_id"], "slack-client");
    assert_eq!(form["client_secret"], "slack-secret");
    assert_eq!(
        form["redirect_uri"],
        "http://127.0.0.1:18888/connections/callback"
    );
    state.forms.lock().unwrap().push(form);
    Json(
        json!({"ok":true,"access_token":"slack-bot-token","refresh_token":"slack-refresh","expires_in":600,"team":{"id":"T123","name":"Fixture workspace"},"app_id":"A123","bot_user_id":"U123"}),
    )
}
async fn fixture() -> (String, Fixture, tokio::task::JoinHandle<()>) {
    let state = Fixture::default();
    let app = Router::new()
        .route("/app", get(github_app))
        .route("/token", post(token))
        .route("/app-manifests/code/conversions", post(github_convert))
        .route("/app/installations/73", get(github_install))
        .route("/app/installations/73/access_tokens", post(github_token))
        .route("/tooling.tokens.rotate", post(slack_rotate))
        .route("/apps.manifest.create", post(slack_create))
        .route("/oauth.v2.access", post(slack_token))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (origin, state, task)
}

#[tokio::test]
async fn custom_oauth_pkce_callback_and_shared_refresh_work_across_restart() {
    let db = Database::new().await;
    let (service_url, fixture, server) = fixture().await;
    let service = service(&db).await;
    let mut config = OAuth::standard(&format!("{service_url}/token"));
    config.authorization_url = Some(format!("{service_url}/authorize"));
    config.scopes = vec!["read".into()];
    config.result_fields = vec![ResultField {
        key: "workspace".into(),
        path: "/account/workspace_id".into(),
        required: true,
    }];
    service
        .register_provider(
            custom("custom/oauth", Driver::OAuthCode, Some(config)),
            None,
        )
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let start = service
        .start(id, "OAuth", "custom/oauth", "account", &[])
        .await
        .unwrap();
    let (flow, connection_setup_token) = parse_brokering_url(&start.brokering_url);
    let view = service.view(flow, &connection_setup_token).await.unwrap();
    let view = service
        .advance(
            flow,
            &connection_setup_token,
            view.action_id,
            values(&[("client_id", "client"), ("client_secret", "secret")]),
        )
        .await
        .unwrap();
    let (state_id, state) = callback_state(&view.action);
    let url = match &view.action {
        Action::Redirect { url } => url::Url::parse(url).unwrap(),
        _ => panic!(),
    };
    let challenge = url
        .query_pairs()
        .find(|(k, _)| k == "code_challenge")
        .unwrap()
        .1
        .into_owned();
    assert!(
        service
            .callback(state_id, "forged", &values(&[("code", "code")]), false)
            .await
            .is_err()
    );
    assert!(fixture.forms.lock().unwrap().is_empty());
    service
        .callback(state_id, &state, &values(&[("code", "code")]), false)
        .await
        .unwrap();
    assert!(matches!(
        service
            .view(flow, &connection_setup_token)
            .await
            .unwrap()
            .action,
        Action::Complete
    ));
    {
        let forms = fixture.forms.lock().unwrap();
        let sent = &forms[0];
        assert_eq!(
            sent["redirect_uri"],
            "http://127.0.0.1:18888/connections/callback"
        );
        assert_eq!(
            challenge,
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(Sha256::digest(sent["code_verifier"].as_bytes()))
        );
    }
    service
        .callback(state_id, &state, &values(&[("code", "code")]), false)
        .await
        .unwrap();
    assert_eq!(fixture.forms.lock().unwrap().len(), 1);
    assert_eq!(
        service.resolve(id).await.unwrap()["workspace"].expose_secret(),
        "workspace-from-oauth"
    );
    sqlx::query("UPDATE connections SET token_expires_at=now()-interval '1 minute' WHERE id=$1")
        .bind(id)
        .execute(&db.pool)
        .await
        .unwrap();
    let (a, b) = tokio::join!(service.resolve(id), service.resolve(id));
    assert!(a.is_ok() || b.is_ok());
    assert_eq!(
        service.resolve(id).await.unwrap()["access_token"].expose_secret(),
        "refreshed-access"
    );
    assert_eq!(
        fixture
            .forms
            .lock()
            .unwrap()
            .iter()
            .filter(|f| f["grant_type"] == "refresh_token")
            .count(),
        1
    );
    fixture
        .reject_refresh
        .store(true, std::sync::atomic::Ordering::SeqCst);
    sqlx::query("UPDATE connections SET token_expires_at=now()-interval '1 minute' WHERE id=$1")
        .bind(id)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(matches!(
        service.resolve(id).await,
        Err(tilde::error::Error::ConnectionAuthorizationRequired)
    ));
    assert_eq!(service.get(id).await.unwrap().status, "requires_action");
    let calls = fixture.forms.lock().unwrap().len();
    assert!(service.resolve(id).await.is_err());
    assert_eq!(fixture.forms.lock().unwrap().len(), calls);
    server.abort();
    db.close().await;
}

#[tokio::test]
async fn github_and_slack_apps_use_the_broker_and_type_scoped_channel_capability() {
    let db = Database::new().await;
    let (origin, state, server) = fixture().await;
    let endpoints = Endpoints(BTreeMap::from([
        ("github_api".into(), origin.clone()),
        ("github_web".into(), origin.clone()),
        ("slack_api".into(), origin.clone()),
        ("slack_authorize".into(), format!("{origin}/authorize")),
    ]));
    let service = Connections::with_endpoints(
        db.pool.clone(),
        Arc::new(Encryption::initialize(&db.pool, seed(7)).await.unwrap()),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
        endpoints,
    )
    .unwrap();
    service.seed().await.unwrap();
    let github = service.provider("github").await.unwrap();
    assert_eq!(github.connection_types.len(), 1);
    assert_eq!(github.connection_types[0].driver(), Driver::Custom);
    assert_eq!(
        github.connection_types[0].capabilities,
        vec![Capability::Channel]
    );
    let id = Uuid::new_v4();
    let start = service
        .start(id, "GitHub", "github", "github_app", &[])
        .await
        .unwrap();
    let (flow, connection_setup_token) = parse_brokering_url(&start.brokering_url);
    let view = app_path(&service, flow, &connection_setup_token, "create").await;
    let view = service
        .advance(
            flow,
            &connection_setup_token,
            view.action_id,
            values(&[("app_name", "My app"), ("owner_type", "user")]),
        )
        .await
        .unwrap();
    if let Action::FormPost { fields, .. } = &view.action {
        let manifest: serde_json::Value =
            serde_json::from_str(&fields.iter().find(|(key, _)| key == "manifest").unwrap().1)
                .unwrap();
        assert_eq!(
            manifest["hook_attributes"]["url"],
            format!("https://ingress.example/connections/webhooks/{id}")
        );
        assert_eq!(
            manifest["redirect_url"],
            "http://127.0.0.1:18888/connections/callback"
        );
    }
    assert!(matches!(view.action, Action::FormPost { .. }));
    let (_, state_token) = callback_state(&view.action);
    service
        .callback(flow, &state_token, &values(&[("code", "code")]), false)
        .await
        .unwrap();
    let installation = service.view(flow, &connection_setup_token).await.unwrap();
    assert!(matches!(installation.action, Action::Redirect { .. }));
    assert!(
        service
            .callback(flow, &state_token, &values(&[("code", "code")]), false)
            .await
            .is_err()
    );
    assert_eq!(
        service
            .view(flow, &connection_setup_token)
            .await
            .unwrap()
            .step,
        "github_install"
    );
    service
        .callback(
            flow,
            &state_token,
            &values(&[("installation_id", "73")]),
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        service.get(id).await.unwrap().account_label.as_deref(),
        Some("fixture-owner")
    );
    assert_eq!(
        service.resolve(id).await.unwrap()["access_token"].expose_secret(),
        "github-private-token"
    );
    let slack_id = Uuid::new_v4();
    let start = service
        .start(slack_id, "Slack", "slack", "slack_app", &[])
        .await
        .unwrap();
    let (slack_flow, slack_connection_setup_token) = parse_brokering_url(&start.brokering_url);
    let view = app_path(
        &service,
        slack_flow,
        &slack_connection_setup_token,
        "create",
    )
    .await;
    let view = service
        .advance(
            slack_flow,
            &slack_connection_setup_token,
            view.action_id,
            values(&[
                ("app_name", "Bot"),
                ("configuration_refresh_token", "configuration-refresh"),
            ]),
        )
        .await
        .unwrap();
    let (_, state_token) = callback_state(&view.action);
    service
        .callback(
            slack_flow,
            &state_token,
            &values(&[("code", "code")]),
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        service.resolve(slack_id).await.unwrap()["access_token"].expose_secret(),
        "slack-bot-token"
    );
    let existing_id = Uuid::new_v4();
    let existing = service
        .start(existing_id, "Existing GitHub", "github", "github_app", &[])
        .await
        .unwrap();
    let (flow_existing, connection_setup_token_existing) =
        parse_brokering_url(&existing.brokering_url);
    let form = app_path(
        &service,
        flow_existing,
        &connection_setup_token_existing,
        "existing",
    )
    .await;
    service
        .advance(
            flow_existing,
            &connection_setup_token_existing,
            form.action_id,
            values(&[
                ("app_id", "42"),
                ("private_key", PEM),
                ("installation_id", "73"),
                ("webhook_secret", "existing-webhook"),
            ]),
        )
        .await
        .unwrap();
    assert_eq!(
        service.resolve(existing_id).await.unwrap()["access_token"].expose_secret(),
        "github-private-token"
    );
    let existing_slack = Uuid::new_v4();
    let existing = service
        .start(existing_slack, "Existing Slack", "slack", "slack_app", &[])
        .await
        .unwrap();
    let (flow_existing, connection_setup_token_existing) =
        parse_brokering_url(&existing.brokering_url);
    let form = app_path(
        &service,
        flow_existing,
        &connection_setup_token_existing,
        "existing",
    )
    .await;
    let view = service
        .advance(
            flow_existing,
            &connection_setup_token_existing,
            form.action_id,
            values(&[
                ("client_id", "slack-client"),
                ("client_secret", "slack-secret"),
                ("signing_secret", "existing-signing"),
            ]),
        )
        .await
        .unwrap();
    let (_, nonce) = callback_state(&view.action);
    service
        .callback(flow_existing, &nonce, &values(&[("code", "code")]), false)
        .await
        .unwrap();
    assert_eq!(
        service.resolve(existing_slack).await.unwrap()["slack_team_id"].expose_secret(),
        "T123"
    );
    for (connection_id, installation) in [(id, Some("73")), (slack_id, None)] {
        let pending = service.reconnect(connection_id).await.unwrap();
        let (setup, connection_setup_token) = parse_brokering_url(&pending.brokering_url);
        let view = service.view(setup, &connection_setup_token).await.unwrap();
        assert!(matches!(view.action, Action::Redirect { .. }));
        let (_, nonce) = callback_state(&view.action);
        service
            .callback(
                setup,
                &nonce,
                &values(&[if let Some(installation) = installation {
                    ("installation_id", installation)
                } else {
                    ("code", "code")
                }]),
                false,
            )
            .await
            .unwrap();
    }
    assert_eq!(
        state
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.as_str() == "github-convert")
            .count(),
        1
    );
    assert_eq!(
        state
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.as_str() == "slack-create")
            .count(),
        1
    );
    server.abort();
    db.close().await;
}

#[tokio::test]
async fn client_credentials_jwt_and_provider_definitions_share_the_same_runtime() {
    let db = Database::new().await;
    let (origin, state, server) = fixture().await;
    let service = service(&db).await;
    for (driver, key, fields) in [
        (
            Driver::OAuthClientCredentials,
            "custom/client",
            values(&[("client_id", "client"), ("client_secret", "secret")]),
        ),
        (
            Driver::OAuthJwtBearer,
            "custom/jwt",
            values(&[("issuer", "fixture@example.com"), ("private_key", PEM)]),
        ),
    ] {
        let config = OAuth::standard(&format!("{origin}/token"));
        service
            .register_provider(custom(key, driver, Some(config)), None)
            .await
            .unwrap();
        let id = Uuid::new_v4();
        let start = service
            .start(id, "Account", key, "account", &[])
            .await
            .unwrap();
        let (flow, connection_setup_token) = parse_brokering_url(&start.brokering_url);
        let view = service.view(flow, &connection_setup_token).await.unwrap();
        service
            .advance(flow, &connection_setup_token, view.action_id, fields)
            .await
            .unwrap();
        assert_eq!(
            service.resolve(id).await.unwrap()["access_token"].expose_secret(),
            "initial-access"
        );
    }
    assert_eq!(state.forms.lock().unwrap().len(), 2);
    server.abort();
    db.close().await;
}

#[tokio::test]
async fn generated_client_and_native_callback_use_the_same_durable_broker() {
    let db = Database::new().await;
    let (oauth_url, _, oauth_server) = fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let service = Connections::new(
        db.pool.clone(),
        Arc::new(Encryption::initialize(&db.pool, seed(7)).await.unwrap()),
        origin.clone(),
        "https://ingress.example".into(),
    )
    .unwrap();
    service.seed().await.unwrap();
    let router = tilde::connections::rpc::management::router(service.clone())
        .merge(tilde::connections::rpc::setup::router(service));
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let script = std::path::Path::new(file!())
        .parent()
        .unwrap()
        .join("../../../web/scripts/test-connections.mjs");
    let script = if script.is_absolute() {
        script
    } else {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../web/scripts/test-connections.mjs")
    };
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("node")
            .arg(script)
            .env("CONNECTION_TEST_URL", origin)
            .env("CONNECTION_OAUTH_URL", oauth_url)
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.abort();
    oauth_server.abort();
    db.close().await;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn a_thousand_runtime_definitions_are_persisted_and_paginated_without_new_code() {
    let db = Database::new().await;
    let service = service(&db).await;
    for i in 0..1001 {
        let mut definition = custom(&format!("custom/catalog-{i:04}"), Driver::Static, None);
        definition.name = "Large runtime catalog".into();
        definition.connection_types[0].credential_source = CredentialSource::Static {
            schema: tilde::connections::schema::empty(),
        };
        service.register_provider(definition, None).await.unwrap();
    }
    let (first, next) = service
        .providers("", Some("Large runtime catalog"), 100)
        .await
        .unwrap();
    assert_eq!(first.len(), 100);
    assert_eq!(next, "custom/catalog-0099");
    let (second, next) = service.providers(&next, None, 100).await.unwrap();
    assert_eq!(second[0].id, "custom/catalog-0100");
    assert_eq!(next, "custom/catalog-0199");
    let (last, next) = service
        .providers("custom/catalog-0999", None, 100)
        .await
        .unwrap();
    assert_eq!(last.len(), 1);
    assert!(next.is_empty());
    let instant = service
        .start(
            Uuid::new_v4(),
            "No credentials needed",
            "custom/catalog-1000",
            "account",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(instant.connection.status, "ready");
    let (setup, token) = parse_brokering_url(&instant.brokering_url);
    assert!(matches!(
        service.view(setup, &token).await.unwrap().action,
        Action::Complete
    ));
    db.close().await;
}

#[tokio::test]
async fn cancellation_fences_late_token_results_and_recovery_preserves_working_credentials() {
    let db = Database::new().await;
    let (origin, state, server) = fixture().await;
    let service = service(&db).await;
    service
        .register_provider(
            custom(
                "custom/cancel",
                Driver::OAuthClientCredentials,
                Some(OAuth::standard(&format!("{origin}/token"))),
            ),
            None,
        )
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let started = service
        .start(id, "Cancel", "custom/cancel", "account", &[])
        .await
        .unwrap();
    let (setup, token) = parse_brokering_url(&started.brokering_url);
    let view = service.view(setup, &token).await.unwrap();
    state.delay.store(true, std::sync::atomic::Ordering::SeqCst);
    let worker = service.clone();
    let worker_token = token.clone();
    let task = tokio::spawn(async move {
        worker
            .advance(
                setup,
                &worker_token,
                view.action_id,
                values(&[("client_id", "client"), ("client_secret", "secret")]),
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), state.entered.notified())
        .await
        .unwrap();
    service.cancel(setup, &token).await.unwrap();
    state.release.notify_one();
    assert!(task.await.unwrap().is_err());
    assert!(matches!(
        service.view(setup, &token).await.unwrap().action,
        Action::Cancelled
    ));
    assert!(service.resolve(id).await.is_err());
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM connection_setup_values WHERE setup_id=$1")
            .bind(setup)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(pending, 0);
    state
        .delay
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let started = service.reconnect(id).await.unwrap();
    let (setup, token) = parse_brokering_url(&started.brokering_url);
    let view = service.view(setup, &token).await.unwrap();
    service
        .advance(
            setup,
            &token,
            view.action_id,
            values(&[("client_id", "client"), ("client_secret", "secret")]),
        )
        .await
        .unwrap();
    let pending = service.reconnect(id).await.unwrap();
    let (setup, token) = parse_brokering_url(&pending.brokering_url);
    sqlx::query("UPDATE connection_setups SET claimed_at=now()-interval '2 minutes' WHERE id=$1")
        .bind(setup)
        .execute(&db.pool)
        .await
        .unwrap();
    service.recover().await.unwrap();
    assert!(matches!(
        service.view(setup, &token).await.unwrap().action,
        Action::Failed
    ));
    assert_eq!(
        service.resolve(id).await.unwrap()["access_token"].expose_secret(),
        "initial-access"
    );
    server.abort();
    db.close().await;
}

#[tokio::test]
async fn provider_owned_forms_still_require_valid_credentials_without_ui_descriptors() {
    let db = Database::new().await;
    let service = service(&db).await;
    service.seed().await.unwrap();
    for (provider, typ) in [
        ("agentmail", "inbox"),
        ("linq", "account"),
        ("whatsapp", "meta"),
        ("telnyx", "whatsapp"),
    ] {
        let started = service
            .start(Uuid::new_v4(), "Private account", provider, typ, &[])
            .await
            .unwrap();
        assert_eq!(started.connection.status, "requires_action");
        let (id, connection_setup_token) = parse_brokering_url(&started.brokering_url);
        let state = service.view(id, &connection_setup_token).await.unwrap();
        assert_eq!(state.ui_path, "/catalog/_standard/ui");
        assert!(!state.setup_instructions.is_empty());
        assert!(
            service
                .advance(id, &connection_setup_token, state.action_id, Values::new())
                .await
                .is_err()
        );
        assert_eq!(
            service
                .view(id, &connection_setup_token)
                .await
                .unwrap()
                .step,
            "fields"
        );
        assert!(service.resolve(started.connection.id).await.is_err());
    }
    db.close().await;
}

#[tokio::test]
async fn remote_provider_sdk_drafts_callbacks_assets_and_cancel_are_end_to_end() {
    let db = Database::new().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    let service = Connections::new(
        db.pool.clone(),
        Arc::new(Encryption::initialize(&db.pool, seed(7)).await.unwrap()),
        origin.clone(),
        "https://ingress.example".into(),
    )
    .unwrap();
    let router = tilde::connections::rpc::management::router(service.clone())
        .merge(tilde::connections::rpc::setup::router(service.clone()))
        .merge(tilde::connections::assets::router(None, Some(service.clone())).unwrap())
        .layer(axum::middleware::from_fn_with_state(
            tilde::network::Boundary::new(address, false, vec![]).unwrap(),
            tilde::network::guard,
        ));
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/scripts/test-remote-connections.mjs");
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("node")
            .arg(script)
            .env("CONNECTION_TEST_URL", origin)
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.abort();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let id = Uuid::parse_str(result["connectionId"].as_str().unwrap()).unwrap();
    assert_eq!(
        service.resolve(id).await.unwrap()["api_key"].expose_secret(),
        "remote-private-api-key"
    );
    let provider_id = result["providerId"].as_str().unwrap();
    let row = sqlx::query_file!(
        "../../queries/connections/provider_backend.sql",
        provider_id
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let encrypted = row.remote_authorization.unwrap();
    let forbidden = b"remote-provider-test";
    assert!(
        !encrypted
            .windows(forbidden.len())
            .any(|part| part == forbidden)
    );
    db.close().await;
}

#[tokio::test]
async fn startup_and_worker_remove_ten_minute_setup_tokens_without_deleting_credentials() {
    let db = Database::new().await;
    let service = service(&db).await;
    service
        .register_provider(custom("custom/expiry", Driver::Static, None), None)
        .await
        .unwrap();
    let active = service
        .start(Uuid::new_v4(), "Draft", "custom/expiry", "account", &[])
        .await
        .unwrap();
    let (active_id, active_token) = parse_brokering_url(&active.brokering_url);
    assert!(
        sqlx::query_file!("tests/sql/connection_setup_lifetime.sql", active_id)
            .fetch_one(&db.pool)
            .await
            .unwrap()
            .ten_minutes
    );
    let view = service.view(active_id, &active_token).await.unwrap();
    service
        .save_draft(
            active_id,
            &active_token,
            view.action_id,
            values(&[("draft", "temporary")]),
        )
        .await
        .unwrap();
    sqlx::query_file!("tests/sql/age_connection_setup.sql", active_id)
        .execute(&db.pool)
        .await
        .unwrap();
    // This is the synchronous cleanup used before the server accepts traffic.
    service.recover().await.unwrap();
    assert!(service.view(active_id, &active_token).await.is_err());
    let material = sqlx::query_file!("tests/sql/connection_setup_material.sql", active_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        (material.setups, material.drafts, material.staged),
        (0, 0, 0)
    );
    let ready = service
        .start(Uuid::new_v4(), "Ready", "custom/expiry", "account", &[])
        .await
        .unwrap();
    let (ready_id, ready_token) = parse_brokering_url(&ready.brokering_url);
    let view = service.view(ready_id, &ready_token).await.unwrap();
    service
        .save_credentials(
            ready_id,
            &ready_token,
            view.action_id,
            values(&[
                ("strange_secret-key", "keep-this-key"),
                ("workspace", "personal"),
            ]),
        )
        .await
        .unwrap();
    sqlx::query_file!("tests/sql/age_connection_setup.sql", ready_id)
        .execute(&db.pool)
        .await
        .unwrap();
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(service.clone().worker(receiver));
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if service.view(ready_id, &ready_token).await.is_err() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    shutdown.send(true).unwrap();
    worker.await.unwrap();
    let material = sqlx::query_file!("tests/sql/connection_setup_material.sql", ready_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        (material.setups, material.drafts, material.staged),
        (0, 0, 0)
    );
    assert_eq!(
        service.resolve(ready.connection.id).await.unwrap()["strange_secret-key"].expose_secret(),
        "keep-this-key"
    );
    db.close().await;
}

#[tokio::test]
async fn unversioned_provider_migration_preserves_encrypted_backend_credentials() {
    let db = Database::unmigrated().await;
    let all = sqlx::migrate!("../../migrations");
    let before = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            all.iter()
                .filter(|migration| migration.version < 203)
                .cloned()
                .collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    };
    before.run(&db.pool).await.unwrap();
    let crypto = Arc::new(Encryption::initialize(&db.pool, seed(7)).await.unwrap());
    let provider_id = "custom/legacy";
    let digest = Sha256::digest(format!("{provider_id}:7"));
    let identity = Uuid::from_bytes(digest[..16].try_into().unwrap());
    let binding = tilde::encryption::SecretBinding {
        resource_kind: "connection_provider",
        resource_id: identity,
        name: "backend_token",
    };
    let encrypted = crypto
        .seal(
            binding,
            &SecretString::from("legacy-backend-credential-0123456789"),
        )
        .unwrap()
        .into_bytes();
    // Historical schemas cannot use macros checked against today's schema.
    sqlx::query(include_str!("sql/legacy_connection_provider.sql"))
        .bind(provider_id)
        .bind(encrypted)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(include_str!("sql/legacy_connection_type.sql"))
        .bind(provider_id)
        .execute(&db.pool)
        .await
        .unwrap();
    tilde::database::migrate(&db.pool).await.unwrap();
    let row = sqlx::query_file!(
        "../../queries/connections/provider_backend.sql",
        provider_id
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(row.remote_authorization_id, Some(identity));
    let plaintext = crypto
        .open(
            tilde::encryption::SecretBinding {
                resource_kind: "connection_provider",
                resource_id: row.remote_authorization_id.unwrap(),
                name: "backend_token",
            },
            tilde::encryption::SealedSecret::from_bytes(&row.remote_authorization.unwrap())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        plaintext.expose_secret(),
        "legacy-backend-credential-0123456789"
    );
    let service = Connections::new(
        db.pool.clone(),
        crypto,
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    let provider = service.provider(provider_id).await.unwrap();
    assert!(matches!(provider.kind, ProviderKind::Remote(_)));
    assert_eq!(provider.categories, vec!["other"]);
    assert!(matches!(
        provider.connection_types[0].credential_source,
        CredentialSource::Custom
    ));
    db.close().await;
}

#[tokio::test]
async fn provider_methods_round_trip_as_independent_credential_sources() {
    let db = Database::new().await;
    let service = service(&db).await;
    let mut provider = custom("custom/methods", Driver::Static, None);
    provider.categories = vec![tilde::connections::categories::CATEGORY_EMAIL.into()];
    provider.connection_types[0].id = "api_key".into();
    let mut oauth = OAuth::standard("https://example.com/token");
    oauth.authorization_url = Some("https://example.com/authorize".into());
    provider.connection_types.push(ConnectionType {
        id: "oauth".into(),
        name: "Sign in with OAuth".into(),
        capabilities: vec![],
        credential_source: CredentialSource::OAuth {
            grant: OAuthGrant::AuthorizationCode,
            configuration: oauth.into(),
            additional_schema: None,
        },
    });
    service.register_provider(provider, None).await.unwrap();
    let saved = service.provider("custom/methods").await.unwrap();
    assert_eq!(saved.categories, vec!["email"]);
    assert!(matches!(
        saved.connection_types[0].credential_source,
        CredentialSource::Static { .. }
    ));
    assert!(matches!(
        saved.connection_types[1].credential_source,
        CredentialSource::OAuth {
            grant: OAuthGrant::AuthorizationCode,
            ..
        }
    ));
    let started = service
        .start(
            Uuid::new_v4(),
            "Static choice",
            "custom/methods",
            "api_key",
            &[],
        )
        .await
        .unwrap();
    let (setup, token) = parse_brokering_url(&started.brokering_url);
    assert_eq!(
        service.view(setup, &token).await.unwrap().type_id,
        "api_key"
    );
    let mut replacement = saved;
    replacement.name = "Should roll back".into();
    replacement.connection_types.remove(0);
    assert!(service.register_provider(replacement, None).await.is_err());
    assert_ne!(
        service.provider("custom/methods").await.unwrap().name,
        "Should roll back"
    );
    db.close().await;
}

#[tokio::test]
async fn static_schema_drives_standard_ui_and_validates_typed_credentials() {
    let db = Database::new().await;
    let service = service(&db).await;
    let schema = json!({"type":"object","additionalProperties":false,"properties":{"api.key":{"type":"string","title":"API key","writeOnly":true,"minLength":4},"enabled":{"type":"boolean"},"region":{"type":"string","enum":["eu","us"]},"limit":{"type":"integer","minimum":1,"maximum":5}},"required":["api.key","enabled","region","limit"]});
    let mut provider = custom("custom/schema", Driver::Static, None);
    provider.kind = ProviderKind::Remote(RemoteProvider {
        endpoint: "http://127.0.0.1:1".into(),
        ui_url: None,
    });
    provider.connection_types[0].credential_source = CredentialSource::Static {
        schema: schema.clone(),
    };
    service.register_provider(provider, None).await.unwrap();
    let started = service
        .start(Uuid::new_v4(), "Typed", "custom/schema", "account", &[])
        .await
        .unwrap();
    let (id, token) = parse_brokering_url(&started.brokering_url);
    let state = service.view(id, &token).await.unwrap();
    assert_eq!(state.ui_path, "/catalog/_standard/ui");
    assert_eq!(state.input_schema, Some(schema));
    assert!(
        service
            .save_credentials(
                id,
                &token,
                state.action_id,
                values(&[
                    ("api.key", "private"),
                    ("enabled", "false"),
                    ("region", "eu"),
                    ("limit", "99")
                ])
            )
            .await
            .is_err()
    );
    service
        .save_credentials(
            id,
            &token,
            state.action_id,
            values(&[
                ("api.key", "private"),
                ("enabled", "false"),
                ("region", "eu"),
                ("limit", "3"),
            ]),
        )
        .await
        .unwrap();
    let stored = service.resolve(started.connection.id).await.unwrap();
    assert_eq!(stored["enabled"].expose_secret(), "false");
    assert_eq!(stored["limit"].expose_secret(), "3");
    let mut invalid = custom("custom/unsupported", Driver::Static, None);
    invalid.connection_types[0].credential_source = CredentialSource::Static {
        schema: json!({"type":"object","additionalProperties":false,"properties":{"nested":{"type":"object"}}}),
    };
    assert!(service.register_provider(invalid, None).await.is_err());
    db.close().await;
}

#[tokio::test]
async fn oauth_additional_signing_key_uses_standard_inputs_and_shared_callback() {
    let db = Database::new().await;
    let service = service(&db).await;
    let (origin, _, server) = fixture().await;
    let mut config = OAuth::standard(&format!("{origin}/token"));
    config.authorization_url = Some(format!("{origin}/authorize"));
    let mut provider = custom("custom/extra-oauth", Driver::OAuthCode, Some(config));
    if let CredentialSource::OAuth {
        additional_schema, ..
    } = &mut provider.connection_types[0].credential_source
    {
        *additional_schema = Some(
            json!({"type":"object","additionalProperties":false,"properties":{"signing_secret":{"type":"string","writeOnly":true,"minLength":1}},"required":["signing_secret"]}),
        );
    }
    service.register_provider(provider, None).await.unwrap();
    let started = service
        .start(
            Uuid::new_v4(),
            "OAuth with signing key",
            "custom/extra-oauth",
            "account",
            &[],
        )
        .await
        .unwrap();
    let (id, token) = parse_brokering_url(&started.brokering_url);
    let state = service.view(id, &token).await.unwrap();
    assert_eq!(state.ui_path, "/catalog/_standard/ui");
    let schema = state.input_schema.unwrap();
    assert!(schema["properties"].get("client_id").is_some());
    assert!(schema["properties"].get("signing_secret").is_some());
    assert!(schema["properties"].get("_pkce").is_none());
    let consent = service
        .start_oauth(
            id,
            &token,
            state.action_id,
            values(&[
                ("client_id", "client"),
                ("client_secret", "secret"),
                ("signing_secret", "private-signing-secret"),
            ]),
        )
        .await
        .unwrap();
    let (setup, nonce) = callback_state(&consent.action);
    service
        .callback(setup, &nonce, &values(&[("code", "code")]), false)
        .await
        .unwrap();
    assert_eq!(
        service.resolve(started.connection.id).await.unwrap()["signing_secret"].expose_secret(),
        "private-signing-secret"
    );
    server.abort();
    db.close().await;
}

#[tokio::test]
async fn account_name_is_scoped_to_setup_and_never_becomes_a_credential() {
    let db = Database::new().await;
    let service = service(&db).await;
    let mut provider = custom("named-account", Driver::Static, None);
    provider.account_name_label = Some("Acme account".into());
    service.register_provider(provider, None).await.unwrap();
    let started = service
        .start(Uuid::new_v4(), "Temporary", "named-account", "account", &[])
        .await
        .unwrap();
    let (id, token) = parse_brokering_url(&started.brokering_url);
    let initial = service.view(id, &token).await.unwrap();
    assert_eq!(initial.account_name_label, "Acme account");
    assert!(
        service
            .set_connection_name(id, "wrong-token", initial.action_id, "Spoofed")
            .await
            .is_err()
    );
    let named = service
        .set_connection_name(id, &token, initial.action_id, "  Personal account  ")
        .await
        .unwrap();
    assert_eq!(named.connection_name, "Personal account");
    assert_ne!(named.action_id, initial.action_id);
    assert!(
        service
            .set_connection_name(id, &token, initial.action_id, "Stale write")
            .await
            .is_err()
    );
    let complete = service
        .save_credentials(
            id,
            &token,
            named.action_id,
            values(&[("strange_secret-key", "private-key"), ("workspace", "work")]),
        )
        .await
        .unwrap();
    assert!(matches!(complete.action, Action::Complete));
    assert_eq!(
        service.get(started.connection.id).await.unwrap().name,
        "Personal account"
    );
    let credentials = service.resolve(started.connection.id).await.unwrap();
    assert!(!credentials.contains_key("name"));
    assert!(!credentials.contains_key("account_name"));
    assert!(
        service
            .set_connection_name(id, &token, complete.action_id, "After completion")
            .await
            .is_err()
    );
    db.close().await;
}

#[tokio::test]
async fn account_name_bindings_share_projection_validation_and_encrypted_staging() {
    let db = Database::new().await;
    let app = Router::new()
        .route(
            "/inboxes/assistant@agentmail.to",
            get(|| async { Json(json!({})) }),
        )
        .route("/phone-numbers", get(|| async { Json(json!({})) }))
        .route(
            "/messaging_profiles/profile",
            get(|| async { Json(json!({})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let service = Connections::with_endpoints(
        db.pool.clone(),
        Arc::new(Encryption::initialize(&db.pool, seed(7)).await.unwrap()),
        "http://127.0.0.1:18888".into(),
        "https://ingress.example".into(),
        Endpoints(BTreeMap::from([
            ("agentmail_api".into(), origin.clone()),
            ("linq_api".into(), origin.clone()),
            ("telnyx_api".into(), origin),
        ])),
    )
    .unwrap();
    service.seed().await.unwrap();
    let public_key = base64::engine::general_purpose::STANDARD.encode([1; 32]);
    for (provider, typ, field, account, credentials) in [
        (
            "agentmail",
            "inbox",
            "inbox_id",
            "assistant@agentmail.to",
            values(&[
                ("api_key", "inbox-scoped-key"),
                ("webhook_secret", "signing-secret"),
            ]),
        ),
        (
            "linq",
            "account",
            "phone_number",
            "+15550001111",
            values(&[
                ("api_token", "linq-key"),
                ("webhook_signing_secret", "signing-secret"),
            ]),
        ),
        (
            "telnyx",
            "whatsapp",
            "phone_number",
            "+15550002222",
            values(&[
                ("api_key", "telnyx-key"),
                ("messaging_profile_id", "profile"),
                ("public_key", public_key.as_str()),
            ]),
        ),
    ] {
        let start = service
            .start(Uuid::new_v4(), provider, provider, typ, &[])
            .await
            .unwrap();
        let (id, token) = parse_brokering_url(&start.brokering_url);
        let initial = service.view(id, &token).await.unwrap();
        if provider == "agentmail" {
            assert_eq!(initial.setup_instructions.len(), 4);
            assert!(initial.setup_instructions[2].contains("all received events"));
        }
        let schema = initial.input_schema.as_ref().unwrap();
        assert!(schema["properties"].get(field).is_none());
        assert!(
            !schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value.as_str() == Some(field))
        );
        assert_eq!(
            initial.webhook_url,
            Some(format!(
                "https://ingress.example/connections/webhooks/{}",
                start.connection.id
            ))
        );
        let mut action = initial.action_id;
        if field == "phone_number" {
            let invalid = service
                .set_connection_name(id, &token, action, "not-a-phone-number")
                .await
                .unwrap();
            assert!(
                service
                    .advance(id, &token, invalid.action_id, credentials.clone())
                    .await
                    .is_err()
            );
            let after = service.view(id, &token).await.unwrap();
            assert_eq!(after.step, "fields");
            action = after.action_id;
        }
        let named = service
            .set_connection_name(id, &token, action, account)
            .await
            .unwrap();
        let mut spoofed = credentials.clone();
        spoofed.insert(field.into(), SecretString::from("another-account"));
        assert!(
            service
                .advance(id, &token, named.action_id, spoofed)
                .await
                .is_err()
        );
        let completed = service
            .advance(id, &token, named.action_id, credentials)
            .await
            .unwrap();
        assert!(matches!(completed.action, Action::Complete));
        let resolved = service.resolve(start.connection.id).await.unwrap();
        assert_eq!(resolved[field].expose_secret(), account);
        drop(resolved);
    }
    server.abort();
    db.close().await;
}
