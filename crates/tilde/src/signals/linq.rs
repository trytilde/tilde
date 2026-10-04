//! Every event in Linq's Partner API V3 webhook catalog.
use super::*;

pub struct Linq;

const EVENTS: &[&str] = &[
    "message.sent",
    "message.received",
    "message.read",
    "message.delivered",
    "message.edited",
    "message.failed",
    "reaction.added",
    "reaction.removed",
    "poll.received",
    "poll.sent",
    "poll.delivered",
    "poll.read",
    "poll.updated",
    "poll.failed",
    "poll.vote.added",
    "poll.vote.removed",
    "poll.reaction.added",
    "participant.added",
    "participant.removed",
    "chat.created",
    "chat.group_name_updated",
    "chat.group_icon_updated",
    "chat.group_name_update_failed",
    "chat.group_icon_update_failed",
    "chat.background_updated",
    "chat.background_update_failed",
    "chat.typing_indicator.started",
    "chat.typing_indicator.stopped",
    "phone_number.status_updated",
    "contact_card.received",
    "payment.succeeded",
    "payment.canceled",
    "payment.expired",
    "payment.declined",
    "payment.authorized",
    "connection.created",
    "connection.revoked",
    "call.initiated",
    "call.ringing",
    "call.answered",
    "call.ended",
    "call.failed",
    "call.declined",
    "call.no_answer",
    "location.sharing.started",
    "location.sharing.stopped",
];

impl Source for Linq {
    fn types(&self) -> Vec<SignalType> {
        EVENTS
            .iter()
            .map(|event| SignalType {
                id: format!("linq.{event}"),
                name: readable(event),
                description: format!("Linq `{event}` webhook event."),
                title: format!("Linq {}", readable(event)),
            })
            .collect()
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "event_type",
                description: "Linq event type.",
                example: "message.received",
            },
            Variable {
                key: "data.chat.id",
                description: "Chat ID when the event belongs to a chat.",
                example: "8f392755-6865-4b18-880a-227f9d8b458f",
            },
            Variable {
                key: "data.chat.owner_handle.handle",
                description: "The Linq line that owns the chat.",
                example: "+12025551234",
            },
            Variable {
                key: "data.sender_handle.handle",
                description: "Sender handle on message events.",
                example: "+12025559876",
            },
        ]
    }
    fn signals(&self, _: &HeaderMap, p: &Value) -> Vec<Signal> {
        let (Some(event), Some(event_id)) = (str_at(p, "/event_type"), str_at(p, "/event_id"))
        else {
            return vec![];
        };
        if !EVENTS.contains(&event) {
            return vec![];
        }
        let chat = str_at(p, "/data/chat/id");
        let handle = str_at(p, "/data/sender_handle/handle")
            .or_else(|| str_at(p, "/data/chat/owner_handle/handle"));
        let summary = match (chat, handle) {
            (Some(chat), Some(handle)) => format!("Linq {event} for {handle} in chat {chat}"),
            (Some(chat), None) => format!("Linq {event} in chat {chat}"),
            _ => format!("Linq {event}"),
        };
        vec![Signal {
            event_id: event_id.into(),
            signal_type: format!("linq.{event}"),
            summary,
            data: p.clone(),
        }]
    }
}
