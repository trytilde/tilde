//! CloudWatch alarm state changes delivered by an SNS topic's HTTPS subscription.
//!
//! SNS signs every message with a certificate it publishes at `SigningCertURL`; [`verify`] only
//! fetches it from `https://sns.<region>.amazonaws.com/…pem`, the region being the topic's, and
//! checks the RSA signature (SHA-1 for `SignatureVersion` 1, SHA-256 for 2) over SNS's canonical
//! string. A valid signature proves SNS sent the message, not that the topic is the customer's,
//! so the connection also names the topics it accepts (`sns_topic_arns`); without them
//! deliveries are refused. The subscription confirmation SNS sends first is answered here by
//! visiting its `SubscribeURL`, held to the same origin.
//!
//! The `aws_sns` endpoint override (tests) replaces that origin for certificates and
//! confirmations alike.
use super::*;
use crate::chat::providers::ingress::{certificate_rsa_key, published};
use base64::Engine;
use serde_json::json;

pub struct Aws;

const TITLE: &str = "{{ alarm.name }} is {{ alarm.state }}";
const STATES: &[(&str, &str, &str, &str)] = &[
    (
        "ALARM",
        "triggered",
        "Alarm triggered",
        "A CloudWatch alarm entered the ALARM state.",
    ),
    (
        "OK",
        "resolved",
        "Alarm resolved",
        "A CloudWatch alarm returned to the OK state.",
    ),
    (
        "INSUFFICIENT_DATA",
        "insufficient_data",
        "Alarm has insufficient data",
        "A CloudWatch alarm entered the INSUFFICIENT_DATA state.",
    ),
];

impl Source for Aws {
    fn types(&self) -> Vec<SignalType> {
        STATES
            .iter()
            .map(|(_, action, name, description)| SignalType {
                id: format!("aws.cloudwatch.alarm.{action}"),
                name: (*name).into(),
                description: (*description).into(),
                default_thread_title: TITLE.into(),
            })
            .collect()
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "alarm.name",
                description: "Alarm name.",
                example: "api-5xx-rate",
            },
            Variable {
                key: "alarm.description",
                description: "Alarm description, when set.",
                example: "5xx responses above 1% for 5 minutes",
            },
            Variable {
                key: "alarm.state",
                description: "New state: ALARM, OK or INSUFFICIENT_DATA.",
                example: "ALARM",
            },
            Variable {
                key: "alarm.previous_state",
                description: "State before this change.",
                example: "OK",
            },
            Variable {
                key: "alarm.reason",
                description: "CloudWatch's explanation of the change.",
                example: "Threshold Crossed: 1 datapoint [3.2] was greater than the threshold (1.0).",
            },
            Variable {
                key: "alarm.changed_at",
                description: "When the state changed.",
                example: "2026-11-10T08:15:00.000+0000",
            },
            Variable {
                key: "alarm.arn",
                description: "Alarm ARN.",
                example: "arn:aws:cloudwatch:eu-west-1:123456789012:alarm:api-5xx-rate",
            },
            Variable {
                key: "alarm.account_id",
                description: "AWS account ID.",
                example: "123456789012",
            },
            Variable {
                key: "alarm.region",
                description: "Region code of the alarm.",
                example: "eu-west-1",
            },
            Variable {
                key: "alarm.namespace",
                description: "Metric namespace.",
                example: "AWS/ApplicationELB",
            },
            Variable {
                key: "alarm.metric_name",
                description: "Metric name.",
                example: "HTTPCode_Target_5XX_Count",
            },
            Variable {
                key: "alarm.statistic",
                description: "Statistic evaluated.",
                example: "Sum",
            },
            Variable {
                key: "alarm.comparison",
                description: "Comparison operator.",
                example: "GreaterThanThreshold",
            },
            Variable {
                key: "alarm.threshold",
                description: "Threshold.",
                example: "1",
            },
            Variable {
                key: "alarm.dimensions.ServiceName",
                description: "A metric dimension by name: ServiceName and ClusterName for ECS, LoadBalancer and TargetGroup for an ALB, FunctionName for Lambda.",
                example: "api",
            },
            Variable {
                key: "alarm.ecs_service",
                description: "ECS service (ServiceName dimension).",
                example: "api",
            },
            Variable {
                key: "alarm.ecs_cluster",
                description: "ECS cluster (ClusterName dimension).",
                example: "production",
            },
            Variable {
                key: "alarm.load_balancer",
                description: "ALB (LoadBalancer dimension).",
                example: "app/prod-alb/0123456789abcdef",
            },
            Variable {
                key: "alarm.target_group",
                description: "ALB target group (TargetGroup dimension).",
                example: "targetgroup/api/0123456789abcdef",
            },
            Variable {
                key: "topic_arn",
                description: "SNS topic that delivered the alarm.",
                example: "arn:aws:sns:eu-west-1:123456789012:alerts",
            },
        ]
    }
    fn signals(&self, _: &Access, _: &HeaderMap, p: &Value) -> Vec<Signal> {
        if str_at(p, "/Type") != Some("Notification") {
            return vec![];
        }
        // CloudWatch publishes the alarm as JSON text; other messages on the topic emit nothing.
        let Some(alarm) = str_at(p, "/Message")
            .and_then(|m| serde_json::from_str::<Value>(m).ok())
            .filter(|m| m.get("AlarmName").is_some() && m.get("NewStateValue").is_some())
        else {
            return vec![];
        };
        let state = alarm["NewStateValue"].as_str().unwrap_or_default();
        let Some((_, action, ..)) = STATES.iter().find(|(s, ..)| *s == state) else {
            return vec![];
        };
        let name = alarm["AlarmName"].as_str().unwrap_or_default();
        let data = json!({
            "alarm": normalize(&alarm),
            "topic_arn": p["TopicArn"],
            "message_id": p["MessageId"],
        });
        vec![Signal {
            event_id: str_at(p, "/MessageId")
                .map(str::to_owned)
                .unwrap_or_else(|| hashed("sns", p)),
            signal_type: format!("aws.cloudwatch.alarm.{action}"),
            summary: format!("CloudWatch alarm {name} is {state}"),
            data,
        }]
    }
}

/// CloudWatch's alarm message as structured fields. Dimensions become an object by name; the
/// common ECS and ALB ones are also lifted to named fields. A metric-math alarm reports the first
/// metric it reads.
fn normalize(alarm: &Value) -> Value {
    let trigger = &alarm["Trigger"];
    let metric = trigger["Metrics"]
        .as_array()
        .and_then(|metrics| metrics.iter().find(|m| m["MetricStat"].is_object()))
        .map(|m| &m["MetricStat"]["Metric"]);
    let namespace = trigger["Namespace"]
        .as_str()
        .or_else(|| metric.and_then(|m| m["Namespace"].as_str()));
    let metric_name = trigger["MetricName"]
        .as_str()
        .or_else(|| metric.and_then(|m| m["MetricName"].as_str()));
    let dimension_list = trigger["Dimensions"]
        .as_array()
        .or_else(|| metric.and_then(|m| m["Dimensions"].as_array()))
        .cloned()
        .unwrap_or_default();
    let mut dimensions = serde_json::Map::new();
    for d in &dimension_list {
        // CloudWatch writes `name`/`value`; metric-math metrics use `Name`/`Value`.
        let name = d["name"].as_str().or_else(|| d["Name"].as_str());
        let value = d["value"].as_str().or_else(|| d["Value"].as_str());
        if let (Some(name), Some(value)) = (name, value) {
            dimensions.insert(name.into(), value.into());
        }
    }
    let arn = alarm["AlarmArn"].as_str().unwrap_or_default();
    let region = arn.split(':').nth(3).unwrap_or_default();
    json!({
        "name": alarm["AlarmName"],
        "description": alarm["AlarmDescription"],
        "state": alarm["NewStateValue"],
        "previous_state": alarm["OldStateValue"],
        "reason": alarm["NewStateReason"],
        "changed_at": alarm["StateChangeTime"],
        "arn": arn,
        "account_id": alarm["AWSAccountId"],
        "region": region,
        "namespace": namespace,
        "metric_name": metric_name,
        "statistic": trigger["Statistic"].as_str().or_else(|| trigger["ExtendedStatistic"].as_str()),
        "comparison": trigger["ComparisonOperator"],
        "threshold": trigger["Threshold"],
        "period": trigger["Period"],
        "evaluation_periods": trigger["EvaluationPeriods"],
        "dimensions": dimensions,
        "ecs_service": dimensions.get("ServiceName"),
        "ecs_cluster": dimensions.get("ClusterName"),
        "load_balancer": dimensions.get("LoadBalancer"),
        "target_group": dimensions.get("TargetGroup"),
    })
}

/// Authenticates an SNS delivery and answers a subscription confirmation for an accepted topic.
pub async fn verify(access: &Access, body: &[u8]) -> ToolResult<()> {
    let refused = |message: &'static str| ConnectError::unauthenticated(message);
    let p: Value = serde_json::from_slice(body).map_err(|_| refused("Invalid SNS message"))?;
    let field = |key: &str| p[key].as_str();
    let kind = field("Type").ok_or_else(|| refused("Invalid SNS message"))?;
    let topic = field("TopicArn").ok_or_else(|| refused("Invalid SNS message"))?;
    let accepted = access
        .secret("sns_topic_arns")
        .map_err(|_| refused("Signals are not configured"))?;
    if !accepted
        .split([',', '\n', ' '])
        .any(|arn| arn.trim() == topic)
    {
        return Err(ConnectError::permission_denied("SNS topic is not accepted"));
    }
    let region = topic
        .split(':')
        .nth(3)
        .filter(|r| !r.is_empty())
        .ok_or_else(|| refused("Invalid SNS topic"))?;
    let origin = sns_origin(access, region);
    let certificate = field("SigningCertURL").ok_or_else(|| refused("Missing SNS certificate"))?;
    if !trusted(&origin, certificate) || !certificate.ends_with(".pem") {
        return Err(refused("Untrusted SNS certificate"));
    }
    let keys: &[&str] = match kind {
        "Notification" => &[
            "Message",
            "MessageId",
            "Subject",
            "Timestamp",
            "TopicArn",
            "Type",
        ],
        "SubscriptionConfirmation" | "UnsubscribeConfirmation" => &[
            "Message",
            "MessageId",
            "SubscribeURL",
            "Timestamp",
            "Token",
            "TopicArn",
            "Type",
        ],
        _ => return Err(refused("Unknown SNS message type")),
    };
    // Each present field as `name\nvalue\n`, in this order; only Subject is optional.
    let mut canonical = String::new();
    for key in keys {
        match field(key) {
            Some(value) => canonical.push_str(&format!("{key}\n{value}\n")),
            None if *key == "Subject" => {}
            None => return Err(refused("Invalid SNS message")),
        }
    }
    let algorithm: &'static dyn ring::signature::VerificationAlgorithm =
        match field("SignatureVersion") {
            Some("1") => &ring::signature::RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY,
            Some("2") => &ring::signature::RSA_PKCS1_2048_8192_SHA256,
            _ => return Err(refused("Unsupported SNS signature version")),
        };
    let signature = field("Signature")
        .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
        .ok_or_else(|| refused("Invalid SNS signature"))?;
    let pem = published(
        &access.http.client,
        certificate,
        std::time::Duration::from_secs(24 * 3600),
    )
    .await?;
    let key = certificate_rsa_key(&pem)?;
    ring::signature::UnparsedPublicKey::new(algorithm, key)
        .verify(canonical.as_bytes(), &signature)
        .map_err(|_| refused("Invalid SNS signature"))?;
    if kind == "SubscriptionConfirmation" {
        let subscribe = field("SubscribeURL").unwrap_or_default();
        if !trusted(&origin, subscribe) {
            return Err(refused("Untrusted SNS subscription URL"));
        }
        let response = access
            .http
            .client
            .get(subscribe)
            .send()
            .await
            .map_err(|_| ConnectError::unavailable("SNS subscription could not be confirmed"))?;
        if !response.status().is_success() {
            return Err(ConnectError::unavailable(
                "SNS subscription could not be confirmed",
            ));
        }
        tracing::info!(connection_id=%access.connection_id, topic, "Confirmed SNS subscription");
    }
    Ok(())
}
fn sns_origin(access: &Access, region: &str) -> String {
    if let Some(origin) = access.endpoints.0.get("aws_sns") {
        return origin.trim_end_matches('/').to_owned();
    }
    let suffix = if region.starts_with("cn-") {
        "amazonaws.com.cn"
    } else {
        "amazonaws.com"
    };
    format!("https://sns.{region}.{suffix}")
}
/// `url` is on exactly `origin` (scheme, host and port), not a lookalike host or userinfo trick.
fn trusted(origin: &str, url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| {
        url.username().is_empty()
            && url.password().is_none()
            && url.origin().ascii_serialization() == origin
    })
}
