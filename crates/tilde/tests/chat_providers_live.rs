//! Opt-in read-only account checks. Local adapter tests cover sends and signed callbacks.
//! Missing credentials fail these tests; ordinary `cargo test` ignores live upstreams.
#[allow(dead_code, reason = "Shared database fixture supports other suites")]
mod common;
use secrecy::{ExposeSecret, SecretString};
use std::{sync::Arc, time::Duration};
use tilde::{
    connections::service::Connections,
    encryption::{Encryption, SecretBinding},
};
use uuid::Uuid;
use zeroize::Zeroizing;

fn setting(name: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| panic!("{name} is required; run task secrets:load and check .env.test"))
}

// Exercise persisted encrypted credentials and the same resolution boundary used by chat.
// No response bodies or credentials are included in assertion failures.
async fn probe(provider: &str, typ: &str, credential: &str, url: String, slack: bool) {
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(31))
            .await
            .unwrap(),
    );
    let connections = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        "http://127.0.0.1".into(),
        "https://ingress.example".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    let id = connections
        .start(Uuid::new_v4(), "Read-only live probe", provider, typ, &[])
        .await
        .unwrap()
        .connection
        .id;
    let key = match provider {
        "agentmail" | "telnyx" => "api_key",
        "linq" => "api_token",
        _ => "access_token",
    };
    let fields = if provider == "github" {
        vec![
            ("private_key", credential),
            ("app_id", "E2E_CHATKIT_GITHUB_APP_ID"),
            ("installation_id", "E2E_CHATKIT_GITHUB_INSTALLATION_ID"),
        ]
    } else {
        vec![(key, credential)]
    };
    for (field, variable) in fields {
        let secret = SecretString::from(setting(variable));
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: id,
                    name: field,
                },
                &secret,
            )
            .unwrap()
            .into_bytes();
        drop(secret);
        sqlx::query_file!(
            "../../queries/connections/values_put.sql",
            id,
            field,
            sealed
        )
        .execute(&db.pool)
        .await
        .unwrap();
    }
    // GitHub resolves an installation token through the production refresh path.
    sqlx::query_file!("tests/sql/live_channel_ready.sql", id, provider == "github")
        .execute(&db.pool)
        .await
        .unwrap();
    let values = match connections.resolve(id).await {
        Ok(values) => values,
        Err(_) => {
            db.close().await;
            panic!("{provider}: configured connection credentials could not be resolved");
        }
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("tilde-chat-provider-tests")
        .build()
        .unwrap();
    let response = client
        .get(url)
        .bearer_auth(values[key].expose_secret())
        .header("accept", "application/json")
        .send()
        .await;
    drop(values); // SecretString zeroizes the decrypted connection values on drop.
    let outcome = match response {
        Err(_) => Err(format!("{provider}: upstream request failed")),
        Ok(response) if !response.status().is_success() => Err(format!(
            "{provider}: upstream returned HTTP {}",
            response.status().as_u16()
        )),
        Ok(response) => {
            if slack {
                #[derive(serde::Deserialize)]
                struct SlackResult {
                    ok: bool,
                }
                let bytes = Zeroizing::new(response.bytes().await.unwrap().to_vec());
                match serde_json::from_slice::<SlackResult>(&bytes) {
                    Ok(result) if result.ok => Ok(()),
                    _ => Err("slack: API rejected the configured credential/channel".into()),
                }
            } else {
                Ok(())
            }
        }
    };
    db.close().await;
    assert!(outcome.is_ok(), "{}", outcome.unwrap_err());
}

#[tokio::test]
#[ignore = "live account; task test:chat:accounts"]
async fn slack_configured_channel_is_accessible() {
    let mut url = url::Url::parse("https://slack.com/api/conversations.info").unwrap();
    url.query_pairs_mut().append_pair(
        "channel",
        &setting("E2E_CHATKIT_SLACK_SELF_MANAGED_CHANNEL_ID"),
    );
    probe(
        "slack",
        "slack_app",
        "E2E_CHATKIT_SLACK_SELF_MANAGED_PROVIDER_BOT_TOKEN",
        url.into(),
        true,
    )
    .await;
}

#[tokio::test]
#[ignore = "live account; task test:chat:accounts"]
async fn github_configured_repository_is_accessible() {
    let url = format!(
        "https://api.github.com/repos/{}/{}",
        setting("E2E_CHATKIT_GITHUB_OWNER"),
        setting("E2E_CHATKIT_GITHUB_REPO")
    );
    probe(
        "github",
        "github_app",
        "E2E_CHATKIT_GITHUB_APP_PRIVATE_KEY",
        url,
        false,
    )
    .await;
}

#[tokio::test]
#[ignore = "live account; task test:chat:accounts"]
async fn agentmail_configured_inbox_is_accessible() {
    let mut url = url::Url::parse("https://api.agentmail.to/v0/inboxes/").unwrap();
    url.path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .push(&setting("E2E_CHATKIT_AGENTMAIL_INBOX_ID"));
    probe(
        "agentmail",
        "inbox",
        "E2E_CHATKIT_AGENTMAIL_API_KEY",
        url.into(),
        false,
    )
    .await;
}

#[tokio::test]
#[ignore = "live account; task test:chat:accounts"]
async fn linq_configured_chat_is_accessible() {
    let mut url = url::Url::parse("https://api.linqapp.com/api/partner/v3/chats/").unwrap();
    url.path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .push(&setting("E2E_MCP_LINQ_CHAT_ID"))
        .push("messages");
    url.query_pairs_mut().append_pair("limit", "1");
    probe(
        "linq",
        "account",
        "E2E_MCP_LINQ_API_TOKEN",
        url.into(),
        false,
    )
    .await;
}

#[tokio::test]
#[ignore = "live account; task test:chat:accounts"]
async fn whatsapp_configured_phone_is_accessible() {
    let url = format!(
        "https://graph.facebook.com/v23.0/{}",
        setting("CHAT_TEST_WHATSAPP_PHONE_NUMBER_ID")
    );
    probe(
        "whatsapp",
        "meta",
        "CHAT_TEST_WHATSAPP_ACCESS_TOKEN",
        url,
        false,
    )
    .await;
}

#[tokio::test]
#[ignore = "live account; task test:chat:accounts"]
async fn telnyx_configured_profile_is_accessible() {
    let url = format!(
        "https://api.telnyx.com/v2/messaging_profiles/{}",
        setting("CHAT_TEST_WHATSAPP_TELNYX_MESSAGING_PROFILE_ID")
    );
    probe(
        "telnyx",
        "whatsapp",
        "CHAT_TEST_WHATSAPP_TELNYX_API_KEY",
        url,
        false,
    )
    .await;
}
