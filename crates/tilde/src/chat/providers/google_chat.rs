//! A Google Chat app reached at its HTTP endpoint, replying through the Chat API as the app.
//!
//! Google signs each request with a bearer JWT issued by `chat@system.gserviceaccount.com`
//! (RS256, keys published as X.509 certificates) whose audience is the Cloud project number the
//! app is configured with ("Authentication Audience: Project Number"). Apps built as Workspace
//! add-ons use another envelope and token and are not supported.
//!
//! In spaces Google delivers only messages that @mention the app (or invoke its slash commands);
//! direct messages always reach it. A DM is one conversation (`spaces/AAA`); in a space each
//! thread is one (`spaces/AAA/threads/BBB`). Senders are identified by their email, which Google
//! vouches for, so private access needs no verification message.
use super::*;
use crate::proto::tilde::types::v1 as types;

pub struct GoogleChat;

const ISSUER: &str = "chat@system.gserviceaccount.com";
const CERTIFICATES: &str =
    "https://www.googleapis.com/service_accounts/v1/metadata/x509/chat@system.gserviceaccount.com";
const API: &str = "https://chat.googleapis.com/v1";
const CONVERSATION: &str = "^spaces/[A-Za-z0-9_-]+(/threads/[A-Za-z0-9_.-]+)?$";
const MESSAGE: &str = "^spaces/[A-Za-z0-9_-]+/messages/[A-Za-z0-9_.-]+$";

impl Adapter for GoogleChat {
    fn identity_types(&self) -> &'static [types::IdentityType] {
        &[types::IdentityType::Email]
    }
    fn verification_recipient(
        &self,
        identity_type: types::IdentityType,
        value: &str,
    ) -> ToolResult<crate::chat::access::identity::Identity> {
        if identity_type != types::IdentityType::Email {
            return Err(ConnectError::invalid_argument(
                "Google Chat identifies people by email",
            ));
        }
        let identity = crate::chat::access::identity::Identity {
            identity_type,
            value: value.trim().to_lowercase(),
        };
        identity.validate()?;
        Ok(identity)
    }
    fn verification_supported(&self) -> bool {
        false
    }
    fn attests_identities(&self) -> bool {
        true
    }
    fn send_verification<'a>(
        &'a self,
        _: &'a Access,
        _: crate::chat::access::identity::VerificationMessage<'a>,
    ) -> BoxFuture<'a, ToolResult<()>> {
        Box::pin(async {
            Err(ConnectError::failed_precondition(
                "Google Chat identities are vouched for by Google; add them directly",
            ))
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
            authenticate(a, h).await?;
            let p = payload(body)?;
            let none = || Ok(Webhook::Messages(vec![]));
            // Adding the app with an @mention carries that message; other events carry none.
            if !matches!(at(&p, "/type"), Some("MESSAGE" | "ADDED_TO_SPACE")) {
                return none();
            }
            let m = &p["message"];
            let Some(name) = at(m, "/name") else {
                return none();
            };
            let sender = if m["sender"].is_object() {
                &m["sender"]
            } else {
                &p["user"]
            };
            if at(sender, "/type") == Some("BOT") {
                return none();
            }
            let space = if p["space"].is_object() {
                &p["space"]
            } else {
                &m["space"]
            };
            let space_name = required(space, "name")?;
            let direct = at(space, "/spaceType") == Some("DIRECT_MESSAGE")
                || at(space, "/type") == Some("DM")
                || space["singleUserBotDm"] == true;
            let mentioned = m["annotations"].as_array().into_iter().flatten().any(|a| {
                (at(a, "/type") == Some("USER_MENTION")
                    && at(a, "/userMention/user/type") == Some("BOT"))
                    || at(a, "/type") == Some("SLASH_COMMAND")
            }) || m["slashCommand"].is_object();
            if !direct && !mentioned {
                return none();
            }
            let reference = match at(m, "/thread/name") {
                Some(thread) if !direct => thread.to_owned(),
                _ => space_name.to_owned(),
            };
            // `argumentText` is the message without the app's own @mention.
            let text = optional(m, "argumentText")
                .or_else(|| optional(m, "text"))
                .unwrap_or("")
                .trim();
            let email = optional(sender, "email").filter(|e| !e.is_empty());
            let sender_identity = match email {
                Some(email) => crate::chat::access::identity::Identity {
                    identity_type: types::IdentityType::Email,
                    value: email.to_lowercase(),
                },
                None => crate::chat::access::identity::Identity {
                    identity_type: types::IdentityType::Username,
                    value: required(sender, "name")?.to_owned(),
                },
            };
            Ok(Webhook::Messages(vec![IncomingMessage {
                subject: None,
                kind: IncomingKind::Message,
                attachments: vec![],
                event_id: name.into(),
                message_id: name.into(),
                thread_id: reference,
                sender_name: optional(sender, "displayName")
                    .or(email)
                    .unwrap_or(&sender_identity.value)
                    .into(),
                sender: sender_identity,
                text: text.into(),
                format: "text",
                reply_to: None,
            }]))
        })
    }

    fn tools(&self) -> Vec<types::ToolDefinition> {
        vec![
            tool(
                "sendMessage",
                "Send a Google Chat message as the app. `conversation` is a space (spaces/AAA, a new thread in a space or the DM itself) or a thread to reply in (spaces/AAA/threads/BBB). Text supports *bold*, _italic_, `code` and <users/123> mentions.",
                json!({"conversation":{"type":"string","pattern":CONVERSATION},"text":text()}),
                &["conversation", "text"],
                true,
            ),
            tool(
                "updateMessage",
                "Replace the text of a message this app sent, such as a progress update.",
                json!({"message":{"type":"string","pattern":MESSAGE,"description":"The message name returned by sendMessage (externalMessageId)."},"text":text()}),
                &["message", "text"],
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
            let token = a.secret("access_token")?;
            match name {
                "sendMessage" => {
                    let conversation = required(&v, "conversation")?;
                    let segments = conversation.split('/').collect::<Vec<_>>();
                    let space = segments[..2].join("/");
                    let mut url = a.url(
                        "google_chat_api",
                        API,
                        &[segments[0], segments[1], "messages"],
                    )?;
                    url.query_pairs_mut()
                        .append_pair("messageReplyOption", "REPLY_MESSAGE_FALLBACK_TO_NEW_THREAD")
                        // Google ignores a repeated create with the same ID.
                        .append_pair("messageId", &format!("client-{}", c.call_id.simple()));
                    let mut body = json!({"text": required(&v, "text")?});
                    if segments.len() == 4 {
                        body["thread"] = json!({"name": conversation});
                    }
                    let response = a
                        .json(a.http.client.post(url).bearer_auth(token).json(&body))
                        .await?;
                    let message = required(&response, "name")?;
                    // A DM stays one conversation; in a space the reply's thread is.
                    let current = c.chat.thread(c.scope().thread_id).await?;
                    let direct = current.channel.is_set()
                        && current.channel.connection_id == a.connection_id.to_string()
                        && current.channel.external_id == space;
                    let destination = if direct {
                        space
                    } else {
                        ingress::at(&response, "/thread/name")
                            .unwrap_or(conversation)
                            .to_owned()
                    };
                    sent(
                        c,
                        a,
                        required(&v, "text")?,
                        "text",
                        &destination,
                        message,
                        None,
                    )
                    .await
                }
                "updateMessage" => {
                    let message = required(&v, "message")?;
                    let segments = message.split('/').collect::<Vec<_>>();
                    let mut url = a.url("google_chat_api", API, &segments)?;
                    url.query_pairs_mut().append_pair("updateMask", "text");
                    let response = a
                        .json(
                            a.http
                                .client
                                .patch(url)
                                .bearer_auth(token)
                                .json(&json!({"text": required(&v, "text")?})),
                        )
                        .await?;
                    Ok(json!({"success": true, "message": response["name"]}))
                }
                _ => Err(ConnectError::not_found("Unknown Google Chat tool")),
            }
        })
    }
}

/// Checks Google's bearer JWT: signed by a current Chat certificate, issued by Chat, for this
/// app's project number, unexpired.
async fn authenticate(a: &Access, h: &http::HeaderMap) -> ToolResult<()> {
    use ingress::{certificate_rsa_key, header, published};
    let refused = || ConnectError::unauthenticated("Invalid Google Chat token");
    let token = header(h, "authorization")?
        .strip_prefix("Bearer ")
        .ok_or_else(refused)?;
    let kid = jsonwebtoken::decode_header(token)
        .ok()
        .and_then(|header| header.kid)
        .ok_or_else(refused)?;
    let url = a.url("google_chat_certificates", CERTIFICATES, &[])?;
    let certificates = published(
        &a.http.client,
        url.as_str(),
        std::time::Duration::from_secs(3600),
    )
    .await?;
    let certificates: serde_json::Map<String, Value> =
        serde_json::from_str(&certificates).map_err(|_| refused())?;
    let pem = certificates
        .get(&kid)
        .and_then(Value::as_str)
        .ok_or_else(refused)?;
    let key = jsonwebtoken::DecodingKey::from_rsa_der(&certificate_rsa_key(pem)?);
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[a.secret("project_number")?]);
    jsonwebtoken::decode::<Value>(token, &key, &validation).map_err(|_| refused())?;
    Ok(())
}
