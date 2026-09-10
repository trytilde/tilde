//! Trace context survives both HTTP body streaming and durable invocation dispatch.
use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use http_body::{Frame, SizeHint};
use opentelemetry::{
    Context, KeyValue,
    propagation::{Extractor, Injector, TextMapPropagator},
    trace::{FutureExt, SpanKind, TraceContextExt, Tracer},
};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use std::{
    pin::Pin,
    task::{Context as TaskContext, Poll},
};

struct Headers<'a>(&'a http::HeaderMap);
impl Extractor for Headers<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.to_str().ok()
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}
struct WriteHeaders<'a>(&'a mut http::HeaderMap);
impl Injector for WriteHeaders<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(k), Ok(v)) = (key.parse::<http::HeaderName>(), value.parse()) {
            self.0.insert(k, v);
        }
    }
}
pub fn extract(headers: &http::HeaderMap) -> Context {
    TraceContextPropagator::new().extract(&Headers(headers))
}
pub fn inject(headers: &mut http::HeaderMap) {
    TraceContextPropagator::new().inject_context(&Context::current(), &mut WriteHeaders(headers));
}
pub fn capture() -> (String, String) {
    let mut headers = http::HeaderMap::new();
    inject(&mut headers);
    (
        headers
            .get("traceparent")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_owned(),
        headers
            .get("tracestate")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_owned(),
    )
}
pub fn restore(parent: &str, state: &str) -> Context {
    let mut headers = http::HeaderMap::new();
    if let Ok(v) = parent.parse() {
        headers.insert("traceparent", v);
    }
    if let Ok(v) = state.parse() {
        headers.insert("tracestate", v);
    }
    extract(&headers)
}
pub fn start(
    name: impl Into<std::borrow::Cow<'static, str>>,
    kind: SpanKind,
    parent: &Context,
    attributes: Vec<KeyValue>,
) -> Context {
    let span = opentelemetry::global::tracer("tilde")
        .span_builder(name)
        .with_kind(kind)
        .with_attributes(attributes)
        .start_with_context(&opentelemetry::global::tracer("tilde"), parent);
    parent.with_span(span)
}

/// Does not record headers, query strings, request bodies, or telemetry uploads.
pub async fn request(request: Request, next: Next) -> Response {
    if request.uri().path() == super::AGENT_TRACES_PATH {
        return next.run(request).await;
    }
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|p| p.as_str())
        .unwrap_or("unmatched")
        .to_owned();
    let parent = extract(request.headers());
    let cx = start(
        format!("{} {}", request.method(), route),
        SpanKind::Server,
        &parent,
        vec![
            KeyValue::new("http.request.method", request.method().to_string()),
            KeyValue::new("http.route", route),
        ],
    );
    let response = next.run(request).with_context(cx.clone()).await;
    cx.span().set_attribute(KeyValue::new(
        "http.response.status_code",
        i64::from(response.status().as_u16()),
    ));
    if response.status().is_server_error() {
        cx.span()
            .set_status(opentelemetry::trace::Status::error("HTTP server error"));
    }
    let (parts, body) = response.into_parts();
    Response::from_parts(
        parts,
        Body::new(TracedBody {
            body,
            cx,
            ended: false,
        }),
    )
}
struct TracedBody {
    body: Body,
    cx: Context,
    ended: bool,
}
impl TracedBody {
    fn end(&mut self) {
        if !self.ended {
            self.cx.span().end();
            self.ended = true;
        }
    }
}
impl http_body::Body for TracedBody {
    type Data = axum::body::Bytes;
    type Error = axum::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let context = self.cx.clone();
        let _guard = context.attach();
        let poll = Pin::new(&mut self.body).poll_frame(cx);
        if matches!(poll, Poll::Ready(None)) {
            self.end();
        }
        if matches!(poll, Poll::Ready(Some(Err(_)))) {
            self.cx
                .span()
                .set_status(opentelemetry::trace::Status::error(
                    "response stream failed",
                ));
            self.end();
        }
        poll
    }
    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}
impl Drop for TracedBody {
    fn drop(&mut self) {
        self.end();
    }
}

pub struct EndOnDrop(pub Context);
impl Drop for EndOnDrop {
    fn drop(&mut self) {
        self.0.span().end();
    }
}
