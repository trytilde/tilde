//! WhatsApp business verbs on the WhatsApp channel connections: Meta's Cloud API (Graph) for
//! `whatsapp/meta` and Telnyx's WhatsApp API for `telnyx/whatsapp`. Both carry the same Cloud
//! API message objects. Failures come back as a JSON error naming a stable kind, e.g.
//! `whatsapp_customer_service_window_closed` (Meta 131047, Telnyx 10015), with a suggested action.
use super::{ToolProvider, rest};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::{Map, Value, json};

const GRAPH: &str = "https://graph.facebook.com/v21.0";
const TELNYX: &str = "https://api.telnyx.com/v2";
const WINDOW_CLOSED: &str = "The 24 hour customer service window is closed. Send an approved template with whatsapp_send_template instead of free-form text or media.";

pub struct Whatsapp {
    pub telnyx: bool,
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn recipient() -> Value {
    json!({"type":"string","minLength":7,"maxLength":32,"description":"Recipient WhatsApp number in international format."})
}
fn reply_to() -> Value {
    json!({"type":"string","minLength":1,"description":"Inbound message ID (wamid...) to quote as a reply."})
}

impl ToolProvider for Whatsapp {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let provider = if self.telnyx { "telnyx" } else { "whatsapp" };
        let read = rest::Hints {
            read_only: true,
            destructive: false,
        };
        let write = rest::Hints::default();
        [
            (
                "whatsapp_send_text",
                "Sent a WhatsApp message",
                "Send free-form text to a WhatsApp user. Only allowed within 24 hours of the user's last message; outside that window use whatsapp_send_template. Bodies are limited to 4096 characters; split longer ones.",
                object(
                    json!({"to":recipient(),"text":{"type":"string","minLength":1,"maxLength":4096},"preview_url":{"type":"boolean","description":"Render a link preview for the first URL."},"reply_to_message_id":reply_to()}),
                    &["to", "text"],
                ),
                write,
            ),
            (
                "whatsapp_send_template",
                "Sent a WhatsApp template",
                "Send an approved message template. This is the only way to start a conversation or reach a user whose 24 hour customer service window has closed. Components carry header, body and button parameters in Meta's format.",
                object(
                    json!({"to":recipient(),"template_name":{"type":"string","minLength":1,"maxLength":512},"language_code":{"type":"string","minLength":2,"maxLength":16,"description":"Template language such as en_US (the default)."},"components":{"type":"array","items":{"type":"object"}},"reply_to_message_id":reply_to()}),
                    &["to", "template_name"],
                ),
                write,
            ),
            (
                "whatsapp_send_media",
                "Sent WhatsApp media",
                "Send an image, video, audio, document or sticker by public HTTPS link or uploaded media ID (exactly one). Captions apply to image, video and documents; filename to documents. Subject to the 24 hour window.",
                object(
                    json!({"to":recipient(),"media_type":{"enum":["image","video","audio","document","sticker"]},"link":{"type":"string","pattern":"^https://"},"media_id":{"type":"string","minLength":1},"caption":{"type":"string","maxLength":1024},"filename":{"type":"string","maxLength":240},"reply_to_message_id":reply_to()}),
                    &["to", "media_type"],
                ),
                write,
            ),
            (
                "whatsapp_send_reaction",
                "Reacted on WhatsApp",
                "Add an emoji reaction to a message the user sent, or remove it with an empty emoji. Reactions do not open or extend the 24 hour window.",
                object(
                    json!({"to":recipient(),"message_id":{"type":"string","minLength":1},"emoji":{"type":"string","maxLength":8}}),
                    &["to", "message_id"],
                ),
                write,
            ),
            (
                "whatsapp_mark_read",
                "Marked a WhatsApp message read",
                "Mark an inbound message read and optionally show a typing indicator while a reply is prepared. A no-op on Telnyx numbers, which have no such endpoint.",
                object(
                    json!({"message_id":{"type":"string","minLength":1},"typing_indicator":{"type":"boolean"}}),
                    &["message_id"],
                ),
                write,
            ),
            (
                "whatsapp_get_media",
                "Read WhatsApp media",
                "Resolve an inbound media ID to its download URL, MIME type and size. The URL is short-lived.",
                object(json!({"media_id":{"type":"string","minLength":1}}), &["media_id"]),
                read,
            ),
            (
                "whatsapp_list_templates",
                "Listed WhatsApp templates",
                "List the business account's message templates with status, language, category and components so template sends can be built correctly.",
                object(
                    json!({"limit":{"type":"integer","minimum":1,"maximum":100},"status":{"type":"string","description":"APPROVED, PENDING, REJECTED, ..."},"name":{"type":"string"},"after":{"type":"string","description":"Cursor from a previous page (Meta only)."}}),
                    &[],
                ),
                read,
            ),
            (
                "whatsapp_get_phone_number",
                "Read the WhatsApp number",
                "Read the sending number's display number, verified name, quality rating, messaging limit tier and status. Pause outbound traffic when the status is not CONNECTED.",
                object(json!({}), &[]),
                read,
            ),
        ]
        .into_iter()
        .map(|(name, summary, description, schema, hints)| {
            rest::definition(provider, name, summary, description, schema, hints)
        })
        .collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let field = |key: &str| input[key].as_str().map(str::trim).filter(|v| !v.is_empty());
            let message = match name {
                "whatsapp_send_text" => {
                    let text = field("text").ok_or_else(|| invalid("`text` is empty"))?;
                    json!({"type":"text","text":{"body":text,"preview_url":input["preview_url"].as_bool().unwrap_or(false)}})
                }
                "whatsapp_send_template" => {
                    let mut template = json!({"name":field("template_name"),"language":{"code":field("language_code").unwrap_or("en_US")}});
                    if input["components"].is_array() {
                        template["components"] = input["components"].clone();
                    }
                    json!({"type":"template","template":template})
                }
                "whatsapp_send_media" => {
                    let kind = field("media_type").unwrap_or_default();
                    let mut media = Map::new();
                    match (field("link"), field("media_id")) {
                        (Some(link), None) => media.insert("link".into(), json!(link)),
                        (None, Some(id)) => media.insert("id".into(), json!(id)),
                        _ => return Err(invalid("Provide exactly one of `link` or `media_id`")),
                    };
                    if let Some(caption) = field("caption") {
                        if !["image", "video", "document"].contains(&kind) {
                            return Err(invalid(format!("{kind} messages take no caption")));
                        }
                        media.insert("caption".into(), json!(caption));
                    }
                    if let Some(filename) = field("filename").filter(|_| kind == "document") {
                        media.insert("filename".into(), json!(filename));
                    }
                    json!({"type":kind,kind:media})
                }
                "whatsapp_send_reaction" => {
                    json!({"type":"reaction","reaction":{"message_id":field("message_id"),"emoji":field("emoji").unwrap_or("")}})
                }
                "whatsapp_mark_read" => {
                    return self.mark_read(access, &input).await;
                }
                "whatsapp_get_media" => {
                    let media = field("media_id").unwrap_or_default();
                    return if self.telnyx {
                        let from = access.secret("phone_number")?;
                        let url = access.url(
                            "telnyx_api",
                            TELNYX,
                            &["whatsapp", "media", from, media],
                        )?;
                        call(self.telnyx, access, Method::GET, url, None).await
                    } else {
                        let mut url = access.url("meta_graph", GRAPH, &[media])?;
                        url.query_pairs_mut()
                            .append_pair("phone_number_id", access.secret("phone_number_id")?);
                        call(false, access, Method::GET, url, None).await
                    };
                }
                "whatsapp_list_templates" => return self.templates(access, &input).await,
                "whatsapp_get_phone_number" => return self.phone_number(access).await,
                _ => return Err(ConnectError::not_found("Unsupported WhatsApp tool")),
            };
            let to: String = field("to")
                .unwrap_or_default()
                .chars()
                .filter(char::is_ascii_digit)
                .collect();
            if !(7..=15).contains(&to.len()) {
                return Err(invalid(
                    "The recipient must be an international number of 7 to 15 digits",
                ));
            }
            let mut message = message;
            if let Some(reply) = field("reply_to_message_id") {
                message["context"] = json!({"message_id":reply});
            }
            // Reactions never count against messaging limits, so only content is gated.
            if name != "whatsapp_send_reaction" {
                self.ensure_sendable(access).await?;
            }
            self.send(access, &to, message).await
        })
    }
}

impl Whatsapp {
    async fn send(&self, access: &Access, to: &str, message: Value) -> ToolResult<Value> {
        if !self.telnyx {
            let mut body = message;
            body["messaging_product"] = json!("whatsapp");
            body["recipient_type"] = json!("individual");
            body["to"] = json!(to);
            let url = access.url(
                "meta_graph",
                GRAPH,
                &[access.secret("phone_number_id")?, "messages"],
            )?;
            return call(false, access, Method::POST, url, Some(body)).await;
        }
        let url = access.url("telnyx_api", TELNYX, &["messages", "whatsapp"])?;
        let body = json!({"from":access.secret("phone_number")?,"to":format!("+{to}"),"type":"WHATSAPP","messaging_profile_id":access.secret("messaging_profile_id")?,"whatsapp_message":message});
        let sent = call(true, access, Method::POST, url, Some(body)).await?;
        let data = sent.get("data").unwrap_or(&sent);
        // Shaped like Meta's acceptance so both transports answer alike.
        Ok(json!({
            "messaging_product":"whatsapp",
            "contacts":[{"input":to,"wa_id":to}],
            "messages":[{"id":data["id"],"message_status":data.pointer("/to/0/status")}]
        }))
    }

    /// Refuse to send from a number the platform reports as unusable, instead of letting another
    /// failure count against it. A failed health read does not block: the send reports better.
    async fn ensure_sendable(&self, access: &Access) -> ToolResult<()> {
        let status = if self.telnyx {
            self.telnyx_number(access)
                .await
                .ok()
                .flatten()
                .and_then(|number| number["status"].as_str().map(str::to_ascii_uppercase))
        } else {
            let mut url = access.url("meta_graph", GRAPH, &[access.secret("phone_number_id")?])?;
            url.query_pairs_mut().append_pair("fields", "status");
            call(false, access, Method::GET, url, None)
                .await
                .ok()
                .and_then(|number| number["status"].as_str().map(str::to_owned))
        };
        match status.as_deref() {
            Some(status @ ("BANNED" | "RESTRICTED" | "RATE_LIMITED" | "DELETED")) => Err(failure(
                "phone_number_unhealthy",
                0,
                None,
                &format!(
                    "The WhatsApp sending number is {status}; outbound messaging is paused until Meta clears it"
                ),
                false,
                None,
            )),
            _ => Ok(()),
        }
    }

    async fn mark_read(&self, access: &Access, input: &Value) -> ToolResult<Value> {
        let message = input["message_id"].as_str().unwrap_or_default();
        if self.telnyx {
            return Ok(
                json!({"success":true,"skipped":true,"reason":"Telnyx's WhatsApp API has no read-receipt or typing-indicator endpoint; nothing was sent.","message_id":message}),
            );
        }
        let mut body = json!({"messaging_product":"whatsapp","status":"read","message_id":message});
        if input["typing_indicator"] == true {
            body["typing_indicator"] = json!({"type":"text"});
        }
        let url = access.url(
            "meta_graph",
            GRAPH,
            &[access.secret("phone_number_id")?, "messages"],
        )?;
        call(false, access, Method::POST, url, Some(body)).await
    }

    async fn templates(&self, access: &Access, input: &Value) -> ToolResult<Value> {
        let limit = input["limit"].as_u64().unwrap_or(25).clamp(1, 100);
        let status = input["status"].as_str().filter(|v| !v.is_empty());
        let name = input["name"].as_str().filter(|v| !v.is_empty());
        if !self.telnyx {
            let mut url = access.url(
                "meta_graph",
                GRAPH,
                &[access.secret("waba_id")?, "message_templates"],
            )?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair(
                    "fields",
                    "name,status,category,language,components,quality_score,rejected_reason",
                );
                query.append_pair("limit", &limit.to_string());
                for (key, value) in [
                    ("status", status),
                    ("name", name),
                    ("after", input["after"].as_str()),
                ] {
                    if let Some(value) = value.filter(|v| !v.is_empty()) {
                        query.append_pair(key, value);
                    }
                }
            }
            return call(false, access, Method::GET, url, None).await;
        }
        // The Telnyx connection stores no WABA; its number record names it. Telnyx returns the
        // whole list, so the filters apply here.
        let number = self.telnyx_number(access).await?;
        let waba = number
            .as_ref()
            .and_then(|number| number["waba_id"].as_str())
            .ok_or_else(|| {
                invalid("The sending number is not a WhatsApp number on this Telnyx account")
            })?;
        let mut url = access.url("telnyx_api", TELNYX, &["whatsapp", "message_templates"])?;
        url.query_pairs_mut().append_pair("waba_id", waba);
        let mut templates = call(true, access, Method::GET, url, None).await?;
        if let Some(data) = templates.get_mut("data").and_then(Value::as_array_mut) {
            data.retain(|template| {
                status.is_none_or(|wanted| {
                    template["status"]
                        .as_str()
                        .is_some_and(|actual| actual.eq_ignore_ascii_case(wanted))
                }) && name.is_none_or(|wanted| template["name"] == wanted)
            });
            data.truncate(limit as usize);
        }
        Ok(templates)
    }

    async fn phone_number(&self, access: &Access) -> ToolResult<Value> {
        if !self.telnyx {
            let mut url = access.url("meta_graph", GRAPH, &[access.secret("phone_number_id")?])?;
            url.query_pairs_mut().append_pair("fields", "id,display_phone_number,verified_name,quality_rating,code_verification_status,status,messaging_limit_tier,name_status,platform_type,throughput");
            return call(false, access, Method::GET, url, None).await;
        }
        let number = self.telnyx_number(access).await?.unwrap_or_default();
        Ok(json!({
            "id":number["phone_number_id"],
            "display_phone_number":number["phone_number"].as_str().unwrap_or(access.secret("phone_number")?),
            "verified_name":number["display_name"],
            "status":number["status"],
            "quality_rating":number["quality_rating"],
            "waba_id":number["waba_id"],
            "transport":"telnyx"
        }))
    }

    /// The connection's sending number as Telnyx lists it.
    async fn telnyx_number(&self, access: &Access) -> ToolResult<Option<Value>> {
        let digits = |raw: &str| raw.chars().filter(char::is_ascii_digit).collect::<String>();
        let wanted = digits(access.secret("phone_number")?);
        let url = access.url("telnyx_api", TELNYX, &["whatsapp", "phone_numbers"])?;
        let mut numbers = call(true, access, Method::GET, url, None).await?;
        Ok(numbers["data"].as_array_mut().and_then(|numbers| {
            numbers
                .iter()
                .position(|n| {
                    n["phone_number"]
                        .as_str()
                        .is_some_and(|n| digits(n) == wanted)
                })
                .map(|index| numbers.swap_remove(index))
        }))
    }
}

async fn call(
    telnyx: bool,
    access: &Access,
    method: Method,
    url: url::Url,
    body: Option<Value>,
) -> ToolResult<Value> {
    let key = if telnyx { "api_key" } else { "access_token" };
    let mut request = access
        .http
        .client
        .request(method, url)
        .bearer_auth(access.secret(key)?)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.map_err(|_| {
        failure(
            "transport",
            0,
            None,
            "WhatsApp could not be reached",
            true,
            None,
        )
    })?;
    let status = response.status().as_u16();
    let bytes = response.bytes().await.map_err(|_| {
        failure(
            "transport",
            status,
            None,
            "The WhatsApp response was interrupted",
            true,
            None,
        )
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"raw": String::from_utf8_lossy(&bytes)}));
    if (200..300).contains(&status) {
        return Ok(value);
    }
    Err(if telnyx {
        telnyx_error(status, &value)
    } else {
        meta_error(status, &value)
    })
}

fn invalid(message: impl Into<String>) -> ConnectError {
    ConnectError::invalid_argument(message.into())
}

/// The error an agent sees: a JSON object naming the kind of failure and how to recover.
fn failure(
    kind: &str,
    status: u16,
    code: Option<i64>,
    message: &str,
    retryable: bool,
    action: Option<&str>,
) -> ConnectError {
    let detail = json!({"error":format!("whatsapp_{kind}"),"http_status":status,"code":code,"message":message,"retryable":retryable,"suggested_action":action}).to_string();
    match kind {
        "customer_service_window_closed"
        | "account_restricted"
        | "phone_number_unhealthy"
        | "undeliverable"
        | "recipient_not_in_allowed_list" => ConnectError::failed_precondition(detail),
        "rate_limited" => ConnectError::resource_exhausted(detail),
        "access_token_invalid" => ConnectError::permission_denied(detail),
        "template_error" | "media_error" | "invalid_parameter" => {
            ConnectError::invalid_argument(detail)
        }
        "transport" => ConnectError::unavailable(detail),
        _ => ConnectError::unknown(detail),
    }
}

fn meta_error(status: u16, body: &Value) -> ConnectError {
    let error = body.get("error").unwrap_or(body);
    let code = error["code"].as_i64();
    let message = error["message"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("WhatsApp Cloud API request failed with {status}"));
    let (kind, retryable, action) = match code.unwrap_or_default() {
        131047 => ("customer_service_window_closed", false, Some(WINDOW_CLOSED)),
        131026 | 131049 | 131050 => (
            "undeliverable",
            false,
            Some(
                "The recipient cannot receive this message. Confirm the number is on WhatsApp and has not blocked the sender.",
            ),
        ),
        131030 => (
            "recipient_not_in_allowed_list",
            false,
            Some(
                "Add the recipient to the Meta app's allowed test numbers or move the number out of test mode.",
            ),
        ),
        130429 | 131056 | 131048 | 80007 | 4 | 131057 => {
            ("rate_limited", true, Some("Back off and retry later."))
        }
        190 | 102 | 10 | 200 => (
            "access_token_invalid",
            false,
            Some("Rotate the WhatsApp access token on the connection."),
        ),
        132000 | 132001 | 132005 | 132007 | 132012 | 132015 | 132016 | 132068 | 132069 => (
            "template_error",
            false,
            Some(
                "Check the template name, language, approval status and parameter count with whatsapp_list_templates.",
            ),
        ),
        131052 | 131053 => (
            "media_error",
            false,
            Some("Check the media link is publicly reachable and the type is supported."),
        ),
        131031 | 131042 | 131045 => (
            "account_restricted",
            false,
            Some(
                "Resolve the WhatsApp Business Account restriction or payment issue in Meta Business Manager.",
            ),
        ),
        131051 | 100 | 131008 | 131009 | 131021 => ("invalid_parameter", false, None),
        1 | 2 | 131000 | 131016 => (
            "unknown",
            true,
            Some("Meta reported a temporary failure. Retry later."),
        ),
        _ => ("unknown", status >= 500 || status == 429, None),
    };
    failure(kind, status, code, &message, retryable, action)
}

/// Telnyx's envelope is `{"errors":[{code,title,detail}]}` with string codes.
fn telnyx_error(status: u16, body: &Value) -> ConnectError {
    let first = body.pointer("/errors/0").unwrap_or(body);
    let code = match &first["code"] {
        Value::String(code) => code.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    let title = first["title"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let detail = first["detail"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let message = match (title, detail) {
        (Some(title), Some(detail)) if title != detail => format!("{title}: {detail}"),
        (Some(text), _) | (None, Some(text)) => text.to_owned(),
        (None, None) => format!("Telnyx WhatsApp request failed with {status}"),
    };
    let (kind, retryable, action) = match (code.as_str(), status) {
        ("10015", _) => ("customer_service_window_closed", false, Some(WINDOW_CLOSED)),
        ("40008", _) => (
            "template_error",
            false,
            Some(
                "The template is pending, rejected, paused or disabled. Check its status with whatsapp_list_templates.",
            ),
        ),
        ("10004", _) => (
            "invalid_parameter",
            false,
            Some("The sending number is not on the connection's Telnyx messaging profile."),
        ),
        (_, 401 | 403) => (
            "access_token_invalid",
            false,
            Some("Rotate the Telnyx API key on the connection."),
        ),
        (_, 429) => ("rate_limited", true, Some("Back off and retry later.")),
        (_, 400 | 422) => ("invalid_parameter", false, None),
        (_, 500..) => (
            "unknown",
            true,
            Some("Telnyx reported a temporary failure. Retry later."),
        ),
        _ => ("unknown", false, None),
    };
    failure(kind, status, code.parse().ok(), &message, retryable, action)
}
