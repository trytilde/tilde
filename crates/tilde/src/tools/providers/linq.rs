//! Linq Partner API V3 business verbs (iMessage, RCS and SMS) on the Linq channel connection's
//! API token. Linq's own MCP server is arbitrary TypeScript execution, so these are typed tools.
//! Inputs keep the Partner API's shapes: path IDs as fields, request bodies under `body` and
//! query parameters under `query`.
use super::{ToolProvider, rest};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::{Map, Value, json};

const BASE: &str = "https://api.linqapp.com/api/partner/v3";

pub struct Linq;

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn id() -> Value {
    json!({"type":"string","minLength":1,"maxLength":256})
}
fn body() -> Value {
    json!({"type":"object","description":"Request body in the Linq Partner API V3 shape."})
}
fn query() -> Value {
    json!({"type":"object","additionalProperties":{"type":["string","number","boolean"]},"description":"Query parameters, e.g. pagination cursors and filters."})
}
fn message() -> Value {
    json!({"type":"object","description":"Linq MessageContent: {\"parts\":[{\"type\":\"text\",\"value\":\"...\"}]} with optional media parts."})
}

impl ToolProvider for Linq {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let read = rest::Hints {
            read_only: true,
            destructive: false,
        };
        let write = rest::Hints::default();
        let destructive = rest::Hints {
            read_only: false,
            destructive: true,
        };
        let only_id = || object(json!({"id":id()}), &["id"]);
        let id_body = || object(json!({"id":id(),"body":body()}), &["id", "body"]);
        let only_query = || object(json!({"query":query()}), &[]);
        [
            (
                "linq_send_message",
                "Sent a Linq message",
                "Send to recipients without choosing a from line; `from` is refused. This is the recommended Linq path: Linq reuses healthy chats and load-balances and fails over across the account's line pool.",
                object(
                    json!({
                        "to":{"type":"array","items":{"type":"string","minLength":1},"minItems":1,"description":"Recipient handles. One starts or reuses a direct chat; several identify a group chat."},
                        "message":message(),
                        "exclude_from":{"type":"array","items":{"type":"string"},"description":"Lines not to select when Linq must create a new chat."},
                        "continuation_message":{"type":"object","description":"Text-only replacement used only when Linq fails over to a new line."},
                        "override_optout":{"type":"boolean","description":"One-request opt-out override for a single courtesy confirmation."}
                    }),
                    &["to", "message"],
                ),
                write,
            ),
            (
                "linq_send_chat_message",
                "Sent a Linq chat message",
                "Send a message in an existing Linq chat after checking its health. OPTED_OUT and CRITICAL chats are refused. A 2024 opt-out error from Linq is terminal and must not be retried.",
                object(
                    json!({"id":id(),"message":message(),"override_optout":{"type":"boolean"}}),
                    &["id", "message"],
                ),
                write,
            ),
            ("linq_list_chats", "Listed Linq chats", "List chats on the account.", only_query(), read),
            ("linq_get_chat", "Read a Linq chat", "Get one chat, including its participants and health status.", only_id(), read),
            (
                "linq_list_chat_messages",
                "Read Linq chat messages",
                "List messages in a chat, newest first.",
                object(json!({"id":id(),"query":query()}), &["id"]),
                read,
            ),
            ("linq_get_message", "Read a Linq message", "Get one message.", only_id(), read),
            ("linq_edit_message", "Edited a Linq message", "Edit a sent message (PATCH /messages/{id}).", id_body(), write),
            ("linq_delete_message", "Deleted a Linq message", "Delete a message.", only_id(), destructive),
            (
                "linq_react_to_message",
                "Reacted to a Linq message",
                "Add or remove a reaction on a message; body is {\"operation\":\"add\"|\"remove\",\"type\":\"love\"|\"like\"|\"dislike\"|\"laugh\"|\"emphasize\"|\"question\"}.",
                id_body(),
                write,
            ),
            ("linq_create_poll", "Created a Linq poll", "Create a poll in the chat `id`.", id_body(), write),
            ("linq_get_poll", "Read a Linq poll", "Get the poll carried by message `id`.", only_id(), read),
            ("linq_add_poll_options", "Added Linq poll options", "Add options to the poll carried by message `id`.", id_body(), write),
            ("linq_vote_on_poll", "Voted on a Linq poll", "Toggle a vote on the poll carried by message `id`.", id_body(), write),
            (
                "linq_manage_participants",
                "Changed Linq chat participants",
                "Add or remove participants of a group chat; body names the handles.",
                object(
                    json!({"chat_id":id(),"operation":{"enum":["add","remove"]},"body":body()}),
                    &["chat_id", "operation", "body"],
                ),
                write,
            ),
            (
                "linq_list_phone_numbers",
                "Listed Linq phone lines",
                "List the account's phone lines and their current reputation, so traffic on unhealthy lines can be paused or slowed.",
                only_query(),
                read,
            ),
            (
                "linq_get_available_number",
                "Chose a Linq onboarding number",
                "Choose a healthy line and contact card for onboarding one new contact. Do not call this before every send.",
                only_query(),
                read,
            ),
            (
                "linq_check_capability",
                "Checked iMessage or RCS capability",
                "Check whether a handle can receive iMessage or RCS; body names the handle.",
                object(
                    json!({"service":{"enum":["imessage","rcs"]},"body":body()}),
                    &["service", "body"],
                ),
                read,
            ),
            (
                "linq_manage_blocked_handle",
                "Changed Linq blocked handles",
                "List, block or unblock handles. `list` reads `query`; `block` sends `body`; `unblock` sends both.",
                object(
                    json!({"operation":{"enum":["list","block","unblock"]},"body":body(),"query":query()}),
                    &["operation"],
                ),
                write,
            ),
            ("linq_get_contact_cards", "Read Linq contact cards", "Get the account's contact cards.", only_query(), read),
            (
                "linq_setup_contact_card",
                "Set up a Linq contact card",
                "Create the contact card for a line.",
                object(json!({"body":body()}), &["body"]),
                write,
            ),
            (
                "linq_update_contact_card",
                "Updated a Linq contact card",
                "Update the contact card of `phone_number`.",
                object(
                    json!({"phone_number":{"type":"string","minLength":1},"body":body()}),
                    &["phone_number", "body"],
                ),
                write,
            ),
            ("linq_share_contact_card", "Shared a Linq contact card", "Share the line's contact card in chat `id`.", only_id(), write),
            (
                "linq_list_webhook_events",
                "Listed Linq webhook events",
                "List the webhook event types Linq can deliver.",
                object(json!({}), &[]),
                read,
            ),
            (
                "linq_manage_webhook_subscription",
                "Changed Linq webhook subscriptions",
                "List, get, create, update or delete webhook subscriptions. get, update and delete need `subscription_id`. Creating returns a signing secret once; store it securely.",
                object(
                    json!({"operation":{"enum":["list","get","create","update","delete"]},"subscription_id":id(),"body":body()}),
                    &["operation"],
                ),
                write,
            ),
        ]
        .into_iter()
        .map(|(name, summary, description, schema, hints)| {
            rest::definition("linq", name, summary, description, schema, hints)
        })
        .collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let mut input = match input {
                Value::Object(input) => input,
                _ => Map::new(),
            };
            let id = text(&mut input, "id");
            let query = match input.remove("query") {
                Some(Value::Object(query)) => query,
                _ => Map::new(),
            };
            let body = input.remove("body").filter(Value::is_object);
            let none = Map::new;
            let (method, path, query, body) = match name {
                // Sends carry the call ID so a retried call cannot message twice.
                "linq_send_message" => {
                    let url = ["messages".into()];
                    return call(
                        access,
                        Method::POST,
                        &url,
                        none(),
                        Some(Value::Object(input)),
                        Some(call_id),
                    )
                    .await;
                }
                "linq_send_chat_message" => {
                    let id = id?;
                    healthy(access, &id).await?;
                    let url = ["chats".into(), id, "messages".into()];
                    return call(
                        access,
                        Method::POST,
                        &url,
                        none(),
                        Some(Value::Object(input)),
                        Some(call_id),
                    )
                    .await;
                }
                "linq_list_chats" => (Method::GET, vec!["chats".into()], query, None),
                "linq_get_chat" => (Method::GET, vec!["chats".into(), id?], none(), None),
                "linq_list_chat_messages" => (
                    Method::GET,
                    vec!["chats".into(), id?, "messages".into()],
                    query,
                    None,
                ),
                "linq_get_message" => (Method::GET, vec!["messages".into(), id?], none(), None),
                "linq_edit_message" => (Method::PATCH, vec!["messages".into(), id?], none(), body),
                "linq_delete_message" => {
                    (Method::DELETE, vec!["messages".into(), id?], none(), None)
                }
                "linq_react_to_message" => (
                    Method::POST,
                    vec!["messages".into(), id?, "reactions".into()],
                    none(),
                    body,
                ),
                "linq_create_poll" => (
                    Method::POST,
                    vec!["chats".into(), id?, "polls".into()],
                    none(),
                    body,
                ),
                "linq_get_poll" => (
                    Method::GET,
                    vec!["messages".into(), id?, "poll".into()],
                    none(),
                    None,
                ),
                "linq_add_poll_options" => (
                    Method::POST,
                    vec!["messages".into(), id?, "poll".into(), "options".into()],
                    none(),
                    body,
                ),
                "linq_vote_on_poll" => (
                    Method::POST,
                    vec!["messages".into(), id?, "poll".into(), "votes".into()],
                    none(),
                    body,
                ),
                "linq_manage_participants" => {
                    let method = if input["operation"] == "add" {
                        Method::POST
                    } else {
                        Method::DELETE
                    };
                    (
                        method,
                        vec![
                            "chats".into(),
                            text(&mut input, "chat_id")?,
                            "participants".into(),
                        ],
                        none(),
                        body,
                    )
                }
                "linq_list_phone_numbers" => {
                    (Method::GET, vec!["phone_numbers".into()], query, None)
                }
                "linq_get_available_number" => {
                    (Method::GET, vec!["available_number".into()], query, None)
                }
                "linq_check_capability" => {
                    let service = text(&mut input, "service")?;
                    (
                        Method::POST,
                        vec!["capability".into(), format!("check_{service}")],
                        none(),
                        body,
                    )
                }
                "linq_manage_blocked_handle" => match input["operation"].as_str() {
                    Some("list") => (Method::GET, vec!["blocked_handles".into()], query, None),
                    Some("block") => (Method::POST, vec!["blocked_handles".into()], none(), body),
                    _ => (Method::DELETE, vec!["blocked_handles".into()], query, body),
                },
                "linq_get_contact_cards" => (Method::GET, vec!["contact_card".into()], query, None),
                "linq_setup_contact_card" => {
                    (Method::POST, vec!["contact_card".into()], none(), body)
                }
                "linq_update_contact_card" => {
                    let number = text(&mut input, "phone_number")?;
                    let query = Map::from_iter([("phone_number".into(), Value::String(number))]);
                    (Method::PATCH, vec!["contact_card".into()], query, body)
                }
                "linq_share_contact_card" => (
                    Method::POST,
                    vec!["chats".into(), id?, "share_contact_card".into()],
                    none(),
                    None,
                ),
                "linq_list_webhook_events" => {
                    (Method::GET, vec!["webhook-events".into()], none(), None)
                }
                "linq_manage_webhook_subscription" => {
                    let operation = input["operation"].as_str().unwrap_or_default().to_owned();
                    let mut path = vec!["webhook-subscriptions".to_owned()];
                    if let Ok(subscription) = text(&mut input, "subscription_id") {
                        path.push(subscription);
                    } else if ["get", "update", "delete"].contains(&operation.as_str()) {
                        return Err(ConnectError::invalid_argument(format!(
                            "`{operation}` needs `subscription_id`"
                        )));
                    }
                    match operation.as_str() {
                        "list" | "get" => (Method::GET, path, none(), None),
                        "create" => (Method::POST, path, none(), body),
                        "update" => (Method::PUT, path, none(), body),
                        _ => (Method::DELETE, path, none(), None),
                    }
                }
                _ => return Err(ConnectError::not_found("Unsupported Linq tool")),
            };
            call(access, method, &path, query, body, None).await
        })
    }
}

fn text(input: &mut Map<String, Value>, key: &str) -> ToolResult<String> {
    match input.remove(key) {
        Some(Value::String(value)) if !value.is_empty() => Ok(value),
        _ => Err(ConnectError::invalid_argument(format!(
            "`{key}` is required"
        ))),
    }
}

async fn call(
    access: &Access,
    method: Method,
    path: &[String],
    query: Map<String, Value>,
    body: Option<Value>,
    idempotency: Option<uuid::Uuid>,
) -> ToolResult<Value> {
    let segments: Vec<&str> = path.iter().map(String::as_str).collect();
    let mut url = access.url("linq_api", BASE, &segments)?;
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in &query {
            match value {
                Value::String(value) => pairs.append_pair(key, value),
                Value::Number(_) | Value::Bool(_) => pairs.append_pair(key, &value.to_string()),
                _ => continue,
            };
        }
    }
    if url.query() == Some("") {
        url.set_query(None);
    }
    let mut builder = access
        .http
        .client
        .request(method, url)
        .bearer_auth(access.secret("api_token")?);
    if let Some(key) = idempotency {
        builder = builder.header("Idempotency-Key", key.to_string());
    }
    if let Some(body) = body {
        builder = builder.json(&body);
    }
    rest::send(builder).await
}

/// Refuse to message a chat Linq reports as opted out or paused.
async fn healthy(access: &Access, chat: &str) -> ToolResult<()> {
    let chat = call(
        access,
        Method::GET,
        &["chats".into(), chat.into()],
        Map::new(),
        None,
        None,
    )
    .await?;
    let status = chat
        .pointer("/data/health_status/status")
        .or_else(|| chat.pointer("/data/chat/health_status/status"))
        .and_then(Value::as_str);
    match status {
        Some("OPTED_OUT") => Err(ConnectError::failed_precondition(
            "The Linq chat is OPTED_OUT; do not send until a new inbound reply clears it",
        )),
        Some("CRITICAL") => Err(ConnectError::failed_precondition(
            "The Linq chat health is CRITICAL; outbound messaging is paused",
        )),
        _ => Ok(()),
    }
}
