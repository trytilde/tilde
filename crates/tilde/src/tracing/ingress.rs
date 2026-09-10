//! Validate agent trace ownership independently of any storage backend.
use super::INVOCATION_HEADER;
use opentelemetry_proto::tonic::{
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::ResourceSpans,
};
use rotel::topology::payload::{Message, RequestContext};
use uuid::Uuid;

pub(super) fn validate_request(message: &Message<ResourceSpans>) -> Result<(), u16> {
    let Some(RequestContext::Http(headers)) = &message.request_context else {
        return Err(400);
    };
    let parent = headers.get("x-tilde-trace-parent").ok_or(400u16)?;
    let parts: Vec<_> = parent.split('-').collect();
    if parts.len() != 4 {
        return Err(400);
    }
    for resource in &message.payload {
        for scope in &resource.scope_spans {
            for span in &scope.spans {
                if span.trace_id.len() != 16
                    || span.span_id.len() != 8
                    || span.span_id.iter().all(|b| *b == 0)
                    || !matches!(span.parent_span_id.len(), 0 | 8)
                    || span.end_time_unix_nano < span.start_time_unix_nano
                    || span.end_time_unix_nano > i64::MAX as u64
                    || parts[1] != hex::encode(&span.trace_id)
                    || parts[2] == hex::encode(&span.span_id)
                {
                    return Err(400);
                }
            }
        }
    }
    Ok(())
}
/// Replace agent-provided ownership attributes with the middleware's verified scope.
pub(super) fn prepare(message: &mut Message<ResourceSpans>) -> Result<(), u16> {
    let Some(RequestContext::Http(headers)) = &message.request_context else {
        return if message.request_context.is_none() {
            Ok(())
        } else {
            Err(400)
        };
    };
    let mut attributes = Vec::new();
    for (header, key) in [
        (INVOCATION_HEADER, "tilde.invocation.id"),
        ("x-tilde-trace-agent", "tilde.agent.id"),
        ("x-tilde-trace-run", "tilde.run.id"),
        ("x-tilde-trace-thread", "tilde.thread.id"),
    ] {
        let value = headers.get(header).ok_or(400u16)?;
        value.parse::<Uuid>().map_err(|_| 400u16)?;
        attributes.push(KeyValue {
            key: key.into(),
            value: Some(AnyValue {
                value: Some(Value::StringValue(value.clone())),
            }),
            ..Default::default()
        });
    }
    for resource in &mut message.payload {
        for scope in &mut resource.scope_spans {
            for span in &mut scope.spans {
                span.attributes.retain(|a| !a.key.starts_with("tilde."));
                span.attributes.extend(attributes.iter().cloned());
            }
        }
    }
    Ok(())
}
