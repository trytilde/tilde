//! The server tracing middleware drops probe and unmatched-route noise but keeps failures.
use axum::{Router, body::Body, http::StatusCode, routing::get};
use tower::ServiceExt;

#[tokio::test]
async fn probes_and_unmatched_routes_are_dropped_unless_they_fail() {
    let exporter = opentelemetry_sdk::trace::InMemorySpanExporter::default();
    opentelemetry::global::set_tracer_provider(
        opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build(),
    );
    let router = Router::new()
        .route("/readyz", get(|| async { "ready" }))
        .route("/healthz", get(|| async { "ok" }))
        .route("/agents/{id}", get(|| async { "agent" }))
        .fallback(|uri: axum::http::Uri| async move {
            if uri.path() == "/broken" {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::NOT_FOUND
            }
        })
        .layer(axum::middleware::from_fn(
            tilde::telemetry::tracing::context::request,
        ));
    for path in [
        "/readyz",
        "/healthz",
        "/wp-login.php",
        "/agents/1",
        "/broken",
    ] {
        let response = router
            .clone()
            .oneshot(axum::http::Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        // The matched-route span ends with its streamed body.
        axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
    }
    let spans = exporter.get_finished_spans().unwrap();
    let mut names: Vec<_> = spans.iter().map(|s| s.name.to_string()).collect();
    names.sort();
    assert_eq!(names, ["GET /agents/{id}", "GET unmatched"]);
    let failed = spans.iter().find(|s| s.name == "GET unmatched").unwrap();
    assert!(matches!(
        failed.status,
        opentelemetry::trace::Status::Error { .. }
    ));
    assert!(failed.start_time < failed.end_time);
}
