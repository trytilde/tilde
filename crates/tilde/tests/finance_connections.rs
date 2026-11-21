//! Finance and accounting connections against a fake of each provider: setup through the shared
//! broker, the values a callback brings back (Plaid's Item, QuickBooks' realm, Xero's tenant),
//! the request credentials ducktail polls with (including a token refresh), and webhooks through
//! the connection webhook route: verification, Xero's intent-to-receive answers, and a Plaid
//! Item that needs its holder sending the connection through Reconnect in update mode.
mod common;
use aws_lc_rs::{
    rand::SystemRandom,
    signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair},
};
use axum::{Json, Router, extract::State, routing::post};
use base64::{Engine, engine::general_purpose::STANDARD, engine::general_purpose::URL_SAFE_NO_PAD};
use common::invocation::{Fixture, all_tools};
use hmac::{Hmac, Mac};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tilde::connections::{
    catalog::Endpoints,
    model::{Action, Values},
};
use uuid::Uuid;

/// Requests the fake saw, by path, and the Plaid webhook signing key.
#[derive(Clone)]
struct Fake {
    seen: Arc<Mutex<Vec<(String, Value)>>>,
    key: Arc<EcdsaKeyPair>,
}
impl Fake {
    fn bodies(&self, path: &str) -> Vec<Value> {
        let seen = self.seen.lock().unwrap();
        seen.iter()
            .filter(|(p, _)| p == path)
            .map(|(_, b)| b.clone())
            .collect()
    }
}

async fn record(fake: &Fake, path: &str, body: Value) {
    fake.seen.lock().unwrap().push((path.into(), body));
}

async fn serve() -> (String, Fake) {
    let fake = Fake {
        seen: Default::default(),
        key: Arc::new(EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING).unwrap()),
    };
    let plaid = |path: &'static str, answer: fn(&Fake, &Value) -> Value| {
        post(
            move |State(fake): State<Fake>, Json(body): Json<Value>| async move {
                assert!(body["client_id"] == "plaid-client" && body["secret"] == "plaid-secret");
                record(&fake, path, body.clone()).await;
                Json(answer(&fake, &body))
            },
        )
    };
    let form = |path: &'static str, answer: fn(&BTreeMap<String, String>) -> Value| {
        post(
            move |State(fake): State<Fake>,
                  axum::Form(body): axum::Form<BTreeMap<String, String>>| async move {
                record(&fake, path, json!(body)).await;
                Json(answer(&body))
            },
        )
    };
    let get = |path: &'static str, answer: Value| {
        axum::routing::get(
            move |State(fake): State<Fake>, headers: http::HeaderMap| async move {
                let auth = headers["authorization"].to_str().unwrap().to_owned();
                record(&fake, path, json!({"authorization": auth})).await;
                Json(answer)
            },
        )
    };
    let router = Router::new()
        .route(
            "/plaid/link/token/create",
            plaid("/link/token/create", |_, body| {
                let n = if body.get("access_token").is_some() { 2 } else { 1 };
                json!({"link_token": format!("link-sandbox-{n}"),
                    "hosted_link_url": format!("https://hosted.plaid.com/link/{n}"),
                    "expiration": "2026-11-14T12:00:00Z"})
            }),
        )
        .route(
            "/plaid/link/token/get",
            plaid("/link/token/get", |_, _| {
                json!({"link_sessions": [{"link_session_id": "s1", "results": {"item_add_results": [{
                    "public_token": "public-sandbox-1",
                    "institution": {"name": "First Platypus Bank", "institution_id": "ins_109508"},
                    "accounts": []}]}}]})
            }),
        )
        .route(
            "/plaid/item/public_token/exchange",
            plaid("/item/public_token/exchange", |_, body| {
                assert_eq!(body["public_token"], "public-sandbox-1");
                json!({"access_token": "access-sandbox-1", "item_id": "item-1"})
            }),
        )
        .route(
            "/plaid/item/get",
            plaid("/item/get", |_, _| json!({"item": {"item_id": "item-1", "error": null}})),
        )
        .route(
            "/plaid/webhook_verification_key/get",
            plaid("/webhook_verification_key/get", |fake, body| {
                assert_eq!(body["key_id"], "key-1");
                let point = fake.key.public_key().as_ref();
                json!({"key": {"alg": "ES256", "crv": "P-256", "kid": "key-1", "kty": "EC", "use": "sig",
                    "x": URL_SAFE_NO_PAD.encode(&point[1..33]), "y": URL_SAFE_NO_PAD.encode(&point[33..]),
                    "created_at": 1700000000, "expired_at": null}})
            }),
        )
        .route(
            "/intuit/token",
            form("/intuit/token", |body| match body["grant_type"].as_str() {
                // A short-lived first token, so the first credentials request refreshes it.
                "authorization_code" => {
                    json!({"access_token": "qbo-1", "refresh_token": "qbo-r1", "expires_in": 30, "token_type": "bearer"})
                }
                _ => {
                    assert_eq!(body["refresh_token"], "qbo-r1");
                    json!({"access_token": "qbo-2", "refresh_token": "qbo-r2", "expires_in": 3600, "token_type": "bearer"})
                }
            }),
        )
        .route(
            "/intuit/api/v3/company/9130357/companyinfo/9130357",
            get("/companyinfo", json!({"CompanyInfo": {"CompanyName": "Craig's Design and Landscaping"}})),
        )
        .route(
            "/xero/token",
            form("/xero/token", |_| {
                let claims = URL_SAFE_NO_PAD.encode(json!({"authentication_event_id": "event-2"}).to_string());
                json!({"access_token": format!("eyJhbGciOiJSUzI1NiJ9.{claims}.sig"), "refresh_token": "xero-r1",
                    "expires_in": 1800, "token_type": "Bearer"})
            }),
        )
        .route(
            "/xero/api/connections",
            get("/connections", json!([
                {"id": "c1", "authEventId": "event-1", "tenantId": "tenant-a", "tenantType": "ORGANISATION", "tenantName": "Earlier Org"},
                {"id": "c2", "authEventId": "event-2", "tenantId": "tenant-b", "tenantType": "ORGANISATION", "tenantName": "Demo Company (US)"}
            ])),
        )
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (origin, fake)
}

fn values(pairs: &[(&str, &str)]) -> Values {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), SecretString::from(*v)))
        .collect()
}

/// The setup ID and token of a brokering URL.
fn brokering(url: &str) -> (Uuid, String) {
    let url = url::Url::parse(url).unwrap();
    let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let token = url
        .query_pairs()
        .find(|(k, _)| k == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    (setup, token)
}

/// The secret half of a callback `state` (`<setup>.<secret>`) in `url`.
fn state(url: &str) -> String {
    let state = url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    state.split_once('.').unwrap().1.to_owned()
}

/// Posts a webhook to the connection webhook route; its status.
async fn deliver(
    router: &Router,
    connection: Uuid,
    headers: &[(&str, String)],
    body: &[u8],
) -> http::StatusCode {
    use tower::ServiceExt;
    let mut request = http::Request::post(format!("/connections/webhooks/{connection}"));
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    router
        .clone()
        .oneshot(request.body(axum::body::Body::from(body.to_vec())).unwrap())
        .await
        .unwrap()
        .status()
}

fn hmac_b64(key: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
    mac.update(body);
    STANDARD.encode(mac.finalize().into_bytes())
}

fn plaid_jwt(key: &EcdsaKeyPair, body: &[u8], iat: i64) -> String {
    let head =
        URL_SAFE_NO_PAD.encode(json!({"alg": "ES256", "kid": "key-1", "typ": "JWT"}).to_string());
    let claims = URL_SAFE_NO_PAD.encode(
        json!({"iat": iat, "request_body_sha256": hex::encode(Sha256::digest(body))}).to_string(),
    );
    let signed = format!("{head}.{claims}");
    let signature = key.sign(&SystemRandom::new(), signed.as_bytes()).unwrap();
    format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature.as_ref()))
}

async fn fixture(origin: &str) -> Fixture {
    let endpoints = Endpoints(BTreeMap::from(
        [
            ("plaid_api", format!("{origin}/plaid")),
            ("quickbooks_token", format!("{origin}/intuit/token")),
            ("quickbooks_authorize", format!("{origin}/intuit/authorize")),
            ("quickbooks_api", format!("{origin}/intuit/api")),
            ("xero_token", format!("{origin}/xero/token")),
            ("xero_authorize", format!("{origin}/xero/authorize")),
            ("xero_api", format!("{origin}/xero/api")),
        ]
        .map(|(k, v)| (k.to_owned(), v)),
    ));
    Fixture::new(81, endpoints, all_tools()).await
}

#[tokio::test]
async fn a_plaid_item_links_gives_credentials_and_relinks_in_update_mode() {
    let (origin, fake) = serve().await;
    let fx = fixture(&origin).await;
    let webhooks = tilde::chat::providers::ingress::router(fx.chat.clone(), fx.connections.clone());
    let connections = fx.connections.clone();
    let id = Uuid::new_v4();
    fx.within(async {
        let started = connections
            .start(id, "Checking", "plaid", "own_keys", &[])
            .await
            .unwrap();
        let (setup, token) = brokering(&started.brokering_url);
        let view = connections.view(setup, &token).await.unwrap();
        let keys = values(&[
            ("client_id", "plaid-client"),
            ("secret", "plaid-secret"),
            ("environment", "sandbox"),
        ]);
        let view = connections
            .advance(setup, &token, view.action_id, keys)
            .await
            .unwrap();
        let Action::Redirect { url } = view.action else {
            panic!("setup continues on Plaid's Hosted Link page")
        };
        assert_eq!(url, "https://hosted.plaid.com/link/1");
        // The link token asks for the products the pipelines read, sends the Item's webhooks to
        // this connection, and returns to the shared callback with the setup's state.
        let create = &fake.bodies("/link/token/create")[0];
        assert_eq!(create["products"], json!(["transactions"]));
        assert_eq!(create["optional_products"], json!(["investments", "liabilities"]));
        assert!(create["webhook"].as_str().unwrap().ends_with(&format!("/connections/webhooks/{id}")));
        let completion = create["hosted_link"]["completion_redirect_uri"].as_str().unwrap();
        connections
            .callback(setup, &state(completion), &Values::new(), false)
            .await
            .unwrap();
        let connection = connections.get(id).await.unwrap();
        assert_eq!(connection.status, "ready");
        assert_eq!(connection.account_label.as_deref(), Some("First Platypus Bank"));

        let values = connections.resolve(id).await.unwrap();
        assert!(!values.contains_key("_link_token"), "setup state is not kept");

        // A webhook signed for another body is refused with 401; the Item's login error, signed,
        // is accepted and asks the holder to reconnect.
        let body = json!({"webhook_type": "ITEM", "webhook_code": "ERROR", "item_id": "item-1",
            "error": {"error_type": "ITEM_ERROR", "error_code": "ITEM_LOGIN_REQUIRED"}, "environment": "sandbox"})
        .to_string();
        let now = chrono::Utc::now().timestamp();
        let forged = plaid_jwt(&fake.key, b"{}", now);
        assert_eq!(
            deliver(&webhooks, id, &[("plaid-verification", forged)], body.as_bytes()).await,
            http::StatusCode::UNAUTHORIZED
        );
        let stale = plaid_jwt(&fake.key, body.as_bytes(), now - 600);
        assert_eq!(
            deliver(&webhooks, id, &[("plaid-verification", stale)], body.as_bytes()).await,
            http::StatusCode::UNAUTHORIZED
        );
        let signed = plaid_jwt(&fake.key, body.as_bytes(), now);
        assert_eq!(
            deliver(&webhooks, id, &[("plaid-verification", signed)], body.as_bytes()).await,
            http::StatusCode::OK
        );
        assert_eq!(connections.get(id).await.unwrap().status, "requires_action");

        // Reconnect re-links the same Item: update mode with its token, no new exchange.
        let started = connections.reconnect(id).await.unwrap();
        let (setup, token) = brokering(&started.brokering_url);
        let view = connections.view(setup, &token).await.unwrap();
        assert_eq!(view.step, "plaid_update");
        let view = connections
            .advance(setup, &token, view.action_id, Values::new())
            .await
            .unwrap();
        let Action::Redirect { url } = view.action else { panic!("update mode opens Link") };
        assert_eq!(url, "https://hosted.plaid.com/link/2");
        let update = &fake.bodies("/link/token/create")[1];
        assert_eq!(update["access_token"], "access-sandbox-1");
        assert!(update.get("products").is_none());
        let completion = update["hosted_link"]["completion_redirect_uri"].as_str().unwrap();
        connections
            .callback(setup, &state(completion), &Values::new(), false)
            .await
            .unwrap();
        assert_eq!(connections.get(id).await.unwrap().status, "ready");
        assert_eq!(fake.bodies("/item/public_token/exchange").len(), 1);
        assert_eq!(
            connections.resolve(id).await.unwrap()["access_token"].expose_secret(),
            "access-sandbox-1"
        );
    })
    .await;
    fx.close().await;
}

#[tokio::test]
async fn quickbooks_keeps_its_realm_through_refresh_and_filters_webhooks_by_company() {
    let (origin, fake) = serve().await;
    let fx = fixture(&origin).await;
    let webhooks = tilde::chat::providers::ingress::router(fx.chat.clone(), fx.connections.clone());
    let connections = fx.connections.clone();
    let id = Uuid::new_v4();
    fx.within(async {
        let started = connections
            .start(id, "Books", "quickbooks", "oauth", &[])
            .await
            .unwrap();
        let (setup, token) = brokering(&started.brokering_url);
        let view = connections.view(setup, &token).await.unwrap();
        let input = values(&[
            ("client_id", "intuit-client"),
            ("client_secret", "intuit-secret"),
            ("environment", "sandbox"),
            ("webhook_verifier_token", "verifier"),
        ]);
        let view = connections
            .advance(setup, &token, view.action_id, input)
            .await
            .unwrap();
        let Action::Redirect { url } = view.action else {
            panic!("setup continues at Intuit")
        };
        assert!(url.starts_with(&format!("{origin}/intuit/authorize")));
        assert!(url.contains("scope=com.intuit.quickbooks.accounting"));
        connections
            .callback(
                setup,
                &state(&url),
                &values(&[("code", "auth-code"), ("realmId", "9130357")]),
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            connections.get(id).await.unwrap().account_label.as_deref(),
            Some("Craig's Design and Landscaping")
        );

        // The authorization code was exchanged once.
        assert_eq!(fake.bodies("/intuit/token").len(), 1);

        let ours = json!({"eventNotifications": [{"realmId": "9130357",
            "dataChangeEvent": {"entities": [{"name": "Invoice", "id": "130", "operation": "Update",
                "lastUpdated": "2026-11-14T10:00:00.000Z"}]}}]})
        .to_string();
        let sign = |body: &str| vec![("intuit-signature", hmac_b64("verifier", body.as_bytes()))];
        assert_eq!(
            deliver(&webhooks, id, &sign(&ours), ours.as_bytes()).await,
            http::StatusCode::OK
        );
        let tampered = ours.replace("Update", "Delete");
        assert_eq!(
            deliver(&webhooks, id, &sign(&ours), tampered.as_bytes()).await,
            http::StatusCode::UNAUTHORIZED
        );
        // Another company on the same app is acknowledged (Intuit would retry otherwise).
        let theirs = ours.replace("9130357", "4620816365");
        assert_eq!(
            deliver(&webhooks, id, &sign(&theirs), theirs.as_bytes()).await,
            http::StatusCode::OK
        );
    })
    .await;
    fx.close().await;
}

#[tokio::test]
async fn xero_picks_the_tenant_of_the_sign_in_and_answers_intent_to_receive() {
    let (origin, _fake) = serve().await;
    let fx = fixture(&origin).await;
    let webhooks = tilde::chat::providers::ingress::router(fx.chat.clone(), fx.connections.clone());
    let connections = fx.connections.clone();
    let id = Uuid::new_v4();
    fx.within(async {
        let started = connections.start(id, "Ledger", "xero", "oauth", &[]).await.unwrap();
        let (setup, token) = brokering(&started.brokering_url);
        let view = connections.view(setup, &token).await.unwrap();
        let input = values(&[
            ("client_id", "xero-client"),
            ("client_secret", "xero-secret"),
            ("webhook_key", "xero-webhook-key"),
        ]);
        let view = connections.advance(setup, &token, view.action_id, input).await.unwrap();
        let Action::Redirect { url } = view.action else { panic!("setup continues at Xero") };
        assert!(url.contains("accounting.journals.read"));
        connections
            .callback(setup, &state(&url), &values(&[("code", "auth-code")]), false)
            .await
            .unwrap();
        // Two organisations reach the token; the one authorized in this sign-in is the connection's.
        assert_eq!(
            connections.get(id).await.unwrap().account_label.as_deref(),
            Some("Demo Company (US)")
        );

        // Intent to receive: a correctly signed probe gets 200, an incorrectly signed one 401.
        let probe = json!({"events": [], "firstEventSequence": 0, "lastEventSequence": 0, "entropy": "S0m3r4Nd0mt3xt"})
            .to_string();
        let good = vec![("x-xero-signature", hmac_b64("xero-webhook-key", probe.as_bytes()))];
        let bad = vec![("x-xero-signature", hmac_b64("another-key", probe.as_bytes()))];
        assert_eq!(deliver(&webhooks, id, &good, probe.as_bytes()).await, http::StatusCode::OK);
        assert_eq!(deliver(&webhooks, id, &bad, probe.as_bytes()).await, http::StatusCode::UNAUTHORIZED);
    })
    .await;
    fx.close().await;
}
