//! Like tilde-api's HookdeckE2eClient, but fetch raw_body so signatures are never forged.
//! https://github.com/hookdeck/hookdeck-cli/blob/main/pkg/hookdeck/events.go
use super::*;
use std::collections::HashSet;

pub async fn replay(
    h: &Harness,
    source: &str,
    marker: &str,
    since: &str,
    sent: &types::Message,
) -> Result<()> {
    let key = secret("HOOKDECK_API_KEY")?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    let mut inspected = HashSet::new();
    loop {
        let mut next = None::<String>;
        loop {
            let mut request = h
                .http
                .get("https://api.hookdeck.com/2025-07-01/events")
                .bearer_auth(key.expose_secret())
                .query(&[
                    ("source_id", source),
                    ("created_at[gte]", since),
                    ("limit", "100"),
                ]);
            if let Some(cursor) = &next {
                request = request.query(&[("next", cursor)]);
            }
            let list = json_response(request).await?;
            for summary in list["models"].as_array().ok_or("Hookdeck omitted events")? {
                let id = text(summary, "id")?;
                if !inspected.insert(id.to_owned()) {
                    continue;
                }
                let event = json_response(
                    h.http
                        .get(format!("https://api.hookdeck.com/2025-07-01/events/{id}"))
                        .bearer_auth(key.expose_secret()),
                )
                .await?;
                if event["source_id"] != source
                    || !event["data"]["body"].to_string().contains(marker)
                {
                    continue;
                }
                let raw = json_response(
                    h.http
                        .get(format!(
                            "https://api.hookdeck.com/2025-07-01/events/{id}/raw_body"
                        ))
                        .bearer_auth(key.expose_secret()),
                )
                .await?;
                // The endpoint wraps the exact original bytes in a JSON string.
                let body = zeroize::Zeroizing::new(text(&raw, "body")?.as_bytes().to_vec());
                ensure(
                    String::from_utf8_lossy(&body).contains(marker),
                    "Hookdeck raw event omitted this run's marker",
                )?;
                let mut headers = http::HeaderMap::new();
                for (name, value) in event["data"]["headers"]
                    .as_object()
                    .ok_or("Hookdeck omitted headers")?
                {
                    let lower = name.to_ascii_lowercase();
                    // Only provider authentication/event headers are needed. Never forward transport auth.
                    if !(lower.starts_with("x-slack-")
                        || lower.starts_with("x-github-")
                        || lower.starts_with("x-hub-")
                        || lower.starts_with("svix-")
                        || lower.starts_with("webhook-")
                        || lower.starts_with("telnyx-"))
                    {
                        continue;
                    }
                    let value = value
                        .as_str()
                        .or_else(|| value.as_array()?.first()?.as_str())
                        .ok_or("Invalid captured header")?;
                    headers.insert(
                        http::header::HeaderName::from_bytes(lower.as_bytes())?,
                        value.parse()?,
                    );
                }
                // Replay twice: real body + original signature, then prove durable deduplication.
                for _ in 0..2 {
                    checked(
                        h.http
                            .post(format!(
                                "{}/connections/webhooks/{}",
                                h.origin, h.connection
                            ))
                            .headers(headers.clone())
                            .body(body.to_vec()),
                    )
                    .await?;
                }
                let messages = h
                    .chat
                    .messages(Uuid::parse_str(&sent.thread_id)?, 100)
                    .await?;
                if messages
                    .iter()
                    .any(|m| m.id != sent.id && m.text.contains(marker))
                {
                    return Ok(());
                }
                let payload: Value = serde_json::from_slice(&body)?;
                if headers.contains_key("x-slack-signature")
                    && payload["event"].get("bot_id").is_some()
                    && payload["event"]["ts"] != sent.delivery.external_message_id
                {
                    return Err("Slack peer is bot-authored and ignored by the adapter; use a human-authored peer token or CHAT_LIVE_SLACK_MANUAL_REPLY=true".into());
                }
                // Provider delivery/status events can also contain the marker; only a
                // real inbound message persisted by the adapter completes this test.
            }
            next = list
                .pointer("/pagination/next")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if next.is_none() {
                break;
            }
            ensure(
                tokio::time::Instant::now() < deadline,
                "Hookdeck pagination exceeded callback deadline",
            )?;
        }
        ensure(
            tokio::time::Instant::now() < deadline,
            "No real inbound callback arrived within 180s; check source subscription and peer reply",
        )?;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
