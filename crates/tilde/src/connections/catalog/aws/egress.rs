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
//! A grant names the services and regions it reaches and whether it is read-only (the default):
//! an action named Get, List, Describe, Filter, Search, Lookup, BatchGet, Query, Scan, StartQuery
//! or StopQuery, read from `X-Amz-Target`, the Query protocol's `Action`, or an RPC v2 path; a
//! REST API's (S3, Lambda) GET and HEAD. Whatever the grant, actions that return credentials
//! (STS other than GetCallerIdentity, `*Token*`, `*Credential*`, secret values, access keys)
//! are refused, so no credential reaches the sandbox through a response. The role's own IAM
//! policy stays the ceiling.
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

const READS: &[&str] = &[
    "Get",
    "List",
    "Describe",
    "Filter",
    "Search",
    "Lookup",
    "BatchGet",
    "Query",
    "Scan",
    "StartQuery",
    "StopQuery",
];

/// The API action a request names, if its protocol names one: JSON (`X-Amz-Target:
/// Logs_20140328.FilterLogEvents`), RPC v2 (`/service/<name>/operation/<action>`) or Query
/// (`Action=` in the query or form body). REST APIs name none.
fn action(
    path: &str,
    query: Option<&str>,
    headers: &http::HeaderMap,
    body: &[u8],
) -> Option<String> {
    if let Some(target) = headers.get("x-amz-target").and_then(|v| v.to_str().ok()) {
        return Some(target.rsplit('.').next().unwrap_or(target).to_owned());
    }
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    if let ["service", _, "operation", action] = segments[..] {
        return Some(action.to_owned());
    }
    let form = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/x-www-form-urlencoded"));
    let named = |pairs: &[u8]| {
        url::form_urlencoded::parse(pairs)
            .find(|(k, _)| k == "Action")
            .map(|(_, v)| v.into_owned())
    };
    query
        .and_then(|q| named(q.as_bytes()))
        .or_else(|| form.then(|| named(body)).flatten())
}

/// Actions whose answers carry credentials, refused under any grant.
fn returns_credentials(service: &str, action: &str) -> bool {
    (service == "sts" && action != "GetCallerIdentity")
        || [
            "Token",
            "Credential",
            "SecretValue",
            "AccessKey",
            "PasswordData",
        ]
        .iter()
        .any(|word| action.contains(word))
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
        let action = action(path, *query, headers, body);
        if let Some(action) = &action
            && returns_credentials(service, action)
        {
            return Decision::deny(format!("{service}:{action} returns credentials"));
        }
        let read_only = config
            .get("read_only")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let reads = match &action {
            Some(action) => READS.iter().any(|prefix| action.starts_with(prefix)),
            None => method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD"),
        };
        if read_only && !reads {
            let what = action.unwrap_or_else(|| format!("{method} {path}"));
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
