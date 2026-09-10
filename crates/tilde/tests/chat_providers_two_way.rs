//! Real tools -> provider delivery -> peer reply -> original signed callback -> stored thread.
//! Cargo requires live-chat-tests; ignored also prevents --all-features from sending messages.
#[allow(dead_code)]
mod common;
mod live_chat;

#[tokio::test]
#[ignore = "real Slack messages; task test:chat:live -- slack"]
async fn slack_two_way() -> live_chat::Result<()> {
    live_chat::run("slack").await
}
#[tokio::test]
#[ignore = "real GitHub issue and comments; task test:chat:live -- github"]
async fn github_two_way() -> live_chat::Result<()> {
    live_chat::run("github").await
}
#[tokio::test]
#[ignore = "real email and temporary inbox/webhook; task test:chat:live -- agentmail"]
async fn agentmail_two_way() -> live_chat::Result<()> {
    live_chat::run("agentmail").await
}
#[tokio::test]
#[ignore = "real Linq message; requires recipient reply; task test:chat:live -- linq"]
async fn linq_two_way() -> live_chat::Result<()> {
    live_chat::run("linq").await
}
#[tokio::test]
#[ignore = "real WhatsApp message; requires recipient reply; task test:chat:live -- whatsapp"]
async fn whatsapp_two_way() -> live_chat::Result<()> {
    live_chat::run("whatsapp").await
}
#[tokio::test]
#[ignore = "real Telnyx WhatsApp message; requires recipient reply; task test:chat:live -- telnyx"]
async fn telnyx_two_way() -> live_chat::Result<()> {
    live_chat::run("telnyx").await
}
