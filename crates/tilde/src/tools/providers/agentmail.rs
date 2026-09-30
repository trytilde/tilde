//! AgentMail compose, reply and thread reads on the AgentMail channel connection's inbox.
use super::{ToolProvider, rest};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

const BASE: &str = "https://api.agentmail.to/v0";

pub struct Agentmail;

impl ToolProvider for Agentmail {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let addresses = json!({"type":"array","items":{"type":"string","minLength":3,"maxLength":320},"maxItems":50});
        vec![
            rest::definition(
                "agentmail",
                "agentmail_send_message",
                "Sent an email",
                "Compose an email from the connection's inbox, or reply to `reply_to_message_id`. A new message needs `to`; a reply goes to the original sender (or everyone with `reply_all`).",
                json!({"type":"object","properties":{
                    "reply_to_message_id":{"type":"string","minLength":1},
                    "to":addresses,"cc":addresses,"bcc":addresses,
                    "reply_all":{"type":"boolean"},
                    "subject":{"type":"string","maxLength":998},
                    "content":{"type":"string","minLength":1,"description":"Plain text body."},
                    "html":{"type":"string","description":"Optional HTML body."}
                },"required":["content"],"additionalProperties":false}),
                rest::Hints::default(),
            ),
            rest::definition(
                "agentmail",
                "agentmail_get_thread",
                "Read an email thread",
                "Get a thread of the connection's inbox with its messages.",
                json!({"type":"object","properties":{"thread_id":{"type":"string","minLength":1}},"required":["thread_id"],"additionalProperties":false}),
                rest::Hints {
                    read_only: true,
                    destructive: false,
                },
            ),
        ]
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let inbox = access.secret("inbox_id")?;
            let client = &access.http.client;
            let request = match name {
                "agentmail_get_thread" => {
                    let thread = input["thread_id"].as_str().unwrap_or_default();
                    client.get(access.url(
                        "agentmail_api",
                        BASE,
                        &["inboxes", inbox, "threads", thread],
                    )?)
                }
                "agentmail_send_message" => {
                    let url = match input["reply_to_message_id"].as_str() {
                        Some(message) => access.url(
                            "agentmail_api",
                            BASE,
                            &["inboxes", inbox, "messages", message, "reply"],
                        )?,
                        None if input["to"].as_array().is_some_and(|to| !to.is_empty()) => access
                            .url(
                            "agentmail_api",
                            BASE,
                            &["inboxes", inbox, "messages", "send"],
                        )?,
                        None => {
                            return Err(ConnectError::invalid_argument("A new message needs `to`"));
                        }
                    };
                    let mut body = serde_json::Map::new();
                    for (to, from) in [
                        ("to", "to"),
                        ("cc", "cc"),
                        ("bcc", "bcc"),
                        ("reply_all", "reply_all"),
                        ("subject", "subject"),
                        ("text", "content"),
                        ("html", "html"),
                    ] {
                        if !input[from].is_null() {
                            body.insert(to.into(), input[from].clone());
                        }
                    }
                    client
                        .post(url)
                        .header("Idempotency-Key", call_id.to_string())
                        .json(&body)
                }
                _ => return Err(ConnectError::not_found("Unsupported AgentMail tool")),
            };
            let mut output = rest::send(request.bearer_auth(access.secret("api_key")?)).await?;
            redact_bcc(&mut output);
            Ok(output)
        })
    }
}

/// Blind recipients stay out of tool outputs, which are recorded and shown to the agent.
fn redact_bcc(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| !key.eq_ignore_ascii_case("bcc"));
            map.values_mut().for_each(redact_bcc);
        }
        Value::Array(items) => {
            items.retain(|item| {
                !item["name"]
                    .as_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case("bcc"))
            });
            items.iter_mut().for_each(redact_bcc);
        }
        _ => {}
    }
}
