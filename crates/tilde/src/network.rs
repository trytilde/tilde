//! Host and origin checks shared by the management API and agent runtime API.
use crate::error::Error;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{collections::HashSet, net::SocketAddr};

#[derive(Clone)]
pub struct Boundary {
    hosts: HashSet<String>,
    origins: HashSet<String>,
}
impl Boundary {
    /// Require deliberate network exposure and explicit browser origins.
    pub fn new(
        listen: SocketAddr,
        allow_network: bool,
        origins: Vec<String>,
    ) -> Result<Self, Error> {
        if !listen.ip().is_loopback() && !allow_network {
            return Err(Error::Invalid(
                "Non-loopback binding requires --allow-network".into(),
            ));
        }
        let mut hosts = HashSet::from([
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
            listen.ip().to_string(),
        ]);
        let mut allowed = HashSet::from([
            format!("http://localhost:{}", listen.port()),
            format!("http://127.0.0.1:{}", listen.port()),
            format!("http://[::1]:{}", listen.port()),
        ]);
        for origin in origins {
            if origin.trim().is_empty() {
                continue;
            }
            let url = url::Url::parse(&origin)
                .map_err(|_| Error::Invalid("Invalid --web-origin".into()))?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || url.path() != "/"
            {
                return Err(Error::Invalid(
                    "--web-origin must be an HTTP(S) origin without credentials or a path".into(),
                ));
            }
            hosts.insert(url.host_str().unwrap().trim_matches(['[', ']']).into());
            allowed.insert(url.origin().ascii_serialization());
        }
        Ok(Self {
            hosts,
            origins: allowed,
        })
    }
}

/// Reject unrecognized hosts and browser origins before an RPC can run.
pub async fn guard(State(boundary): State<Boundary>, request: Request, next: Next) -> Response {
    let host = request
        .uri()
        .authority()
        .map(|a| a.host().to_string())
        .or_else(|| {
            request
                .headers()
                .get(http::header::HOST)
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.parse::<http::uri::Authority>().ok())
                .map(|a| a.host().to_string())
        });
    if !host.is_some_and(|host| boundary.hosts.contains(host.trim_matches(['[', ']']))) {
        return connectrpc::ConnectError::permission_denied("Host is not allowed").into_response();
    }
    // Sandboxed provider pages fetch public module assets with an opaque ("null") origin.
    // Only read-only asset requests bypass origin checks; all RPCs retain the normal boundary.
    let public_connection_asset =
        matches!(*request.method(), http::Method::GET | http::Method::HEAD)
            && (request.uri().path().starts_with("/connections/ui/")
                || request
                    .uri()
                    .path()
                    .strip_prefix("/catalog/")
                    .and_then(|path| path.strip_suffix("/ui"))
                    .is_some_and(|provider| {
                        !provider.is_empty()
                            && provider
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                    }));
    // OAuth form_post redirects are cross-origin by design and validate their own state.
    let provider_callback = request.uri().path() == crate::connections::CALLBACK_PATH
        && matches!(*request.method(), http::Method::GET | http::Method::POST);
    if !public_connection_asset
        && !provider_callback
        && let Some(origin) = request.headers().get(http::header::ORIGIN)
        && !origin
            .to_str()
            .is_ok_and(|value| boundary.origins.contains(value))
    {
        return connectrpc::ConnectError::permission_denied("Origin is not allowed")
            .into_response();
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        http::header::X_CONTENT_TYPE_OPTIONS,
        http::HeaderValue::from_static("nosniff"),
    );
    response
}
