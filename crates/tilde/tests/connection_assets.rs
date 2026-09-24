use axum::{Router, routing::get};
use tilde::{
    connections::assets,
    network::{self, Boundary},
};

#[tokio::test]
async fn sandbox_assets_are_public_but_rpc_origins_remain_restricted() {
    let vite = tokio::net::TcpListener::bind("127.0.0.2:0").await.unwrap();
    let origin = format!("http://{}", vite.local_addr().unwrap());
    let upstream = tokio::spawn(async move {
        axum::serve(
            vite,
            Router::new()
                .route(
                    "/connections/ui/slack/ui.html",
                    get(|| async { axum::response::Html("<div>Slack TSX entry</div>") }),
                )
                .route(
                    "/connections/ui/_host/ui.html",
                    get(|| async { axum::response::Html("<iframe sandbox></iframe>") }),
                ),
        )
        .await
        .unwrap()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = assets::router(Some(origin), None)
        .unwrap()
        .route("/engine.test/Method", get(|| async { "protected" }))
        .layer(axum::middleware::from_fn_with_state(
            Boundary::new(address, false, vec![]).unwrap(),
            network::guard,
        ));
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{address}/connections/ui/slack/ui.html"))
        .header("origin", "null")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["access-control-allow-origin"], "*");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    assert!(response.text().await.unwrap().contains("Slack TSX entry"));
    let host = client
        .get(format!(
            "http://{address}/connections/broker/{}?connection_setup_token=private",
            uuid::Uuid::new_v4()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(host.status(), 200);
    assert!(!host.text().await.unwrap().contains("private"));
    let rejected = client
        .get(format!("http://{address}/engine.test/Method"))
        .header("origin", "null")
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 403);
    upstream.abort();
    server.abort();
}

#[cfg(feature = "embedded-web")]
#[tokio::test]
async fn release_assets_include_independent_provider_entry_points() {
    use tower::ServiceExt;
    let router = assets::router(None, None).unwrap();
    for provider in ["github", "slack", "_standard"] {
        let response = router
            .clone()
            .oneshot(
                http::Request::builder()
                    .uri(format!("/catalog/{provider}/ui"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "{provider}");
        let html = String::from_utf8(
            axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        let script = html
            .split("src=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        assert!(script.starts_with("/connections/ui/assets/"));
        let response = router
            .clone()
            .oneshot(
                http::Request::builder()
                    .uri(script)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
    }
}
