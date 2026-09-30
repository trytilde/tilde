//! OTLP/HTTP transport to an external collector, separate from durable delivery.
use crate::{config::SecretEnv, error::Error};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use prost::Message as _;
use secrecy::ExposeSecret;
use std::time::Duration;

#[derive(Clone)]
pub struct Destination {
    url: url::Url,
    headers: http::HeaderMap,
    pub(super) client: reqwest::Client,
}
impl Destination {
    pub fn new(
        endpoint: Option<String>,
        headers: Option<SecretEnv>,
    ) -> Result<Option<Self>, Error> {
        let Some(endpoint) = endpoint else {
            if headers.is_some() {
                return Err(Error::Invalid("OTLP headers require their endpoint".into()));
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
pub(super) enum Outcome {
    Finished,
    Retry(Option<Duration>),
}

pub(super) async fn send(
    destination: &Destination,
    request: &ExportTraceServiceRequest,
) -> Outcome {
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
            // Some collectors acknowledge protobuf requests with JSON. Accept that as well
            // as standard OTLP responses.
            let json_response = response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .is_some_and(|v| v.starts_with("application/json"));
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
            } else if json_response {
                match serde_json::from_slice::<serde_json::Value>(&body) {
                    Ok(value) if value.is_object() => {
                        let partial = value
                            .get("partialSuccess")
                            .or_else(|| value.get("partial_success"));
                        let rejected = partial
                            .and_then(|p| {
                                p.get("rejectedSpans").or_else(|| p.get("rejected_spans"))
                            })
                            .and_then(|n| {
                                n.as_i64()
                                    .or_else(|| n.as_str().and_then(|s| s.parse().ok()))
                            })
                            .unwrap_or(0);
                        if rejected > 0 {
                            ::tracing::warn!("External collector partially rejected a trace batch");
                        }
                        Outcome::Finished
                    }
                    _ => Outcome::Retry(None),
                }
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
        Ok(response) if matches!(response.status().as_u16(), 401 | 403) => {
            ::tracing::warn!(
                "External collector rejected the credentials; retaining queued telemetry"
            );
            Outcome::Retry(Some(Duration::from_secs(30)))
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

impl Destination {
    /// Logs use the standard OTLP acknowledgement only.
    pub async fn send_logs(&self, bytes: Vec<u8>) -> Result<(), Duration> {
        use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceResponse;
        self.send_otlp(bytes, |body| {
            ExportLogsServiceResponse::decode(body).map(|r| {
                r.partial_success
                    .is_some_and(|p| p.rejected_log_records > 0)
            })
        })
        .await
    }
    pub async fn send_metrics(&self, bytes: Vec<u8>) -> Result<(), Duration> {
        use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceResponse;
        self.send_otlp(bytes, |body| {
            ExportMetricsServiceResponse::decode(body).map(|r| {
                r.partial_success
                    .is_some_and(|p| p.rejected_data_points > 0)
            })
        })
        .await
    }
    /// `partial` decodes the collector's acknowledgement and reports whether it rejected part
    /// of the batch, which is terminal and logged rather than retried.
    async fn send_otlp(
        &self,
        bytes: Vec<u8>,
        partial: impl Fn(&[u8]) -> Result<bool, prost::DecodeError>,
    ) -> Result<(), Duration> {
        let response = self
            .client
            .post(self.url.clone())
            .headers(self.headers.clone())
            .header(http::header::CONTENT_TYPE, "application/x-protobuf")
            .body(bytes)
            .send()
            .await
            .map_err(|_| Duration::from_secs(2))?;
        let retry = response
            .headers()
            .get(http::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(2)
            .clamp(1, 300);
        if !response.status().is_success() {
            return Err(Duration::from_secs(retry));
        }
        if response.content_length().is_some_and(|n| n > 65536) {
            return Err(Duration::from_secs(30));
        }
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Duration::from_secs(2))? {
            if bytes.len() + chunk.len() > 65536 {
                return Err(Duration::from_secs(30));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Ok(());
        }
        match partial(&bytes) {
            Ok(true) => {
                ::tracing::warn!("External collector partially rejected a batch");
                Ok(())
            }
            Ok(false) => Ok(()),
            Err(_) => Err(Duration::from_secs(30)),
        }
    }
    pub fn endpoint(&self) -> &str {
        self.url.as_str()
    }
}
