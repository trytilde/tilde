//! Managed tool providers on channel connections (Linq, AgentMail, WhatsApp over Meta and
//! Telnyx) and E2B: an encrypted connection's credentials reach each upstream through the agent's tools
//! function called from an agent's invocation. One fake upstream stands in for every API and
//! records what arrived.
mod common;
use common::invocation::{Fixture, all_tools};
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::{
    connections::catalog::Endpoints,
    encryption::{Encryption, SecretBinding},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
struct Seen {
    method: String,
    uri: String,
    auth: String,
    body: String,
}

fn frame(flags: u8, value: Value) -> Vec<u8> {
    let bytes = value.to_string().into_bytes();
    let mut out = vec![flags];
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend(bytes);
    out
}

/// What each fake upstream answers.
fn answer(method: &str, uri: &str, body: &str) -> (u16, Vec<u8>) {
    let json = |status: u16, value: Value| (status, value.to_string().into_bytes());
    let path = uri.split('?').next().unwrap();
    match (method, path) {
        ("GET", "/linq/chats/healthy") => json(
            200,
            json!({"id":"healthy","health_status":{"status":"HEALTHY"}}),
        ),
        ("GET", "/linq/chats/opted") => json(
            200,
            json!({"id":"opted","health_status":{"status":"OPTED_OUT"}}),
        ),
        ("GET", "/agentmail/inboxes/bot@agentmail.to/threads/t1") => json(
            200,
            json!({"thread_id":"t1","messages":[{"to":["visible@example.com"],"bcc":["hidden@example.com"],"headers":[{"name":"Bcc","value":"hidden@example.com"}]}]}),
        ),
        ("GET", "/graph/pn-ok") => json(200, json!({"id":"pn-ok","status":"CONNECTED"})),
        ("GET", "/graph/pn-banned") => json(200, json!({"id":"pn-banned","status":"BANNED"})),
        ("POST", "/graph/pn-ok/messages") if body.contains("15550000000") => json(
            400,
            json!({"error":{"code":131047,"message":"(#131047) Re-engagement message"}}),
        ),
        ("POST", "/graph/pn-ok/messages") => json(
            200,
            json!({"messaging_product":"whatsapp","messages":[{"id":"wamid.1"}]}),
        ),
        ("GET", "/telnyx/whatsapp/phone_numbers") => json(
            200,
            json!({"data":[{"phone_number":"+15551112222","phone_number_id":"tx-pn","waba_id":"waba-7","status":"connected"}]}),
        ),
        ("GET", "/telnyx/whatsapp/message_templates") => json(
            200,
            json!({"data":[{"name":"late_reply","status":"APPROVED"},{"name":"draft","status":"PENDING"}]}),
        ),
        ("POST", "/e2b/sandboxes/sb1/connect") => json(
            200,
            json!({"sandboxID":"sb1","envdAccessToken":"envd-token","sandboxDomain":"e2b.test"}),
        ),
        ("GET", "/envd/files") => (
            200,
            b"def one():\n    return 1\ndef two():\n    return 1\n".to_vec(),
        ),
        ("POST", "/envd/filesystem.Filesystem/MakeDir") => {
            json(400, json!({"code":"already_exists","message":"exists"}))
        }
        ("POST", "/envd/process.Process/Start") => {
            let mut stream = frame(0, json!({"event":{"start":{"pid":7}}}));
            stream.extend(frame(0, json!({"event":{"data":{"stdout":"aGkK"}}})));
            stream.extend(frame(
                0,
                json!({"event":{"end":{"exitCode":3,"status":"exited"}}}),
            ));
            stream.extend(frame(2, json!({})));
            (200, stream)
        }
        _ => json(200, json!({"ok":true})),
    }
}

async fn connection(
    fx: &Fixture,
    crypto: &Encryption,
    provider: &str,
    typ: &str,
    values: &[(&str, &str)],
) -> Uuid {
    let id = Uuid::new_v4();
    fx.connections
        .start(id, provider, provider, typ, &[])
        .await
        .unwrap();
    let pg = fx.db.pool.get().await.unwrap();
    for (key, value) in values {
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: id,
                    name: key,
                },
                &SecretString::from(*value),
            )
            .unwrap()
            .into_bytes();
        pg.execute(
            "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)",
            &[&id, key, &sealed],
        )
        .await
        .unwrap();
    }
    pg.execute("UPDATE connections SET status='ready' WHERE id=$1", &[&id])
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn channel_and_sandbox_providers_call_their_apis_with_connection_credentials() {
    let seen: Arc<Mutex<Vec<Seen>>> = Default::default();
    let sink = seen.clone();
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", upstream.local_addr().unwrap());
    let upstream_server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(
            move |method: axum::http::Method,
                  uri: axum::http::Uri,
                  headers: axum::http::HeaderMap,
                  body: axum::body::Bytes| {
                let sink = sink.clone();
                async move {
                    let auth = ["authorization", "x-api-key", "x-access-token"]
                        .iter()
                        .filter_map(|h| headers.get(*h).map(|v| v.to_str().unwrap().to_owned()))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let uri = uri.to_string();
                    let body = String::from_utf8_lossy(&body).into_owned();
                    sink.lock().unwrap().push(Seen {
                        method: method.to_string(),
                        uri: uri.clone(),
                        auth,
                        body: body.clone(),
                    });
                    let (status, body) = answer(method.as_str(), &uri, &body);
                    (axum::http::StatusCode::from_u16(status).unwrap(), body)
                }
            },
        );
        axum::serve(upstream, app).await.unwrap()
    });
    let endpoints = Endpoints(BTreeMap::from_iter(
        [
            ("linq_api", "linq"),
            ("agentmail_api", "agentmail"),
            ("meta_graph", "graph"),
            ("telnyx_api", "telnyx"),
            ("e2b_api", "e2b"),
            ("e2b_envd", "envd"),
        ]
        .map(|(key, path)| (key.to_owned(), format!("{origin}/{path}"))),
    ));
    let fx = Fixture::new(41, endpoints, all_tools()).await;
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(41))
        .await
        .unwrap();
    let linq = connection(
        &fx,
        &crypto,
        "linq",
        "account",
        &[
            ("api_token", "linq-token"),
            ("phone_number", "+15550001111"),
            ("webhook_signing_secret", "s"),
        ],
    )
    .await;
    let mail = connection(
        &fx,
        &crypto,
        "agentmail",
        "inbox",
        &[
            ("inbox_id", "bot@agentmail.to"),
            ("api_key", "am-key"),
            ("webhook_secret", "s"),
        ],
    )
    .await;
    let meta_values = |number: &'static str| {
        [
            ("access_token", "meta-token"),
            ("app_secret", "s"),
            ("verify_token", "v"),
            ("phone_number_id", number),
            ("waba_id", "waba-1"),
        ]
    };
    let meta = connection(&fx, &crypto, "whatsapp", "meta", &meta_values("pn-ok")).await;
    let banned = connection(&fx, &crypto, "whatsapp", "meta", &meta_values("pn-banned")).await;
    let telnyx = connection(
        &fx,
        &crypto,
        "telnyx",
        "whatsapp",
        &[
            ("api_key", "tx-key"),
            ("phone_number", "+15551112222"),
            ("messaging_profile_id", "mp-1"),
            ("public_key", "k"),
        ],
    )
    .await;
    let e2b = connection(&fx, &crypto, "e2b", "api", &[("api_key", "e2b_key")]).await;

    assert_eq!(
        fx.use_tools(
            linq,
            &[
                "linq_send_message",
                "linq_send_chat_message",
                "linq_manage_webhook_subscription"
            ]
        )
        .await,
        "linq"
    );
    assert_eq!(
        fx.use_tools(mail, &["agentmail_send_message", "agentmail_get_thread"])
            .await,
        "agentmail"
    );
    assert_eq!(
        fx.use_tools(meta, &["whatsapp_send_text", "whatsapp_mark_read"])
            .await,
        "whatsapp"
    );
    assert_eq!(
        fx.use_tools(banned, &["whatsapp_send_text"]).await,
        "whatsapp_2"
    );
    assert_eq!(
        fx.use_tools(
            telnyx,
            &[
                "whatsapp_send_text",
                "whatsapp_mark_read",
                "whatsapp_list_templates"
            ]
        )
        .await,
        "telnyx"
    );
    assert_eq!(
        fx.use_tools(
            e2b,
            &[
                "e2b_list_sandboxes",
                "e2b_delete_sandbox",
                "e2b_exec_command",
                "e2b_apply_patch"
            ]
        )
        .await,
        "e2b"
    );
    let listed = fx.tools().await;
    assert_eq!(
        listed
            .iter()
            .filter(|t| {
                let prefix = t["name"].as_str().unwrap().split('.').next().unwrap();
                [
                    "linq",
                    "agentmail",
                    "whatsapp",
                    "whatsapp_2",
                    "telnyx",
                    "e2b",
                ]
                .contains(&prefix)
            })
            .count(),
        15
    );
    let take = || std::mem::take(&mut *seen.lock().unwrap());

    // Linq: sends are idempotent per call and `from` is refused before reaching Linq.
    let refused = fx
        .invoke(
            "linq.linq_send_message",
            json!({"to":["+15557654321"],"message":{"parts":[]},"from":"+1555"}),
        )
        .await;
    assert!(refused.get("error").is_some(), "{refused}");
    assert!(take().is_empty());
    let sent = fx
        .output(
            "linq.linq_send_message",
            json!({"to":["+15557654321"],"message":{"parts":[{"type":"text","value":"hi"}]}}),
        )
        .await;
    assert_eq!(sent["data"]["ok"], true);
    let calls = take();
    assert_eq!(
        (
            calls[0].method.as_str(),
            calls[0].uri.as_str(),
            calls[0].auth.as_str()
        ),
        ("POST", "/linq/messages", "Bearer linq-token")
    );
    assert_eq!(
        serde_json::from_str::<Value>(&calls[0].body).unwrap()["to"],
        json!(["+15557654321"])
    );
    let opted = fx
        .invoke(
            "linq.linq_send_chat_message",
            json!({"id":"opted","message":{"parts":[]}}),
        )
        .await;
    assert!(opted["error"].to_string().contains("OPTED_OUT"), "{opted}");
    assert_eq!(take().len(), 1, "an opted-out chat is only read");
    fx.output(
        "linq.linq_send_chat_message",
        json!({"id":"healthy","message":{"parts":[]}}),
    )
    .await;
    let calls = take();
    assert_eq!(calls[1].uri, "/linq/chats/healthy/messages");
    assert_eq!(
        serde_json::from_str::<Value>(&calls[1].body).unwrap(),
        json!({"message":{"parts":[]}})
    );
    fx.output(
        "linq.linq_manage_webhook_subscription",
        json!({"operation":"delete","subscription_id":"sub 1"}),
    )
    .await;
    let calls = take();
    assert_eq!(
        (calls[0].method.as_str(), calls[0].uri.as_str()),
        ("DELETE", "/linq/webhook-subscriptions/sub%201")
    );

    // AgentMail: the connection's inbox; blind recipients never come back to the agent.
    fx.output(
        "agentmail.agentmail_send_message",
        json!({"reply_to_message_id":"m1","content":"Thanks","bcc":["audit@example.com"]}),
    )
    .await;
    let calls = take();
    assert_eq!(
        calls[0].uri,
        "/agentmail/inboxes/bot@agentmail.to/messages/m1/reply"
    );
    assert_eq!(calls[0].auth, "Bearer am-key");
    assert_eq!(
        serde_json::from_str::<Value>(&calls[0].body).unwrap(),
        json!({"text":"Thanks","bcc":["audit@example.com"]})
    );
    let thread = fx
        .output("agentmail.agentmail_get_thread", json!({"thread_id":"t1"}))
        .await;
    assert!(
        !thread.to_string().contains("hidden@example.com"),
        "{thread}"
    );
    assert_eq!(
        thread["data"]["messages"][0]["to"],
        json!(["visible@example.com"])
    );

    take();

    // WhatsApp over Meta: the number's health gates sends; the body is a Cloud API message.
    let text = fx
        .output(
            "whatsapp.whatsapp_send_text",
            json!({"to":"+1 (555) 765-4321","text":"hello","reply_to_message_id":"wamid.0"}),
        )
        .await;
    assert_eq!(text["messages"][0]["id"], "wamid.1");
    let calls = take();
    assert_eq!(
        (calls[0].uri.as_str(), calls[0].auth.as_str()),
        ("/graph/pn-ok?fields=status", "Bearer meta-token")
    );
    assert_eq!(
        serde_json::from_str::<Value>(&calls[1].body).unwrap(),
        json!({"messaging_product":"whatsapp","recipient_type":"individual","to":"15557654321","type":"text","text":{"body":"hello","preview_url":false},"context":{"message_id":"wamid.0"}})
    );
    let closed = fx
        .invoke(
            "whatsapp.whatsapp_send_text",
            json!({"to":"15550000000","text":"late"}),
        )
        .await;
    let closed: Value = serde_json::from_str(
        closed["error"]["message"]
            .as_str()
            .unwrap_or_else(|| panic!("{closed}")),
    )
    .unwrap();
    assert_eq!(closed["error"], "whatsapp_customer_service_window_closed");
    assert!(
        closed["suggested_action"]
            .as_str()
            .unwrap()
            .contains("whatsapp_send_template")
    );
    let long = fx
        .invoke(
            "whatsapp.whatsapp_send_text",
            json!({"to":"15557654321","text":"x".repeat(4097)}),
        )
        .await;
    assert!(long.get("error").is_some());
    take();
    let banned = fx
        .invoke(
            "whatsapp_2.whatsapp_send_text",
            json!({"to":"15557654321","text":"hello"}),
        )
        .await;
    assert!(
        banned["error"]
            .to_string()
            .contains("whatsapp_phone_number_unhealthy"),
        "{banned}"
    );
    assert!(
        take().iter().all(|c| c.method == "GET"),
        "a banned number never sends"
    );
    fx.output(
        "whatsapp.whatsapp_mark_read",
        json!({"message_id":"wamid.9","typing_indicator":true}),
    )
    .await;
    assert_eq!(
        serde_json::from_str::<Value>(&take()[0].body).unwrap(),
        json!({"messaging_product":"whatsapp","status":"read","message_id":"wamid.9","typing_indicator":{"type":"text"}})
    );

    // WhatsApp over Telnyx: the same message inside Telnyx's envelope; the window error is typed.
    let _ = fx
        .output(
            "telnyx.whatsapp_send_text",
            json!({"to":"15557654321","text":"hi"}),
        )
        .await;
    let calls = take();
    let send = calls
        .iter()
        .find(|c| c.uri == "/telnyx/messages/whatsapp")
        .unwrap();
    assert_eq!(send.auth, "Bearer tx-key");
    assert_eq!(
        serde_json::from_str::<Value>(&send.body).unwrap(),
        json!({"from":"+15551112222","to":"+15557654321","type":"WHATSAPP","messaging_profile_id":"mp-1","whatsapp_message":{"type":"text","text":{"body":"hi","preview_url":false}}})
    );
    let skipped = fx
        .output("telnyx.whatsapp_mark_read", json!({"message_id":"m"}))
        .await;
    assert_eq!(skipped["skipped"], true);
    assert!(take().is_empty());
    let templates = fx
        .output(
            "telnyx.whatsapp_list_templates",
            json!({"status":"approved"}),
        )
        .await;
    assert_eq!(
        templates["data"],
        json!([{"name":"late_reply","status":"APPROVED"}])
    );
    assert!(
        take()
            .iter()
            .any(|c| c.uri == "/telnyx/whatsapp/message_templates?waba_id=waba-7")
    );

    // E2B: REST with the API key; sandbox work through envd with the connect token.
    fx.output("e2b.e2b_list_sandboxes", json!({"state":"running"}))
        .await;
    fx.output("e2b.e2b_delete_sandbox", json!({"sandbox_id":"sb1"}))
        .await;
    let calls = take();
    assert_eq!(
        (calls[0].uri.as_str(), calls[0].auth.as_str()),
        ("/e2b/sandboxes?state=running", "e2b_key")
    );
    assert_eq!(
        (calls[1].method.as_str(), calls[1].uri.as_str()),
        ("DELETE", "/e2b/sandboxes/sb1")
    );
    let run = fx
        .output(
            "e2b.e2b_exec_command",
            json!({"sandbox_id":"sb1","cmd":"echo hi; exit 3"}),
        )
        .await;
    assert_eq!(
        run,
        json!({"exit_code":3,"stdout":"hi\n","stderr":"","timed_out":false})
    );
    let calls = take();
    assert_eq!(calls[1].uri, "/envd/process.Process/Start");
    assert_eq!(calls[1].auth, "envd-token");
    let patched = fx
        .output(
            "e2b.e2b_apply_patch",
            json!({"sandbox_id":"sb1","patch":"*** Begin Patch\n*** Update File: /app/main.py\n@@ def two():\n-    return 1\n+    return 2\n*** End Patch"}),
        )
        .await;
    assert_eq!(
        patched,
        json!({"added":[],"modified":["/app/main.py"],"deleted":[]})
    );
    let calls = take();
    let write = calls
        .iter()
        .find(|c| c.method == "POST" && c.uri.starts_with("/envd/files?"))
        .unwrap();
    assert_eq!(write.uri, "/envd/files?path=%2Fapp%2Fmain.py");
    assert_eq!(
        write.body,
        "def one():\n    return 1\ndef two():\n    return 2\n"
    );

    upstream_server.abort();
    fx.close().await;
}
