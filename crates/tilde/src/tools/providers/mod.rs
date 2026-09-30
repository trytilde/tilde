//! Managed tool providers: Tilde-shipped tools backed by a connection's credentials. A provider
//! owns its definitions, schemas and upstream calls; the catalog owns naming, assignment,
//! input validation, credentials and audit.
use crate::chat::{providers::Access, tools::ToolResult};
use crate::proto::tilde::types::v1 as types;
use futures::future::BoxFuture;
use serde_json::Value;

pub mod agentmail;
pub mod aws;
pub mod e2b;
pub mod firecrawl;
pub mod github;
pub mod google;
pub mod linq;
pub mod modal;
pub mod payload;
pub mod posthog;
pub mod rest;
pub mod sandbox_patch;
pub mod sentry;
pub mod slack;
pub mod stripe;
pub mod tavily;
pub mod whatsapp;

pub trait ToolProvider: Send + Sync {
    /// Names are provider-local; an agent using the connection prefixes them with its source slug.
    fn tools(&self) -> Vec<types::ToolDefinition>;
    /// `input` already matches the tool's schema. Use `call_id` for upstream idempotency.
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>>;
}
pub(crate) fn provider(provider: &str, typ: &str) -> Option<&'static dyn ToolProvider> {
    match (provider, typ) {
        ("tavily", "api") => Some(&tavily::Tavily),
        ("google_mail", "oauth") => Some(&google::mail::GoogleMail),
        ("google_calendar", "oauth") => Some(&google::calendar::GoogleCalendar),
        ("google_drive", "oauth") => Some(&google::drive::GoogleDrive),
        ("google_docs", "oauth") => Some(&google::docs::GoogleDocs),
        ("google_sheets", "oauth") => Some(&google::sheets::GoogleSheets),
        ("google_search_console", "oauth") => Some(&google::search_console::GoogleSearchConsole),
        ("google_analytics", "oauth") => Some(&google::analytics::GoogleAnalytics),
        ("sentry", "token") => Some(&sentry::TOKEN),
        ("sentry", "oauth") => Some(&sentry::OAUTH),
        ("github", "github_app" | "oauth") => Some(&github::ACCESS_TOKEN),
        ("github", "pat") => Some(&github::PERSONAL_TOKEN),
        ("slack", "slack_app") => Some(&slack::Slack),
        ("posthog", "api") => Some(&posthog::Posthog),
        ("firecrawl", "api") => Some(&firecrawl::Firecrawl),
        ("stripe", "api") => Some(&stripe::Stripe),
        ("payload", "api") => Some(&payload::Payload),
        ("linq", "account") => Some(&linq::Linq),
        ("agentmail", "inbox") => Some(&agentmail::Agentmail),
        ("whatsapp", "meta") => Some(&whatsapp::Whatsapp { telnyx: false }),
        ("telnyx", "whatsapp") => Some(&whatsapp::Whatsapp { telnyx: true }),
        ("e2b", "api") => Some(&e2b::E2b),
        ("aws", "iam") => Some(&aws::Aws),
        ("modal", "api") => Some(&modal::Modal),
        _ => None,
    }
}
