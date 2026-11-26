//! AWS egress: a sandbox's AWS SDK or CLI calls any AWS API through the route, and the gateway
//! signs each request with the connection's credentials (its access keys, its web identity
//! role's session, or the role it lists in the grant's account), which never enter the sandbox.
//!
//! The sandbox's client is pointed at the route (`AWS_ENDPOINT_URL`) and signs with the egress
//! token as its access key ID and any secret: the gateway reads the token, and the service and
//! region the client signed for, from that signature's credential scope, and does not check the
//! signature itself. The request goes to that service's regional endpoint
//! (`https://<service>.<region>.amazonaws.com`, a global service's own host), re-signed. The
//! service is the SigV4 signing name: `logs` (CloudWatch Logs), `monitoring` (CloudWatch
//! metrics), `xray`, `s3`, `lambda`.
//!
//! A grant names the services and regions it reaches and whether it is read-only (the default).
//! An RPC service's request reads when its action is named Get, List, Describe, Filter, Search,
//! Lookup, BatchGet, Query or Scan followed by a word, or exactly StartQuery or StopQuery
//! (`reads`); the action is read from `X-Amz-Target`, the Query protocol's `Action` or an RPC v2
//! path, every one the request carries must agree, a write naming none is refused, and the
//! request carries one `X-Amz-Target`, one content type and an unencoded body, so the check reads
//! what AWS reads. A REST service (`REST`: S3, Lambda, X-Ray, API Gateway, AppSync) ignores
//! action names, so only its GET and HEAD read. Whatever the grant, the actions
//! `returns_credentials` lists, API Gateway key values, AppSync API keys and Glue connections
//! without `HidePassword` are refused. Other stored secrets a read can return (a Lambda
//! function's environment, an object in S3) are the role's IAM policy's to guard: it stays the
//! ceiling.
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
                    "items": {"type": "string", "pattern": "^[a-z0-9-]{2,40}$"},
                    "minItems": 1,
                    "uniqueItems": true,
                    "default": ["logs", "monitoring"],
                    "description": "SigV4 signing names: logs (CloudWatch Logs), monitoring (CloudWatch metrics), xray, s3, lambda..."
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

/// Services whose APIs are REST (the method and path name the operation), not RPC.
const REST: &[&str] = &[
    "s3",
    "lambda",
    "xray",
    "apigateway",
    "execute-api",
    "appsync",
];

/// Every API action a request could be read as naming: `X-Amz-Target` (JSON,
/// `Logs_20140328.FilterLogEvents`), an RPC v2 path (`/service/<name>/operation/<action>`), and
/// `Action` in the query or a form body (Query). A service reads only its own protocol's, so all
/// are checked and they must agree: a read in one place cannot carry a write in another. REST
/// APIs name none.
fn actions(path: &str, query: Option<&str>, headers: &http::HeaderMap, body: &[u8]) -> Vec<String> {
    let mut actions = vec![];
    if let Some(target) = headers.get("x-amz-target").and_then(|v| v.to_str().ok()) {
        actions.push(target.rsplit('.').next().unwrap_or(target).to_owned());
    }
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    if let ["service", _, "operation", action] = segments[..] {
        actions.push(action.to_owned());
    }
    let named = |pairs: &[u8]| {
        url::form_urlencoded::parse(pairs)
            .filter(|(k, _)| k.eq_ignore_ascii_case("action"))
            .map(|(_, v)| v.into_owned())
            .collect::<Vec<_>>()
    };
    actions.extend(named(query.unwrap_or_default().as_bytes()));
    let form = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.trim()
                .to_ascii_lowercase()
                .starts_with("application/x-www-form-urlencoded")
        });
    if form {
        actions.extend(named(body));
    }
    actions
}

/// Actions whose answers carry credentials or stored secrets, refused under any grant: STS
/// other than GetCallerIdentity, names with `Token`, `Credential`, `SecretValue`, `AccessKey`,
/// `Password` (EC2 password data, database master passwords) or `ApiKey`, SSM parameter reads
/// (decrypted SecureStrings), Lightsail access details, Location Service key values
/// (`geo:DescribeKey`) and Cognito app client secrets (`cognito-idp:DescribeUserPoolClient`). Other stored secrets (a Lambda function's
/// environment, say) are the role's IAM policy's to guard.
fn returns_credentials(service: &str, action: &str) -> bool {
    let named = |names: &[&str]| names.contains(&action);
    (service == "sts" && action != "GetCallerIdentity")
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
        || (service == "ssm"
            && named(&[
                "GetParameter",
                "GetParameters",
                "GetParametersByPath",
                "GetParameterHistory",
            ]))
        || (service == "lightsail" && action.contains("AccessDetails"))
        || (service == "geo" && action == "DescribeKey")
        || (service == "cognito-idp" && action == "DescribeUserPoolClient")
}

/// The endpoint of `service` in `region`: regional, except for global services.
fn endpoint(service: &str, region: &str) -> String {
    let suffix = if region.starts_with("cn-") {
        "amazonaws.com.cn"
    } else {
        "amazonaws.com"
    };
    match service {
        "iam" | "cloudfront" | "route53" | "organizations" => format!("https://{service}.{suffix}"),
        _ => format!("https://{service}.{region}.{suffix}"),
    }
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
        if !strings(config.get("services")).contains(&service) {
            return Decision::deny(format!("The {service} service is not granted"));
        }
        let regions = strings(config.get("regions"));
        if !regions.is_empty() && !regions.contains(&region) {
            return Decision::deny(format!("The {region} region is not granted"));
        }
        let rest = REST.contains(&service);
        // An RPC request is read as AWS reads it: one target, one content type, an identity body.
        if !rest {
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
        }
        // A REST service ignores action names: its method and path are the operation, so only
        // the method says whether it reads.
        let actions = if rest {
            vec![]
        } else {
            actions(path, *query, headers, body)
        };
        if actions.iter().any(|a| *a != actions[0]) {
            return Decision::deny(format!(
                "The request names more than one action: {}",
                actions.join(", ")
            ));
        }
        // API Gateway returns API key values when asked to (`includeValue(s)=true`) and with a
        // usage plan's keys.
        if service == "apigateway"
            && (url::form_urlencoded::parse(query.unwrap_or_default().as_bytes())
                .any(|(k, v)| k.starts_with("includeValue") && v.eq_ignore_ascii_case("true"))
                || crate::connections::egress::rules::matches(
                    "* /usageplans/*/keys/**",
                    method,
                    path,
                )
                .is_some())
        {
            return Decision::deny("API Gateway key values are not readable");
        }
        if service == "appsync"
            && crate::connections::egress::rules::matches("* /v1/apis/*/apikeys/**", method, path)
                .is_some()
        {
            return Decision::deny("AppSync API keys are not readable");
        }
        // Glue connections carry their passwords unless asked not to.
        if service == "glue"
            && actions
                .first()
                .is_some_and(|a| a == "GetConnection" || a == "GetConnections")
            && serde_json::from_slice::<Value>(body)
                .ok()
                .and_then(|b| b.get("HidePassword")?.as_bool())
                != Some(true)
        {
            return Decision::deny("Glue connections are readable with HidePassword only");
        }
        if let Some(action) = actions.first()
            && returns_credentials(service, action)
        {
            return Decision::deny(format!("{service}:{action} returns credentials"));
        }
        let get = method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD");
        if actions.is_empty() && !rest && !get {
            return Decision::deny(format!("The {service} request names no action"));
        }
        let read_only = config
            .get("read_only")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let reads = match actions.first() {
            Some(action) => reads(action),
            None => get,
        };
        if read_only && !reads {
            let what = actions
                .first()
                .cloned()
                .unwrap_or_else(|| format!("{method} {path}"));
            return Decision::deny(format!(
                "{service}:{what} is not a read; the grant is read-only"
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
