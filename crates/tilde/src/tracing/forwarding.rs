//! Forward Rotel batches from bounded memory. No trace storage or database outbox.
use crate::{config::SecretEnv, error::Error};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use prost::Message as _;
use secrecy::ExposeSecret;
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
use opentelemetry_proto::tonic::trace::v1::ResourceSpans;
use rotel::{bounded_channel::BoundedReceiver, topology::payload::Message};

enum Outcome {
    Finished,
    Retry(Option<Duration>),
}

pub(super) async fn run(
    mut receiver: BoundedReceiver<Vec<Message<ResourceSpans>>>,
    destination: Option<Destination>,
    cancel: CancellationToken,
) {
    while let Some(batch) = receiver.next().await {
        let Some(destination) = &destination else {
            continue;
        };
        let mut request = ExportTraceServiceRequest::default();
        for mut message in batch {
            if super::ingress::prepare(&mut message).is_err() {
                ::tracing::warn!("Rejected trace batch with invalid internal scope");
                continue;
            }
            request.resource_spans.extend(message.payload);
        }
        if request.resource_spans.is_empty() {
            continue;
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
        let mut backoff = Duration::from_secs(1);
        loop {
            match tokio::time::timeout_at(deadline, send(destination, &request)).await {
                Ok(Outcome::Finished) => break,
                Ok(Outcome::Retry(after)) => {
                    let delay = after.unwrap_or(backoff).max(backoff);
                    if tokio::time::Instant::now() + delay >= deadline {
                        ::tracing::warn!("Trace export retry budget exhausted; batch dropped");
                        break;
                    }
                    tokio::select! {
                        _=cancel.cancelled()=>{::tracing::warn!("Trace export interrupted during retry; batch dropped");break;},
                        _=tokio::time::sleep(delay)=>{},
                    }
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                }
                Err(_) => {
                    ::tracing::warn!("Trace export retry budget exhausted; batch dropped");
                    break;
                }
            }
        }
    }
}
async fn send(destination: &Destination, request: &ExportTraceServiceRequest) -> Outcome {
    match destination
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
                Outcome::Retry(None)
            } else {
                match ExportTraceServiceResponse::decode(body.as_slice()) {
                    Ok(reply)
                        if reply
                            .partial_success
                            .as_ref()
                            .is_some_and(|p| p.rejected_spans > 0) =>
                    {
                        ::tracing::warn!("External collector partially rejected a trace batch");
                        Outcome::Finished
                    }
                    Ok(_) => Outcome::Finished,
                    Err(_) => Outcome::Retry(None),
                }
            }
        }
        Ok(response) if !matches!(response.status().as_u16(), 429 | 502 | 503 | 504) => {
            ::tracing::warn!(
                status = response.status().as_u16(),
                "External collector rejected a trace batch"
            );
            Outcome::Finished
        }
        Ok(response) => {
            let retry_after = response
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
            Outcome::Retry(retry_after.map(|seconds| Duration::from_secs(seconds as u64)))
        }
        Err(_) => Outcome::Retry(None),
    }
}
