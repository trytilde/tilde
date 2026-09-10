//! Public setup HTML/assets. The host owns the setup token; provider frames have opaque origins.
use crate::error::Error;
use axum::{
    Router,
    body::Body,
    extract::{OriginalUri, Path, State},
    response::{IntoResponse, Response},
    routing::get,
};
use http::{StatusCode, header};

#[derive(Clone)]
struct Assets {
    dev: Option<String>,
    connections: Option<super::service::Connections>,
    http: reqwest::Client,
}
#[cfg(feature = "embedded-web")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../../web/provider-dist/"]
struct Embedded;

/// Mount after management authentication: these pages contain no credentials or account data.
pub fn router(
    dev: Option<String>,
    connections: Option<super::service::Connections>,
) -> Result<Router, Error> {
    if let Some(origin) = &dev {
        let url = url::Url::parse(origin)
            .map_err(|_| Error::Invalid("Invalid connection UI dev origin".into()))?;
        // Deployment configuration may point at Vite on a Tailscale/private network.
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(Error::Invalid(
                "Connection UI dev origin must be an HTTP(S) origin without credentials, path, query or fragment".into(),
            ));
        }
    }
    let state = Assets {
        dev,
        connections,
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|_| Error::Invalid("Unable to initialize UI proxy".into()))?,
    };
    Ok(Router::new()
        .route("/connections/ui/{*path}", get(asset))
        .route("/catalog/{provider}/ui", get(provider_ui))
        .route("/connections/broker/{setup_id}", get(host))
        .with_state(state))
}
async fn host(State(state): State<Assets>, Path(setup_id): Path<String>) -> Response {
    if uuid::Uuid::parse_str(&setup_id).is_err() {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve(&state, "_host/ui.html", None).await
}
async fn provider_ui(State(state): State<Assets>, Path(provider): Path<String>) -> Response {
    if !provider
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve(&state, &format!("{provider}/ui.html"), None).await
}
async fn asset(
    State(state): State<Assets>,
    Path(path): Path<String>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    serve(&state, &path, uri.query()).await
}
async fn serve(state: &Assets, path: &str, query: Option<&str>) -> Response {
    if path.split('/').any(|part| part == "..") || path.contains('\\') {
        return StatusCode::NOT_FOUND.into_response();
    }
    if let Some(remote_path) = path.strip_prefix("remote/") {
        use base64::Engine;
        let mut parts = remote_path.splitn(2, '/');
        let Some(encoded) = parts.next() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Some(asset) = parts.next() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Some(connections) = &state.connections else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Ok(id) = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded)
            .and_then(|v| String::from_utf8(v).map_err(|_| base64::DecodeError::InvalidLength(0)))
        else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Ok(provider) = connections.provider(&id).await else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Some(remote) = provider.kind.remote() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Some(ui_url) = &remote.ui_url else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Ok(mut url) = url::Url::parse(ui_url) else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        {
            let Ok(mut segments) = url.path_segments_mut() else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            segments.pop_if_empty();
            for segment in asset.split('/') {
                segments.push(segment);
            }
        }
        return fetch(state, url.as_str()).await;
    }
    if let Some(dev) = &state.dev {
        let url = format!("{}/connections/ui/{path}", dev.trim_end_matches('/'));
        let url = if let Some(query) = query {
            format!("{url}?{query}")
        } else {
            url
        };
        return fetch(state, &url).await;
    }
    #[cfg(feature = "embedded-web")]
    if let Some(file) = Embedded::get(path) {
        return response_with_headers(
            StatusCode::OK,
            mime_guess::from_path(path).first_or_octet_stream().as_ref(),
            file.data.into_owned(),
        );
    }
    StatusCode::NOT_FOUND.into_response()
}
fn response_with_headers(status: StatusCode, content_type: &str, bytes: Vec<u8>) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header("referrer-policy", "no-referrer")
        // Module scripts in a sandboxed opaque-origin iframe need anonymous CORS access.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(bytes))
        .expect("valid asset response")
}

async fn fetch(state: &Assets, url: &str) -> Response {
    let Ok(mut response) = state.http.get(url).send().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) if bytes.len() + chunk.len() <= 8 * 1024 * 1024 => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(None) => break,
            _ => return StatusCode::BAD_GATEWAY.into_response(),
        }
    }
    response_with_headers(status, &content_type, bytes)
}
