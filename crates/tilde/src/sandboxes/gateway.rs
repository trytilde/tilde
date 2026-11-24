//! The egress route on the runtime listener: `/sandboxes/egress/{slug}/{*path}`.
//!
//! A sandbox process authenticates with its egress token (`Authorization: Bearer`, `token`, or a
//! Basic password, which is how git sends the credential helper's answer). The token names the
//! sandbox, its blueprint's enabled grant `slug` names the connection; the provider's hook checks
//! the request (for a push, the ref updates and the pack it carries, read before anything is
//! forwarded) against its rules and the grant's config. The client's credentials are dropped,
//! the narrowest credential the grant needs is minted and attached, and the request streams to
//! the provider's upstream (fixed, or the connection's own instance or cluster). The path is
//! checked and forwarded as the client sent it, still percent-encoded. Redirects are not
//! followed: a same-host `Location` is rewritten to the route, any other passes to the client,
//! which follows it without a credential. The one exception is a git upstream that redirects to
//! replica nodes under its own host (`replicas`), which the gateway follows with the credential.
//! Each request is a `sandbox.egress` span on the sandbox's latest invocation.
use super::{Sandboxes, db, digest, git};
use crate::connections::egress::{Decision, EgressProtocol, EgressRequest, GitAuth};
use crate::connections::mints::{self, MintScope};
use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::Engine;
use futures::TryStreamExt;
use opentelemetry::{KeyValue, trace::SpanKind, trace::TraceContextExt};
use secrecy::ExposeSecret;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

/// A push is read whole before it is forwarded, so its pack can be checked.
const PUSH_LIMIT: usize = 100 * 1024 * 1024;
/// REST request bodies.
const HTTP_LIMIT: usize = 10 * 1024 * 1024;
/// Requests per second each sandbox may make through egress, and the burst above it.
const RATE: f64 = 20.0;
const BURST: f64 = 200.0;
pub const PREFIX: &str = "/sandboxes/egress";

pub fn router(sandboxes: Sandboxes) -> axum::Router {
    axum::Router::new()
        .route(
            &format!("{PREFIX}/{{slug}}/{{*path}}"),
            axum::routing::any(egress),
        )
        .with_state(sandboxes)
}

/// The egress token a request presents, whichever way its client sends credentials
/// (`PRIVATE-TOKEN` is GitLab clients' header).
pub fn presented(headers: &HeaderMap) -> Option<String> {
    if let Some(token) = headers.get("private-token") {
        return Some(token.to_str().ok()?.trim().to_owned());
    }
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, credential) = value.split_once(' ')?;
    match scheme.to_ascii_lowercase().as_str() {
        "bearer" | "token" => Some(credential.trim().to_owned()),
        "basic" => {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(credential.trim())
                .ok()?;
            let decoded = String::from_utf8(decoded).ok()?;
            Some(decoded.split_once(':')?.1.to_owned())
        }
        _ => None,
    }
}

fn refuse(status: StatusCode, message: &str) -> Response {
    let mut response = (status, format!("{message}\n")).into_response();
    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"tilde egress\""),
        );
    }
    response
}

static BUCKETS: LazyLock<Mutex<HashMap<uuid::Uuid, (f64, Instant)>>> =
    LazyLock::new(Default::default);
fn admit(sandbox: uuid::Uuid) -> bool {
    let mut buckets = BUCKETS.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    let (tokens, at) = buckets.entry(sandbox).or_insert((BURST, now));
    *tokens = (*tokens + now.duration_since(*at).as_secs_f64() * RATE).min(BURST);
    *at = now;
    if *tokens < 1.0 {
        return false;
    }
    *tokens -= 1.0;
    true
}

async fn egress(
    State(sandboxes): State<Sandboxes>,
    Path((slug, _)): Path<(String, String)>,
    request: Request,
) -> Response {
    // The path as sent: decoding it here would turn an encoded slash into a path separator.
    let path = request
        .uri()
        .path()
        .strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| rest.split_once('/'))
        .map(|(_, path)| format!("/{path}"))
        .unwrap_or_default();
    let Some(token) = presented(request.headers()) else {
        return refuse(StatusCode::UNAUTHORIZED, "Egress token required");
    };
    let grant = match sandboxes.egress_grant(&digest(&token), &slug).await {
        Ok(Some(grant)) => grant,
        Ok(None) => return refuse(StatusCode::UNAUTHORIZED, "Invalid egress token or grant"),
        Err(error) => {
            tracing::error!(?error, "Resolving an egress grant failed");
            return refuse(StatusCode::SERVICE_UNAVAILABLE, "Egress is unavailable");
        }
    };
    if !admit(grant.sandbox_id) {
        return refuse(StatusCode::TOO_MANY_REQUESTS, "Egress rate limit exceeded");
    }
    let scope = match sandboxes.connections.pool.get().await {
        Ok(client) => db::trace_scope_opt(&client, grant.sandbox_id, None)
            .await
            .ok()
            .flatten(),
        Err(_) => None,
    };
    let cx = span(&slug, grant.sandbox_id, scope.as_ref());
    let _end = crate::telemetry::tracing::context::EndOnDrop(cx.clone());
    let response = forward(&sandboxes, &grant, &slug, &path, request, &cx).await;
    let status = response.status().as_u16();
    cx.span()
        .set_attribute(KeyValue::new("http.response.status_code", status as i64));
    response
}

/// The `sandbox.egress` span of one request, on the invocation the sandbox serves.
fn span(
    slug: &str,
    sandbox: uuid::Uuid,
    scope: Option<&db::TraceScopeRow>,
) -> opentelemetry::Context {
    let mut attributes = vec![
        KeyValue::new("tilde.sandbox.id", sandbox.to_string()),
        KeyValue::new("tilde.egress.slug", slug.to_owned()),
    ];
    if let Some(scope) = scope {
        for (key, value) in [
            ("tilde.invocation.id", scope.invocation_id),
            ("tilde.agent.id", scope.agent_id),
            ("tilde.run.id", scope.run_id),
            ("tilde.thread.id", scope.thread_id),
        ] {
            attributes.push(KeyValue::new(key, value.to_string()));
        }
    }
    let parent = scope
        .filter(|s| !s.traceparent.is_empty())
        .map(|s| crate::telemetry::tracing::context::restore(&s.traceparent, &s.tracestate))
        .unwrap_or_default();
    crate::telemetry::tracing::context::start(
        format!("sandbox.egress {slug}"),
        SpanKind::Client,
        &parent,
        attributes,
    )
}

async fn forward(
    sandboxes: &Sandboxes,
    grant: &super::egress::ResolvedGrant,
    slug: &str,
    path: &str,
    request: Request,
    cx: &opentelemetry::Context,
) -> Response {
    let span = cx.span();
    let hook = grant.hook;
    let definition = hook.definition();
    if grant.config_version != definition.config_version {
        return refuse(
            StatusCode::FORBIDDEN,
            "The egress grant was saved for an older configuration; save it again",
        );
    }
    let (parts, body) = request.into_parts();
    let mut body = Some(body);
    let query = parts.uri.query();
    let git_route = git::route(path, query);
    let protocol = definition.protocols.iter().find(|p| {
        matches!(
            (p, git_route.is_some()),
            (EgressProtocol::Git { .. }, true) | (EgressProtocol::Http { .. }, false)
        )
    });
    let Some(protocol) = protocol else {
        return refuse(
            StatusCode::FORBIDDEN,
            "The provider has no egress for this protocol",
        );
    };
    span.set_attribute(KeyValue::new("tilde.egress.protocol", protocol.name()));
    let access = match sandboxes.connections.access(grant.connection_id).await {
        Ok(access) => access,
        Err(_) => return refuse(StatusCode::FORBIDDEN, "The connection is not usable"),
    };
    let mint = |scope: MintScope| {
        let access = &access;
        async move { mints::mint(access, &grant.provider_id, &grant.type_id, &scope, "egress").await }
    };
    // Isolated tests point a provider's upstreams at fakes (`<provider>_api`, `<provider>_git`),
    // as its tools' endpoints are; production has no overrides.
    let endpoint = match protocol {
        EgressProtocol::Http { .. } => format!("{}_api", grant.provider_id),
        EgressProtocol::Git { .. } => format!("{}_git", grant.provider_id),
    };
    let upstream = match hook.upstream(protocol, &access) {
        Ok(upstream) => upstream,
        Err(_) => return refuse(StatusCode::FORBIDDEN, "The connection names no upstream"),
    };
    let upstream = access
        .endpoints
        .get(&endpoint, &upstream)
        .trim_end_matches('/')
        .to_owned();
    let (deny_default_branch, replicas) = match protocol {
        EgressProtocol::Git {
            deny_default_branch,
            replicas,
            ..
        } => (*deny_default_branch, *replicas),
        EgressProtocol::Http { .. } => (false, false),
    };

    // Read what the decision needs: a push's commands and pack; a REST body is bounded.
    let mut body_bytes = None;
    let decision = match git_route {
        Some((repository, service, advertisement)) => {
            span.set_attribute(KeyValue::new("tilde.egress.git.service", service.as_str()));
            span.set_attribute(KeyValue::new(
                "tilde.egress.git.repository",
                repository.to_owned(),
            ));
            let pushing =
                service == crate::connections::egress::GitService::ReceivePack && !advertisement;
            let (updates, fast_forward, default_branch) = if pushing {
                let bytes = match axum::body::to_bytes(body.take().unwrap_or_default(), PUSH_LIMIT)
                    .await
                {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        return refuse(StatusCode::PAYLOAD_TOO_LARGE, "Pushes are at most 100 MiB");
                    }
                };
                let gzip = parts
                    .headers
                    .get(header::CONTENT_ENCODING)
                    .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"gzip"));
                let plain = if gzip {
                    let mut out = vec![];
                    let read = std::io::Read::read_to_end(
                        &mut std::io::Read::take(
                            flate2::read::GzDecoder::new(&bytes[..]),
                            PUSH_LIMIT as u64 + 1,
                        ),
                        &mut out,
                    );
                    if read.is_err() || out.len() > PUSH_LIMIT {
                        return refuse(StatusCode::BAD_REQUEST, "Unreadable push body");
                    }
                    axum::body::Bytes::from(out)
                } else {
                    bytes.clone()
                };
                let (updates, pack) = match git::commands(&plain) {
                    Ok(parsed) => parsed,
                    Err(message) => return refuse(StatusCode::BAD_REQUEST, &message),
                };
                let fast_forward = git::fast_forwards(&updates, pack);
                body_bytes = Some(bytes);
                // Only a protocol that protects the default branch needs to learn it.
                let default_branch = if deny_default_branch {
                    let read = mint(MintScope {
                        permissions: vec![("contents".into(), "read".into())],
                        ..hook.scope(&grant.config)
                    })
                    .await;
                    // The repository as the client names it, with or without `.git`.
                    let repository = path.strip_suffix("/git-receive-pack").unwrap_or(path);
                    match read {
                        Ok(minted) => {
                            default_branch(&access, protocol, &upstream, repository, &minted).await
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                };
                (updates, fast_forward, default_branch)
            } else {
                (vec![], vec![], None)
            };
            for update in &updates {
                span.add_event(
                    "ref_update",
                    vec![
                        KeyValue::new("ref", update.name.clone()),
                        KeyValue::new("old", update.old.clone()),
                        KeyValue::new("new", update.new.clone()),
                    ],
                );
            }
            hook.check(
                &EgressRequest::Git {
                    protocol,
                    repository,
                    service,
                    updates: &updates,
                    fast_forward: &fast_forward,
                    default_branch: default_branch.as_deref(),
                },
                &grant.config,
            )
        }
        None => {
            let method = parts.method.as_str();
            span.set_attribute(KeyValue::new("http.request.method", method.to_owned()));
            span.set_attribute(KeyValue::new("url.path", path.to_owned()));
            match axum::body::to_bytes(body.take().unwrap_or_default(), HTTP_LIMIT).await {
                Ok(bytes) => body_bytes = Some(bytes),
                Err(_) => {
                    return refuse(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "Request bodies are at most 10 MiB",
                    );
                }
            }
            hook.check(
                &EgressRequest::Http {
                    protocol,
                    method,
                    path,
                },
                &grant.config,
            )
        }
    };
    if let Decision::Deny(reason) = decision {
        span.set_attribute(KeyValue::new("tilde.egress.decision", "deny"));
        span.set_attribute(KeyValue::new("tilde.egress.reason", reason.clone()));
        return refuse(StatusCode::FORBIDDEN, &reason);
    }
    span.set_attribute(KeyValue::new("tilde.egress.decision", "allow"));

    let minted = match mint(hook.scope(&grant.config)).await {
        Ok(minted) => minted,
        Err(error) => {
            return refuse(
                StatusCode::BAD_GATEWAY,
                &format!(
                    "The connection's credential could not be minted: {}",
                    error.message.unwrap_or_default()
                ),
            );
        }
    };
    let mut url = format!("{upstream}{path}");
    if let Some(query) = query {
        url.push('?');
        url.push_str(query);
    }
    // A request that may be redirected to a replica is replayed there, so its body is read
    // first; fetch negotiation is small.
    if replicas && body_bytes.is_none() {
        match axum::body::to_bytes(body.take().unwrap_or_default(), HTTP_LIMIT).await {
            Ok(bytes) => body_bytes = Some(bytes),
            Err(_) => {
                return refuse(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Request bodies are at most 10 MiB",
                );
            }
        }
    }
    let mut hops = 0;
    let upstream_response = loop {
        let mut outbound = access
            .http
            .client
            .request(parts.method.clone(), &url)
            .timeout(std::time::Duration::from_secs(600));
        for (name, value) in &parts.headers {
            if !forwarded(name.as_str()) {
                continue;
            }
            outbound = outbound.header(name, value);
        }
        for (name, value) in credential(protocol, &minted) {
            outbound = outbound.header(name, value);
        }
        outbound = match &body_bytes {
            Some(bytes) => outbound.body(bytes.clone()),
            // Fetch negotiation streams through.
            None => outbound.body(reqwest::Body::wrap_stream(
                body.take()
                    .unwrap_or_default()
                    .into_data_stream()
                    .map_err(std::io::Error::other),
            )),
        };
        let response = match outbound.send().await {
            Ok(response) => response,
            Err(_) => return refuse(StatusCode::BAD_GATEWAY, "The upstream could not be reached"),
        };
        let replica = (replicas && hops < 3 && response.status().is_redirection())
            .then(|| replica(&response, &upstream))
            .flatten();
        match replica {
            Some(next) => {
                url = next;
                hops += 1;
            }
            None => break response,
        }
    };
    let status = upstream_response.status();
    let mut response = Response::builder().status(status.as_u16());
    for (name, value) in upstream_response.headers() {
        let name = name.as_str();
        if matches!(
            name,
            "connection" | "keep-alive" | "transfer-encoding" | "set-cookie" | "content-length"
        ) {
            continue;
        }
        if name == "location" {
            response = response.header(
                name,
                relocate(value, &upstream, &sandboxes.runtime_url, slug),
            );
            continue;
        }
        response = response.header(name, value);
    }
    if let Some(length) = upstream_response.content_length() {
        span.set_attribute(KeyValue::new("http.response.body.size", length as i64));
    }
    let stream = upstream_response.bytes_stream();
    response
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| refuse(StatusCode::BAD_GATEWAY, "Invalid upstream response"))
}

/// Request headers that travel upstream: never the client's credentials, cookies or hop-by-hop
/// headers; the host and length are the outbound request's own.
fn forwarded(name: &str) -> bool {
    !matches!(
        name,
        "authorization"
            | "proxy-authorization"
            | "private-token"
            | "cookie"
            | "host"
            | "connection"
            | "keep-alive"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "forwarded"
            | "x-forwarded-for"
            | "x-forwarded-host"
            | "x-forwarded-proto"
    )
}

/// The minted credential as the protocol takes it: a git upstream that authenticates with Basic
/// gets the token as the password of the protocol's user.
fn credential(
    protocol: &EgressProtocol,
    minted: &mints::Minted,
) -> Vec<(header::HeaderName, HeaderValue)> {
    minted
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            let value = value.expose_secret();
            let value = match (protocol, value.strip_prefix("Bearer ")) {
                (
                    EgressProtocol::Git {
                        auth: GitAuth::Basic(user),
                        ..
                    },
                    Some(token),
                ) if name == header::AUTHORIZATION => {
                    format!(
                        "Basic {}",
                        base64::engine::general_purpose::STANDARD.encode(format!("{user}:{token}"))
                    )
                }
                _ => value.to_owned(),
            };
            let mut value = HeaderValue::from_str(&value).ok()?;
            value.set_sensitive(true);
            Some((name.clone(), value))
        })
        .collect()
}

/// A redirect to the upstream itself goes back through the route; elsewhere it is left alone.
fn relocate(location: &HeaderValue, upstream: &str, runtime: &str, slug: &str) -> HeaderValue {
    location
        .to_str()
        .ok()
        .and_then(|l| l.strip_prefix(upstream))
        .filter(|rest| rest.is_empty() || rest.starts_with('/') || rest.starts_with('?'))
        .and_then(|rest| {
            HeaderValue::from_str(&format!(
                "{}{PREFIX}/{slug}{rest}",
                runtime.trim_end_matches('/')
            ))
            .ok()
        })
        .unwrap_or_else(|| location.clone())
}

/// The replica a git upstream redirects to, if it is the upstream's own origin or a host under
/// it: the only other place the credential may go.
fn replica(response: &reqwest::Response, upstream: &str) -> Option<String> {
    let location = response.headers().get(header::LOCATION)?.to_str().ok()?;
    let next = response.url().join(location).ok()?;
    let upstream = url::Url::parse(upstream).ok()?;
    let host = next.host_str()?;
    let within = upstream
        .host_str()
        .is_some_and(|own| host == own || host.ends_with(&format!(".{own}")));
    (within
        && next.scheme() == upstream.scheme()
        && next.port_or_known_default() == upstream.port_or_known_default())
    .then(|| next.to_string())
}

/// The default branch of the repository at `repository` (its path under the upstream), from
/// upstream's upload-pack advertisement.
async fn default_branch(
    access: &crate::chat::providers::Access,
    protocol: &EgressProtocol,
    upstream: &str,
    repository: &str,
    minted: &mints::Minted,
) -> Option<String> {
    let mut request = access
        .http
        .client
        .get(format!(
            "{upstream}{repository}/info/refs?service=git-upload-pack"
        ))
        .header("git-protocol", "version=1");
    for (name, value) in credential(protocol, minted) {
        request = request.header(name, value);
    }
    let response = request.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.bytes().await.ok()?;
    // The advertisement's first ref line carries the capabilities.
    git::default_branch(&body[..body.len().min(64 * 1024)])
}
