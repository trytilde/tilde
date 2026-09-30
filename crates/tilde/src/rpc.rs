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

/// A list request's optional search term: trimmed, blank as none, at most 200 bytes as other
/// list searches allow.
pub(crate) fn search(value: Option<&str>) -> Result<Option<&str>, crate::error::Error> {
    let value = value.map(str::trim).filter(|v| !v.is_empty());
    if value.is_some_and(|v| v.len() > 200) {
        return Err(crate::error::Error::Invalid("Search is too long".into()));
    }
    Ok(value)
}
