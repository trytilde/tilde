//! WhatsApp messages, delivery receipts, account and template changes. Meta batches changes
//! per delivery; each message and status becomes its own signal keyed by its stable `wamid`.
//! Telnyx connections deliver one message event at a time in their own envelope.
use super::*;
use serde_json::json;

pub struct Whatsapp;

const RECEIVED: &str = "message.received";
const SENT: &str = "message.status.sent";
const DELIVERED: &str = "message.status.delivered";
const READ: &str = "message.status.read";
const FAILED: &str = "message.status.failed";
const ACCOUNT: &str = "account.update";
const TEMPLATE: &str = "template.status_update";
const EVENTS: &[(&str, &str)] = &[
    (
        RECEIVED,
        "A WhatsApp user sent a message to the business number.",
    ),
    (SENT, "An outbound message was sent."),
    (DELIVERED, "An outbound message was delivered."),
    (READ, "An outbound message was read."),
    (
        FAILED,
        "An outbound message failed; `data.errors` carries the error.",
    ),
    (
        ACCOUNT,
        "The business account or its phone number changed state.",
    ),
    (
        TEMPLATE,
        "A message template was approved, rejected, paused or disabled.",
    ),
];

impl Source for Whatsapp {
    fn types(&self) -> Vec<SignalType> {
        EVENTS
            .iter()
            .map(|(event, description)| SignalType {
                id: format!("whatsapp.{event}"),
                name: readable(event),
                description: (*description).into(),
                title: format!("WhatsApp {}", readable(event)),
            })
            .collect()
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "event_type",
                description: "WhatsApp event type.",
                example: RECEIVED,
            },
            Variable {
                key: "metadata.display_phone_number",
                description: "Business phone number that received the event.",
                example: "15550001111",
            },
            Variable {
                key: "data.from",
                description: "Sender (inbound) or recipient (receipts) phone number.",
                example: "16505551234",
            },
            Variable {
                key: "data.id",
                description: "Message ID, stable across retries.",
                example: "wamid.HBgLMTY1MDUwNzY1MjAVAgARGBI5QTNDQTVCM0Q0Q0Q2RTY3RTcA",
            },
            Variable {
                key: "data.text.body",
                description: "Text of an inbound Meta text message.",
                example: "Is my order on its way?",
            },
        ]
    }
    fn signals(&self, _: &HeaderMap, p: &Value) -> Vec<Signal> {
        if let Some(event) = str_at(p, "/data/event_type") {
            return telnyx(p, event).into_iter().collect();
        }
        let mut out = vec![];
        for entry in p["entry"].as_array().into_iter().flatten() {
            let waba = entry["id"].as_str().unwrap_or_default();
            for change in entry["changes"].as_array().into_iter().flatten() {
                let field = change["field"].as_str().unwrap_or_default();
                let value = &change["value"];
                let signal = |event: &str, id: String, data: &Value, contacts: &Value| {
                    let payload = json!({
                        "event_type": event,
                        "waba_id": waba,
                        "field": field,
                        "metadata": value["metadata"],
                        "contacts": contacts,
                        "data": data,
                    });
                    Signal {
                        event_id: id,
                        signal_type: format!("whatsapp.{event}"),
                        summary: summary(&payload),
                        data: payload,
                    }
                };
                match field {
                    "messages" => {
                        for message in value["messages"].as_array().into_iter().flatten() {
                            let id = message["id"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| hashed("whatsapp-message", message));
                            out.push(signal(RECEIVED, id, message, &value["contacts"]));
                        }
                        for status in value["statuses"].as_array().into_iter().flatten() {
                            let event = match status["status"].as_str() {
                                Some("sent") => SENT,
                                Some("delivered") => DELIVERED,
                                Some("read") => READ,
                                Some("failed") => FAILED,
                                _ => continue,
                            };
                            let id = status["id"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| hashed("whatsapp-status", status));
                            // One message moves through each status under the same wamid.
                            out.push(signal(event, format!("{id}:{event}"), status, &Value::Null));
                        }
                    }
                    "message_template_status_update" => out.push(signal(
                        TEMPLATE,
                        hashed("whatsapp-template", change),
                        value,
                        &Value::Null,
                    )),
                    _ => out.push(signal(
                        ACCOUNT,
                        hashed("whatsapp-account", change),
                        value,
                        &Value::Null,
                    )),
                }
            }
        }
        out
    }
}

fn telnyx(p: &Value, event: &str) -> Option<Signal> {
    let payload = &p["data"]["payload"];
    let outbound = payload["direction"] == "outbound";
    let signal_event = match event {
        "message.received" => RECEIVED,
        "message.sent" if outbound => SENT,
        "message.finalized" | "message.delivered" | "message.read" | "message.failed"
            if outbound =>
        {
            let status =
                str_at(payload, "/to/0/status").unwrap_or(event.trim_start_matches("message."));
            match status {
                "delivered" => DELIVERED,
                "read" => READ,
                "sent" | "sending" | "queued" => SENT,
                _ => FAILED,
            }
        }
        _ => return None,
    };
    let id = payload["id"]
        .as_str()
        .or_else(|| str_at(p, "/data/id"))
        .unwrap_or_default();
    let to = payload
        .pointer("/to/0/phone_number")
        .cloned()
        .unwrap_or_default();
    let from = payload
        .pointer("/from/phone_number")
        .cloned()
        .unwrap_or_default();
    let (business, counterpart) = if signal_event == RECEIVED {
        (to, from)
    } else {
        (from, to)
    };
    let mut data = payload.clone();
    if let Some(object) = data.as_object_mut() {
        object.insert("from".into(), counterpart.clone());
        if signal_event != RECEIVED {
            object.insert(
                "status".into(),
                signal_event.trim_start_matches("message.status.").into(),
            );
            object.insert("recipient_id".into(), counterpart);
        }
    }
    let payload = json!({
        "event_type": signal_event,
        "waba_id": "",
        "field": "telnyx",
        "metadata": {"display_phone_number": business, "transport": "telnyx"},
        "contacts": [],
        "data": data,
    });
    Some(Signal {
        event_id: format!("{id}:{event}"),
        signal_type: format!("whatsapp.{signal_event}"),
        summary: summary(&payload),
        data: payload,
    })
}

fn summary(p: &Value) -> String {
    let number = str_at(p, "/metadata/display_phone_number")
        .or_else(|| str_at(p, "/metadata/phone_number_id"));
    match p["event_type"].as_str().unwrap_or_default() {
        RECEIVED => {
            let from = str_at(p, "/data/from").unwrap_or("unknown sender");
            let kind = str_at(p, "/data/type").unwrap_or("message");
            match number {
                Some(number) => format!("WhatsApp {kind} from {from} to {number}"),
                None => format!("WhatsApp {kind} from {from}"),
            }
        }
        event if event.starts_with("message.status.") => format!(
            "WhatsApp {event} for {}",
            str_at(p, "/data/recipient_id").unwrap_or("unknown recipient")
        ),
        event => format!(
            "WhatsApp {event} on WABA {}",
            p["waba_id"].as_str().unwrap_or_default()
        ),
    }
}
