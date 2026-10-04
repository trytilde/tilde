//! Issue, pull request, comment, review and CI check events of a GitHub App installation.
use super::*;

pub struct Github;

const ISSUE_TITLE: &str = "{{ repository.full_name }}#{{ issue.number }} {{ issue.title }}";
const PR_TITLE: &str =
    "{{ repository.full_name }}#{{ pull_request.number }} {{ pull_request.title }}";
const CHECK_TITLE: &str = "{{ repository.full_name }} CI {{ check_run.name }}";
/// Limited to the events the app manifest subscribes to (`catalog/github/manifest.rs`).
const TYPES: &[(&str, &str, &str)] = &[
    ("issue.opened", "Issue opened", ISSUE_TITLE),
    ("issue.reopened", "Issue reopened", ISSUE_TITLE),
    ("issue.closed", "Issue closed", ISSUE_TITLE),
    ("issue.edited", "Issue edited", ISSUE_TITLE),
    ("issue.labeled", "Issue labeled", ISSUE_TITLE),
    ("issue.assigned", "Issue assigned", ISSUE_TITLE),
    ("pull_request.opened", "Pull request opened", PR_TITLE),
    ("pull_request.reopened", "Pull request reopened", PR_TITLE),
    (
        "pull_request.closed",
        "Pull request closed without merging",
        PR_TITLE,
    ),
    ("pull_request.merged", "Pull request merged", PR_TITLE),
    ("pull_request.edited", "Pull request edited", PR_TITLE),
    (
        "pull_request.synchronized",
        "Pull request synchronized",
        PR_TITLE,
    ),
    (
        "pull_request.ready_for_review",
        "Pull request ready for review",
        PR_TITLE,
    ),
    (
        "pull_request.converted_to_draft",
        "Pull request converted to draft",
        PR_TITLE,
    ),
    (
        "pull_request.review_requested",
        "Pull request review requested",
        PR_TITLE,
    ),
    (
        "issue_comment.created",
        "Issue or pull request comment",
        ISSUE_TITLE,
    ),
    (
        "pull_request_review.submitted",
        "Pull request review submitted",
        PR_TITLE,
    ),
    ("ci_check.passed", "CI check passed", CHECK_TITLE),
    ("ci_check.failed", "CI check failed", CHECK_TITLE),
];

impl Source for Github {
    fn types(&self) -> Vec<SignalType> {
        TYPES
            .iter()
            .map(|(id, name, title)| SignalType {
                id: format!("github.{id}"),
                name: (*name).into(),
                description: format!("{name} in a repository the GitHub App is installed on."),
                title: (*title).into(),
            })
            .collect()
    }
    fn variables(&self) -> &'static [Variable] {
        &[
            Variable {
                key: "repository.full_name",
                description: "Repository owner and name.",
                example: "acme/app",
            },
            Variable {
                key: "sender.login",
                description: "GitHub user who triggered the event.",
                example: "octocat",
            },
            Variable {
                key: "issue.number",
                description: "Issue number on issue and comment events.",
                example: "1842",
            },
            Variable {
                key: "issue.title",
                description: "Issue title on issue and comment events.",
                example: "Fix OAuth redirect",
            },
            Variable {
                key: "issue.html_url",
                description: "Issue URL on issue and comment events.",
                example: "https://github.com/acme/app/issues/1842",
            },
            Variable {
                key: "pull_request.number",
                description: "Pull request number on pull request and review events.",
                example: "317",
            },
            Variable {
                key: "pull_request.title",
                description: "Pull request title on pull request and review events.",
                example: "Add signals webhook routing",
            },
            Variable {
                key: "pull_request.html_url",
                description: "Pull request URL on pull request and review events.",
                example: "https://github.com/acme/app/pull/317",
            },
            Variable {
                key: "comment.body",
                description: "Comment text on comment events.",
                example: "Can you take a look?",
            },
            Variable {
                key: "review.state",
                description: "Review state on review events.",
                example: "approved",
            },
            Variable {
                key: "check_run.name",
                description: "Check name on CI check events.",
                example: "cargo nextest",
            },
            Variable {
                key: "check_run.conclusion",
                description: "Check conclusion on CI check events.",
                example: "failure",
            },
        ]
    }
    /// Bot senders, the installation's own app among them, emit nothing, so a routine that
    /// comments cannot trigger itself.
    fn signals(&self, _: &Access, h: &HeaderMap, p: &Value) -> Vec<Signal> {
        let event = h.get("x-github-event").and_then(|v| v.to_str().ok());
        let delivery = h.get("x-github-delivery").and_then(|v| v.to_str().ok());
        let (Some(event), Some(delivery)) = (event, delivery) else {
            return vec![];
        };
        if str_at(p, "/sender/type") == Some("Bot") {
            return vec![];
        }
        let action = str_at(p, "/action").unwrap_or_default();
        let repo = str_at(p, "/repository/full_name").unwrap_or("unknown/repo");
        let item = |key: &str| {
            let number = p.pointer(&format!("/{key}/number")).cloned();
            let title = str_at(p, &format!("/{key}/title")).unwrap_or("Untitled");
            format!("{repo}#{} - {title}", number.unwrap_or_default())
        };
        let (signal_type, summary) = match (event, action) {
            ("issues", action) => (
                format!("issue.{action}"),
                format!("GitHub issue {action}: {}", item("issue")),
            ),
            ("pull_request", action) => {
                let action = match action {
                    "closed" if p.pointer("/pull_request/merged") == Some(&Value::Bool(true)) => {
                        "merged"
                    }
                    "synchronize" => "synchronized",
                    other => other,
                };
                (
                    format!("pull_request.{action}"),
                    format!("GitHub pull request {action}: {}", item("pull_request")),
                )
            }
            ("issue_comment", "created") => (
                "issue_comment.created".into(),
                format!("GitHub comment on {}", item("issue")),
            ),
            ("pull_request_review", "submitted") => (
                "pull_request_review.submitted".into(),
                format!("GitHub review submitted on {}", item("pull_request")),
            ),
            ("check_run" | "check_suite", _) => {
                let Some(check) = p.get("check_run").or_else(|| p.get("check_suite")) else {
                    return vec![];
                };
                if check["status"] != "completed" {
                    return vec![];
                }
                let outcome = match check["conclusion"].as_str().unwrap_or_default() {
                    "success" => "passed",
                    "failure" | "timed_out" | "action_required" | "startup_failure" | "stale"
                    | "cancelled" => "failed",
                    _ => return vec![],
                };
                let name = check["name"]
                    .as_str()
                    .or_else(|| check.pointer("/app/name").and_then(Value::as_str))
                    .unwrap_or("CI check");
                (
                    format!("ci_check.{outcome}"),
                    format!("GitHub CI check {outcome}: {repo} - {name}"),
                )
            }
            _ => return vec![],
        };
        if !TYPES.iter().any(|(id, ..)| *id == signal_type) {
            return vec![];
        }
        vec![Signal {
            event_id: delivery.into(),
            signal_type: format!("github.{signal_type}"),
            summary,
            data: p.clone(),
        }]
    }
}
