//! The signal-only providers across their boundaries: a signed webhook (Linear, Sentry, or a
//! CloudWatch alarm over SNS) is verified at the public ingress, normalized into signals, matched
//! against routine filters, and starts the routine's run, which the runtime worker delivers as a
//! wake to a dialled-in agent host. Linear's tools are exercised through an agent's catalog
//! against a fake GraphQL API.
mod common;
use base64::Engine;
use common::host::Gateway;
use hmac::{Hmac, Mac};
use secrecy::SecretString;
use serde_json::{Value, json};
use sha2::Sha256;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tilde::{
    connections::catalog::Endpoints,
    encryption::{Encryption, SecretBinding},
    routines::{Filter, RoutineInput, Trigger},
};
use uuid::Uuid;

const KEY: &str = include_str!("fixtures/connection-test-key.pem");
const CERTIFICATE: &str = include_str!("fixtures/signing-test-cert.pem");

async fn fixture(seed: u8, endpoints: Endpoints) -> Gateway {
    Gateway::start(seed, endpoints, Default::default()).await
}
async fn routine(
    f: &Gateway,
    connection: Uuid,
    signal_type: &str,
    prompt: &str,
    filters: &[(&str, &[&str])],
) -> Uuid {
    f.routines
        .create(f.agent, input(connection, signal_type, prompt, filters))
        .await
        .unwrap()
        .id
}
async fn post(f: &Gateway, connection: Uuid, headers: &[(&str, String)], body: &[u8]) -> u16 {
    f.deliver(connection, headers, body).await.0
}
fn input(
    connection: Uuid,
    signal_type: &str,
    prompt: &str,
    filters: &[(&str, &[&str])],
) -> RoutineInput {
    RoutineInput {
        name: signal_type.into(),
        prompt: prompt.into(),
        thread_title: String::new(),
        enabled: true,
        trigger: Trigger::Signal {
            connection_id: connection,
            signal_type: signal_type.into(),
            filters: filters
                .iter()
                .map(|(path, values)| Filter {
                    path: (*path).into(),
                    values: values.iter().map(|v| (*v).into()).collect(),
                })
                .collect(),
        },
    }
}
fn hmac_hex(key: &[u8], body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

/// Linear signs the body with the webhook's secret and dates it with `webhookTimestamp`; the
/// routine's filters pick the team and label it handles.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn linear_issue_webhooks_start_filtered_routines_on_the_agent_host() {
    let mut f = fixture(61, Endpoints::default()).await;
    let connection = f
        .connection(
            "linear",
            "api_key",
            &[
                ("api_key", "lin_api_fixture"),
                ("webhook_secret", "linear-secret"),
            ],
            &[],
        )
        .await;
    // Filters name dotted context paths.
    assert!(
        f.routines
            .create(
                f.agent,
                input(
                    connection,
                    "linear.issue.created",
                    "x",
                    &[("issue..team", &["ENG"])]
                ),
            )
            .await
            .is_err()
    );
    let routine = routine(
        &f,
        connection,
        "linear.issue.created",
        "Fix {{ issue.identifier }} for {{ issue.team.key }}.",
        &[("issue.team.key", &["ENG"]), ("issue.labels", &["Bug"])],
    )
    .await;
    let stored = f.routines.get(routine).await.unwrap();
    let Trigger::Signal { filters, .. } = stored.trigger else {
        panic!("signal trigger")
    };
    assert_eq!(filters.len(), 2);
    assert_eq!(filters[1].values, ["Bug"]);

    let issue = |team: &str, labels: &[&str], at: i64| {
        serde_json::to_vec(&json!({
            "action": "create",
            "type": "Issue",
            "actor": {"id": "u1", "name": "Ada", "email": "ada@acme.com", "type": "user"},
            "data": {
                "id": "issue-1", "identifier": format!("{team}-1"), "title": "Checkout fails",
                "url": format!("https://linear.app/acme/issue/{team}-1"), "priority": 1,
                "state": {"name": "Todo", "type": "unstarted"}, "team": {"key": team, "name": team},
                "labels": labels.iter().map(|l| json!({"id": l, "name": l})).collect::<Vec<_>>(),
            },
            "url": format!("https://linear.app/acme/issue/{team}-1"),
            "organizationId": "org-1",
            "webhookTimestamp": at,
            "webhookId": "webhook-1",
        }))
        .unwrap()
    };
    let now = || chrono::Utc::now().timestamp_millis();
    let signed = |body: &[u8], delivery: &str| {
        vec![
            ("linear-signature", hmac_hex(b"linear-secret", body)),
            ("linear-delivery", delivery.to_owned()),
            ("linear-event", "Issue".to_owned()),
        ]
    };
    let eng = issue("ENG", &["Bug", "Frontend"], now());
    assert_eq!(post(&f, connection, &[], &eng).await, 400);
    assert_eq!(
        post(
            &f,
            connection,
            &[("linear-signature", hmac_hex(b"wrong", &eng))],
            &eng
        )
        .await,
        400
    );
    // A correctly signed replay from five minutes ago is refused.
    let stale = issue("ENG", &["Bug"], now() - 300_000);
    assert_eq!(
        post(&f, connection, &signed(&stale, "d-stale"), &stale).await,
        400
    );
    // Another team's issue, and an unlabelled one, are filtered out.
    let ops = issue("OPS", &["Bug"], now());
    assert_eq!(
        post(&f, connection, &signed(&ops, "d-ops"), &ops).await,
        204
    );
    let unlabelled = issue("ENG", &[], now());
    assert_eq!(
        post(&f, connection, &signed(&unlabelled, "d-plain"), &unlabelled).await,
        204
    );
    assert_eq!(f.runs().await, 0);
    for _ in 0..2 {
        assert_eq!(
            post(&f, connection, &signed(&eng, "d-eng"), &eng).await,
            204
        );
    }
    let wake = f.wake().await;
    assert!(
        wake.objective.starts_with("Fix ENG-1 for ENG."),
        "{}",
        wake.objective
    );
    assert!(wake.objective.contains("`linear.issue.created`"));
    assert!(wake.objective.contains("\"Frontend\""));
    assert_eq!(f.runs().await, 1, "a redelivery starts no second run");
    f.close().await;
}

/// A Sentry internal integration's issue and alert webhooks: a regression is its own signal
/// beside the unresolve, and an alert rule firing is another.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sentry_regressions_and_alerts_reach_the_agent_host() {
    let mut f = fixture(62, Endpoints::default()).await;
    let connection = f
        .connection(
            "sentry",
            "token",
            &[
                ("auth_token", "sntrys_x"),
                ("client_secret", "integration-secret"),
            ],
            &[],
        )
        .await;
    routine(
        &f,
        connection,
        "sentry.issue.regressed",
        "Regressed: {{ data.issue.shortId }}.",
        &[("data.issue.project.slug", &["api"])],
    )
    .await;
    routine(
        &f,
        connection,
        "sentry.alert.triggered",
        "Alert {{ data.triggered_rule }} on issue {{ data.event.issue_id }}.",
        &[],
    )
    .await;
    let send = |resource: &'static str, request: &'static str, body: Value| {
        let body = serde_json::to_vec(&body).unwrap();
        let headers = vec![
            ("sentry-hook-resource", resource.to_owned()),
            ("request-id", request.to_owned()),
            (
                "sentry-hook-signature",
                hmac_hex(b"integration-secret", &body),
            ),
        ];
        (headers, body)
    };
    let unresolved = |substatus: &str| {
        json!({"action": "unresolved", "data": {"issue": {"id": "42", "shortId": "API-42",
            "title": "KeyError: user", "substatus": substatus, "project": {"slug": "api"}}}})
    };
    let (headers, body) = send("issue", "r-1", unresolved("ongoing"));
    assert_eq!(post(&f, connection, &headers, &body).await, 204);
    assert_eq!(f.runs().await, 0, "a manual unresolve is no regression");
    let (headers, body) = send("issue", "r-2", unresolved("regressed"));
    assert_eq!(post(&f, connection, &headers, &body).await, 204);
    let wake = f.wake().await;
    assert!(
        wake.objective.starts_with("Regressed: API-42."),
        "{}",
        wake.objective
    );
    assert!(wake.objective.contains("`sentry.issue.regressed`"));
    let (headers, body) = send(
        "event_alert",
        "r-3",
        json!({"action": "triggered", "data": {"triggered_rule": "New errors in prod",
            "event": {"issue_id": "42", "title": "KeyError: user", "web_url": "https://acme.sentry.io/issues/42/events/e1/"}}}),
    );
    assert_eq!(post(&f, connection, &headers, &body).await, 204);
    let wake = f.wake().await;
    assert!(
        wake.objective
            .starts_with("Alert New errors in prod on issue 42."),
        "{}",
        wake.objective
    );
    f.close().await;
}

/// SNS's canonical string for a message, signed as SNS does with SignatureVersion 2.
fn sns(fields: &[(&str, &str)], certificate_url: &str) -> Value {
    let order = [
        "Message",
        "MessageId",
        "Subject",
        "SubscribeURL",
        "Timestamp",
        "Token",
        "TopicArn",
        "Type",
    ];
    let mut canonical = String::new();
    for key in order {
        if let Some((_, value)) = fields.iter().find(|(k, _)| *k == key) {
            canonical.push_str(&format!("{key}\n{value}\n"));
        }
    }
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(KEY.as_bytes()).unwrap();
    let signature =
        jsonwebtoken::crypto::sign(canonical.as_bytes(), &key, jsonwebtoken::Algorithm::RS256)
            .unwrap();
    let signature = base64::engine::general_purpose::STANDARD.encode(
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(signature)
            .unwrap(),
    );
    let mut message = serde_json::Map::new();
    for (key, value) in fields {
        message.insert((*key).into(), (*value).into());
    }
    message.insert("SignatureVersion".into(), "2".into());
    message.insert("Signature".into(), signature.into());
    message.insert("SigningCertURL".into(), certificate_url.into());
    Value::Object(message)
}

/// SNS delivers a CloudWatch alarm: the subscription is confirmed automatically for an accepted
/// topic, every message's signature is checked against the certificate fetched from SNS's own
/// origin, and the alarm becomes a structured signal a routine filters on.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cloudwatch_alarms_over_sns_reach_the_agent_host() {
    let confirmed = Arc::new(Mutex::new(Vec::<String>::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let sns_origin = format!("http://{}", listener.local_addr().unwrap());
    let record = confirmed.clone();
    let sns_server = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new()
                .route(
                    "/SimpleNotificationService-test.pem",
                    axum::routing::get(|| async { CERTIFICATE }),
                )
                .route(
                    "/",
                    axum::routing::get(
                        move |axum::extract::RawQuery(query): axum::extract::RawQuery| {
                            let record = record.clone();
                            async move {
                                record.lock().unwrap().push(query.unwrap_or_default());
                                "<ConfirmSubscriptionResponse/>"
                            }
                        },
                    ),
                ),
        )
        .into_future(),
    );
    let mut f = fixture(
        63,
        Endpoints([("aws_sns".to_owned(), sns_origin.clone())].into()),
    )
    .await;
    let topic = "arn:aws:sns:eu-west-1:123456789012:alerts";
    let connection = f
        .connection(
            "aws",
            "iam",
            &[
                ("access_key_id", "AKIDFIXTURE"),
                ("secret_access_key", "secret-fixture"),
                ("region", "eu-west-1"),
                ("sns_topic_arns", topic),
            ],
            &[],
        )
        .await;
    routine(
        &f,
        connection,
        "aws.cloudwatch.alarm.triggered",
        "Investigate {{ alarm.name }} on {{ alarm.ecs_service }}: {{ alarm.reason }}",
        &[("alarm.ecs_service", &["api", "worker"])],
    )
    .await;
    let certificate = format!("{sns_origin}/SimpleNotificationService-test.pem");
    let deliver = |message: Value| {
        let body = serde_json::to_vec(&message).unwrap();
        let kind = message["Type"].as_str().unwrap().to_owned();
        let f = &f;
        async move { post(f, connection, &[("x-amz-sns-message-type", kind)], &body).await }
    };
    let subscribe =
        format!("{sns_origin}/?Action=ConfirmSubscription&TopicArn={topic}&Token=token-1");
    let confirmation = |topic: &str, certificate: &str| {
        sns(
            &[
                ("Type", "SubscriptionConfirmation"),
                ("MessageId", "m-confirm"),
                ("Token", "token-1"),
                ("TopicArn", topic),
                ("Message", "You have chosen to subscribe to the topic."),
                ("SubscribeURL", &subscribe),
                ("Timestamp", "2026-11-10T08:00:00.000Z"),
            ],
            certificate,
        )
    };
    // Another account's topic, and a certificate from anywhere but SNS, are refused.
    assert_eq!(
        deliver(confirmation(
            "arn:aws:sns:eu-west-1:999999999999:other",
            &certificate
        ))
        .await,
        400
    );
    assert_eq!(
        deliver(confirmation(topic, "https://attacker.example/cert.pem")).await,
        400
    );
    assert!(confirmed.lock().unwrap().is_empty());
    assert_eq!(deliver(confirmation(topic, &certificate)).await, 204);
    assert_eq!(confirmed.lock().unwrap().len(), 1);
    assert!(confirmed.lock().unwrap()[0].contains("Token=token-1"));

    let alarm = |state: &str, service: &str| {
        json!({
            "AlarmName": format!("{service}-5xx"),
            "AlarmDescription": "5xx above 1%",
            "AWSAccountId": "123456789012",
            "NewStateValue": state,
            "OldStateValue": "OK",
            "NewStateReason": "Threshold Crossed: 1 datapoint [3.0] was greater than the threshold (1.0).",
            "StateChangeTime": "2026-11-10T08:15:00.000+0000",
            "Region": "EU (Ireland)",
            "AlarmArn": format!("arn:aws:cloudwatch:eu-west-1:123456789012:alarm:{service}-5xx"),
            "Trigger": {
                "MetricName": "HTTPCode_Target_5XX_Count", "Namespace": "AWS/ApplicationELB",
                "Statistic": "SUM", "Period": 60, "EvaluationPeriods": 1,
                "ComparisonOperator": "GreaterThanThreshold", "Threshold": 1.0,
                "Dimensions": [
                    {"name": "ServiceName", "value": service},
                    {"name": "TargetGroup", "value": "targetgroup/api/0123456789abcdef"},
                ],
            },
        })
        .to_string()
    };
    let notification = |id: &str, message: &str| {
        sns(
            &[
                ("Type", "Notification"),
                ("MessageId", id),
                ("TopicArn", topic),
                ("Subject", "ALARM: api-5xx"),
                ("Message", message),
                ("Timestamp", "2026-11-10T08:15:01.000Z"),
            ],
            &certificate,
        )
    };
    let mut tampered = notification("m-0", &alarm("ALARM", "api"));
    tampered["Message"] = alarm("ALARM", "worker").into();
    assert_eq!(deliver(tampered).await, 400);
    // Recovery and an unfiltered service start nothing.
    assert_eq!(deliver(notification("m-1", &alarm("OK", "api"))).await, 204);
    assert_eq!(
        deliver(notification("m-2", &alarm("ALARM", "billing"))).await,
        204
    );
    assert_eq!(f.runs().await, 0);
    assert_eq!(
        deliver(notification("m-3", &alarm("ALARM", "api"))).await,
        204
    );
    let wake = f.wake().await;
    assert!(
        wake.objective
            .starts_with("Investigate api-5xx on api: Threshold Crossed"),
        "{}",
        wake.objective
    );
    for field in [
        "`aws.cloudwatch.alarm.triggered`",
        "\"namespace\": \"AWS/ApplicationELB\"",
        "\"target_group\": \"targetgroup/api/0123456789abcdef\"",
        "\"region\": \"eu-west-1\"",
    ] {
        assert!(
            wake.objective.contains(field),
            "{field}: {}",
            wake.objective
        );
    }
    f.idle(Duration::from_millis(500)).await;
    f.close().await;
    sns_server.abort();
}

/// Linear's tools through an agent's catalog: read an issue, comment, and move it by state name,
/// against a fake GraphQL API that checks the API key arrives unprefixed.
#[tokio::test]
async fn linear_tools_read_comment_and_move_issues() {
    use common::invocation::{Fixture, all_tools};
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let record = calls.clone();
    let graphql = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new().route(
                "/",
                axum::routing::post(
                    move |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| {
                        let record = record.clone();
                        async move {
                            assert_eq!(headers["authorization"], "lin_api_fixture");
                            record.lock().unwrap().push(body.clone());
                            let query = body["query"].as_str().unwrap();
                            let issue = json!({"id": "issue-1", "identifier": "ENG-1", "title": "Checkout fails",
                                "labels": {"nodes": [{"name": "Bug"}]}, "comments": {"nodes": []},
                                "state": {"id": "s-todo", "name": "Todo", "type": "unstarted"}});
                            axum::Json(if query.contains("commentCreate") {
                                json!({"data": {"commentCreate": {"success": true, "comment": {"id": "c-1", "url": "https://linear.app/c-1", "body": body["variables"]["input"]["body"]}}}})
                            } else if query.contains("issueUpdate") {
                                json!({"data": {"issueUpdate": {"success": true, "issue": {"id": "issue-1", "state": {"id": body["variables"]["input"]["stateId"], "name": "In Progress"}}}}})
                            } else if query.contains("states") {
                                json!({"data": {"issue": {"id": "issue-1", "team": {"states": {"nodes": [
                                    {"id": "s-todo", "name": "Todo", "type": "unstarted"},
                                    {"id": "s-doing", "name": "In Progress", "type": "started"},
                                ]}}}}})
                            } else {
                                json!({"data": {"issue": issue}})
                            })
                        }
                    },
                ),
            ),
        )
        .into_future(),
    );
    let fx = Fixture::new(
        64,
        Endpoints([("linear_api".to_owned(), origin)].into()),
        all_tools(),
    )
    .await;
    let pg = fx.db.pool.get().await.unwrap();
    let connection = Uuid::new_v4();
    pg.execute(
        "INSERT INTO connections(id,name,provider_id,type_id,status) VALUES($1,'acme','linear','api_key','ready')",
        &[&connection],
    )
    .await
    .unwrap();
    let sealed = Encryption::initialize(&fx.db.pool, common::seed(64))
        .await
        .unwrap()
        .seal(
            SecretBinding {
                resource_kind: "connection",
                resource_id: connection,
                name: "api_key",
            },
            &SecretString::from("lin_api_fixture"),
        )
        .unwrap()
        .into_bytes();
    pg.execute(
        "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,'api_key',$2)",
        &[&connection, &sealed],
    )
    .await
    .unwrap();
    let slug = fx
        .use_tools(
            connection,
            &["get_issue", "create_comment", "update_issue_state"],
        )
        .await;
    let issue = fx
        .output(&format!("{slug}.get_issue"), json!({"issue_id": "ENG-1"}))
        .await;
    assert_eq!(issue["labels"], json!(["Bug"]));
    let comment = fx
        .output(
            &format!("{slug}.create_comment"),
            json!({"issue_id": "issue-1", "body": "Fixed in #12"}),
        )
        .await;
    assert_eq!(comment["body"], "Fixed in #12");
    let moved = fx
        .output(
            &format!("{slug}.update_issue_state"),
            json!({"issue_id": "ENG-1", "state": "in progress"}),
        )
        .await;
    assert_eq!(moved["state"]["id"], "s-doing");
    let unknown = fx
        .invoke(
            &format!("{slug}.update_issue_state"),
            json!({"issue_id": "ENG-1", "state": "Shipped"}),
        )
        .await;
    assert!(
        unknown.to_string().contains("Todo, In Progress"),
        "{unknown}"
    );
    assert_eq!(calls.lock().unwrap().len(), 5);
    drop(pg);
    fx.close().await;
    graphql.abort();
}

/// One connection accepts alarm topics from several accounts and regions, so alarms anywhere
/// in an organisation reach the same agent; each still needs a valid SNS signature.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn alarms_from_two_accounts_and_regions_reach_one_agent() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let sns_origin = format!("http://{}", listener.local_addr().unwrap());
    let sns_server = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new().route(
                "/SimpleNotificationService-test.pem",
                axum::routing::get(|| async { CERTIFICATE }),
            ),
        )
        .into_future(),
    );
    let mut f = fixture(
        65,
        Endpoints([("aws_sns".to_owned(), sns_origin.clone())].into()),
    )
    .await;
    let production = "arn:aws:sns:eu-west-1:111111111111:alerts";
    let payments = "arn:aws:sns:us-east-1:222222222222:payments-alerts";
    let connection = f
        .connection(
            "aws",
            "oidc",
            &[
                ("role_arn", "arn:aws:iam::111111111111:role/tilde"),
                ("sns_topic_arns", &format!("{production},\n{payments}")),
            ],
            &[],
        )
        .await;
    routine(
        &f,
        connection,
        "aws.cloudwatch.alarm.triggered",
        "Investigate {{ alarm.name }} in {{ alarm.account_id }} ({{ alarm.region }}).",
        &[],
    )
    .await;
    let certificate = format!("{sns_origin}/SimpleNotificationService-test.pem");
    for (id, topic, account, region) in [
        ("m-a", production, "111111111111", "eu-west-1"),
        ("m-b", payments, "222222222222", "us-east-1"),
        (
            "m-c",
            "arn:aws:sns:us-east-1:333333333333:other",
            "333333333333",
            "us-east-1",
        ),
    ] {
        let alarm = json!({
            "AlarmName": format!("{account}-errors"), "AWSAccountId": account,
            "NewStateValue": "ALARM", "OldStateValue": "OK", "NewStateReason": "Threshold Crossed",
            "AlarmArn": format!("arn:aws:cloudwatch:{region}:{account}:alarm:{account}-errors"),
            "Trigger": {"MetricName": "Errors", "Namespace": "AWS/Lambda", "Dimensions": []},
        })
        .to_string();
        let message = sns(
            &[
                ("Type", "Notification"),
                ("MessageId", id),
                ("TopicArn", topic),
                ("Message", &alarm),
                ("Timestamp", "2026-11-10T08:15:01.000Z"),
            ],
            &certificate,
        );
        let status = post(&f, connection, &[], &serde_json::to_vec(&message).unwrap()).await;
        assert_eq!(status, if id == "m-c" { 400 } else { 204 }, "{topic}");
    }
    let mut objectives = [f.wake().await.objective, f.wake().await.objective];
    objectives.sort();
    assert!(
        objectives[0].starts_with("Investigate 111111111111-errors in 111111111111 (eu-west-1).")
    );
    assert!(
        objectives[1].starts_with("Investigate 222222222222-errors in 222222222222 (us-east-1).")
    );
    f.idle(Duration::from_millis(500)).await;
    f.close().await;
    sns_server.abort();
}
