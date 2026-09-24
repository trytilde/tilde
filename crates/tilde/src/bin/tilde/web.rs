//! Only the server binary embeds assets; code generation never depends on this module.
use axum::{
    body::Body,
    extract::Request,
    response::{IntoResponse, Response},
};
use http::{Method, StatusCode, header};

#[derive(rust_embed::RustEmbed)]
#[folder = "../../web/dist/"]
struct Assets;

pub async fn serve(request: Request) -> Response {
    let method = request.method();
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = request.uri().path().trim_start_matches('/');
    if path.starts_with("tilde.") || path == "api" || path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let connection_setup_page = path.starts_with("connections/broker/");
    let asset = Assets::get(if path.is_empty() { "index.html" } else { path })
        .map(|asset| (path, asset))
        .or_else(|| {
            let navigation = request
                .headers()
                .get(header::ACCEPT)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.contains("text/html"));
            if navigation
                && !path.starts_with("assets/")
                && !path.rsplit('/').next().unwrap_or_default().contains('.')
            {
                Assets::get("index.html").map(|asset| ("index.html", asset))
            } else {
                None
            }
        });
    let Some((path, asset)) = asset else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = mime_guess::from_path(if path.is_empty() { "index.html" } else { path })
        .first_or_octet_stream();
    let cache = if connection_setup_page {
        "no-store"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let length = asset.data.len();
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(asset.data.into_owned())
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type.as_ref())
        .header(header::CACHE_CONTROL, cache)
        .header(header::CONTENT_LENGTH, length)
        .header("referrer-policy", "no-referrer")
        .body(body)
        .expect("valid static asset headers")
}
