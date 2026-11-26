//! AWS egress: a sandbox's AWS SDK or CLI calls the supported AWS APIs through the route, and
//! the gateway signs each request with the connection's credentials (its access keys, its web
//! identity role's session, or the role it lists in the grant's account), which never enter the
//! sandbox.
//!
//! The sandbox's client is pointed at the route (`AWS_ENDPOINT_URL`) and signs with the egress
//! token as its access key ID and any secret: the gateway reads the token, and the service and
//! region the client signed for, from that signature's credential scope, and does not check the
//! signature itself. The request goes to that service's regional endpoint
//! (`https://<service>.<region>.amazonaws.com`), re-signed.
//!
//! Only the services in `SERVICES` are supported, each with the protocol its requests name their
//! action in; a grant may name no other (its schema's enum) and a request for any other is
//! refused. More are added deliberately, after checking what their reads return.
//! - RPC services: the path is `/` (or exactly `/service/<name>/operation/<action>` for RPC v2)
//!   and the action is read only where the service's protocol carries it (`X-Amz-Target` for
//!   JSON, `Action` in the query or a form body for Query, the path for RPC v2); one carried
//!   anywhere else, none at all, or several that disagree are refused, and the request carries
//!   one target, one content type and an unencoded body, so the check reads what AWS reads.
//!   Under a read-only grant (the default) the action must be named Get, List, Describe, Filter,
//!   Search, Lookup, BatchGet, Query or Scan followed by a word, or exactly StartQuery or
//!   StopQuery (`reads`).
//! - REST services (S3, Lambda): a request naming an action anywhere is refused; only GET and
//!   HEAD read.
//!
//! Paths are checked as AWS receives them: empty, `.` and `..` segments, encoded or not, are
//! refused. Whatever the grant, the actions `returns_credentials` lists are refused (STS but
//! GetCallerIdentity, SSM parameter reads, names with Token, Credential, SecretValue, AccessKey,
//! Password or ApiKey). Other stored secrets a read returns (a Lambda function's environment, an
//! object in S3) are the role's IAM policy's to guard: it stays the ceiling.
use crate::chat::providers::Access;
use crate::connections::egress::{
    Decision, Egress, EgressHook, EgressProtocol, EgressRequest, EnvTemplate, forge::strings,
};
use crate::connections::mints::MintScope;
use crate::error::Error;
use serde_json::{Value, json};
use std::sync::LazyLock;

pub struct Aws(Egress);
static HOOK: LazyLock<Aws> = LazyLock::new(|| Aws(definition()));
pub fn hook() -> &'static Aws {
    &HOOK
}

fn definition() -> Egress {
    Egress {
        protocols: vec![EgressProtocol::Http {
            upstream: "https://amazonaws.com",
            // Requests are checked by service, region and action, not path.
            allow: vec!["* /**"],
            deny: vec![],
            wiring: vec![
                EnvTemplate {
                    name: "AWS_ENDPOINT_URL",
                    value: "{url}",
                },
                EnvTemplate {
                    name: "AWS_ACCESS_KEY_ID",
                    value: "{token}",
                },
                // Not a secret: the gateway ignores the client's signature.
                EnvTemplate {
                    name: "AWS_SECRET_ACCESS_KEY",
                    value: "tilde-egress",
                },
            ],
        }],
        config_schema: json!({
            "type": "object",
            "properties": {
                "services": {
                    "type": "array",
                    "title": "Services",
                    "items": {"type": "string", "enum": SERVICES.iter().map(|(name, _)| *name).collect::<Vec<_>>()},
                    "minItems": 1,
                    "uniqueItems": true,
                    "default": ["logs", "monitoring"],
                    "description": "SigV4 signing names of the supported services: logs (CloudWatch Logs), monitoring (CloudWatch metrics), ecs, ec2, rds, cloudtrail, dynamodb, sqs, sns, cloudformation, elasticloadbalancing, autoscaling, sts (GetCallerIdentity), ssm (no parameter reads), s3, lambda"
                },
                "regions": {
                    "type": "array",
                    "title": "Regions",
                    "items": {"type": "string", "pattern": "^[a-z]{2}(-[a-z]+)+-[0-9]$"},
                    "uniqueItems": true,
                    "default": [],
                    "description": "Empty: every region"
                },
                "read_only": {
                    "type": "boolean",
                    "title": "Read-only",
                    "default": true,
                    "description": "Only actions that read (Get, List, Describe, Filter, Search, Query...)"
                },
                "account": {
                    "type": "string",
                    "title": "Account",
                    "pattern": "^[0-9]{12}$",
                    "description": "Optional. Another account the connection lists a role in; empty: its own"
                }
            },
            "additionalProperties": false
        }),
        config_version: 1,
        setup_queries: vec![],
    }
}

/// Read action names: a prefix followed by a word (`GetMetricData`) or the whole name (DynamoDB's
/// `Query`). CloudWatch Logs Insights' `StartQuery` and `StopQuery` are reads by name alone, so
/// `StartQueryExecution` (Athena) is not.
const READS: &[&str] = &[
    "Get", "List", "Describe", "Filter", "Search", "Lookup", "BatchGet", "Query", "Scan",
];
const READ_NAMES: &[&str] = &["StartQuery", "StopQuery"];
fn reads(action: &str) -> bool {
    READ_NAMES.contains(&action)
        || READS.iter().any(|prefix| {
            action.strip_prefix(prefix).is_some_and(|rest| {
                rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_uppercase())
            })
        })
}

/// Where a service's requests name their action.
#[derive(Clone, Copy)]
struct Protocol {
    /// JSON: `X-Amz-Target: Logs_20140328.FilterLogEvents`.
    target: bool,
    /// Query: `Action=` in the query or a form body.
    query: bool,
    /// RPC v2: `/service/<name>/operation/<action>`.
    path: bool,
}
const JSON: Option<Protocol> = Some(Protocol {
    target: true,
    query: false,
    path: false,
});
const QUERY: Option<Protocol> = Some(Protocol {
    target: false,
    query: true,
    path: false,
});

/// The supported services by signing name, with their protocol (`None`: REST, whose method and
/// path are the operation). CloudWatch takes Query, JSON and RPC v2; SQS JSON and Query.
const SERVICES: &[(&str, Option<Protocol>)] = &[
    ("logs", JSON),
    (
        "monitoring",
        Some(Protocol {
            target: true,
            query: true,
            path: true,
        }),
    ),
    ("ecs", JSON),
    ("ec2", QUERY),
    ("rds", QUERY),
    ("cloudtrail", JSON),
    ("dynamodb", JSON),
    (
        "sqs",
        Some(Protocol {
            target: true,
            query: true,
            path: false,
        }),
    ),
    ("sns", QUERY),
    ("cloudformation", QUERY),
    ("elasticloadbalancing", QUERY),
    ("autoscaling", QUERY),
    ("sts", QUERY),
    ("ssm", JSON),
    ("s3", None),
    ("lambda", None),
];

/// The action names a request carries, by where: `X-Amz-Target`, an RPC v2 path, and `Action`
/// in the query or a form body (the content type read case-insensitively).
struct Named {
    target: Vec<String>,
    path: Vec<String>,
    query: Vec<String>,
}
fn named(path: &str, query: Option<&str>, headers: &http::HeaderMap, body: &[u8]) -> Named {
    let target = headers
        .get_all("x-amz-target")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|t| t.rsplit('.').next().unwrap_or(t).to_owned())
        .collect();
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    let path = match segments[..] {
        ["service", _, "operation", action] => vec![action.to_owned()],
        _ => vec![],
    };
    let actions = |pairs: &[u8]| {
        url::form_urlencoded::parse(pairs)
            .filter(|(k, _)| k.eq_ignore_ascii_case("action"))
            .map(|(_, v)| v.into_owned())
            .collect::<Vec<_>>()
    };
    let mut query = actions(query.unwrap_or_default().as_bytes());
    let form = headers
        .get_all(http::header::CONTENT_TYPE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|v| {
            v.trim()
                .to_ascii_lowercase()
                .starts_with("application/x-www-form-urlencoded")
        });
    if form {
        query.extend(actions(body));
    }
    Named {
        target,
        path,
        query,
    }
}

/// Actions whose answers carry credentials or stored secrets, refused under any grant: STS
/// other than GetCallerIdentity, SSM parameter reads (decrypted SecureStrings), and names with
/// `Token`, `Credential`, `SecretValue`, `AccessKey`, `Password` (EC2 password data, database
/// master passwords) or `ApiKey`.
fn returns_credentials(service: &str, action: &str) -> bool {
    (service == "sts" && action != "GetCallerIdentity")
        || (service == "ssm" && action.starts_with("GetParameter"))
        || [
            "Token",
            "Credential",
            "SecretValue",
            "AccessKey",
            "Password",
            "ApiKey",
        ]
        .iter()
        .any(|word| action.contains(word))
}

/// The endpoint of `service` in `region` (every supported service is regional).
fn endpoint(service: &str, region: &str) -> String {
    let suffix = if region.starts_with("cn-") {
        "amazonaws.com.cn"
    } else {
        "amazonaws.com"
    };
    format!("https://{service}.{region}.{suffix}")
}

/// A region code (`us-east-1`, `us-gov-west-1`): it chooses the host the request goes to.
fn valid_region(region: &str) -> bool {
    let parts: Vec<&str> = region.split('-').collect();
    parts.len() >= 3
        && parts[0].len() == 2
        && parts[..parts.len() - 1]
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase()))
        && parts[parts.len() - 1].len() == 1
        && parts[parts.len() - 1].bytes().all(|b| b.is_ascii_digit())
}

fn scope<'a>(request: &EgressRequest<'a>) -> Result<(&'a str, &'a str), Decision> {
    let EgressRequest::Http { headers, .. } = request else {
        return Err(Decision::deny("AWS is reached over HTTP"));
    };
    let (region, service) = headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::connections::egress::sigv4)
        .map(|(_, region, service)| (region, service))
        .ok_or_else(|| {
            Decision::deny("Sign the request with AWS SigV4, the egress token as the access key ID")
        })?;
    let service_name = !service.is_empty()
        && service
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !valid_region(region) || !service_name {
        return Err(Decision::deny("Not an AWS region and service"));
    }
    Ok((region, service))
}

#[async_trait::async_trait]
impl EgressHook for Aws {
    fn definition(&self) -> &Egress {
        &self.0
    }
    fn check(&self, request: &EgressRequest<'_>, config: &Value) -> Decision {
        let (region, service) = match scope(request) {
            Ok(scope) => scope,
            Err(denied) => return denied,
        };
        let EgressRequest::Http {
            method,
            path,
            query,
            headers,
            body,
            ..
        } = request
        else {
            return Decision::deny("AWS is reached over HTTP");
        };
        let Some(&(_, protocol)) = SERVICES.iter().find(|(name, _)| *name == service) else {
            return Decision::deny(format!("The {service} service is not supported"));
        };
        if !strings(config.get("services")).contains(&service) {
            return Decision::deny(format!("The {service} service is not granted"));
        }
        let regions = strings(config.get("regions"));
        if !regions.is_empty() && !regions.contains(&region) {
            return Decision::deny(format!("The {region} region is not granted"));
        }
        // The path is checked as AWS receives it: no empty, `.` or `..` segments, encoded or not.
        if crate::connections::egress::rules::matches("* /**", method, path).is_none() {
            return Decision::deny("The path has empty, dot or encoded-slash segments");
        }
        let named = named(path, *query, headers, body);
        let get = method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD");
        let read_only = config
            .get("read_only")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let Some(protocol) = protocol else {
            // REST: the method and path are the operation; an action named anywhere is refused.
            if !(named.target.is_empty() && named.path.is_empty() && named.query.is_empty()) {
                return Decision::deny(format!("{service} requests name no action"));
            }
            if read_only && !get {
                return Decision::deny(format!(
                    "{service}: {method} {path} is not a read; the grant is read-only"
                ));
            }
            return Decision::Allow;
        };
        // RPC: read as AWS reads it, from the root (or an RPC v2 operation path), one target, one
        // content type, an unencoded body, the action where the protocol carries it.
        if !path.trim_end_matches('/').is_empty() && named.path.is_empty() {
            return Decision::deny(format!("{service} requests go to /"));
        }
        for name in ["x-amz-target", "content-type", "content-encoding"] {
            if headers.get_all(name).iter().count() > 1 {
                return Decision::deny(format!("More than one {name}"));
            }
        }
        if headers
            .get(http::header::CONTENT_ENCODING)
            .is_some_and(|v| !v.as_bytes().eq_ignore_ascii_case(b"identity"))
        {
            return Decision::deny("Encoded request bodies are not checked");
        }
        let misplaced = (!protocol.target && !named.target.is_empty())
            || (!protocol.query && !named.query.is_empty())
            || (!protocol.path && !named.path.is_empty());
        if misplaced {
            return Decision::deny(format!(
                "The request names its action where {service} does not read it"
            ));
        }
        let actions: Vec<&String> = named
            .target
            .iter()
            .chain(&named.path)
            .chain(&named.query)
            .collect();
        let Some(action) = actions.first() else {
            return Decision::deny(format!("The {service} request names no action"));
        };
        if actions.iter().any(|a| a != action) {
            return Decision::deny(format!(
                "The request names more than one action: {}",
                actions
                    .iter()
                    .map(|a| a.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if actions.iter().any(|a| returns_credentials(service, a)) {
            return Decision::deny(format!("{service}:{action} returns credentials"));
        }
        if read_only && !reads(action) {
            return Decision::deny(format!(
                "{service}:{action} is not a read; the grant is read-only"
            ));
        }
        Decision::Allow
    }
    fn scope(&self, config: &Value) -> MintScope {
        MintScope {
            resources: config
                .get("account")
                .and_then(Value::as_str)
                .map(|account| vec![account.to_owned()])
                .unwrap_or_default(),
            permissions: vec![],
        }
    }
    fn route(&self, _upstream: String, request: &EgressRequest<'_>) -> Result<String, Decision> {
        let (region, service) = scope(request)?;
        Ok(endpoint(service, region))
    }
    async fn setup_query(
        &self,
        _access: &Access,
        name: &str,
        _input: Value,
    ) -> Result<Value, Error> {
        Err(Error::Invalid(format!("Unknown setup query {name}")))
    }
}
