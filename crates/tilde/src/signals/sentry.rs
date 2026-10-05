//! Issue lifecycle webhooks of a Sentry internal integration.
use super::*;

pub struct Sentry;

const ACTIONS: &[(&str, &str)] = &[
    ("created", "Issue created"),
    ("assigned", "Issue assigned"),
    ("resolved", "Issue resolved"),
    ("unresolved", "Issue unresolved"),
    ("ignored", "Issue ignored"),
];

impl Source for Sentry {
    fn types(&self) -> Vec<SignalType> {
        ACTIONS
            .iter()
            .map(|(action, name)| SignalType {
                id: format!("sentry.issue.{action}"),
                name: (*name).into(),
                description: format!("A Sentry issue was {action}."),
                default_thread_title: "{{ data.issue.shortId }} {{ data.issue.title }}".into(),
            })
            .collect()
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "data.issue.id",
                description: "Stable numeric issue ID.",
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
                key: "data.project.slug",
                description: "Project slug when present.",
                example: "api",
            },
        ]
    }
    fn signals(&self, _: &Access, h: &HeaderMap, p: &Value) -> Vec<Signal> {
        let action = str_at(p, "/action").unwrap_or_default();
        let resource = h.get("sentry-hook-resource").and_then(|v| v.to_str().ok());
        if resource.is_some_and(|r| r != "issue")
            || !ACTIONS.iter().any(|(supported, _)| *supported == action)
        {
            return vec![];
        }
        let Some(title) = str_at(p, "/data/issue/title") else {
            return vec![];
        };
        let short = str_at(p, "/data/issue/shortId")
            .or_else(|| str_at(p, "/data/issue/id"))
            .unwrap_or_default();
        let event_id = h
            .get("request-id")
            .or_else(|| h.get("sentry-hook-id"))
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
            .unwrap_or_else(|| hashed("sentry", p));
        vec![Signal {
            event_id,
            signal_type: format!("sentry.issue.{action}"),
            summary: format!("Sentry issue {action}: {short} - {title}"),
            data: p.clone(),
        }]
    }
}
