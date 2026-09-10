//! Postgres is also the durable forwarding queue. External downtime never keeps
//! an ingress request open. Expired retention bounds the backlog on disk.
use crate::{config::SecretEnv, error::Error};
use futures::TryStreamExt;
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use prost::Message;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub struct Destination {
    url: url::Url,
    headers: http::HeaderMap,
    client: reqwest::Client,
}
impl Destination {
    pub fn new(
        endpoint: Option<String>,
        headers: Option<SecretEnv>,
    ) -> Result<Option<Self>, Error> {
        let Some(endpoint) = endpoint else {
            if headers.is_some() {
                return Err(Error::Invalid(
                    "Trace export headers require ENGINE_TRACING_EXPORT_ENDPOINT".into(),
                ));
            }
            return Ok(None);
        };
        let url = url::Url::parse(&endpoint)
            .map_err(|_| Error::Invalid("Invalid trace export endpoint".into()))?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.query().is_some()
        {
            return Err(Error::Invalid("Trace export endpoint must be an HTTP(S) URL without credentials, query or fragment".into()));
        }
        let mut parsed = http::HeaderMap::new();
        if let Some(headers) = headers {
            for item in headers
                .0
                .expose_secret()
                .split(',')
                .filter(|s| !s.trim().is_empty())
            {
                let (key, value) = item.split_once('=').ok_or_else(|| {
                    Error::Invalid(
                        "Trace export headers must be comma-separated name=value pairs".into(),
                    )
                })?;
                let key = key
                    .trim()
                    .parse::<http::HeaderName>()
                    .map_err(|_| Error::Invalid("Invalid trace export header name".into()))?;
                if matches!(
                    key.as_str(),
                    "host"
                        | "content-length"
                        | "content-type"
                        | "content-encoding"
                        | "transfer-encoding"
                ) {
                    return Err(Error::Invalid("Reserved trace export header".into()));
                }
                let mut value = value
                    .trim()
                    .parse::<http::HeaderValue>()
                    .map_err(|_| Error::Invalid("Invalid trace export header value".into()))?;
                value.set_sensitive(true);
                parsed.insert(key, value);
            }
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Invalid("Cannot build trace exporter".into()))?;
        Ok(Some(Self {
            url,
            headers: parsed,
            client,
        }))
    }
}
pub(super) async fn run(
    pool: PgPool,
    destination: Option<Destination>,
    days: i32,
    cancel: CancellationToken,
) {
    let notifications = crate::database::notifications::Notifications::default();
    let mut cleanup = tokio::time::interval(Duration::from_secs(3600));
    let Some(destination) = destination else {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = cleanup.tick() => {
                    if sqlx::query_file!("../../queries/tracing/cleanup.sql", days).execute(&pool).await.is_err() {
                        ::tracing::warn!("Trace retention cleanup failed");
                    }
                }
            }
        }
    };
    let mut changed = loop {
        tokio::select! {
            _ = cancel.cancelled() => return,
            result = notifications.subscribe(&pool, "tilde_tracing") => match result {
                Ok(changed) => break changed,
                Err(_) => ::tracing::warn!("Unable to listen for trace exports"),
            }
        }
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
    };
    changed.mark_changed();
    let mut retry = None;
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = cleanup.tick() => {
                if sqlx::query_file!("../../queries/tracing/cleanup.sql", days).execute(&pool).await.is_err() {
                    ::tracing::warn!("Trace retention cleanup failed");
                }
                continue;
            }
            _ = changed.changed() => {}
            _ = async {
                match retry {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {}
        }
        changed.borrow_and_update();
        let result = async {
            if forward(&pool, &destination).await? {
                // A batch was processed. Drain any remaining due batches immediately.
                changed.mark_changed();
            }
            let row = sqlx::query_file!("../../queries/tracing/next_retry.sql")
                .fetch_one(&pool)
                .await?;
            Ok::<_, Error>(row.retry_at)
        }
        .await;
        retry = match result {
            Ok(next) => next.map(|at| {
                // A past deadline with no claimable rows means another exporter owns them.
                // Its delivery notification wakes us; a bounded retry covers owner failure.
                let delay = (at - chrono::Utc::now())
                    .to_std()
                    .unwrap_or_default()
                    .max(Duration::from_secs(1));
                tokio::time::Instant::now() + delay
            }),
            Err(_) => {
                ::tracing::warn!("Trace forwarding database operation failed");
                Some(tokio::time::Instant::now() + Duration::from_secs(1))
            }
        };
    }
}

async fn forward(pool: &PgPool, destination: &Destination) -> Result<bool, Error> {
    let mut tx = pool.begin().await?;
    let mut rows = sqlx::query_file!("../../queries/tracing/pending.sql").fetch(&mut *tx);
    let mut request = ExportTraceServiceRequest::default();
    let mut trace_ids = vec![];
    let mut span_ids = vec![];
    while let Some(row) = rows.try_next().await? {
        let decoded = ExportTraceServiceRequest::decode(row.otlp.as_slice())
            .map_err(|_| Error::Invalid("Stored trace payload is not valid OTLP".into()))?;
        request.resource_spans.extend(decoded.resource_spans);
        trace_ids.push(row.trace_id);
        span_ids.push(row.span_id);
        // Bound each HTTP export by bytes as well as span count.
        if request.encoded_len() >= 4 * 1024 * 1024 {
            break;
        }
    }
    drop(rows);
    if trace_ids.is_empty() {
        return Ok(false);
    }
    let mut retry_after = None;
    let delivery = match destination
        .client
        .post(destination.url.clone())
        .headers(destination.headers.clone())
        .header(http::header::CONTENT_TYPE, "application/x-protobuf")
        .body(request.encode_to_vec())
        .send()
        .await
    {
        Ok(mut response) if response.status().is_success() => {
            // OTLP partial success is terminal, not retryable. Never log its body.
            let mut body = Vec::new();
            let mut valid = true;
            loop {
                match response.chunk().await {
                    Ok(Some(chunk)) if body.len() + chunk.len() <= 64 * 1024 => {
                        body.extend_from_slice(&chunk)
                    }
                    Ok(None) => break,
                    _ => {
                        valid = false;
                        break;
                    }
                }
            }
            if !valid {
                "pending"
            } else {
                match ExportTraceServiceResponse::decode(body.as_slice()) {
                    Ok(reply)
                        if reply
                            .partial_success
                            .as_ref()
                            .is_some_and(|p| p.rejected_spans > 0) =>
                    {
                        ::tracing::warn!("External collector partially rejected a trace batch");
                        "rejected"
                    }
                    Ok(_) => "sent",
                    Err(_) => "pending",
                }
            }
        }
        Ok(response) if !matches!(response.status().as_u16(), 429 | 502 | 503 | 504) => {
            ::tracing::warn!(
                status = response.status().as_u16(),
                "External collector rejected a trace batch"
            );
            "rejected"
        }
        Ok(response) => {
            retry_after = response
                .headers()
                .get(http::header::RETRY_AFTER)
                .and_then(|h| h.to_str().ok())
                .and_then(|s| {
                    s.parse::<i32>().ok().or_else(|| {
                        chrono::DateTime::parse_from_rfc2822(s).ok().map(|date| {
                            (date.timestamp() - chrono::Utc::now().timestamp()).clamp(0, 86400)
                                as i32
                        })
                    })
                })
                .map(|seconds| seconds.clamp(0, 86400));
            "pending"
        }
        Err(_) => "pending",
    };
    sqlx::query_file!(
        "../../queries/tracing/delivery.sql",
        &trace_ids,
        &span_ids,
        delivery,
        retry_after
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}
