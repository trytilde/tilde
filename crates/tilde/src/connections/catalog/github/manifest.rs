use super::{Endpoints, url};
use crate::connections::model::*;
use crate::error::Error;
use serde_json::json;
pub(super) fn manifest(
    endpoints: &Endpoints,
    values: &Values,
    homepage: &str,
    callback: &str,
    state: &str,
    connection_id: uuid::Uuid,
) -> Result<Action, Error> {
    let kind = value(values, "owner_type")?;
    let segments = if kind == "organization" {
        let account = value(values, "account")?;
        if !account
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(invalid("Invalid GitHub organization name"));
        }
        vec!["organizations", account, "settings", "apps", "new"]
    } else if kind == "user" {
        vec!["settings", "apps", "new"]
    } else {
        return Err(invalid("Invalid GitHub app owner"));
    };
    let mut action = url(endpoints.get("github_web", "https://github.com"), &segments)?;
    action.query_pairs_mut().append_pair("state", state);
    // The app has channel-capable permissions. The channel binding must configure/activate
    // webhook delivery; no unbound messages should be acknowledged or silently discarded here.
    let manifest = json!({"name":value(values,"app_name")?,"url":homepage,"redirect_url":callback,"setup_url":callback,"callback_urls":[callback],"public":false,"default_permissions":{"metadata":"read","issues":"write","pull_requests":"write","contents":"read"},"hook_attributes":{"url":format!("{}/connections/webhooks/{connection_id}",homepage.trim_end_matches('/')),"active":true},"default_events":["issue_comment","pull_request_review_comment","pull_request_review","pull_request","issues"]});
    Ok(Action::FormPost {
        url: action.to_string(),
        fields: vec![("manifest".into(), manifest.to_string())],
    })
}
