mod hookdeck;
use crate::common;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{self, Chat, tools::Registry},
    connections::{
        model::{Assignment, Capability},
        service::Connections,
    },
    encryption::{Encryption, SecretBinding},
    proto::tilde::types::v1 as types,
};
use uuid::Uuid;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub fn setting(name: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("Missing {name}; set it in .env.test.local").into())
}
fn secret(name: &str) -> Result<SecretString> {
    Ok(SecretString::from(setting(name)?))
}
fn ensure(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Response omitted {key}").into())
}
async fn checked(request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
    let path = request
        .try_clone()
        .and_then(|r| r.build().ok())
        .map(|r| r.url().path().to_owned())
        .unwrap_or_default();
    let r = request
        .send()
        .await
        .map_err(|_| "Provider transport failed")?;
    ensure(
        r.status().is_success(),
        &format!("Provider returned HTTP {} at {path}", r.status().as_u16()),
    )?;
    Ok(r)
}
// API responses may contain freshly issued webhook secrets. Clear owned strings on drop.
struct PrivateJson(Value);
impl std::ops::Deref for PrivateJson {
    type Target = Value;
    fn deref(&self) -> &Value {
        &self.0
    }
}
impl Drop for PrivateJson {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        fn clear(value: &mut Value) {
            match value {
                Value::String(s) => s.zeroize(),
                Value::Array(values) => values.iter_mut().for_each(clear),
                Value::Object(values) => values.values_mut().for_each(clear),
                _ => {}
            }
        }
        clear(&mut self.0);
    }
}
async fn json_response(request: reqwest::RequestBuilder) -> Result<PrivateJson> {
    let bytes = zeroize::Zeroizing::new(checked(request).await?.bytes().await?.to_vec());
    let result: Value = serde_json::from_slice(&bytes)?;
    if result.get("ok") == Some(&Value::Bool(false)) {
        let code = result["error"].as_str().unwrap_or("");
        let safe = match code {
            "invalid_auth" | "token_revoked" | "account_inactive" | "missing_scope"
            | "not_in_channel" | "channel_not_found" | "not_authed" | "is_archived"
            | "restricted_action" => code,
            _ => "request_rejected",
        };
        return Err(format!("Provider API rejected request ({safe})").into());
    }
    Ok(PrivateJson(result))
}
struct Harness {
    db: common::Database,
    chat: Chat,
    crypto: Arc<Encryption>,
    http: reqwest::Client,
    connection: Uuid,
    origin: String,
    token: SecretString,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}
impl Harness {
    async fn new(provider: &str) -> Result<Self> {
        let db = common::Database::new().await;
        let crypto = Arc::new(Encryption::initialize(&db.pool, common::seed(35)).await?);
        let agent = Agents::new(db.pool.clone(), crypto.clone())
            .create(CreateAgent {
                concurrency_policy: Default::default(),
                id: Uuid::new_v4(),
                name: "Live chat integration".into(),
                endpoint_url: "http://127.0.0.1:9999".into(),
                webhook_signing_key: SecretString::from(
                    "live-test-agent-signing-key-at-least-32-bytes",
                ),
                capabilities: tilde::iam::capabilities::Capabilities(BTreeMap::from([(
                    tilde::iam::capabilities::Capability::ToolsInvoke,
                    tilde::iam::capabilities::Reach::All,
                )])),
            })
            .await?;
        let connections = Connections::new(
            db.pool.clone(),
            crypto.clone(),
            "http://127.0.0.1".into(),
            "https://ingress.example".into(),
        )?;
        connections.seed().await?;
        let typ = match provider {
            "slack" => "slack_app",
            "github" => "github_app",
            "agentmail" => "inbox",
            "linq" => "account",
            "whatsapp" => "meta",
            "telnyx" => "whatsapp",
            _ => return Err("Unknown provider".into()),
        };
        let connection = connections
            .start(
                Uuid::new_v4(),
                "Live integration",
                provider,
                typ,
                &[Assignment {
                    capability: Capability::Channel,
                    agent_id: agent.id,
                }],
            )
            .await?
            .connection
            .id;
        let chat = Chat::new(db.pool.clone(), crypto.clone(), "http://127.0.0.1".into())
            .with_connections(connections.clone());
        // Live provider fixtures exercise unrestricted inbound delivery explicitly.
        tilde::chat::access::AgentAccess::new(connections.clone(), chat.clone())
            .set_mode(connection, agent.id, types::ChannelAccessMode::Public)
            .await?;
        let thread = chat
            .create_thread(chat::CreateThread {
                title: "Live provider tool invocation".into(),
                primary_agent_id: agent.id.to_string(),
                participants: vec![types::ParticipantRef {
                    agent_id: Some(agent.id.to_string()),
                    ..Default::default()
                }],
            })
            .await?;
        let run = chat
            .start_run(chat::StartRun {
                thread_id: thread.id.clone(),
                agent_id: agent.id.to_string(),
                objective: "Verify live two-way chat".into(),
                idempotency_key: Uuid::new_v4().to_string(),
                ..Default::default()
            })
            .await?;
        let invocation = Uuid::parse_str(&run.invocation_id)?;
        sqlx::query_file!("tests/sql/live_chat_lease.sql", invocation)
            .execute(&db.pool)
            .await?;
        let token = chat
            .tokens
            .issue(
                agent.id,
                invocation,
                Uuid::parse_str(&thread.id)?,
                Uuid::parse_str(&run.id)?,
            )
            .await?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let origin = format!("http://{}", listener.local_addr()?);
        let router =
            chat::rpc::runtime::router_with_tools(chat.clone(), Registry::for_chat(&chat)).merge(
                chat::providers::ingress::router(chat.clone(), connections.clone()),
            );
        let server = tokio::spawn(axum::serve(listener, router).into_future());
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("tilde-live-chat-tests")
            .build()?;
        Ok(Self {
            db,
            chat,
            crypto,
            http,
            connection,
            origin,
            token,
            server,
        })
    }
    async fn store(&self, field: &str, value: SecretString) -> Result<()> {
        let sealed = self
            .crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: self.connection,
                    name: field,
                },
                &value,
            )?
            .into_bytes();
        drop(value);
        sqlx::query_file!(
            "../../queries/connections/values_put.sql",
            self.connection,
            field,
            sealed
        )
        .execute(&self.db.pool)
        .await?;
        Ok(())
    }
    async fn credentials(&self, provider: &str) -> Result<()> {
        let fields: &[(&str, &str)] = match provider {
            "slack" => &[
                (
                    "access_token",
                    "E2E_CHATKIT_SLACK_SELF_MANAGED_PROVIDER_BOT_TOKEN",
                ),
                (
                    "signing_secret",
                    "E2E_CHATKIT_SLACK_SELF_MANAGED_PROVIDER_SIGNING_SECRET",
                ),
            ],
            "github" => &[
                ("private_key", "E2E_CHATKIT_GITHUB_APP_PRIVATE_KEY"),
                ("app_id", "E2E_CHATKIT_GITHUB_APP_ID"),
                ("installation_id", "E2E_CHATKIT_GITHUB_INSTALLATION_ID"),
                ("webhook_secret", "E2E_CHATKIT_GITHUB_WEBHOOK_SECRET"),
            ],
            "agentmail" => &[
                ("api_key", "E2E_CHATKIT_AGENTMAIL_API_KEY"),
                ("inbox_id", "E2E_CHATKIT_AGENTMAIL_INBOX_ID"),
            ],
            "linq" => &[
                ("api_token", "E2E_MCP_LINQ_API_TOKEN"),
                ("phone_number", "CHAT_LIVE_LINQ_PHONE_NUMBER"),
                ("webhook_signing_secret", "E2E_LINQ_WEBHOOK_SIGNING_SECRET"),
            ],
            "whatsapp" => &[
                ("access_token", "CHAT_TEST_WHATSAPP_ACCESS_TOKEN"),
                ("phone_number_id", "CHAT_TEST_WHATSAPP_PHONE_NUMBER_ID"),
                ("app_secret", "CHAT_TEST_WHATSAPP_APP_SECRET"),
                ("verify_token", "CHAT_TEST_WHATSAPP_VERIFY_TOKEN"),
            ],
            "telnyx" => &[
                ("api_key", "CHAT_TEST_WHATSAPP_TELNYX_API_KEY"),
                ("phone_number", "CHAT_TEST_WHATSAPP_PHONE_NUMBER"),
                (
                    "messaging_profile_id",
                    "CHAT_TEST_WHATSAPP_TELNYX_MESSAGING_PROFILE_ID",
                ),
                ("public_key", "CHAT_TEST_WHATSAPP_TELNYX_PUBLIC_KEY"),
            ],
            _ => return Err("Unknown provider".into()),
        };
        for (field, var) in fields {
            self.store(field, secret(var)?).await?;
        }
        if provider == "slack" {
            let token = secret("E2E_CHATKIT_SLACK_SELF_MANAGED_PROVIDER_BOT_TOKEN")?;
            let auth = json_response(
                self.http
                    .post("https://slack.com/api/auth.test")
                    .bearer_auth(token.expose_secret()),
            )
            .await?;
            self.store("slack_team_id", SecretString::from(text(&auth, "team_id")?))
                .await?;
            self.store(
                "slack_bot_user_id",
                SecretString::from(text(&auth, "user_id")?),
            )
            .await?;
        }
        sqlx::query_file!(
            "tests/sql/live_channel_ready.sql",
            self.connection,
            provider == "github"
        )
        .execute(&self.db.pool)
        .await?;
        Ok(())
    }
    async fn invoke(&self, input: Value, marker: &str) -> Result<types::Message> {
        let name = format!("channel_{}.sendMessage", self.connection.simple());
        let catalog = json_response(
            self.http
                .post(format!(
                    "{}/tilde.runtime.v1.ChatService/ListTools",
                    self.origin
                ))
                .bearer_auth(self.token.expose_secret())
                .json(&json!({})),
        )
        .await?;
        ensure(
            catalog["tools"]
                .as_array()
                .is_some_and(|tools| tools.iter().any(|t| t["name"] == name)),
            "Assigned provider send tool missing from runtime catalog",
        )?;
        let call = Uuid::new_v4();
        let frame = serde_json::to_vec(
            &json!({"name":name,"callId":call,"sequence":0,"inputJson":input.to_string(),"finish":true}),
        )?;
        let mut body = vec![0];
        body.extend_from_slice(&(frame.len() as u32).to_be_bytes());
        body.extend(frame);
        let response = checked(
            self.http
                .post(format!(
                    "{}/tilde.runtime.v1.ChatService/InvokeTool",
                    self.origin
                ))
                .bearer_auth(self.token.expose_secret())
                .header("content-type", "application/connect+json")
                .header("connect-protocol-version", "1")
                .body(body),
        )
        .await?
        .bytes()
        .await?;
        let mut bytes = response.as_ref();
        let mut output = None;
        while !bytes.is_empty() {
            ensure(bytes.len() >= 5, "Truncated Connect response")?;
            let length = u32::from_be_bytes(bytes[1..5].try_into()?) as usize;
            ensure(bytes.len() >= 5 + length, "Truncated Connect response body")?;
            let value: Value = serde_json::from_slice(&bytes[5..5 + length])?;
            ensure(
                value.get("error").is_none(),
                "Live provider tool returned a Connect error",
            )?;
            if let Some(result) = value["outputJson"].as_str() {
                output = Some(serde_json::from_str::<Value>(result)?);
            }
            bytes = &bytes[5 + length..];
        }
        let output = output.ok_or("Tool omitted outputJson")?;
        ensure(
            output["accepted"] == true && output["messageId"] == call.to_string(),
            "Tool did not return canonical accepted message",
        )?;
        let message = self.chat.message(call).await?;
        ensure(
            message.text.contains(marker),
            "Outbound message text was not persisted",
        )?;
        ensure(
            message.delivery.connection_id == self.connection.to_string(),
            "Outbound delivery used another connection",
        )?;
        ensure(
            !message.delivery.external_message_id.is_empty(),
            "Provider omitted external delivery ID",
        )?;
        ensure(
            output["externalMessageId"] == message.delivery.external_message_id,
            "Tool response differs from stored delivery",
        )?;
        let audit = sqlx::query_file!("../../queries/chat/tool_get.sql", call)
            .fetch_one(&self.db.pool)
            .await?;
        ensure(
            audit.status == "completed" && audit.error.is_empty(),
            "Tool audit did not record success",
        )?;
        ensure(
            serde_json::from_str::<Value>(&audit.input_json)? == input,
            "Audited tool input differs from the RPC request",
        )?;
        ensure(
            serde_json::from_str::<Value>(&audit.output_json)? == output,
            "Audited tool response differs from the RPC response",
        )?;
        Ok(message)
    }
    async fn close(self) {
        self.server.abort();
        self.db.close().await;
    }
}

enum Cleanup {
    Slack {
        channel: String,
        ts: String,
        token: &'static str,
    },
    Github {
        repo: String,
        number: u64,
    },
    MailInbox(String),
    MailWebhook(String),
}
impl Cleanup {
    async fn run(self, h: &Harness) -> Result<()> {
        match self {
            Self::Slack { channel, ts, token } => {
                json_response(
                    h.http
                        .post("https://slack.com/api/chat.delete")
                        .bearer_auth(secret(token)?.expose_secret())
                        .json(&json!({"channel":channel,"ts":ts})),
                )
                .await?;
            }
            Self::Github { repo, number } => {
                json_response(
                    h.http
                        .patch(format!(
                            "https://api.github.com/repos/{repo}/issues/{number}"
                        ))
                        .bearer_auth(secret("E2E_CHATKIT_GITHUB_PAT")?.expose_secret())
                        .json(&json!({"state":"closed"})),
                )
                .await?;
            }
            Self::MailInbox(id) => {
                checked(
                    h.http
                        .delete(mail_url(&["inboxes", &id])?)
                        .bearer_auth(secret("E2E_CHATKIT_AGENTMAIL_API_KEY")?.expose_secret()),
                )
                .await?;
            }
            Self::MailWebhook(id) => {
                checked(
                    h.http
                        .delete(mail_url(&["webhooks", &id])?)
                        .bearer_auth(secret("E2E_CHATKIT_AGENTMAIL_API_KEY")?.expose_secret()),
                )
                .await?;
            }
        }
        Ok(())
    }
}
fn mail_url(parts: &[&str]) -> Result<url::Url> {
    let mut url = url::Url::parse("https://api.agentmail.to/v0")?;
    url.path_segments_mut()
        .map_err(|_| "Invalid email API URL")?
        .extend(parts.iter().copied());
    Ok(url)
}

fn slack_manual() -> bool {
    std::env::var("CHAT_LIVE_SLACK_MANUAL_REPLY").as_deref() == Ok("true")
}
pub async fn run(provider: &str) -> Result<()> {
    use futures::FutureExt;
    // Fail before any remote writes if the callback source is not configured.
    let source = setting(&format!("CHAT_LIVE_{}_SOURCE_ID", provider.to_uppercase()))?;
    let _ = secret("HOOKDECK_API_KEY")?;
    match provider {
        "slack" if !slack_manual() => {
            let _ = secret("E2E_CHATKIT_SLACK_USER_TOKEN")?;
        }
        "github" => {
            let _ = secret("E2E_CHATKIT_GITHUB_PAT")?;
        }
        "agentmail" => {
            let _ = setting("CHAT_LIVE_AGENTMAIL_SOURCE_URL")?;
        }
        "whatsapp" | "telnyx" => {
            let _ = setting(&format!("CHAT_LIVE_{}_RECIPIENT", provider.to_uppercase()))?;
        }
        _ => {}
    }
    let h = Harness::new(provider).await?;
    let mut cleanup = Vec::new();
    let result = std::panic::AssertUnwindSafe(async {
        h.credentials(provider).await?;
        scenario(&h, provider, &source, &mut cleanup).await
    })
    .catch_unwind()
    .await;
    let mut cleanup_failed = false;
    for action in cleanup.into_iter().rev() {
        if action.run(&h).await.is_err() {
            cleanup_failed = true;
            eprintln!(
                "{provider}: remote test fixture cleanup failed; inspect the uniquely marked test resources"
            );
        }
    }
    h.close().await;
    match result {
        Ok(result) => {
            result?;
            ensure(!cleanup_failed, "Remote fixture cleanup failed")
        }
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn scenario(
    h: &Harness,
    provider: &str,
    source: &str,
    cleanup: &mut Vec<Cleanup>,
) -> Result<()> {
    let marker = format!("tilde-live-{}", Uuid::new_v4().simple());
    let inbound = format!("{marker}-reply");
    let outbound_text = format!("Tilde live test {marker}. Please reply with exactly: {inbound}");
    let mut input = json!({"text":outbound_text});
    let mut repo = String::new();
    let mut issue = 0u64;
    let mut peer_inbox = String::new();
    match provider {
        "slack" => {
            input["channelId"] = json!(setting("E2E_CHATKIT_SLACK_SELF_MANAGED_CHANNEL_ID")?);
        }
        "github" => {
            repo = format!(
                "{}/{}",
                setting("E2E_CHATKIT_GITHUB_OWNER")?,
                setting("E2E_CHATKIT_GITHUB_REPO")?
            );
            let created = json_response(
                h.http
                    .post(format!("https://api.github.com/repos/{repo}/issues"))
                    .bearer_auth(secret("E2E_CHATKIT_GITHUB_PAT")?.expose_secret())
                    .json(
                        &json!({"title":marker,"body":"Temporary two-way chat integration test"}),
                    ),
            )
            .await?;
            issue = created["number"]
                .as_u64()
                .ok_or("GitHub omitted issue number")?;
            cleanup.push(Cleanup::Github {
                repo: repo.clone(),
                number: issue,
            });
            input["owner"] = json!(setting("E2E_CHATKIT_GITHUB_OWNER")?);
            input["repository"] = json!(setting("E2E_CHATKIT_GITHUB_REPO")?);
            input["number"] = json!(issue);
        }
        "agentmail" => {
            let key = secret("E2E_CHATKIT_AGENTMAIL_API_KEY")?;
            let peer = json_response(
                h.http
                    .post(mail_url(&["inboxes"])?)
                    .bearer_auth(key.expose_secret())
                    .json(&json!({"client_id":marker})),
            )
            .await?;
            peer_inbox = text(&peer, "inbox_id")?.into();
            cleanup.push(Cleanup::MailInbox(peer_inbox.clone()));
            let main = setting("E2E_CHATKIT_AGENTMAIL_INBOX_ID")?;
            let webhook=json_response(h.http.post(mail_url(&["webhooks"])?).bearer_auth(key.expose_secret()).json(&json!({"url":setting("CHAT_LIVE_AGENTMAIL_SOURCE_URL")?,"event_types":["message.received"],"inbox_ids":[main],"client_id":marker}))).await?;
            cleanup.push(Cleanup::MailWebhook(text(&webhook, "webhook_id")?.into()));
            h.store(
                "webhook_secret",
                SecretString::from(text(&webhook, "secret")?),
            )
            .await?;
            input["to"] = json!([peer_inbox]);
            input["subject"] = json!(marker);
        }
        "linq" => {
            input["chatId"] = json!(setting("E2E_MCP_LINQ_CHAT_ID")?);
        }
        "whatsapp" | "telnyx" => {
            input["to"] = json!(setting(&format!(
                "CHAT_LIVE_{}_RECIPIENT",
                provider.to_uppercase()
            ))?);
        }
        _ => return Err("Unknown provider".into()),
    }
    let since = (chrono::Utc::now() - chrono::Duration::seconds(10)).to_rfc3339();
    let sent = h.invoke(input, &marker).await?;
    let external = sent.delivery.external_message_id.as_str();
    match provider {
        "slack" => {
            let channel = setting("E2E_CHATKIT_SLACK_SELF_MANAGED_CHANNEL_ID")?;
            cleanup.push(Cleanup::Slack {
                channel: channel.clone(),
                ts: external.into(),
                token: "E2E_CHATKIT_SLACK_SELF_MANAGED_PROVIDER_BOT_TOKEN",
            });
            let history = json_response(
                h.http
                    .get("https://slack.com/api/conversations.history")
                    .bearer_auth(
                        secret("E2E_CHATKIT_SLACK_SELF_MANAGED_PROVIDER_BOT_TOKEN")?
                            .expose_secret(),
                    )
                    .query(&[
                        ("channel", channel.as_str()),
                        ("oldest", external),
                        ("inclusive", "true"),
                        ("limit", "1"),
                    ]),
            )
            .await?;
            ensure(
                history["messages"].as_array().is_some_and(|a| {
                    a.iter().any(|m| {
                        m["ts"] == external
                            && m["text"].as_str().is_some_and(|t| t.contains(&marker))
                    })
                }),
                "Slack peer could not read the outbound tool message",
            )?;
            if slack_manual() {
                eprintln!(
                    "slack: in channel {channel}, reply in the new test thread ({external}) with {inbound} and mention the test app within 180 seconds."
                );
            } else {
                let token = secret("E2E_CHATKIT_SLACK_USER_TOKEN")?;
                // The original test app subscribes to app_mention; address it explicitly.
                let bot = json_response(
                    h.http.post("https://slack.com/api/auth.test").bearer_auth(
                        secret("E2E_CHATKIT_SLACK_SELF_MANAGED_PROVIDER_BOT_TOKEN")?
                            .expose_secret(),
                    ),
                )
                .await?;
                let reply_text = format!("<@{}> {inbound}", text(&bot, "user_id")?);
                let reply = json_response(
                h.http
                    .post("https://slack.com/api/chat.postMessage")
                    .bearer_auth(token.expose_secret())
                    .json(&json!({"channel":channel,"thread_ts":external,"text":reply_text,"as_user":true})),
            )
            .await?;
                cleanup.push(Cleanup::Slack {
                    channel,
                    ts: text(&reply, "ts")?.into(),
                    token: "E2E_CHATKIT_SLACK_USER_TOKEN",
                });
            }
        }
        "github" => {
            let token = secret("E2E_CHATKIT_GITHUB_PAT")?;
            let read = json_response(
                h.http
                    .get(format!(
                        "https://api.github.com/repos/{repo}/issues/comments/{external}"
                    ))
                    .bearer_auth(token.expose_secret()),
            )
            .await?;
            ensure(
                text(&read, "body")?.contains(&marker),
                "GitHub peer could not read the outbound comment",
            )?;
            json_response(
                h.http
                    .post(format!(
                        "https://api.github.com/repos/{repo}/issues/{issue}/comments"
                    ))
                    .bearer_auth(token.expose_secret())
                    .json(&json!({"body":inbound})),
            )
            .await?;
        }
        "agentmail" => {
            let token = secret("E2E_CHATKIT_AGENTMAIL_API_KEY")?;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
            let message_id = loop {
                let messages = json_response(
                    h.http
                        .get(mail_url(&["inboxes", &peer_inbox, "messages"])?)
                        .bearer_auth(token.expose_secret()),
                )
                .await?;
                let found = messages["messages"]
                    .as_array()
                    .and_then(|a| a.iter().find(|m| m.to_string().contains(&marker)));
                if let Some(message) = found {
                    break text(message, "message_id")?.to_owned();
                }
                ensure(
                    tokio::time::Instant::now() < deadline,
                    "Email never reached the real peer inbox",
                )?;
                tokio::time::sleep(Duration::from_secs(2)).await;
            };
            json_response(
                h.http
                    .post(mail_url(&[
                        "inboxes",
                        &peer_inbox,
                        "messages",
                        &message_id,
                        "reply",
                    ])?)
                    .bearer_auth(token.expose_secret())
                    .json(&json!({"text":inbound})),
            )
            .await?;
        }
        _ => eprintln!(
            "{provider}: real outbound send accepted. Test recipient must reply with {inbound} within 180 seconds. For WhatsApp, open the customer-service window by messaging the business line before starting."
        ),
    }
    hookdeck::replay(h, source, &inbound, &since, &sent).await?;
    let messages = h
        .chat
        .messages(Uuid::parse_str(&sent.thread_id)?, 100)
        .await?;
    ensure(
        messages.len() == 2,
        "Expected exactly one outbound and one deduplicated inbound message in the same conversation",
    )?;
    let received = messages
        .iter()
        .find(|m| m.id != sent.id && m.text.contains(&inbound))
        .ok_or("Real peer response not persisted in the outbound thread")?;
    ensure(
        received.participant_id != sent.participant_id,
        "Inbound peer response was attributed to the sending agent",
    )?;
    println!(
        "{provider}: live tool response, remote receipt, signed callback and duplicate replay passed"
    );
    Ok(())
}
