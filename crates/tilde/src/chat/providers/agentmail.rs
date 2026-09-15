//! AgentMail compose/reply and thread reads retain email-specific subjects and HTML.
use super::*;
use crate::proto::tilde::types::v1 as types;
pub struct Agentmail;
impl Adapter for Agentmail {
    fn identity_types(&self) -> &'static [types::IdentityType] {
        &[types::IdentityType::Email]
    }
    fn verification_recipient(
        &self,
        identity_type: types::IdentityType,
        value: &str,
    ) -> ToolResult<crate::chat::access::identity::Identity> {
        let recipient = provider_identity(value)?;
        if recipient.identity_type != identity_type {
            return Err(ConnectError::invalid_argument(
                "This provider does not support that identity type",
            ));
        }
        recipient.validate()?;
        Ok(recipient)
    }
    fn send_verification<'a>(
        &'a self,
        a: &'a Access,
        m: crate::chat::access::identity::VerificationMessage<'a>,
    ) -> BoxFuture<'a, ToolResult<()>> {
        Box::pin(async move {
            let text = m.invitation("this email address");
            let escape = |value: &str| {
                value
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;")
                    .replace('\'', "&#39;")
            };
            let html = zeroize::Zeroizing::new(format!(
                "<p>You've received an invitation to access {} from this email address. Please follow <a href=\"{}\">this link</a> to accept.</p><p>{}</p>",
                escape(m.agent_name),
                escape(m.url),
                crate::chat::access::identity::INVITATION_EXPIRY
            ));
            a.json(a.post(a.url("agentmail_api","https://api.agentmail.to/v0",&["inboxes",a.secret("inbox_id")?,"messages","send"])?,"api_key")?.json(&json!({"to":[m.value],"subject":format!("Invitation to access {}", m.agent_name),"text":text.as_str(),"html":html.as_str()}))).await?;
            Ok(())
        })
    }

    fn webhook<'a>(
        &'a self,
        a: &'a Access,
        h: &'a http::HeaderMap,
        body: &'a [u8],
    ) -> BoxFuture<'a, ToolResult<ingress::Webhook>> {
        Box::pin(async move {
            use ingress::*;
            standard(a, h, body, "webhook_secret", "svix")?;
            let mut p = payload(body)?;
            // Spam/blocked/unauthenticated mail is not submitted to the agent automatically.
            if at(&p, "/event_type") != Some("message.received") {
                return Ok(Webhook::Messages(vec![]));
            }
            let m = &p["message"];
            if at(m, "/inbox_id") != Some(a.secret("inbox_id")?) {
                return Err(ConnectError::permission_denied("AgentMail inbox mismatch"));
            }
            if m.get("text").is_none() && m.get("html").is_none() {
                let full = a
                    .json(
                        a.http
                            .client
                            .get(a.url(
                                "agentmail_api",
                                "https://api.agentmail.to/v0",
                                &[
                                    "inboxes",
                                    a.secret("inbox_id")?,
                                    "messages",
                                    required(m, "message_id")?,
                                ],
                            )?)
                            .bearer_auth(a.secret("api_key")?),
                    )
                    .await?;
                if required(&full, "message_id")? != required(m, "message_id")?
                    || required(&full, "inbox_id")? != a.secret("inbox_id")?
                {
                    return Err(ConnectError::failed_precondition(
                        "AgentMail returned a different message",
                    ));
                }
                p["message"] = full;
            }
            let m = &p["message"];
            let sender = m
                .get("from")
                .or_else(|| m.get("from_"))
                .and_then(|v| {
                    v.as_str()
                        .or_else(|| v.as_array().and_then(|v| v.first()).and_then(Value::as_str))
                })
                .ok_or_else(|| ConnectError::invalid_argument("Missing email sender"))?;
            Ok(Webhook::Messages(vec![IncomingMessage {
                subject: optional(m, "subject").map(str::to_owned),
                kind: IncomingKind::Message,
                attachments: m["attachments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|f| {
                        let attachment = required(f, "attachment_id").ok()?;
                        Some(RemoteAttachment {
                            id: attachment.into(),
                            filename: optional(f, "filename").unwrap_or("attachment").into(),
                            media_type: optional(f, "content_type")
                                .unwrap_or("application/octet-stream")
                                .into(),
                            size: f["size"].as_i64().unwrap_or(0),
                            url: None,
                            provider_id: Some(
                                serde_json::to_string(&(
                                    required(m, "message_id").ok()?,
                                    attachment,
                                ))
                                .ok()?,
                            ),
                        })
                    })
                    .collect(),
                event_id: required(&p, "event_id")?.into(),
                message_id: required(m, "message_id")?.into(),
                thread_id: required(m, "thread_id")?.into(),
                sender: provider_identity(sender)?,
                sender_name: sender.into(),
                text: optional(m, "html")
                    .or_else(|| optional(m, "text"))
                    .or_else(|| optional(m, "extracted_text"))
                    .unwrap_or("")
                    .into(),
                format: if m.get("html").is_some() {
                    "html"
                } else {
                    "text"
                },
                reply_to: optional(m, "in_reply_to").map(str::to_owned),
            }]))
        })
    }

    fn tools(&self) -> Vec<types::ToolDefinition> {
        let addresses =
            json!({"type":"array","items":{"type":"string","minLength":1},"maxItems":100});
        vec![
            tool(
                "sendMessage",
                "Compose an email from this inbox. Supports plain text, optional HTML, subject and email recipients.",
                json!({"to":addresses,"cc":addresses,"bcc":addresses,"subject":string(),"text":text(),"html":text()}),
                &["to", "subject", "text"],
                true,
            ),
            tool(
                "replyToMessage",
                "Reply to an email; replyAll sends to all original participants.",
                json!({"messageId":string(),"replyAll":{"type":"boolean"},"text":text(),"html":text()}),
                &["messageId", "text"],
                true,
            ),
            tool(
                "getThread",
                "Read the messages in an AgentMail thread for this inbox.",
                json!({"threadId":string()}),
                &["threadId"],
                false,
            ),
        ]
    }
    fn invoke<'a>(
        &'a self,
        a: &'a Access,
        c: &'a Context,
        name: &'a str,
        v: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let inbox = a.secret("inbox_id")?;
            if name == "getThread" {
                return a
                    .json(
                        a.http
                            .client
                            .get(a.url(
                                "agentmail_api",
                                "https://api.agentmail.to/v0",
                                &["inboxes", inbox, "threads", required(&v, "threadId")?],
                            )?)
                            .bearer_auth(a.secret("api_key")?),
                    )
                    .await;
            }
            let mut body = json!({"text":required(&v,"text")?});
            if let Some(html) = optional(&v, "html") {
                body["html"] = json!(html);
            }
            let (path, _destination) = if name == "sendMessage" {
                for key in ["to", "cc", "bcc", "subject"] {
                    if let Some(value) = v.get(key) {
                        body[key] = value.clone();
                    }
                }
                if v["to"].as_array().is_none_or(Vec::is_empty) {
                    return Err(ConnectError::invalid_argument("Email needs recipients"));
                }
                (
                    vec!["inboxes", inbox, "messages", "send"],
                    v["to"].to_string(),
                )
            } else if name == "replyToMessage" {
                (
                    vec![
                        "inboxes",
                        inbox,
                        "messages",
                        required(&v, "messageId")?,
                        if v["replyAll"].as_bool() == Some(true) {
                            "reply-all"
                        } else {
                            "reply"
                        },
                    ],
                    required(&v, "messageId")?.into(),
                )
            } else {
                return Err(ConnectError::not_found("Unknown AgentMail tool"));
            };
            let response = a
                .json(
                    a.post(
                        a.url("agentmail_api", "https://api.agentmail.to/v0", &path)?,
                        "api_key",
                    )?
                    .header("Idempotency-Key", c.call_id.to_string())
                    .json(&body),
                )
                .await?;
            sent(
                c,
                a,
                optional(&v, "html").unwrap_or(required(&v, "text")?),
                if v.get("html").is_some() {
                    "html"
                } else {
                    "text"
                },
                required(&response, "thread_id")?,
                required(&response, "message_id")?,
                optional(&v, "subject"),
            )
            .await
        })
    }
}

/// Canonical sender values belong to this provider adapter.
fn provider_identity(raw: &str) -> ToolResult<crate::chat::access::identity::Identity> {
    let value = raw.trim();
    let value = if let Some((_, mailbox)) = value.rsplit_once('<') {
        mailbox
            .strip_suffix('>')
            .ok_or_else(|| ConnectError::invalid_argument("Invalid AgentMail recipient"))?
    } else {
        value
    };
    if value.contains(['\r', '\n', ' ', '<', '>']) || value.matches('@').count() != 1 {
        return Err(ConnectError::invalid_argument("Use an email address"));
    }
    Ok(crate::chat::access::identity::Identity {
        identity_type: types::IdentityType::Email,
        value: value.to_lowercase(),
    })
}
