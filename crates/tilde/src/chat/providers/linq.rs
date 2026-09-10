//! Linq Partner v3 chat sends, history and reactions, using the existing Tilde provider wire shapes.
use super::*;
use crate::proto::tilde::types::v1 as types;
pub struct Linq;
impl Adapter for Linq {
    fn webhook<'a>(
        &'a self,
        a: &'a Access,
        h: &'a http::HeaderMap,
        body: &'a [u8],
    ) -> BoxFuture<'a, ToolResult<ingress::Webhook>> {
        Box::pin(async move {
            use ingress::*;
            standard(a, h, body, "webhook_signing_secret", "webhook")?;
            let p = payload(body)?;
            let kind = match at(&p, "/event_type") {
                Some("message.received") => IncomingKind::Message,
                Some("message.edited") => IncomingKind::Updated,
                Some("chat.typing_indicator.started") => IncomingKind::Typing(true),
                Some("chat.typing_indicator.stopped") => IncomingKind::Typing(false),
                Some("participant.added") => IncomingKind::Membership(true),
                Some("participant.removed") => IncomingKind::Membership(false),
                _ => return Ok(Webhook::Messages(vec![])),
            };
            let d = &p["data"];
            if at(d, "/chat/owner_handle/handle") != Some(a.secret("phone_number")?) {
                return Err(ConnectError::permission_denied("Linq line mismatch"));
            }
            let sender = at(d, "/sender_handle/handle")
                .or_else(|| at(d, "/participant/handle"))
                .ok_or_else(|| ConnectError::invalid_argument("Missing Linq sender"))?;
            let text = d
                .get("parts")
                .or_else(|| d.pointer("/message/parts"))
                .and_then(Value::as_array)
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|p| {
                            if p["type"] == "text" {
                                p["value"].as_str()
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            let message = at(d, "/id")
                .or_else(|| at(d, "/message/id"))
                .or_else(|| at(&p, "/event_id"))
                .ok_or_else(|| ConnectError::invalid_argument("Missing Linq message"))?;
            Ok(Webhook::Messages(vec![IncomingMessage {
                subject: None,
                kind,
                attachments: d
                    .get("parts")
                    .or_else(|| d.pointer("/message/parts"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .enumerate()
                    .filter_map(|(i, f)| {
                        if f["type"] == "text" {
                            return None;
                        }
                        Some(RemoteAttachment {
                            id: i.to_string(),
                            filename: optional(f, "filename").unwrap_or("attachment").into(),
                            media_type: optional(f, "mime_type")
                                .unwrap_or("application/octet-stream")
                                .into(),
                            size: 0,
                            url: Some(optional(f, "url").or_else(|| optional(f, "value"))?.into()),
                            provider_id: None,
                        })
                    })
                    .collect(),
                event_id: required(&p, "event_id")?.into(),
                message_id: message.into(),
                thread_id: at(d, "/chat/id")
                    .ok_or_else(|| ConnectError::invalid_argument("Missing Linq chat"))?
                    .into(),
                sender_id: sender.into(),
                sender_name: sender.into(),
                text,
                format: "text",
                reply_to: None,
            }]))
        })
    }

    fn tools(&self) -> Vec<types::ToolDefinition> {
        vec![
            tool(
                "sendMessage",
                "Send text to an existing Linq chat (iMessage, RCS or SMS).",
                json!({"chatId":string(),"text":text()}),
                &["chatId", "text"],
                true,
            ),
            tool(
                "reactToMessage",
                "Add or remove a Linq message reaction.",
                json!({"messageId":string(),"operation":{"enum":["add","remove"]},"reaction":{"enum":["love","like","dislike","laugh","emphasize","question"]}}),
                &["messageId", "operation", "reaction"],
                false,
            ),
            tool(
                "getMessages",
                "Read recent messages in a Linq chat.",
                json!({"chatId":string()}),
                &["chatId"],
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
            const BASE: &str = "https://api.linqapp.com/api/partner/v3";
            if name == "reactToMessage" {
                let response=a.json(a.post(a.url("linq_api",BASE,&["messages",required(&v,"messageId")?,"reactions"])?,"api_token")?.json(&json!({"operation":required(&v,"operation")?,"type":required(&v,"reaction")?}))).await?;
                return Ok(response);
            }
            let chat = required(&v, "chatId")?;
            let url = a.url("linq_api", BASE, &["chats", chat, "messages"])?;
            if name == "getMessages" {
                return a
                    .json(
                        a.http
                            .client
                            .get(url)
                            .bearer_auth(a.secret("api_token")?)
                            .query(&[("limit", "50")]),
                    )
                    .await;
            }
            if name != "sendMessage" {
                return Err(ConnectError::not_found("Unknown Linq tool"));
            }
            let response=a.json(a.post(url,"api_token")?.header("Idempotency-Key",c.call_id.to_string()).json(&json!({"message":{"parts":[{"type":"text","value":required(&v,"text")?}]}}))).await?;
            let external = response
                .pointer("/message/id")
                .and_then(Value::as_str)
                .ok_or_else(|| ConnectError::internal("Linq omitted message ID"))?;
            sent(c, a, required(&v, "text")?, "text", chat, external, None).await
        })
    }
}
