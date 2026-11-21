//! Webhooks of a Sentry internal integration: issue lifecycle (with regressions told apart from
//! other unresolves), issue alert rules firing, and metric alerts changing severity.
use super::*;

pub struct Sentry;

const ISSUE_TITLE: &str = "{{ data.issue.shortId }} {{ data.issue.title }}";
/// `(resource, action, signal type, name, default thread title)`.
const TYPES: &[(&str, &str, &str, &str, &str)] = &[
    (
        "issue",
        "created",
        "issue.created",
        "Issue created",
        ISSUE_TITLE,
    ),
    (
        "issue",
        "assigned",
        "issue.assigned",
        "Issue assigned",
        ISSUE_TITLE,
    ),
    (
        "issue",
        "resolved",
        "issue.resolved",
        "Issue resolved",
        ISSUE_TITLE,
    ),
    (
        "issue",
        "unresolved",
        "issue.unresolved",
        "Issue unresolved",
        ISSUE_TITLE,
    ),
    // Sentry's archive action, still sent as `ignored` by older integrations.
    (
        "issue",
        "archived",
        "issue.archived",
        "Issue archived",
        ISSUE_TITLE,
    ),
    (
        "issue",
        "ignored",
        "issue.archived",
        "Issue archived",
        ISSUE_TITLE,
    ),
    (
        "event_alert",
        "triggered",
        "alert.triggered",
        "Issue alert triggered",
        "{{ data.triggered_rule }}: {{ data.event.title }}",
    ),
    (
        "metric_alert",
        "critical",
        "metric_alert.critical",
        "Metric alert critical",
        "{{ data.description_title }}",
    ),
    (
        "metric_alert",
        "warning",
        "metric_alert.warning",
        "Metric alert warning",
        "{{ data.description_title }}",
    ),
    (
        "metric_alert",
        "resolved",
        "metric_alert.resolved",
        "Metric alert resolved",
        "{{ data.description_title }}",
    ),
];
const REGRESSED: &str = "sentry.issue.regressed";

impl Source for Sentry {
    fn types(&self) -> Vec<SignalType> {
        let mut types: Vec<SignalType> = TYPES
            .iter()
            .filter(|(_, action, ..)| *action != "ignored")
            .map(|(resource, _, id, name, title)| SignalType {
                id: format!("sentry.{id}"),
                name: (*name).into(),
                description: match *resource {
                    "issue" => format!("{name} in Sentry."),
                    "event_alert" => "An issue alert rule fired for an event.".into(),
                    _ => format!("A metric alert changed: {name}."),
                },
                default_thread_title: (*title).into(),
            })
            .collect();
        types.push(SignalType {
            id: REGRESSED.into(),
            name: "Issue regressed".into(),
            description: "A resolved Sentry issue came back. It is also an unresolve.".into(),
            default_thread_title: ISSUE_TITLE.into(),
        });
        types
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "data.issue.id",
                description: "Stable numeric issue ID, for Sentry tools.",
                example: "123456",
            },
            Variable {
                key: "data.issue.shortId",
                description: "Issue short ID.",
                example: "API-123",
            },
            Variable {
                key: "data.issue.title",
                description: "Issue title.",
                example: "request failed",
            },
            Variable {
                key: "data.issue.culprit",
                description: "Where the error happened.",
                example: "app.handlers.checkout in post",
            },
            Variable {
                key: "data.issue.permalink",
                description: "Link to the issue in Sentry.",
                example: "https://acme.sentry.io/issues/123456/",
            },
            Variable {
                key: "data.issue.level",
                description: "Issue level.",
                example: "error",
            },
            Variable {
                key: "data.issue.substatus",
                description: "Issue substatus, such as new, regressed or escalating.",
                example: "regressed",
            },
            Variable {
                key: "data.issue.project.slug",
                description: "Project slug on issue events.",
                example: "api",
            },
            Variable {
                key: "data.event.title",
                description: "Event title on issue alerts.",
                example: "ZeroDivisionError: division by zero",
            },
            Variable {
                key: "data.event.issue_id",
                description: "Issue ID of the event on issue alerts.",
                example: "123456",
            },
            Variable {
                key: "data.event.web_url",
                description: "Link to the event on issue alerts.",
                example: "https://acme.sentry.io/issues/123456/events/abc/",
            },
            Variable {
                key: "data.triggered_rule",
                description: "Name of the issue alert rule that fired.",
                example: "New errors in production",
            },
            Variable {
                key: "data.description_title",
                description: "Metric alert title.",
                example: "Critical: p95 latency above 2s",
            },
            Variable {
                key: "data.web_url",
                description: "Link to the metric alert.",
                example: "https://acme.sentry.io/alerts/rules/details/7/",
            },
        ]
    }
    fn signals(&self, _: &Access, h: &HeaderMap, p: &Value) -> Vec<Signal> {
        let action = str_at(p, "/action").unwrap_or_default();
        let resource = h
            .get("sentry-hook-resource")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("issue");
        let Some((.., signal_type, _, _)) = TYPES
            .iter()
            .find(|(r, a, ..)| *r == resource && *a == action)
        else {
            return vec![];
        };
        let summary = match resource {
            "issue" => {
                let Some(title) = str_at(p, "/data/issue/title") else {
                    return vec![];
                };
                let short = str_at(p, "/data/issue/shortId")
                    .or_else(|| str_at(p, "/data/issue/id"))
                    .unwrap_or_default();
                format!("{short} - {title}")
            }
            "event_alert" => format!(
                "{}: {}",
                str_at(p, "/data/triggered_rule").unwrap_or("Alert rule"),
                str_at(p, "/data/event/title").unwrap_or("event")
            ),
            _ => str_at(p, "/data/description_title")
                .unwrap_or("Metric alert")
                .to_owned(),
        };
        let event_id = h
            .get("request-id")
            .or_else(|| h.get("sentry-hook-id"))
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
            .unwrap_or_else(|| hashed("sentry", p));
        let signal = |signal_type: String, label: &str| Signal {
            event_id: event_id.clone(),
            summary: format!("Sentry {label}: {summary}"),
            signal_type,
            data: p.clone(),
        };
        let mut out = vec![signal(
            format!("sentry.{signal_type}"),
            &readable(signal_type),
        )];
        if *signal_type == "issue.unresolved"
            && str_at(p, "/data/issue/substatus") == Some("regressed")
        {
            out.push(signal(REGRESSED.into(), "issue regressed"));
        }
        out
    }
}
