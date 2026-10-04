//! Message, delivery, security and domain events of an AgentMail inbox.
use super::*;

pub struct Agentmail;

const EVENTS: &[(&str, &str)] = &[
    ("domain.verified", "Domain verified"),
    ("message.bounced", "Message bounced"),
    ("message.complained", "Message complained"),
    ("message.delivered", "Message delivered"),
    ("message.received", "Message received"),
    ("message.received.blocked", "Blocked message received"),
    ("message.received.spam", "Spam message received"),
    (
        "message.received.unauthenticated",
        "Unauthenticated message received",
    ),
    ("message.rejected", "Message rejected"),
    ("message.security.completed", "Security scan completed"),
    ("message.security.override", "Security review overridden"),
    ("message.security.review", "Security review requested"),
    ("message.sent", "Message sent"),
];

impl Source for Agentmail {
    fn types(&self) -> Vec<SignalType> {
        EVENTS
            .iter()
            .map(|(event, name)| SignalType {
                id: format!("agentmail.{event}"),
                name: (*name).into(),
                description: format!("AgentMail `{event}` webhook event."),
                default_thread_title: if *event == "domain.verified" {
                    "AgentMail domain {{ domain.domain }} verified".into()
                } else {
                    "AgentMail: {{ message.subject }}".into()
                },
            })
            .collect()
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "message.inbox_id",
                description: "Inbox that owns the message.",
                example: "inb_01JABCDEF",
            },
            Variable {
                key: "message.thread_id",
                description: "Email thread ID.",
                example: "thr_01JABCDEF",
            },
            Variable {
                key: "message.subject",
                description: "Email subject when present.",
                example: "Re: Project update",
            },
            Variable {
                key: "message.from",
                description: "Sender address.",
                example: "Ada <ada@example.com>",
            },
            Variable {
                key: "message.text",
                description: "Plain-text body when present.",
                example: "Can we move the call to Friday?",
            },
            Variable {
                key: "domain.domain",
                description: "Domain on domain events.",
                example: "mail.example.com",
            },
        ]
    }
    fn signals(&self, a: &Access, h: &HeaderMap, p: &Value) -> Vec<Signal> {
        // Message events name their inbox; another inbox's are not this connection's.
        if let Some(inbox) = str_at(p, "/message/inbox_id")
            && Some(inbox) != a.secret("inbox_id").ok()
        {
            return vec![];
        }
        let Some(event) = str_at(p, "/event_type") else {
            return vec![];
        };
        if !EVENTS.iter().any(|(supported, _)| *supported == event) {
            return vec![];
        }
        let Some(event_id) =
            str_at(p, "/event_id").or_else(|| h.get("svix-id").and_then(|v| v.to_str().ok()))
        else {
            return vec![];
        };
        let summary = if event == "domain.verified" {
            format!(
                "AgentMail domain verified: {}",
                str_at(p, "/domain/domain").unwrap_or("unknown domain")
            )
        } else {
            let subject = str_at(p, "/message/subject")
                .filter(|s| !s.is_empty())
                .unwrap_or("Untitled message");
            format!("AgentMail {event}: {subject}")
        };
        vec![Signal {
            event_id: event_id.into(),
            signal_type: format!("agentmail.{event}"),
            summary,
            data: p.clone(),
        }]
    }
}
