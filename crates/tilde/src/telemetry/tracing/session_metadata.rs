//! Session metadata rides on existing OTLP spans, captured once before durable trace
//! delivery. No identity display values or message bodies are added here.
use super::db;
use crate::{database::GenericClient, error::Error};
use opentelemetry_proto::tonic::{
    common::v1::{AnyValue, ArrayValue, KeyValue, any_value::Value},
    trace::v1::{ResourceSpans, Span},
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(super) fn clear(resources: &mut [ResourceSpans]) {
    let retain = |a: &KeyValue| !a.key.starts_with("tilde.session.");
    for resource in resources {
        if let Some(attributes) = resource.resource.as_mut() {
            attributes.attributes.retain(retain);
        }
        for scope in &mut resource.scope_spans {
            if let Some(attributes) = scope.scope.as_mut() {
                attributes.attributes.retain(retain);
            }
            for span in &mut scope.spans {
                span.attributes.retain(retain);
            }
        }
    }
}
fn scope(span: &Span) -> Option<(Uuid, Uuid)> {
    let get = |key: &str| {
        span.attributes
            .iter()
            .find(|a| a.key == key)
            .and_then(|a| a.value.as_ref())
            .and_then(|v| match &v.value {
                Some(Value::StringValue(s)) => Uuid::parse_str(s).ok(),
                _ => None,
            })
    };
    Some((get("tilde.agent.id")?, get("tilde.thread.id")?))
}
fn put(span: &mut Span, field: &str, value: Value) {
    span.attributes.push(KeyValue {
        key: format!("tilde.session.{field}"),
        value: Some(AnyValue { value: Some(value) }),
        ..Default::default()
    });
}
pub(super) async fn capture(
    db: &impl GenericClient,
    resources: &mut [ResourceSpans],
) -> Result<(), Error> {
    let pairs: BTreeSet<_> = resources
        .iter()
        .flat_map(|r| &r.scope_spans)
        .flat_map(|s| &s.spans)
        .filter_map(scope)
        .collect();
    if pairs.is_empty() {
        return Ok(());
    }
    let (agents, sessions): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
    let metadata: BTreeMap<_, _> = db::session_metadata_all(db, &agents, &sessions)
        .await?
        .into_iter()
        .map(|row| ((row.agent_id, row.session_id), row))
        .collect();
    for span in resources
        .iter_mut()
        .flat_map(|r| &mut r.scope_spans)
        .flat_map(|s| &mut s.spans)
    {
        let Some(row) = scope(span).and_then(|pair| metadata.get(&pair)) else {
            continue;
        };
        for (field, value) in [
            ("provider_id", row.provider_id.clone()),
            ("provider_name", row.provider_name.clone()),
            ("provider_icon_url", row.provider_icon_url.clone()),
            (
                "connection_id",
                row.connection_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
            ),
            (
                "last_turn_time",
                row.last_turn_time
                    .map(|at| at.to_rfc3339())
                    .unwrap_or_default(),
            ),
            ("metadata_time", row.captured_at.to_rfc3339()),
        ] {
            put(span, field, Value::StringValue(value));
        }
        put(span, "message_count", Value::IntValue(row.message_count));
        put(
            span,
            "identity_ids",
            Value::ArrayValue(ArrayValue {
                values: row
                    .identity_ids
                    .iter()
                    .map(|id| AnyValue {
                        value: Some(Value::StringValue(id.to_string())),
                    })
                    .collect(),
            }),
        );
    }
    Ok(())
}
