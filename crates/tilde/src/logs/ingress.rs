//! Validate OTLP before batching and replace ownership using authenticated request context.
use opentelemetry_proto::tonic::{
    common::v1::{AnyValue, KeyValue, any_value::Value},
    logs::v1::ResourceLogs,
};
use prost::Message as _;
use rotel::topology::payload::{Message, RequestContext};
use sha2::{Digest, Sha256};
use uuid::Uuid;
pub fn validate(message: &Message<ResourceLogs>) -> Result<(), u16> {
    let Some(RequestContext::Http(headers)) = &message.request_context else {
        return Err(400);
    };
    let parent = headers.get("x-tilde-trace-parent").ok_or(400u16)?;
    let trace = parent.split('-').nth(1).filter(|t| !t.is_empty());
    let mut count = 0;
    for resource in &message.payload {
        for scope in &resource.scope_logs {
            for log in &scope.log_records {
                count += 1;
                if count > 4096
                    || log.encoded_len() > 256 * 1024
                    || !(0..=24).contains(&log.severity_number)
                    || !matches!(log.trace_id.len(), 0 | 16)
                    || !matches!(log.span_id.len(), 0 | 8)
                    || (!log.trace_id.is_empty()
                        && trace.is_some_and(|t| hex::encode(&log.trace_id) != t))
                    || (!log.span_id.is_empty()
                        && (log.trace_id.is_empty() || log.span_id.iter().all(|b| *b == 0)))
                    || log.time_unix_nano > i64::MAX as u64
                    || log.observed_time_unix_nano > i64::MAX as u64
                {
                    return Err(400);
                }
            }
        }
    }
    Ok(())
}
pub fn prepare(message: &mut Message<ResourceLogs>) -> Result<String, u16> {
    let Some(RequestContext::Http(headers)) = &message.request_context else {
        return Err(400);
    };
    let mut attributes = Vec::new();
    for (header, key) in [
        ("x-tilde-trace-agent", "tilde.agent.id"),
        ("x-tilde-trace-invocation", "tilde.invocation.id"),
        ("x-tilde-trace-run", "tilde.run.id"),
        ("x-tilde-trace-thread", "tilde.thread.id"),
    ] {
        let value = headers.get(header).ok_or(400u16)?;
        Uuid::parse_str(value).map_err(|_| 400u16)?;
        attributes.push(kv(key, value.clone()));
    }
    for resource in &mut message.payload {
        if let Some(r) = &mut resource.resource {
            r.attributes.retain(|a| !a.key.starts_with("tilde."));
        }
        let resource_bytes = resource
            .resource
            .as_ref()
            .map(|r| r.encode_to_vec())
            .unwrap_or_default();
        for scope in &mut resource.scope_logs {
            if let Some(s) = &mut scope.scope {
                s.attributes.retain(|a| !a.key.starts_with("tilde."));
            }
            let scope_bytes = scope
                .scope
                .as_ref()
                .map(|s| s.encode_to_vec())
                .unwrap_or_default();
            for log in &mut scope.log_records {
                log.attributes.retain(|a| !a.key.starts_with("tilde."));
                log.attributes.extend(attributes.iter().cloned());
                if log.time_unix_nano == 0 {
                    log.time_unix_nano = if log.observed_time_unix_nano > 0 {
                        log.observed_time_unix_nano
                    } else {
                        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default() as u64
                    };
                }
                // Preserve fields omitted by the baseline ClickHouse schema in the standard attributes map.
                log.attributes.push(kv(
                    "tilde.log.observed_time",
                    log.observed_time_unix_nano.to_string(),
                ));
                log.attributes
                    .push(kv("tilde.log.event_name", log.event_name.clone()));
                log.attributes
                    .push(kv("tilde.log.flags", log.flags.to_string()));
                log.attributes.push(kv(
                    "tilde.log.dropped_attributes",
                    log.dropped_attributes_count.to_string(),
                ));
                let mut hash = Sha256::new();
                hash.update(&resource_bytes);
                hash.update(&scope_bytes);
                hash.update(log.encode_to_vec());
                log.attributes
                    .push(kv("tilde.log.id", hex::encode(hash.finalize())));
            }
        }
    }
    Ok(headers["x-tilde-trace-agent"].clone())
}
fn kv(key: &str, value: String) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: Some(AnyValue {
            value: Some(Value::StringValue(value)),
        }),
        ..Default::default()
    }
}
