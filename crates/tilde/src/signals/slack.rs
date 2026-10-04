//! App mentions and channel messages delivered by the connection's Slack app.
use super::*;

pub struct Slack;

impl Source for Slack {
    fn types(&self) -> Vec<SignalType> {
        vec![
            SignalType {
                id: "slack.app_mention".into(),
                name: "App mention".into(),
                description: "The Slack app was mentioned in a conversation.".into(),
                default_thread_title: "Slack mention in {{ event.channel }}".into(),
            },
            SignalType {
                id: "slack.message.posted".into(),
                name: "Message posted".into(),
                description: "A message was posted in a conversation the app is in.".into(),
                default_thread_title: "Slack message in {{ event.channel }}".into(),
            },
        ]
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "event.channel",
                description: "Slack channel ID of the event.",
                example: "C0123456789",
            },
            Variable {
                key: "event.user",
                description: "Slack user ID of the author.",
                example: "U0123456789",
            },
            Variable {
                key: "event.text",
                description: "Message text.",
                example: "@tilde summarise this thread",
            },
            Variable {
                key: "event.thread_ts",
                description: "Thread timestamp; absent on root messages.",
                example: "1710000000.000100",
            },
            Variable {
                key: "event.ts",
                description: "Message timestamp.",
                example: "1710000000.000200",
            },
        ]
    }
    /// Bot messages, the app's own replies among them, emit nothing.
    fn signals(&self, _: &Access, _: &HeaderMap, p: &Value) -> Vec<Signal> {
        let Some(event) = p.get("event") else {
            return vec![];
        };
        if event.get("bot_id").is_some() || event["subtype"] == "bot_message" {
            return vec![];
        }
        let channel = event["channel"].as_str().unwrap_or("unknown-channel");
        let (signal_type, summary) = match event["type"].as_str() {
            Some("app_mention") => (
                "slack.app_mention",
                format!("Slack app mention in {channel}"),
            ),
            Some("message") => (
                "slack.message.posted",
                format!("Slack message posted in {channel}"),
            ),
            _ => return vec![],
        };
        let event_id = str_at(p, "/event_id")
            .map(str::to_owned)
            .unwrap_or_else(|| hashed("slack", p));
        vec![Signal {
            event_id,
            signal_type: signal_type.into(),
            summary,
            data: p.clone(),
        }]
    }
}
