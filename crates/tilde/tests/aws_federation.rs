//! AWS tool credentials across their boundaries, against fake STS and AWS MCP endpoints: an
//! agent's tool call in another account assumes the connection's listed role (once, then from
//! cache) and an unlisted account is refused; two AWS connections are two tool sources; and an
//! `oidc` connection federates with Tilde's own OIDC issuer (discovery, JWKS, a token STS can
//! verify, `AssumeRoleWithWebIdentity`) before chaining into another account.
mod common;
use axum::response::IntoResponse;
use common::invocation::{Fixture, all_tools};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};
use tilde::{
    connections::catalog::Endpoints,
    encryption::{Encryption, SecretBinding},
};
use uuid::Uuid;

/// What a fake saw: STS calls as (form parameters, signing access key), MCP calls as
/// (signing access key, session token, call parameters).
#[derive(Default)]
struct Seen {
    sts: Vec<(HashMap<String, String>, Option<String>)>,
    mcp: Vec<(String, Option<String>, Value)>,
}
type Shared = Arc<Mutex<Seen>>;
/// The access key in a SigV4 `Authorization` header.
fn signing_key(headers: &axum::http::HeaderMap) -> Option<String> {
    let authorization = headers.get("authorization")?.to_str().ok()?;
    Some(
        authorization
            .split("Credential=")
            .nth(1)?
            .split('/')
            .next()?
            .to_owned(),
    )
}
fn credentials_xml(action: &str, key: &str) -> String {
    let expires = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    format!(
        "<{action}Response><{action}Result><Credentials><AccessKeyId>{key}</AccessKeyId><SecretAccessKey>secret-{key}</SecretAccessKey><SessionToken>token-{key}</SessionToken><Expiration>{expires}</Expiration></Credentials></{action}Result></{action}Response>"
    )
}
async fn upstreams(seen: Shared) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let sts_seen = seen.clone();
    let sts = axum::routing::post(
        move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
            let seen = sts_seen.clone();
            async move {
                let form: HashMap<String, String> =
                    url::form_urlencoded::parse(&body).into_owned().collect();
                let signer = signing_key(&headers);
                seen.lock()
                    .unwrap()
                    .sts
                    .push((form.clone(), signer.clone()));
                let action = form["Action"].as_str();
                match action {
                    "GetCallerIdentity" => {
                        let account = match signer.as_deref() {
                            Some("AKIDSTAGING") => "999999999999",
                            _ => "111111111111",
                        };
                        format!("<GetCallerIdentityResponse><GetCallerIdentityResult><Account>{account}</Account></GetCallerIdentityResult></GetCallerIdentityResponse>").into_response()
                    }
                    "AssumeRole" => {
                        let account = form["RoleArn"].split(':').nth(4).unwrap().to_owned();
                        credentials_xml(action, &format!("ASIAROLE{account}")).into_response()
                    }
                    "AssumeRoleWithWebIdentity" => {
                        credentials_xml(action, "ASIAWEB").into_response()
                    }
                    _ => axum::http::StatusCode::BAD_REQUEST.into_response(),
                }
            }
        },
    );
    let mcp_seen = seen.clone();
    let mcp = axum::routing::post(
        move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
            let seen = mcp_seen.clone();
            async move {
                let body: Value = serde_json::from_slice(&body).unwrap();
                let reply = |result: Value| {
                    axum::Json(json!({"jsonrpc":"2.0","id":body["id"],"result":result}))
                };
                match body["method"].as_str().unwrap() {
                    "initialize" => ([("mcp-session-id", "aws-1")], reply(json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"aws","version":"1"}}))).into_response(),
                    "notifications/initialized" => axum::http::StatusCode::ACCEPTED.into_response(),
                    "tools/call" => {
                        seen.lock().unwrap().mcp.push((
                            signing_key(&headers).unwrap(),
                            headers.get("x-amz-security-token").map(|t| t.to_str().unwrap().to_owned()),
                            body["params"].clone(),
                        ));
                        reply(json!({"content":[{"type":"text","text":"{\"regions\":[\"eu-west-1\"]}"}]})).into_response()
                    }
                    _ => axum::http::StatusCode::BAD_REQUEST.into_response(),
                }
            }
        },
    );
    let app = axum::Router::new().route("/sts", sts).route("/mcp", mcp);
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (origin, server)
}
async fn fixture(seed: u8, origin: &str) -> Fixture {
    Fixture::new(
        seed,
        Endpoints(BTreeMap::from([
            ("aws_mcp".into(), format!("{origin}/mcp")),
            ("aws_sts".into(), format!("{origin}/sts")),
        ])),
        all_tools(),
    )
    .await
}
async fn connection(
    fx: &Fixture,
    seed: u8,
    name: &str,
    typ: &str,
    values: &[(&str, &str)],
) -> Uuid {
    let id = Uuid::new_v4();
    fx.within(async {
    let pg = fx.db.pool.get().await.unwrap();
    pg.execute(
        "INSERT INTO connections(id,name,provider_id,type_id,status) VALUES($1,$2,'aws',$3,'ready')",
        &[&id, &name, &typ],
    )
    .await
    .unwrap();
    // A second handle on the installation's keys.
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(seed))
        .await
        .unwrap();
    for (key, secret) in values {
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: id,
                    name: key,
                },
                &SecretString::from(secret.to_string()),
            )
            .unwrap()
            .into_bytes();
        pg.execute(
            "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)",
            &[&id, key, &sealed],
        )
        .await
        .unwrap();
    }
    })
    .await;
    id
}
fn assume_roles(seen: &Shared) -> Vec<(HashMap<String, String>, Option<String>)> {
    seen.lock()
        .unwrap()
        .sts
        .iter()
        .filter(|(form, _)| form["Action"] == "AssumeRole")
        .cloned()
        .collect()
}

#[tokio::test]
async fn tool_calls_assume_listed_roles_in_other_accounts_and_connections_stay_apart() {
    let seen = Shared::default();
    let (origin, server) = upstreams(seen.clone()).await;
    let fx = fixture(91, &origin).await;
    let prod = connection(
        &fx, 91,
        "prod",
        "iam",
        &[
            ("access_key_id", "AKIDBASE"),
            ("secret_access_key", "secret-base"),
            ("region", "eu-west-1"),
            (
                "assume_role_arns",
                "arn:aws:iam::222222222222:role/tilde-agent, arn:aws:iam::333333333333:role/ops/tilde",
            ),
            ("external_id", "acme-external"),
        ],
    )
    .await;
    let staging = connection(
        &fx,
        91,
        "staging",
        "iam",
        &[
            ("access_key_id", "AKIDSTAGING"),
            ("secret_access_key", "secret-staging"),
        ],
    )
    .await;
    let tools = ["aws_list_regions", "aws_list_accounts"];
    let prod_slug = fx.use_tools(prod, &tools).await;
    let staging_slug = fx.use_tools(staging, &tools).await;
    assert_ne!(
        prod_slug, staging_slug,
        "two AWS connections are two sources"
    );
    let listed = fx.tools().await;
    for slug in [&prod_slug, &staging_slug] {
        let regions = listed
            .iter()
            .find(|t| t["name"] == format!("{slug}.aws_list_regions"))
            .unwrap_or_else(|| panic!("{slug} in {listed:?}"));
        assert!(
            regions["inputSchemaJson"]
                .as_str()
                .unwrap()
                .contains("account_id")
        );
    }
    let call = |slug: &str, tool: &str, input: Value| {
        let name = format!("{slug}.{tool}");
        let fx = &fx;
        async move { fx.invoke(&name, input).await }
    };

    // The agent learns what it can reach.
    let reach: Value = serde_json::from_str(
        call(&prod_slug, "aws_list_accounts", json!({})).await["outputJson"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(reach["default_account"], "111111111111");
    assert_eq!(
        reach["accounts"][1],
        json!({"account_id":"333333333333","role_arn":"arn:aws:iam::333333333333:role/ops/tilde"})
    );

    // Its own account (by default or by ID) uses the connection's keys.
    for input in [json!({}), json!({"account_id":"111111111111"})] {
        let result = call(&prod_slug, "aws_list_regions", input).await;
        assert!(result.get("outputJson").is_some(), "{result}");
    }
    assert!(assume_roles(&seen).is_empty());
    // Another listed account: one AssumeRole signed with the base keys, then the cache.
    for _ in 0..2 {
        let result = call(
            &prod_slug,
            "aws_list_regions",
            json!({"account_id":"222222222222"}),
        )
        .await;
        assert!(result.get("outputJson").is_some(), "{result}");
    }
    let assumed = assume_roles(&seen);
    assert_eq!(assumed.len(), 1, "the second call reuses the session");
    let (form, signer) = &assumed[0];
    assert_eq!(
        form["RoleArn"],
        "arn:aws:iam::222222222222:role/tilde-agent"
    );
    assert_eq!(form["RoleSessionName"], format!("tilde-{prod}"));
    assert_eq!(form["ExternalId"], "acme-external");
    assert_eq!(signer.as_deref(), Some("AKIDBASE"));
    // An account it lists no role for is refused before anything is called.
    let refused = call(
        &prod_slug,
        "aws_list_regions",
        json!({"account_id":"444444444444"}),
    )
    .await;
    assert!(refused.get("outputJson").is_none());
    assert!(refused.to_string().contains("not reachable"), "{refused}");
    // The other connection signs with its own keys.
    let result = call(&staging_slug, "aws_list_regions", json!({})).await;
    assert!(result.get("outputJson").is_some(), "{result}");

    let mcp = seen.lock().unwrap().mcp.clone();
    let keys = mcp.iter().map(|(key, token, params)| {
        assert!(
            params["arguments"].get("account_id").is_none(),
            "account_id stays with Tilde"
        );
        (key.as_str(), token.as_deref())
    });
    assert_eq!(
        keys.collect::<Vec<_>>(),
        [
            ("AKIDBASE", None),
            ("AKIDBASE", None),
            ("ASIAROLE222222222222", Some("token-ASIAROLE222222222222")),
            ("ASIAROLE222222222222", Some("token-ASIAROLE222222222222")),
            ("AKIDSTAGING", None),
        ]
    );
    fx.close().await;
    server.abort();
}

#[tokio::test]
async fn oidc_connections_federate_with_tildes_issuer_then_chain_roles() {
    let seen = Shared::default();
    let (origin, server) = upstreams(seen.clone()).await;
    let fx = fixture(92, &origin).await;
    // The issuer's discovery document and JWKS are public on the event ingress.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ingress = format!("http://{}", listener.local_addr().unwrap());
    let router =
        tilde::iam::listeners::public_event_ingress_router(fx.chat.clone(), fx.connections.clone());
    let ingress_server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let http = reqwest::Client::new();
    let issuer = fx.connections.oidc().url().to_owned();
    let discovery: Value = http
        .get(format!("{ingress}/oidc/.well-known/openid-configuration"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(discovery["issuer"], issuer);
    assert_eq!(discovery["jwks_uri"], format!("{issuer}/jwks"));
    assert_eq!(
        discovery["id_token_signing_alg_values_supported"],
        json!(["RS256"])
    );

    let connection = connection(
        &fx,
        92,
        "federated",
        "oidc",
        &[
            ("role_arn", "arn:aws:iam::555555555555:role/tilde-web"),
            (
                "assume_role_arns",
                "arn:aws:iam::666666666666:role/tilde-agent",
            ),
        ],
    )
    .await;
    let slug = fx.use_tools(connection, &["aws_list_regions"]).await;
    for account in [None, Some("666666666666")] {
        let input = account.map_or(json!({}), |a| json!({"account_id": a}));
        let result = fx.invoke(&format!("{slug}.aws_list_regions"), input).await;
        assert!(result.get("outputJson").is_some(), "{result}");
    }
    let (web, chained) = {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.sts.len(), 2);
        (seen.sts[0].clone(), seen.sts[1].clone())
    };
    // The web identity exchange is unsigned and carries Tilde's token for this connection.
    let (form, signer) = web;
    assert_eq!(form["Action"], "AssumeRoleWithWebIdentity");
    assert_eq!(signer, None);
    assert_eq!(form["RoleArn"], "arn:aws:iam::555555555555:role/tilde-web");
    assert_eq!(form["RoleSessionName"], format!("tilde-{connection}"));
    let token = &form["WebIdentityToken"];
    let jwks: jsonwebtoken::jwk::JwkSet = http
        .get(format!("{ingress}/oidc/jwks"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let kid = jsonwebtoken::decode_header(token).unwrap().kid.unwrap();
    let jwk = jwks.find(&kid).expect("the token's key is published");
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_audience(&["sts.amazonaws.com"]);
    validation.set_issuer(&[issuer.as_str()]);
    let claims = jsonwebtoken::decode::<Value>(
        token,
        &jsonwebtoken::DecodingKey::from_jwk(jwk).unwrap(),
        &validation,
    )
    .unwrap()
    .claims;
    assert_eq!(claims["sub"], format!("connection:{connection}"));
    assert!(claims["exp"].as_i64().unwrap() - claims["iat"].as_i64().unwrap() <= 300);
    // The other account's role is assumed with the federated session.
    let (form, signer) = chained;
    assert_eq!(form["Action"], "AssumeRole");
    assert_eq!(
        form["RoleArn"],
        "arn:aws:iam::666666666666:role/tilde-agent"
    );
    assert_eq!(signer.as_deref(), Some("ASIAWEB"));
    assert_eq!(seen.lock().unwrap().mcp[1].0, "ASIAROLE666666666666");

    // Rotation: the new key signs, the old one stays published for tokens already issued.
    fx.connections.oidc().rotate().await.unwrap();
    let rotated = fx
        .connections
        .oidc()
        .mint("connection:x", "sts.amazonaws.com")
        .await
        .unwrap();
    let new_kid = jsonwebtoken::decode_header(rotated.expose_secret())
        .unwrap()
        .kid
        .unwrap();
    assert_ne!(new_kid, kid);
    let published = fx.connections.oidc().jwks().await.unwrap();
    let kids = published["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["kid"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(kids, [new_kid, kid]);
    fx.close().await;
    server.abort();
    ingress_server.abort();
}
