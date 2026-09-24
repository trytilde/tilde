//! Mount every method in explicitly selected services. Audience membership lives in contracts.
pub(crate) fn mount(router: connectrpc::Router, max_bytes: usize) -> axum::Router {
    let paths = router.methods().map(str::to_owned).collect::<Vec<_>>();
    let service = router.into_axum_service().with_limits(
        connectrpc::Limits::default()
            .with_max_message_size(max_bytes)
            .with_max_request_body_size(max_bytes),
    );
    paths.into_iter().fold(axum::Router::new(), |router, path| {
        router.route_service(
            &format!("/{}", path.trim_start_matches('/')),
            service.clone(),
        )
    })
}
