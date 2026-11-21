//! A harness's sessions as Tilde records them. A session is one native thread between the
//! person and the agent; its messages are canonical chat messages (so everything that reads
//! chat reads them), and every event is also a span, a turn being one trace, which is what
//! the agent's Sessions and Tracing tabs show.
//!
//! Ids are derived from the harness's own, so resending an event records nothing twice: a
//! message that exists is skipped, and the span store collapses a span sent again.
use super::Caller;
use crate::chat::{Chat, ChatError, Result, activity, db as chat_db, providers::ingress::stable};
use crate::proto::tilde::local::v1 as wire;
use chrono::{DateTime, Utc};
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::{ResourceSpans, ScopeSpans, Span, Status},
};
use prost::Message as _;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MAX_EVENTS: usize = 500;
const MAX_TEXT: usize = 1024 * 1024;
/// The hook cuts tool payloads to 64 KiB; the margin is for its truncation notice.
const MAX_TOOL: usize = 65 * 1024;

fn at(value: Option<&buffa_types::google::protobuf::Timestamp>) -> Option<DateTime<Utc>> {
    value.and_then(|t| DateTime::from_timestamp(t.seconds, t.nanos.max(0) as u32))
}
fn invalid(message: &str) -> ChatError {
    ChatError::Invalid(message.into())
}
fn validate(request: &wire::IngestRequest) -> Result<()> {
    let id = |value: &str| !value.is_empty() && value.len() <= 200;
    if !id(&request.session_id) || request.events.len() > MAX_EVENTS {
        return Err(invalid("A session id and at most 500 events are required"));
    }
    for event in &request.events {
        let sized = match &event.detail {
            Some(wire::event::Detail::User(m)) => !m.text.is_empty() && m.text.len() <= MAX_TEXT,
            Some(wire::event::Detail::Agent(m)) => {
                !m.text.is_empty() && m.text.len() <= MAX_TEXT && m.model.len() <= 200
            }
            Some(wire::event::Detail::Tool(t)) => {
                id(&t.name) && t.input.len() <= MAX_TOOL && t.output.len() <= MAX_TOOL
            }
            None => false,
        };
        if !sized || !id(&event.id) || !id(&event.turn_id) {
            return Err(invalid("Invalid session event"));
        }
    }
    Ok(())
}

/// The conversation identity of the person a local agent's sessions are with.
async fn person(_db: &impl crate::database::GenericClient, _agent: Uuid) -> Result<String> {
    Ok("local".into())
}

pub(super) async fn ingest(
    chat: &Chat,
    traces: Option<&crate::telemetry::tracing::delivery::Queue>,
    caller: &Caller,
    request: wire::IngestRequest,
) -> Result<()> {
    validate(&request)?;
    let Caller {
        agent, connection, ..
    } = *caller;
    let pool = chat.pg()?;
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let person = person(&tx, agent).await?;
    let user = stable(connection, "user", &format!("username:{person}"));
    chat_db::channel_user_execute(&tx, user, &person).await?;
    crate::chat::access::db::identity_upsert_one(&tx, user, connection, "username", &person)
        .await?;
    // The agent's key vouches for the identity, as an application's does.
    crate::chat::providers::tilde::db::attest_identity(&tx, user).await?;
    let thread = stable(connection, "local-thread", &request.session_id);
    let title: String = request.title.trim().chars().take(200).collect();
    let title = if title.is_empty() { "Session" } else { &title };
    let created = chat_db::channel_thread_create_execute(&tx, thread, title, agent).await? > 0;
    chat_db::thread_lock_one(&tx, thread).await?;
    let human = stable(connection, "participant", &format!("{thread}:{user}"));
    let assistant = stable(connection, "agent", &thread.to_string());
    for (participant, user, agent) in [(human, Some(user), None), (assistant, None, Some(agent))] {
        if chat_db::participant_join_execute(&tx, participant, thread, user, agent).await? > 0 {
            activity(pool, &tx, thread, "participant.joined", participant, "").await?;
        }
    }
    if created {
        activity(pool, &tx, thread, "thread.created", thread, "").await?;
    }
    let now = Utc::now();
    for event in &request.events {
        let (participant, text) = match &event.detail {
            Some(wire::event::Detail::User(m)) => (human, &m.text),
            Some(wire::event::Detail::Agent(m)) => (assistant, &m.text),
            _ => continue,
        };
        let message = stable(
            connection,
            "local-message",
            &format!("{thread}:{}", event.id),
        );
        let said = at(event.at.as_option()).unwrap_or(now);
        if super::db::message_import_execute(&tx, message, thread, participant, text, said).await? {
            activity(pool, &tx, thread, "message.completed", message, "").await?;
        }
    }
    tx.commit().await?;
    drop(client);
    if let Some(traces) = traces {
        let mut resources = vec![ResourceSpans {
            scope_spans: vec![ScopeSpans {
                spans: request
                    .events
                    .iter()
                    .filter_map(|event| span(caller, thread, event, now))
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }];
        crate::telemetry::tracing::mapping::normalize(&mut resources);
        let batch = ExportTraceServiceRequest {
            resource_spans: resources,
        };
        traces
            .accept(&batch.encode_to_vec())
            .await
            .map_err(|_| ChatError::Transport)?;
    }
    Ok(())
}

fn digest<const N: usize>(thread: Uuid, kind: &str, key: &str) -> Vec<u8> {
    Sha256::digest(format!("{thread}:{kind}:{key}"))[..N].to_vec()
}
/// One event as an observation. The session transcript reads messages from generations only,
/// so the person's prompt is a generation with an input and no output.
fn span(caller: &Caller, thread: Uuid, event: &wire::Event, now: DateTime<Utc>) -> Option<Span> {
    let nanos = |time: DateTime<Utc>| time.timestamp_nanos_opt().unwrap_or_default().max(0) as u64;
    let end = at(event.at.as_option()).unwrap_or(now);
    let mut start = end;
    let text = |key: &str, value: &str| KeyValue {
        key: key.into(),
        value: Some(AnyValue {
            value: Some(Value::StringValue(value.into())),
        }),
        ..Default::default()
    };
    let mut attributes = vec![
        text("tilde.agent.id", &caller.agent.to_string()),
        text("tilde.thread.id", &thread.to_string()),
    ];
    let mut status = None;
    let name = match event.detail.as_ref()? {
        wire::event::Detail::User(m) => {
            attributes.push(text("tilde.observation.type", "generation"));
            attributes.push(text("tilde.observation.input", &m.text));
            "prompt".to_owned()
        }
        wire::event::Detail::Agent(m) => {
            attributes.push(text("tilde.observation.type", "generation"));
            attributes.push(text("tilde.observation.output", &m.text));
            if !m.model.is_empty() {
                attributes.push(text("tilde.observation.model", &m.model));
            }
            "response".to_owned()
        }
        wire::event::Detail::Tool(t) => {
            start = at(t.started_at.as_option())
                .filter(|s| *s <= end)
                .unwrap_or(end);
            attributes.push(text("tilde.observation.type", "tool"));
            attributes.push(text("tilde.observation.input", &t.input));
            attributes.push(text("tilde.observation.output", &t.output));
            attributes.push(text("gen_ai.tool.name", &t.name));
            attributes.push(text("gen_ai.tool.call.id", &event.id));
            if t.failed {
                status = Some(Status {
                    code: 2,
                    message: "Tool failed".into(),
                });
            }
            t.name.clone()
        }
    };
    Some(Span {
        trace_id: digest::<16>(thread, "trace", &event.turn_id),
        span_id: digest::<8>(thread, "span", &event.id),
        name: format!("{} {name}", caller.harness.as_str()),
        kind: 1,
        start_time_unix_nano: nanos(start),
        end_time_unix_nano: nanos(end),
        attributes,
        status,
        ..Default::default()
    })
}
