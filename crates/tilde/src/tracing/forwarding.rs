//! Langfuse project configuration and OTLP transport, separate from durable delivery.
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
    pub(super) base: Option<url::Url>,
    pub(super) public_base: Option<url::Url>,
}
impl Destination {
    pub fn langfuse(
        base: Option<String>,
        public: Option<String>,
        key: Option<SecretEnv>,
        secret: Option<SecretEnv>,
    ) -> Result<Option<Self>, Error> {
        use base64::Engine as _;
        let (Some(base), Some(key), Some(secret)) = (base, key, secret) else {
            ::tracing::info!("Langfuse is not fully configured; trace ingestion discards payloads");
            return Ok(None);
        };
        if base.trim().is_empty()
            || key.0.expose_secret().is_empty()
            || secret.0.expose_secret().is_empty()
        {
            return Ok(None);
        }
        let base = validate_base(&base)?;
        let public = validate_base(public.as_deref().unwrap_or(base.as_str()))?;
        let credentials = secrecy::SecretString::from(format!(
            "{}:{}",
            key.0.expose_secret(),
            secret.0.expose_secret()
        ));
        let encoded = secrecy::SecretString::from(
            base64::engine::general_purpose::STANDARD.encode(credentials.expose_secret()),
        );
        let headers = SecretEnv(secrecy::SecretString::from(format!(
            "Authorization=Basic {},x-langfuse-ingestion-version=4",
            encoded.expose_secret()
        )));
        let endpoint = format!(
            "{}/api/public/otel/v1/traces",
            base.as_str().trim_end_matches('/')
        );
        let mut result = Self::new(Some(endpoint), Some(headers))?.expect("complete configuration");
        result.base = Some(base);
        result.public_base = Some(public);
        Ok(Some(result))
    }
    pub(super) fn headers(&self) -> http::HeaderMap {
        self.headers.clone()
    }
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
            base: None,
            public_base: None,
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
            // Langfuse v4 may return a JSON queue-job acknowledgement even for
            // protobuf requests. Accept that as well as standard OTLP responses.
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
                            ::tracing::warn!("Langfuse partially rejected a trace batch");
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
            ::tracing::warn!("Langfuse credentials were rejected; retaining queued telemetry");
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

fn validate_base(value: &str) -> Result<url::Url, Error> {
    let url =
        url::Url::parse(value).map_err(|_| Error::Invalid("Invalid Langfuse base URL".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Invalid(
            "Langfuse URL must be HTTP(S) without credentials, query, or fragment".into(),
        ));
    }
    Ok(url)
}

impl Destination {
    /// Logs retain OTLP semantics; Langfuse's trace-specific JSON acknowledgement does not apply.
    pub async fn send_logs(&self, bytes: Vec<u8>) -> Result<(), Duration> {
        use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceResponse;
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
        match ExportLogsServiceResponse::decode(bytes.as_slice()) {
            Ok(r)
                if r.partial_success
                    .as_ref()
                    .is_some_and(|p| p.rejected_log_records > 0) =>
            {
                ::tracing::warn!("External collector partially rejected a log batch");
                Ok(())
            }
            Ok(_) => Ok(()),
            Err(_) => Err(Duration::from_secs(30)),
        }
    }
    pub fn endpoint(&self) -> &str {
        self.url.as_str()
    }
}

impl Destination {
    /// Queue identity changes with destination credentials as well as its URL.
    pub fn queue_identity(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(self.url.as_str());
        let mut headers: Vec<_> = self.headers.iter().collect();
        headers.sort_by_key(|(name, _)| name.as_str());
        for (name, value) in headers {
            hash.update(name.as_str());
            hash.update([0]);
            hash.update(value.as_bytes());
            hash.update([0]);
        }
        hex::encode(hash.finalize())
    }
}
