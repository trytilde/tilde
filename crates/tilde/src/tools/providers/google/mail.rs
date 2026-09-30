//! Gmail: send, search, read, reply, draft, trash and label messages. Outputs are flattened
//! from Gmail's message resources (headers, decoded bodies, attachment metadata).
use super::{Api, DELETE, READ, WRITE, call, optional, string_list, text, tool};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_EMAIL, model};
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::{ToolProvider, rest};
use base64::Engine;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

const ID: &str = "google_mail";
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

pub fn definition() -> model::Provider {
    super::definition(
        ID,
        "Google Mail",
        "Google Mail integration for sending and reading emails via the Gmail API.",
        CATEGORY_EMAIL,
        &[
            "gmail.send",
            "gmail.compose",
            "gmail.readonly",
            "gmail.modify",
            "gmail.labels",
        ],
    )
}

pub struct GoogleMail;

fn compose(extra: Value) -> Value {
    let mut properties = json!({
        "body":{"type":"string","description":"Email body content."},
        "cc":{"type":"string","description":"Optional CC recipients."},
        "bcc":{"type":"string","description":"Optional BCC recipients."},
        "is_html":{"type":"boolean","description":"Whether the body is HTML. Defaults to false."}
    });
    if let (Some(properties), Value::Object(extra)) = (properties.as_object_mut(), extra) {
        properties.extend(extra);
    }
    properties
}
impl ToolProvider for GoogleMail {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let message_id = json!({"type":"string","description":"The ID of the message."});
        vec![
            tool(
                ID,
                "google_mail_send_email",
                "Send an email",
                "Send an email via Gmail API. Requires appropriate Gmail scopes.",
                WRITE,
                compose(json!({"to":{"type":"string"},"subject":{"type":"string"}})),
                &["to", "subject", "body"],
            ),
            tool(
                ID,
                "google_mail_fetch_emails",
                "Search and retrieve emails",
                "Search and retrieve emails using Gmail search syntax",
                READ,
                json!({
                    "query":{"type":"string","description":"Gmail search query (e.g. \"from:user@example.com is:unread\")."},
                    "max_results":{"type":"integer","minimum":0,"description":"Maximum number of messages to return. Defaults to 10."},
                    "page_token":{"type":"string","description":"Page token from a previous response for pagination."},
                    "include_body":{"type":"boolean","description":"Whether to include the message body in each summary. Defaults to false."}
                }),
                &[],
            ),
            tool(
                ID,
                "google_mail_get_email",
                "Retrieve a single email by ID",
                "Retrieve a single email by ID with full headers, body, and attachment metadata",
                READ,
                json!({"message_id":{"type":"string","description":"The ID of the message to retrieve."}}),
                &["message_id"],
            ),
            tool(
                ID,
                "google_mail_get_thread",
                "Retrieve all messages in a conversation thread",
                "Retrieve all messages in a conversation thread",
                READ,
                json!({
                    "thread_id":{"type":"string","description":"The ID of the thread to retrieve."},
                    "include_body":{"type":"boolean","description":"Whether to include message bodies. Defaults to true."}
                }),
                &["thread_id"],
            ),
            tool(
                ID,
                "google_mail_reply_to_thread",
                "Send a reply within an existing email thread",
                "Send a reply within an existing email thread",
                WRITE,
                compose(
                    json!({"thread_id":{"type":"string","description":"The thread ID to reply to."}}),
                ),
                &["thread_id", "body"],
            ),
            tool(
                ID,
                "google_mail_create_draft",
                "Create an email draft without sending",
                "Create an email draft without sending",
                WRITE,
                compose(json!({
                    "to":{"type":"string","description":"Recipient email address."},
                    "subject":{"type":"string","description":"Email subject."},
                    "thread_id":{"type":"string","description":"Optional thread ID to associate the draft with an existing thread."}
                })),
                &["to", "subject", "body"],
            ),
            tool(
                ID,
                "google_mail_trash_email",
                "Move a message to trash",
                "Move a message to trash",
                DELETE,
                json!({"message_id":{"type":"string","description":"The ID of the message to move to trash."}}),
                &["message_id"],
            ),
            tool(
                ID,
                "google_mail_modify_labels",
                "Add or remove labels from a message",
                "Add or remove labels from a message",
                WRITE,
                json!({
                    "message_id":message_id,
                    "add_labels":{"type":"array","items":{"type":"string"},"description":"Label IDs to add to the message."},
                    "remove_labels":{"type":"array","items":{"type":"string"},"description":"Label IDs to remove from the message."}
                }),
                &["message_id"],
            ),
            tool(
                ID,
                "google_mail_list_labels",
                "List all labels",
                "List all labels",
                READ,
                json!({}),
                &[],
            ),
            tool(
                ID,
                "google_mail_create_label",
                "Create a new user label",
                "Create a new user label",
                WRITE,
                json!({
                    "name":{"type":"string","description":"Display name for the label."},
                    "text_color":{"type":"string","description":"Optional text color (hex, e.g. \"#000000\")."},
                    "background_color":{"type":"string","description":"Optional background color (hex, e.g. \"#ffffff\")."}
                }),
                &["name"],
            ),
            tool(
                ID,
                "google_mail_get_attachment",
                "Download an attachment from an email",
                "Download an attachment from an email",
                READ,
                json!({
                    "message_id":{"type":"string","description":"The ID of the message containing the attachment."},
                    "attachment_id":{"type":"string","description":"The ID of the attachment."}
                }),
                &["message_id", "attachment_id"],
            ),
        ]
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let api = Api::new(
                access,
                "google_gmail_api",
                "https://gmail.googleapis.com/gmail/v1",
            );
            match name {
                "google_mail_send_email" => {
                    let raw = raw_email(&input, text(&input, "to"), text(&input, "subject"), None);
                    let sent = call(
                        api.post(&["users", "me", "messages", "send"])?
                            .json(&json!({"raw": raw})),
                    )
                    .await?;
                    let id = sent["id"]
                        .as_str()
                        .ok_or_else(|| ConnectError::unknown("No message ID in response"))?;
                    Ok(json!({"message_id": id}))
                }
                "google_mail_fetch_emails" => fetch(&api, &input).await,
                "google_mail_get_email" => {
                    let message =
                        full_or_metadata(&api, "messages", text(&input, "message_id")).await?;
                    Ok(full_email(&message))
                }
                "google_mail_get_thread" => {
                    let thread =
                        full_or_metadata(&api, "threads", text(&input, "thread_id")).await?;
                    let messages: Vec<Value> = thread["messages"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(full_email)
                        .collect();
                    Ok(
                        json!({"id": thread["id"].as_str().unwrap_or_default(), "messages": messages}),
                    )
                }
                "google_mail_reply_to_thread" => reply(&api, &input).await,
                "google_mail_create_draft" => {
                    let raw = raw_email(&input, text(&input, "to"), text(&input, "subject"), None);
                    let mut message = json!({"raw": raw});
                    if let Some(thread) = optional(&input, "thread_id") {
                        message["threadId"] = json!(thread);
                    }
                    let draft = call(
                        api.post(&["users", "me", "drafts"])?
                            .json(&json!({"message": message})),
                    )
                    .await?;
                    Ok(json!({
                        "draft_id": draft["id"].as_str().unwrap_or_default(),
                        "message_id": draft["message"]["id"].as_str().unwrap_or_default()
                    }))
                }
                "google_mail_trash_email" => {
                    call(
                        api.post(&[
                            "users",
                            "me",
                            "messages",
                            text(&input, "message_id"),
                            "trash",
                        ])?
                        .header(reqwest::header::CONTENT_LENGTH, "0"),
                    )
                    .await?;
                    Ok(json!({"success": true}))
                }
                "google_mail_modify_labels" => {
                    let message = call(
                        api.post(&[
                            "users",
                            "me",
                            "messages",
                            text(&input, "message_id"),
                            "modify",
                        ])?
                        .json(&json!({
                            "addLabelIds": string_list(&input["add_labels"]),
                            "removeLabelIds": string_list(&input["remove_labels"])
                        })),
                    )
                    .await?;
                    Ok(json!({
                        "message_id": message["id"].as_str().unwrap_or_default(),
                        "label_ids": string_list(&message["labelIds"])
                    }))
                }
                "google_mail_list_labels" => {
                    let body = call(api.get(&["users", "me", "labels"])?).await?;
                    let labels: Vec<Value> = body["labels"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|label| {
                            Some(json!({
                                "id": label["id"].as_str()?,
                                "name": label["name"].as_str()?,
                                "type": label["type"].as_str()
                            }))
                        })
                        .collect();
                    Ok(json!({"labels": labels}))
                }
                "google_mail_create_label" => {
                    let mut body = json!({"name": text(&input, "name")});
                    let mut color = serde_json::Map::new();
                    if let Some(value) = optional(&input, "text_color") {
                        color.insert("textColor".into(), json!(value));
                    }
                    if let Some(value) = optional(&input, "background_color") {
                        color.insert("backgroundColor".into(), json!(value));
                    }
                    if !color.is_empty() {
                        body["color"] = Value::Object(color);
                    }
                    let label = call(api.post(&["users", "me", "labels"])?.json(&body)).await?;
                    Ok(json!({
                        "id": label["id"].as_str().unwrap_or_default(),
                        "name": label["name"].as_str().unwrap_or_default()
                    }))
                }
                "google_mail_get_attachment" => {
                    let attachment = call(api.get(&[
                        "users",
                        "me",
                        "messages",
                        text(&input, "message_id"),
                        "attachments",
                        text(&input, "attachment_id"),
                    ])?)
                    .await?;
                    Ok(json!({
                        "data": attachment["data"].as_str().unwrap_or_default(),
                        "size": attachment["size"].as_u64().unwrap_or(0)
                    }))
                }
                _ => Err(ConnectError::not_found("Unsupported Google Mail tool")),
            }
        })
    }
}

/// Lists matching message IDs, then reads each message (metadata only unless bodies are asked
/// for); a message that cannot be read is skipped.
async fn fetch(api: &Api<'_>, input: &Value) -> ToolResult<Value> {
    let mut query = vec![(
        "maxResults",
        input["max_results"].as_u64().unwrap_or(10).to_string(),
    )];
    if let Some(q) = optional(input, "query") {
        query.push(("q", q.to_owned()));
    }
    if let Some(token) = optional(input, "page_token") {
        query.push(("pageToken", token.to_owned()));
    }
    let list = call(api.get(&["users", "me", "messages"])?.query(&query)).await?;
    let include_body = input["include_body"].as_bool().unwrap_or(false);
    let format = if include_body { "full" } else { "metadata" };
    let mut messages = Vec::new();
    for id in list["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|message| message["id"].as_str())
    {
        let request = api
            .get(&["users", "me", "messages", id])?
            .query(&[("format", format)]);
        let (status, bytes) = rest::response(request).await?;
        if !status.is_success() {
            continue;
        }
        let message: Value = serde_json::from_slice(&bytes)
            .map_err(|_| ConnectError::unknown("Failed to parse message"))?;
        let headers = &message["payload"]["headers"];
        messages.push(json!({
            "id": message["id"].as_str().unwrap_or_default(),
            "thread_id": message["threadId"].as_str().unwrap_or_default(),
            "snippet": message["snippet"].as_str().unwrap_or_default(),
            "from": header(headers, "From"),
            "to": header(headers, "To"),
            "subject": header(headers, "Subject"),
            "date": header(headers, "Date"),
            "label_ids": string_list(&message["labelIds"]),
            "body": if include_body { body_text(&message["payload"]) } else { None }
        }));
    }
    Ok(json!({
        "messages": messages,
        "next_page_token": list["nextPageToken"].as_str(),
        "result_size_estimate": list["resultSizeEstimate"].as_u64().unwrap_or(0)
    }))
}

/// Reads a message or thread with `format=full`, falling back to `format=metadata` when the
/// grant refuses full bodies (403).
async fn full_or_metadata(api: &Api<'_>, collection: &str, id: &str) -> ToolResult<Value> {
    let path = ["users", "me", collection, id];
    let (status, bytes) = rest::response(api.get(&path)?.query(&[("format", "full")])).await?;
    let output = if status == reqwest::StatusCode::FORBIDDEN {
        rest::send(api.get(&path)?.query(&[("format", "metadata")])).await?
    } else {
        rest::output(status, &bytes)?
    };
    Ok(output["data"].clone())
}

async fn reply(api: &Api<'_>, input: &Value) -> ToolResult<Value> {
    let thread_id = text(input, "thread_id");
    let thread = call(api.get(&["users", "me", "threads", thread_id])?.query(&[
        ("format", "metadata"),
        ("metadataHeaders", "Message-ID"),
        ("metadataHeaders", "Subject"),
        ("metadataHeaders", "From"),
    ]))
    .await
    .map_err(|error| {
        ConnectError::unknown(format!(
            "Failed to fetch thread: {}",
            error.message.as_deref().unwrap_or_default()
        ))
    })?;
    let headers = thread["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .map(|last| &last["payload"]["headers"])
        .unwrap_or(&Value::Null);
    let message_id = header(headers, "Message-ID").unwrap_or_default();
    let subject = header(headers, "Subject").unwrap_or_default();
    let subject = if subject.starts_with("Re:") || subject.starts_with("re:") {
        subject
    } else {
        format!("Re: {subject}")
    };
    let raw = raw_email(
        input,
        &header(headers, "From").unwrap_or_default(),
        &subject,
        Some(&message_id),
    );
    let sent = call(
        api.post(&["users", "me", "messages", "send"])?
            .json(&json!({"raw": raw, "threadId": thread_id})),
    )
    .await?;
    Ok(json!({
        "message_id": sent["id"].as_str().unwrap_or_default(),
        "thread_id": sent["threadId"].as_str().unwrap_or_default()
    }))
}

/// An RFC 2822 message from the compose fields of `input`, base64url-encoded for Gmail.
fn raw_email(input: &Value, to: &str, subject: &str, in_reply_to: Option<&str>) -> String {
    let mut email = format!("To: {to}\r\nSubject: {subject}\r\n");
    for (name, value) in [
        ("Cc", optional(input, "cc")),
        ("Bcc", optional(input, "bcc")),
        ("In-Reply-To", in_reply_to),
        ("References", in_reply_to),
    ] {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            email.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    let html = input["is_html"].as_bool().unwrap_or(false);
    email.push_str(&format!(
        "Content-Type: text/{}; charset=utf-8\r\n\r\n",
        if html { "html" } else { "plain" }
    ));
    email.push_str(text(input, "body"));
    B64.encode(email.as_bytes())
}

fn header(headers: &Value, name: &str) -> Option<String> {
    headers.as_array()?.iter().find_map(|header| {
        header["name"]
            .as_str()
            .filter(|found| found.eq_ignore_ascii_case(name))
            .and(header["value"].as_str().map(str::to_owned))
    })
}
/// Gmail's base64url bodies may carry `=` padding.
fn decode(data: &Value) -> Option<String> {
    String::from_utf8(B64.decode(data.as_str()?.trim_end_matches('=')).ok()?).ok()
}
/// The first part of `mime` in a (possibly nested multipart) payload.
fn body_of(payload: &Value, mime: &str) -> Option<String> {
    if payload["mimeType"].as_str() == Some(mime)
        && let Some(text) = decode(&payload["body"]["data"])
    {
        return Some(text);
    }
    payload["parts"]
        .as_array()?
        .iter()
        .find_map(|part| body_of(part, mime))
}
/// The payload's own body, else its plain text part, else its HTML part.
fn body_text(payload: &Value) -> Option<String> {
    decode(&payload["body"]["data"])
        .or_else(|| body_of(payload, "text/plain"))
        .or_else(|| body_of(payload, "text/html"))
}
fn attachments(payload: &Value, out: &mut Vec<Value>) {
    if let Some(id) = payload["body"]["attachmentId"].as_str() {
        out.push(json!({
            "attachment_id": id,
            "filename": payload["filename"].as_str().unwrap_or_default(),
            "mime_type": payload["mimeType"].as_str().unwrap_or_default(),
            "size": payload["body"]["size"].as_u64().unwrap_or(0)
        }));
    }
    for part in payload["parts"].as_array().into_iter().flatten() {
        attachments(part, out);
    }
}
fn full_email(message: &Value) -> Value {
    let payload = &message["payload"];
    let headers = &payload["headers"];
    let mut files = Vec::new();
    attachments(payload, &mut files);
    json!({
        "id": message["id"].as_str().unwrap_or_default(),
        "thread_id": message["threadId"].as_str().unwrap_or_default(),
        "from": header(headers, "From"),
        "to": header(headers, "To"),
        "cc": header(headers, "Cc"),
        "subject": header(headers, "Subject"),
        "date": header(headers, "Date"),
        "body_text": body_of(payload, "text/plain"),
        "body_html": body_of(payload, "text/html"),
        "label_ids": string_list(&message["labelIds"]),
        "snippet": message["snippet"].as_str().unwrap_or_default(),
        "attachments": files
    })
}
