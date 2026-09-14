//! Meta and Telnyx WhatsApp share message shapes, but advertise different runtime tools.
//! Reference: tilde-api's mcp-provider-whatsapp/{api,telnyx}.rs and chat provider egress.
use super::*;
use crate::proto::tilde::types::v1 as types;
pub struct Whatsapp {
    pub telnyx: bool,
}
impl Adapter for Whatsapp {
    fn identity_types(&self) -> &'static [types::IdentityType] {
        &[types::IdentityType::PhoneNumber]
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
    fn supports_verification_template(&self) -> bool {
        true
    }
    fn send_verification<'a>(
        &'a self,
        a: &'a Access,
        m: crate::chat::access::identity::VerificationMessage<'a>,
    ) -> BoxFuture<'a, ToolResult<()>> {
        Box::pin(async move {
            let mut body = if let Some(name) = m.template_name {
                json!({"type":"template","template":{"name":name,"language":{"code":m.template_language.unwrap_or("en")},"components":[{"type":"body","parameters":[{"type":"text","text":m.url}]}]}})
            } else {
                json!({"type":"text","text":{"body":m.text,"preview_url":false}})
            };
            if self.telnyx {
                a.json(a.post(a.url("telnyx_api","https://api.telnyx.com/v2",&["messages","whatsapp"])?,"api_key")?.json(&json!({"from":a.secret("phone_number")?,"to":m.value,"type":"WHATSAPP","messaging_profile_id":a.secret("messaging_profile_id")?,"whatsapp_message":body}))).await?;
            } else {
                body["messaging_product"] = json!("whatsapp");
                body["recipient_type"] = json!("individual");
                body["to"] = json!(m.value.trim_start_matches('+'));
                a.json(
                    a.post(
                        a.url(
                            "meta_graph",
                            "https://graph.facebook.com/v23.0",
                            &[a.secret("phone_number_id")?, "messages"],
                        )?,
                        "access_token",
                    )?
                    .json(&body),
                )
                .await?;
            }
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
            if self.telnyx {
                use base64::Engine;
                let timestamp = header(h, "telnyx-timestamp")?;
                fresh(timestamp)?;
                let key = base64::engine::general_purpose::STANDARD
                    .decode(a.secret("public_key")?)
                    .map_err(|_| ConnectError::unauthenticated("Invalid Telnyx key"))?;
                let key = ed25519_dalek::VerifyingKey::from_bytes(
                    key.as_slice()
                        .try_into()
                        .map_err(|_| ConnectError::unauthenticated("Invalid Telnyx key"))?,
                )
                .map_err(|_| ConnectError::unauthenticated("Invalid Telnyx key"))?;
                let sig = base64::engine::general_purpose::STANDARD
                    .decode(header(h, "telnyx-signature-ed25519")?)
                    .map_err(|_| ConnectError::unauthenticated("Invalid Telnyx signature"))?;
                let sig = ed25519_dalek::Signature::from_slice(&sig)
                    .map_err(|_| ConnectError::unauthenticated("Invalid Telnyx signature"))?;
                let mut signed = format!("{timestamp}|").into_bytes();
                signed.extend_from_slice(body);
                key.verify_strict(&signed, &sig)
                    .map_err(|_| ConnectError::unauthenticated("Invalid Telnyx signature"))?;
                let p = payload(body)?;
                if at(&p, "/data/event_type") != Some("message.received") {
                    return Ok(Webhook::Messages(vec![]));
                }
                let m = &p["data"]["payload"];
                if !m["to"].as_array().is_some_and(|to| {
                    to.iter()
                        .any(|to| at(to, "/phone_number") == a.secret("phone_number").ok())
                }) {
                    return Err(ConnectError::permission_denied("Telnyx line mismatch"));
                }
                let sender = at(m, "/from/phone_number")
                    .ok_or_else(|| ConnectError::invalid_argument("Missing sender"))?;
                return Ok(Webhook::Messages(vec![IncomingMessage {
                    subject: None,
                    kind: IncomingKind::Message,
                    attachments: m["media"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .enumerate()
                        .filter_map(|(i, f)| {
                            Some(RemoteAttachment {
                                id: i.to_string(),
                                filename: "attachment".into(),
                                media_type: optional(f, "content_type")
                                    .unwrap_or("application/octet-stream")
                                    .into(),
                                size: 0,
                                url: Some(required(f, "url").ok()?.into()),
                                provider_id: None,
                            })
                        })
                        .collect(),
                    event_id: at(&p, "/data/id")
                        .ok_or_else(|| ConnectError::invalid_argument("Missing event ID"))?
                        .into(),
                    message_id: required(m, "id")?.into(),
                    thread_id: sender.chars().filter(char::is_ascii_digit).collect(),
                    sender: provider_identity(sender)?,
                    sender_name: sender.into(),
                    text: optional(m, "text").unwrap_or("").into(),
                    format: "whatsapp",
                    reply_to: None,
                }]));
            }
            sha256_header(a, h, body, "app_secret")?;
            let p = payload(body)?;
            let mut messages = vec![];
            if let Some(entries) = p["entry"].as_array() {
                for entry in entries {
                    if let Some(changes) = entry["changes"].as_array() {
                        for change in changes {
                            let v = &change["value"];
                            if at(v, "/metadata/phone_number_id")
                                != Some(a.secret("phone_number_id")?)
                            {
                                continue;
                            }
                            if let Some(inbound) = v["messages"].as_array() {
                                for m in inbound {
                                    let sender = required(m, "from")?;
                                    let message = required(m, "id")?;
                                    let text = at(m, "/text/body")
                                        .or_else(|| at(m, "/button/text"))
                                        .or_else(|| at(m, "/interactive/button_reply/title"))
                                        .or_else(|| at(m, "/interactive/list_reply/title"))
                                        .or_else(|| at(m, "/image/caption"))
                                        .or_else(|| at(m, "/document/caption"))
                                        .unwrap_or("");
                                    messages.push(IncomingMessage {
                                        subject: None,
                                        kind: IncomingKind::Message,
                                        attachments: [
                                            "image", "video", "audio", "document", "sticker",
                                        ]
                                        .into_iter()
                                        .filter_map(|kind| {
                                            let f = m.get(kind)?;
                                            Some(RemoteAttachment {
                                                id: required(f, "id").ok()?.into(),
                                                filename: optional(f, "filename")
                                                    .unwrap_or(kind)
                                                    .into(),
                                                media_type: optional(f, "mime_type")
                                                    .unwrap_or("application/octet-stream")
                                                    .into(),
                                                size: 0,
                                                url: None,
                                                provider_id: Some(required(f, "id").ok()?.into()),
                                            })
                                        })
                                        .collect(),
                                        event_id: message.into(),
                                        message_id: message.into(),
                                        thread_id: sender
                                            .chars()
                                            .filter(char::is_ascii_digit)
                                            .collect(),
                                        sender: provider_identity(sender)?,
                                        sender_name: at(v, "/contacts/0/profile/name")
                                            .unwrap_or(sender)
                                            .into(),
                                        text: text.into(),
                                        format: "whatsapp",
                                        reply_to: at(m, "/context/id").map(str::to_owned),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Ok(Webhook::Messages(messages))
        })
    }

    fn tools(&self) -> Vec<types::ToolDefinition> {
        let mut tools = vec![
            tool(
                "sendMessage",
                "Send WhatsApp text within the customer-service window. Use a template to initiate outside that window.",
                json!({"to":string(),"text":{"type":"string","maxLength":4096},"replyToMessageId":string()}),
                &["to", "text"],
                true,
            ),
            tool(
                "sendTemplate",
                "Send an approved WhatsApp template with its provider-defined components.",
                json!({"to":string(),"templateName":string(),"language":string(),"components":{"type":"array","maxItems":20}}),
                &["to", "templateName", "language"],
                false,
            ),
            tool(
                "sendMedia",
                "Send WhatsApp media by provider media ID or HTTPS link. Captions apply to image, video and document only.",
                json!({"to":string(),"mediaType":{"enum":["image","video","audio","document","sticker"]},"mediaId":string(),"link":{"type":"string","format":"uri"},"caption":text(),"filename":string(),"replyToMessageId":string()}),
                &["to", "mediaType"],
                false,
            ),
            tool(
                "reactToMessage",
                "React with a single emoji; an empty emoji removes your reaction.",
                json!({"to":string(),"messageId":string(),"emoji":{"type":"string","maxLength":32}}),
                &["to", "messageId", "emoji"],
                false,
            ),
        ];
        if !self.telnyx {
            tools.push(tool("markRead","Mark an inbound WhatsApp message read, optionally showing a typing indicator. Meta only.",json!({"messageId":string(),"typing":{"type":"boolean"}}),&["messageId"],false));
        }
        tools
    }
    fn invoke<'a>(
        &'a self,
        a: &'a Access,
        c: &'a Context,
        name: &'a str,
        v: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let to = optional(&v, "to").unwrap_or("");
            let mut body =
                json!({"messaging_product":"whatsapp","recipient_type":"individual","to":to});
            let visible = match name {
                "sendMessage" => {
                    body["type"] = json!("text");
                    body["text"] = json!({"body":required(&v,"text")?});
                    Some(required(&v, "text")?.to_string())
                }
                "sendTemplate" => {
                    body["type"] = json!("template");
                    body["template"] = json!({"name":required(&v,"templateName")?,"language":{"code":required(&v,"language")?},"components":v.get("components").cloned().unwrap_or_else(||json!([]))});
                    Some(format!(
                        "[WhatsApp template: {}]",
                        required(&v, "templateName")?
                    ))
                }
                "sendMedia" => {
                    let typ = required(&v, "mediaType")?;
                    let media_id = optional(&v, "mediaId");
                    let link = optional(&v, "link");
                    if media_id.is_some() == link.is_some() {
                        return Err(ConnectError::invalid_argument("Specify mediaId or link"));
                    }
                    if let Some(link) = link {
                        let url = url::Url::parse(link)
                            .map_err(|_| ConnectError::invalid_argument("Invalid media link"))?;
                        if url.scheme() != "https"
                            || !url.username().is_empty()
                            || url.password().is_some()
                        {
                            return Err(ConnectError::invalid_argument(
                                "Media links require HTTPS without credentials",
                            ));
                        }
                    }
                    let mut media = json!({});
                    if let Some(id) = media_id {
                        media["id"] = json!(id);
                    }
                    if let Some(link) = link {
                        media["link"] = json!(link);
                    }
                    if let Some(caption) = optional(&v, "caption") {
                        if !["image", "video", "document"].contains(&typ) {
                            return Err(ConnectError::invalid_argument(
                                "This media type cannot have a caption",
                            ));
                        }
                        media["caption"] = json!(caption);
                    }
                    if let Some(filename) = optional(&v, "filename") {
                        if typ != "document" {
                            return Err(ConnectError::invalid_argument(
                                "Only documents support a filename",
                            ));
                        }
                        media["filename"] = json!(filename);
                    }
                    body["type"] = json!(typ);
                    body[typ] = media;
                    Some(
                        optional(&v, "caption")
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("[{typ}]")),
                    )
                }
                "reactToMessage" => {
                    body["type"] = json!("reaction");
                    body["reaction"] = json!({"message_id":required(&v,"messageId")?,"emoji":required(&v,"emoji")?});
                    None
                }
                "markRead" if !self.telnyx => {
                    body = json!({"messaging_product":"whatsapp","status":"read","message_id":required(&v,"messageId")?});
                    if v["typing"].as_bool() == Some(true) {
                        body["typing_indicator"] = json!({"type":"text"});
                    }
                    None
                }
                _ => return Err(ConnectError::not_found("Unsupported WhatsApp tool")),
            };
            if let Some(reply) = optional(&v, "replyToMessageId") {
                body["context"] = json!({"message_id":reply});
            }
            let (url, key) = if self.telnyx {
                for key in ["to", "messaging_product", "recipient_type"] {
                    body.as_object_mut().expect("object").remove(key);
                }
                body = json!({"from":a.secret("phone_number")?,"to":to,"type":"WHATSAPP","messaging_profile_id":a.secret("messaging_profile_id")?,"whatsapp_message":body});
                (
                    a.url(
                        "telnyx_api",
                        "https://api.telnyx.com/v2",
                        &["messages", "whatsapp"],
                    )?,
                    "api_key",
                )
            } else {
                (
                    a.url(
                        "meta_graph",
                        "https://graph.facebook.com/v23.0",
                        &[a.secret("phone_number_id")?, "messages"],
                    )?,
                    "access_token",
                )
            };
            let response = a.json(a.post(url, key)?.json(&body)).await?;
            if let Some(text) = visible {
                let external = response
                    .pointer(if self.telnyx {
                        "/data/id"
                    } else {
                        "/messages/0/id"
                    })
                    .and_then(Value::as_str)
                    .ok_or_else(|| ConnectError::internal("WhatsApp omitted message ID"))?;
                sent(
                    c,
                    a,
                    &text,
                    "whatsapp",
                    &to.chars().filter(char::is_ascii_digit).collect::<String>(),
                    external,
                    None,
                )
                .await
            } else {
                Ok(json!({"success":true}))
            }
        })
    }
}

/// Canonical sender values belong to this provider adapter.
fn provider_identity(raw: &str) -> ToolResult<crate::chat::access::identity::Identity> {
    let value = raw.trim();
    let digits = value.strip_prefix('+').unwrap_or(value);
    if !(7..=15).contains(&digits.len()) || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return Err(ConnectError::invalid_argument(
            "Use an international WhatsApp number",
        ));
    }
    Ok(crate::chat::access::identity::Identity {
        identity_type: types::IdentityType::PhoneNumber,
        value: format!("+{digits}"),
    })
}
