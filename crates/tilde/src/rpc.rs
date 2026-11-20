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

static HEARTBEAT_MILLIS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(10_000);

/// How often RPCs that wrap long work in [`with_keepalive`] send a heartbeat (10 s by default,
/// well inside a load balancer's 60 s idle timeout). Tests shorten it to prove the heartbeat,
/// not the work's speed, keeps a call alive behind a short-timeout proxy.
pub fn set_heartbeat_interval(interval: std::time::Duration) {
    let millis = u64::try_from(interval.as_millis())
        .unwrap_or(u64::MAX)
        .max(1);
    HEARTBEAT_MILLIS.store(millis, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn heartbeat_interval() -> std::time::Duration {
    std::time::Duration::from_millis(HEARTBEAT_MILLIS.load(std::sync::atomic::Ordering::Relaxed))
}

/// `items` with a `None` after every heartbeat interval in which it yielded nothing. RPCs whose
/// work can sit silent longer than a proxy's idle timeout stream these: only response bytes keep
/// the whole path alive, because HTTP/2 PINGs end at the first proxy (the load balancer speaks
/// HTTP/1.1 to us). Dropping the stream (the caller went away) drops `items`.
pub(crate) fn with_keepalive<S: futures::Stream + Send + 'static>(
    items: S,
) -> impl futures::Stream<Item = Option<S::Item>> + Send + 'static
where
    S::Item: Send,
{
    use futures::StreamExt;
    let every = heartbeat_interval();
    async_stream::stream! {
        let mut items = std::pin::pin!(items);
        loop {
            match tokio::time::timeout(every, items.next()).await {
                Ok(Some(item)) => yield Some(item),
                Ok(None) => break,
                Err(_) => yield None,
            }
        }
    }
}

/// [`with_keepalive`] for one long call: `None`s while `work` runs, then `Some(output)`.
pub(crate) fn with_heartbeats<T: Send + 'static>(
    work: impl std::future::Future<Output = T> + Send + 'static,
) -> impl futures::Stream<Item = Option<T>> + Send + 'static {
    with_keepalive(futures::stream::once(work))
}

/// The longest a unary RPC waits inline on outside work (a provider, an MCP server, a Lambda):
/// a load balancer cuts a request whose response stays silent for 60 s, so past this the RPC
/// fails with its own clear error instead. Work that may legitimately take longer streams
/// heartbeats ([`with_keepalive`]) or carries on in the background.
pub(crate) const INLINE_LIMIT: std::time::Duration = std::time::Duration::from_secs(45);

/// `work`, failed with DeadlineExceeded when it outlasts [`INLINE_LIMIT`] (dropping it).
pub(crate) async fn inline<T>(
    what: &str,
    work: impl std::future::Future<Output = T>,
) -> Result<T, connectrpc::ConnectError> {
    tokio::time::timeout(INLINE_LIMIT, work).await.map_err(|_| {
        connectrpc::ConnectError::deadline_exceeded(format!(
            "{what} took longer than {} s",
            INLINE_LIMIT.as_secs()
        ))
    })
}
