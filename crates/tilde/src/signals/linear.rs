//! Issue and comment webhooks of a Linear workspace, normalized so templates and filters name
//! stable fields: `issue.identifier`, `issue.state.name`, `issue.team.key`, `issue.labels` (names).
use super::*;
use serde_json::json;

pub struct Linear;

const ISSUE_TITLE: &str = "{{ issue.identifier }} {{ issue.title }}";
const TYPES: &[(&str, &str, &str)] = &[
    (
        "issue.created",
        "Issue created",
        "A Linear issue was created.",
    ),
    (
        "issue.updated",
        "Issue updated",
        "A Linear issue changed; `changed` lists the fields.",
    ),
    (
        "issue.removed",
        "Issue removed",
        "A Linear issue was deleted or archived.",
    ),
    (
        "comment.created",
        "Comment created",
        "A comment was added to a Linear issue.",
    ),
    (
        "comment.updated",
        "Comment updated",
        "A comment on a Linear issue was edited.",
    ),
    (
        "comment.removed",
        "Comment removed",
        "A comment on a Linear issue was deleted.",
    ),
];

impl Source for Linear {
    fn types(&self) -> Vec<SignalType> {
        TYPES
            .iter()
            .map(|(id, name, description)| SignalType {
                id: format!("linear.{id}"),
                name: (*name).into(),
                description: (*description).into(),
                default_thread_title: ISSUE_TITLE.into(),
            })
            .collect()
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "issue.identifier",
                description: "Issue identifier.",
                example: "ENG-123",
            },
            Variable {
                key: "issue.id",
                description: "Issue ID, for Linear tools.",
                example: "2f1b0c8e-1b8f-4f0a-9a57-3c1d1c8b2a11",
            },
            Variable {
                key: "issue.title",
                description: "Issue title.",
                example: "Checkout fails on Safari",
            },
            Variable {
                key: "issue.description",
                description: "Issue description (markdown).",
                example: "Steps to reproduce...",
            },
            Variable {
                key: "issue.url",
                description: "Link to the issue.",
                example: "https://linear.app/acme/issue/ENG-123",
            },
            Variable {
                key: "issue.state.name",
                description: "Workflow state.",
                example: "Todo",
            },
            Variable {
                key: "issue.state.type",
                description: "Workflow state category: triage, backlog, unstarted, started, completed or canceled.",
                example: "unstarted",
            },
            Variable {
                key: "issue.team.key",
                description: "Team key.",
                example: "ENG",
            },
            Variable {
                key: "issue.labels",
                description: "Label names; a filter matches any of them.",
                example: "[\"Bug\"]",
            },
            Variable {
                key: "issue.priority",
                description: "Priority: 0 none, 1 urgent, 2 high, 3 medium, 4 low.",
                example: "1",
            },
            Variable {
                key: "issue.assignee.email",
                description: "Assignee's email, when assigned.",
                example: "ada@acme.com",
            },
            Variable {
                key: "comment.body",
                description: "Comment text on comment events.",
                example: "@agent can you take this?",
            },
            Variable {
                key: "comment.user.name",
                description: "Comment author on comment events.",
                example: "Ada Lovelace",
            },
            Variable {
                key: "changed",
                description: "Fields that changed on issue updates (Linear's updatedFrom keys).",
                example: "[\"stateId\"]",
            },
            Variable {
                key: "actor.name",
                description: "Who made the change.",
                example: "Ada Lovelace",
            },
            Variable {
                key: "url",
                description: "Link to the changed issue or comment.",
                example: "https://linear.app/acme/issue/ENG-123#comment-1",
            },
        ]
    }
    /// Changes made by apps and integrations (an agent's own OAuth app among them) emit nothing,
    /// so a routine that comments cannot trigger itself.
    fn signals(&self, _: &Access, h: &HeaderMap, p: &Value) -> Vec<Signal> {
        let action = match str_at(p, "/action") {
            Some("create") => "created",
            Some("update") => "updated",
            Some("remove") => "removed",
            _ => return vec![],
        };
        let object = match str_at(p, "/type") {
            Some("Issue") => "issue",
            Some("Comment") => "comment",
            _ => return vec![],
        };
        if str_at(p, "/actor/type").is_some_and(|t| t != "user")
            || p.pointer("/data/botActor").is_some_and(|b| !b.is_null())
        {
            return vec![];
        }
        let data = &p["data"];
        let (issue, comment) = if object == "issue" {
            (issue(data), Value::Null)
        } else {
            (
                issue(&data["issue"]),
                json!({
                    "id": data["id"],
                    "body": data["body"],
                    "url": data["url"],
                    "user": person(&data["user"]),
                }),
            )
        };
        let changed = p["updatedFrom"]
            .as_object()
            .map(|from| from.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let identifier = issue["identifier"]
            .as_str()
            .or_else(|| issue["id"].as_str())
            .unwrap_or("issue");
        let title = issue["title"].as_str().unwrap_or_default();
        let summary = match object {
            "issue" => format!("Linear issue {action}: {identifier} {title}"),
            _ => format!("Linear comment {action} on {identifier} {title}"),
        };
        let event_id = h
            .get("linear-delivery")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
            .unwrap_or_else(|| hashed("linear", p));
        vec![Signal {
            event_id,
            signal_type: format!("linear.{object}.{action}"),
            summary: summary.trim_end().into(),
            data: json!({
                "action": action,
                "url": p["url"],
                "organization_id": p["organizationId"],
                "actor": person(&p["actor"]),
                "issue": issue,
                "comment": comment,
                "changed": changed,
            }),
        }]
    }
}

fn person(v: &Value) -> Value {
    if !v.is_object() {
        return Value::Null;
    }
    json!({"id": v["id"], "name": v["name"], "email": v["email"]})
}
/// The issue fields Linear sends, with labels reduced to their names.
fn issue(v: &Value) -> Value {
    if !v.is_object() {
        return Value::Null;
    }
    let labels = v["labels"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| l["name"].as_str())
        .collect::<Vec<_>>();
    json!({
        "id": v["id"],
        "identifier": v["identifier"],
        "title": v["title"],
        "description": v["description"],
        "url": v["url"],
        "priority": v["priority"],
        "priority_label": v["priorityLabel"],
        "state": {"name": v["state"]["name"], "type": v["state"]["type"]},
        "team": {"key": v["team"]["key"], "name": v["team"]["name"]},
        "labels": labels,
        "assignee": person(&v["assignee"]),
    })
}
