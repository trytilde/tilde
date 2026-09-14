//! Langfuse-specific projection occurs only at the gateway export boundary.
use opentelemetry_proto::tonic::{
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::ResourceSpans,
};
fn text(attributes: &[KeyValue], key: &str) -> Option<String> {
    attributes
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_ref())
        .and_then(|v| match &v.value {
            Some(Value::StringValue(s)) => Some(s.clone()),
            _ => None,
        })
}
fn set(attributes: &mut Vec<KeyValue>, key: &str, value: Value) {
    attributes.retain(|a| a.key != key);
    attributes.push(KeyValue {
        key: key.into(),
        value: Some(AnyValue { value: Some(value) }),
        ..Default::default()
    });
}
pub fn normalize(resources: &mut [ResourceSpans]) {
    for resource in resources {
        for scope in &mut resource.scope_spans {
            for span in &mut scope.spans {
                for (source, target) in [
                    ("tilde.agent.id", "tilde_agent_id"),
                    ("tilde.invocation.id", "tilde_invocation_id"),
                    ("tilde.run.id", "tilde_run_id"),
                    ("tilde.thread.id", "tilde_thread_id"),
                ] {
                    if let Some(value) = text(&span.attributes, source) {
                        set(
                            &mut span.attributes,
                            &format!("langfuse.observation.metadata.{target}"),
                            Value::StringValue(value.clone()),
                        );
                        if source == "tilde.thread.id" {
                            set(
                                &mut span.attributes,
                                "langfuse.session.id",
                                Value::StringValue(value),
                            );
                        }
                    }
                }
                let ai_model =
                    span.name.ends_with(".doGenerate") || span.name.ends_with(".doStream");
                let ai_wrapper = matches!(span.name.as_str(), "ai.generateText" | "ai.streamText")
                    || span.name.ends_with(":ai.generateText")
                    || span.name.ends_with(":ai.streamText");
                let operation = text(&span.attributes, "gen_ai.operation.name").unwrap_or_default();
                let kind = if ai_wrapper {
                    "chain"
                } else if ai_model
                    || matches!(
                        operation.as_str(),
                        "chat" | "text_completion" | "generate_content"
                    )
                {
                    "generation"
                } else if span.name.contains("toolCall")
                    || operation == "execute_tool"
                    || (span.name.ends_with("/InvokeTool") && span.kind == 3)
                {
                    "tool"
                } else if matches!(span.name.as_str(), "agent.invoke" | "tilde.invocation") {
                    "agent"
                } else {
                    "span"
                };
                if text(&span.attributes, "langfuse.observation.type").is_none() {
                    set(
                        &mut span.attributes,
                        "langfuse.observation.type",
                        Value::StringValue(kind.into()),
                    );
                }
                for (sources, target) in [
                    (
                        &["ai.model.id", "gen_ai.request.model"][..],
                        "langfuse.observation.model.name",
                    ),
                    (
                        &[
                            "ai.prompt.messages",
                            "ai.prompt",
                            "gen_ai.input.messages",
                            "gen_ai.prompt",
                            "ai.toolCall.args",
                        ][..],
                        "langfuse.observation.input",
                    ),
                    (
                        &[
                            "ai.response.text",
                            "ai.response.object",
                            "gen_ai.output.messages",
                            "gen_ai.completion",
                            "ai.toolCall.result",
                        ][..],
                        "langfuse.observation.output",
                    ),
                ] {
                    if text(&span.attributes, target).is_none()
                        && let Some(value) =
                            sources.iter().find_map(|key| text(&span.attributes, key))
                    {
                        set(&mut span.attributes, target, Value::StringValue(value));
                    }
                }
                if ai_model
                    && text(
                        &span.attributes,
                        "langfuse.observation.completion_start_time",
                    )
                    .is_none()
                {
                    let milliseconds = span
                        .attributes
                        .iter()
                        .find(|a| a.key == "ai.response.msToFirstChunk")
                        .and_then(|a| a.value.as_ref())
                        .and_then(|a| match a.value {
                            Some(Value::DoubleValue(v)) => Some(v),
                            Some(Value::IntValue(v)) => Some(v as f64),
                            _ => None,
                        });
                    if let Some(ms) = milliseconds.filter(|n| n.is_finite() && *n >= 0.) {
                        let time = span
                            .start_time_unix_nano
                            .saturating_add((ms * 1_000_000.) as u64);
                        if time <= span.end_time_unix_nano
                            && let Some(time) = chrono::DateTime::from_timestamp(
                                (time / 1_000_000_000) as i64,
                                (time % 1_000_000_000) as u32,
                            )
                        {
                            set(
                                &mut span.attributes,
                                "langfuse.observation.completion_start_time",
                                Value::StringValue(time.to_rfc3339()),
                            );
                        }
                    }
                }
                if ai_model {
                    for (source, target) in [
                        ("ai.usage.promptTokens", "gen_ai.usage.input_tokens"),
                        ("ai.usage.completionTokens", "gen_ai.usage.output_tokens"),
                    ] {
                        if let Some(value) = span
                            .attributes
                            .iter()
                            .find(|a| a.key == source)
                            .and_then(|a| a.value.as_ref())
                            .and_then(|v| v.value.clone())
                        {
                            set(&mut span.attributes, target, value);
                        }
                    }
                }
                // AI SDK wrappers summarize their child model calls; do not bill both layers.
                if ai_wrapper {
                    span.attributes.retain(|a| {
                        !a.key.starts_with("gen_ai.usage.")
                            && !a.key.starts_with("ai.usage.")
                            && !matches!(
                                a.key.as_str(),
                                "langfuse.observation.usage_details"
                                    | "langfuse.observation.cost_details"
                            )
                    });
                }
                if span.status.as_ref().is_some_and(|s| s.code == 2) {
                    set(
                        &mut span.attributes,
                        "langfuse.observation.level",
                        Value::StringValue("ERROR".into()),
                    );
                }
            }
        }
    }
}
