use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
};
use tilde::network::{Boundary, guard};
use tower::ServiceExt;

#[tokio::test]
async fn no_auth_listener_still_rejects_rebinding_and_foreign_browser_origins() {
    let boundary = Boundary::new(
        "127.0.0.1:8080".parse().unwrap(),
        false,
        vec!["https://engine.example.com".into()],
    )
    .unwrap();
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .layer(axum::middleware::from_fn_with_state(boundary, guard));
    for (host, origin, expected) in [
        ("127.0.0.1:8080", None, StatusCode::OK),
        (
            "127.0.0.1:8080",
            Some("http://127.0.0.1:8080"),
            StatusCode::OK,
        ),
        ("attacker.example", None, StatusCode::FORBIDDEN),
        (
            "127.0.0.1:8080",
            Some("https://attacker.example"),
            StatusCode::FORBIDDEN,
        ),
        (
            "engine.example.com",
            Some("https://engine.example.com"),
            StatusCode::OK,
        ),
    ] {
        let mut request = Request::builder().uri("/healthz").header("host", host);
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        assert_eq!(
            app.clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            expected
        );
    }
    assert!(Boundary::new("0.0.0.0:8080".parse().unwrap(), false, vec![]).is_err());
    assert!(Boundary::new("0.0.0.0:8080".parse().unwrap(), true, vec![]).is_ok());
}
