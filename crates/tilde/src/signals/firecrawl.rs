//! Firecrawl monitor results: one signal per checked page and one per completed check.
use super::*;
use serde_json::json;

pub struct Firecrawl;

const STATUSES: &[(&str, &str, &str)] = &[
    (
        "same",
        "Monitor page unchanged",
        "Firecrawl unchanged: {{ page.url }}",
    ),
    (
        "new",
        "Monitor page new",
        "Firecrawl new page: {{ page.url }}",
    ),
    (
        "changed",
        "Monitor page changed",
        "Firecrawl changed: {{ page.url }}",
    ),
    (
        "removed",
        "Monitor page removed",
        "Firecrawl removed: {{ page.url }}",
    ),
    (
        "error",
        "Monitor page error",
        "Firecrawl error: {{ page.url }}",
    ),
];

impl Source for Firecrawl {
    fn types(&self) -> Vec<SignalType> {
        let mut types: Vec<_> = STATUSES
            .iter()
            .map(|(status, name, title)| SignalType {
                id: format!("firecrawl.monitor.page.{status}"),
                name: (*name).into(),
                description: format!("A Firecrawl monitor checked a page: {status}."),
                title: (*title).into(),
            })
            .collect();
        types.push(SignalType {
            id: "firecrawl.monitor.check.completed".into(),
            name: "Monitor check completed".into(),
            description: "A Firecrawl monitor check completed.".into(),
            title: "Firecrawl check completed: {{ monitor.id }}".into(),
        });
        types
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "monitor.id",
                description: "Firecrawl monitor ID.",
                example: "mon_123",
            },
            Variable {
                key: "check.id",
                description: "Firecrawl monitor check ID.",
                example: "chk_456",
            },
            Variable {
                key: "page.url",
                description: "Page URL on page events.",
                example: "https://example.com/pricing",
            },
            Variable {
                key: "page.status",
                description: "Page status: same, new, changed, removed or error.",
                example: "changed",
            },
            Variable {
                key: "page.isMeaningful",
                description: "Whether Firecrawl judged the change meaningful.",
                example: "true",
            },
        ]
    }
    fn signals(&self, _: &Access, _: &HeaderMap, p: &Value) -> Vec<Signal> {
        let metadata = p.get("metadata").cloned().unwrap_or(Value::Null);
        match p["type"].as_str() {
            Some("monitor.page") => p["data"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(index, page)| {
                    // Firecrawl reports one of these; anything else reads as a change.
                    let status = page["status"]
                        .as_str()
                        .filter(|s| ["same", "new", "changed", "removed", "error"].contains(s))
                        .unwrap_or("changed");
                    let url = page["url"].as_str().unwrap_or("unknown page");
                    let meaningful = if page["isMeaningful"] == true {
                        " meaningful"
                    } else {
                        ""
                    };
                    let event_id = match (page["id"].as_str(), p["id"].as_str()) {
                        (Some(id), _) => format!("firecrawl-page-{id}"),
                        (None, Some(id)) => format!("{id}:{index}"),
                        _ => hashed("firecrawl-page", page),
                    };
                    Signal {
                        event_id,
                        signal_type: format!("firecrawl.monitor.page.{status}"),
                        summary: format!("Firecrawl monitor{meaningful} page {status}: {url}"),
                        data: json!({
                            "monitor": {"id": page["monitorId"]},
                            "check": {"id": page["checkId"]},
                            "page": page,
                            "metadata": metadata,
                        }),
                    }
                })
                .collect(),
            Some("monitor.check.completed") => {
                let data = &p["data"];
                let monitor = data["monitorId"].as_str().unwrap_or("unknown monitor");
                let check = data["checkId"]
                    .as_str()
                    .or_else(|| data["id"].as_str())
                    .unwrap_or("unknown check");
                let status = data["status"].as_str().unwrap_or("completed");
                let counts = ["same", "new", "changed", "removed", "error"]
                    .into_iter()
                    .filter_map(|field| data[field].as_u64().map(|n| format!("{field}: {n}")))
                    .collect::<Vec<_>>();
                let counts = if counts.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", counts.join(", "))
                };
                let event_id = p["id"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("firecrawl-check-{monitor}-{check}"));
                vec![Signal {
                    event_id,
                    signal_type: "firecrawl.monitor.check.completed".into(),
                    summary: format!("Firecrawl monitor check {status}: {monitor}/{check}{counts}"),
                    data: json!({
                        "monitor": {"id": monitor},
                        "check": {"id": check},
                        "result": data,
                        "metadata": metadata,
                    }),
                }]
            }
            _ => vec![],
        }
    }
}
