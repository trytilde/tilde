//! Slack Web API wire shapes ported from tilde-api's Slack chat and MCP providers.
use super::*;
use crate::proto::tilde::types::v1 as types;
pub struct Slack;
impl Adapter for Slack {
    fn identity_types(&self) -> &'static [types::IdentityType] {
        &[types::IdentityType::Username]
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
            let dm = a
                .json(
                    a.post(
                        a.url(
                            "slack_api",
                            "https://slack.com/api",
                            &["conversations.open"],
                        )?,
                        "access_token",
                    )?
                    .json(&json!({"users":m.value})),
                )
                .await?;
            if dm["ok"] != true {
                return Err(ConnectError::failed_precondition(
                    "Slack could not open a private message. Check the app's chat and im permissions.",
                ));
            }
            let channel = dm
                .pointer("/channel/id")
                .and_then(Value::as_str)
                .ok_or_else(|| ConnectError::internal("Slack did not return a private channel"))?;
            let escape = |value: &str| {
                value
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            };
            let text = zeroize::Zeroizing::new(format!(
                "You've received an invitation to access {} from this Slack account. Please follow <{}|this link> to accept.\n\n{}",
                escape(m.agent_name),
                escape(m.url),
                crate::chat::access::identity::INVITATION_EXPIRY
            ));
            let result=a.json(a.post(a.url("slack_api","https://slack.com/api",&["chat.postMessage"])?,"access_token")?.json(&json!({"channel":channel,"text":text.as_str(),"unfurl_links":false,"unfurl_media":false}))).await?;
            if result["ok"] != true {
                return Err(ConnectError::failed_precondition(
                    "Slack could not deliver verification",
                ));
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
            let timestamp = header(h, "x-slack-request-timestamp")?;
            fresh(timestamp)?;
            let mut signed = format!("v0:{timestamp}:").into_bytes();
            signed.extend_from_slice(body);
            let signature = hex::decode(
                header(h, "x-slack-signature")?
                    .strip_prefix("v0=")
                    .ok_or_else(|| ConnectError::unauthenticated("Invalid Slack signature"))?,
            )
            .map_err(|_| ConnectError::unauthenticated("Invalid Slack signature"))?;
            hmac_verify(a.secret("signing_secret")?.as_bytes(), &signed, &signature)?;
            let p = payload(body)?;
            if at(&p, "/type") == Some("url_verification") {
                return Ok(Webhook::Challenge(required(&p, "challenge")?.into()));
            }
            if at(&p, "/team_id") != Some(a.secret("slack_team_id")?) {
                return Err(ConnectError::permission_denied("Slack workspace mismatch"));
            }
            let event = &p["event"];
            if ![Some("message"), Some("app_mention")].contains(&at(event, "/type"))
                || event.get("bot_id").is_some()
                || at(event, "/user")
                    == crate::connections::model::optional(&a.values, "slack_bot_user_id")
            {
                return Ok(Webhook::Messages(vec![]));
            }
            let channel = required(event, "channel")?;
            let ts = required(event, "ts")?;
            let thread = optional(event, "thread_ts").unwrap_or(ts);
            let reference =
                if channel.starts_with('D') || at(event, "/channel_type") == Some("mpim") {
                    channel.to_string()
                } else {
                    format!("{channel}:{thread}")
                };
            let sender = required(event, "user")?;
            Ok(Webhook::Messages(vec![IncomingMessage {
                subject: None,
                kind: IncomingKind::Message,
                attachments: event["files"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|f| {
                        Some(RemoteAttachment {
                            id: required(f, "id").ok()?.into(),
                            filename: optional(f, "name").unwrap_or("attachment").into(),
                            media_type: optional(f, "mimetype")
                                .unwrap_or("application/octet-stream")
                                .into(),
                            size: f["size"].as_i64().unwrap_or(0),
                            url: Some(required(f, "url_private").ok()?.into()),
                            provider_id: None,
                        })
                    })
                    .collect(),
                event_id: required(&p, "event_id")?.into(),
                message_id: ts.into(),
                thread_id: reference,
                sender: provider_identity(sender)?,
                sender_name: sender.into(),
                text: optional(event, "text").unwrap_or("").into(),
                format: "slack_mrkdwn",
                reply_to: None,
            }]))
        })
    }

    fn tools(&self) -> Vec<types::ToolDefinition> {
        vec![
            tool(
                "sendMessage",
                "Send Slack mrkdwn to a channel, optionally inside a thread.",
                json!({"channelId":string(),"text":text(),"threadTs":string()}),
                &["channelId", "text"],
                true,
            ),
            tool(
                "reactToMessage",
                "Add a Slack emoji reaction; use the emoji name without colons.",
                json!({"channelId":string(),"messageTs":string(),"emoji":string()}),
                &["channelId", "messageTs", "emoji"],
                false,
            ),
            tool(
                "removeReaction",
                "Remove this bot's Slack emoji reaction.",
                json!({"channelId":string(),"messageTs":string(),"emoji":string()}),
                &["channelId", "messageTs", "emoji"],
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
            let channel = required(&v, "channelId")?;
            let (method, body) = match name {
                "sendMessage" => {
                    let mut b = json!({"channel":channel,"text":required(&v,"text")?,"client_msg_id":c.call_id});
                    if let Some(ts) = optional(&v, "threadTs") {
                        b["thread_ts"] = json!(ts);
                    }
                    ("chat.postMessage", b)
                }
                "reactToMessage" | "removeReaction" => (
                    if name == "reactToMessage" {
                        "reactions.add"
                    } else {
                        "reactions.remove"
                    },
                    json!({"channel":channel,"timestamp":required(&v,"messageTs")?,"name":required(&v,"emoji")?.trim_matches(':')}),
                ),
                _ => return Err(ConnectError::not_found("Unknown Slack tool")),
            };
            let response = a
                .json(
                    a.post(
                        a.url("slack_api", "https://slack.com/api", &[method])?,
                        "access_token",
                    )?
                    .json(&body),
                )
                .await?;
            if response.get("ok").and_then(Value::as_bool) != Some(true) {
                return Err(ConnectError::failed_precondition(
                    "Slack rejected the operation",
                ));
            }
            if name == "sendMessage" {
                let current = c.chat.thread(c.scope().thread_id).await?;
                let direct = channel.starts_with('D')
                    || (current.channel.is_set()
                        && current.channel.connection_id == a.connection_id.to_string()
                        && current.channel.external_id == channel);
                let destination = if direct {
                    channel.to_string()
                } else {
                    format!(
                        "{channel}:{}",
                        optional(&v, "threadTs").unwrap_or(required(&response, "ts")?)
                    )
                };
                sent(
                    c,
                    a,
                    required(&v, "text")?,
                    "slack_mrkdwn",
                    &destination,
                    required(&response, "ts")?,
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
    Ok(crate::chat::access::identity::Identity {
        identity_type: types::IdentityType::Username,
        value: raw.trim().to_owned(),
    })
}
