//! Google account for ingestion: one read-only OAuth grant covering Gmail, Calendar and Contacts,
//! so a person links Google once and an ontology's `gmail`, `google_calendar` and
//! `google_contacts` ducktail pipelines all name the same connection slot.
//!
//! Why a provider of its own: the Google tool providers (`google_mail`, `google_calendar`, ...)
//! are one grant per product, because each carries that product's write scopes for agent tools.
//! A connection type has one fixed scope set, so the read-only, cross-product grant ingestion
//! wants is its own type; putting it on one product provider would mislabel the other two.
//! It shares Tilde's Google app (CLOUD ONLY managed OAuth: `GOOGLE`) with the tool providers.
//! It declares no capability: it gives request credentials (bearer token, refreshed) to
//! ontology environments and nothing to agents.
use crate::connections::{categories::CATEGORY_EMAIL, model::*};

pub const PROVIDER_ID: &str = "google";
pub const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/gmail.readonly",
    "https://www.googleapis.com/auth/calendar.readonly",
    "https://www.googleapis.com/auth/contacts.readonly",
    "https://www.googleapis.com/auth/contacts.other.readonly",
    "https://www.googleapis.com/auth/userinfo.email",
    "openid",
];

pub fn definition() -> Provider {
    let mut oauth = OAuth::standard("https://oauth2.googleapis.com/token");
    oauth.authorization_url = Some("https://accounts.google.com/o/oauth2/v2/auth".into());
    oauth.scopes = SCOPES.iter().map(|s| (*s).to_owned()).collect();
    oauth.authorization_parameters = [("access_type", "offline"), ("prompt", "consent")]
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .into();
    let own = ConnectionType {
        mcp: None,
        id: "readonly".into(),
        name: "Your Google OAuth client".into(),
        credential_source: CredentialSource::OAuth {
            grant: OAuthGrant::AuthorizationCode,
            configuration: Box::new(oauth),
            additional_schema: None,
        },
        capabilities: vec![],
    };
    Provider {
        api_hosts: vec!["*.googleapis.com".into()],
        account_name_label: Some("Google account".into()),
        icon_url: Some("/provider-icons/google_mail.svg".into()),
        instructions: Some(
            "Link a Google account read-only so an ontology can ingest its email (Gmail), calendars and events (Calendar) and contacts (People API). With your own OAuth client, enable the Gmail, Google Calendar and People APIs in its Google Cloud project."
                .into(),
        ),
        id: PROVIDER_ID.into(),
        name: "Google (mail, calendar, contacts)".into(),
        kind: ProviderKind::BuiltIn,
        categories: vec![CATEGORY_EMAIL.into()],
        connection_types: vec![
            own,
        ],
    }
}
