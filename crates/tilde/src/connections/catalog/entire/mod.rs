//! Entire (entire.com): service account credentials and sandbox egress to its git network.
//!
//! From Entire's public documentation (docs.entire.com, "Service Accounts", "Authentication",
//! "Regions and routing"): a service account signs in with a secret laid out as
//! `ent_svc_<region>_<service account ULID>_<secret ULID>_<random><checksum>`, exchanged at
//! `https://<region>.auth.entire.io/oauth/token` (RFC 8693, client `entire-cli`, the service
//! account `svc_<ULID>` as `resource`) for a bearer token that lives at most ten minutes. The
//! secret itself is never sent as a bearer. Core API calls go to
//! `https://<region>.console.entire.io/api/v1`.
//!
//! Repositories are cloned as `entire://<cluster host>/et/<project>/<repo>` through Entire's
//! `git-remote-entire` helper. The documentation does not describe the helper's wire protocol;
//! its open-source code (github.com/entireio/cli, `cmd/git-remote-entire`) speaks git smart-HTTP
//! to `https://<cluster host>/et/<project>/<repo>/info/refs|git-upload-pack|git-receive-pack`
//! with `Authorization: Bearer`, and follows the entry host's redirects to replica nodes under
//! the same host. Egress does the same from the gateway, so a sandbox needs neither the helper
//! nor a token: its `entire://<cluster host>/` remotes are rewritten to the route.
//!
//! A grant's config narrows it to repositories (`project/repo`; empty: every repository the
//! service account reaches) and the branches pushes may update. Pushes never delete or rewrite
//! refs. The default branch is not refused here: a tenant's own repository may grant it by name.
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::egress::{
    Decision, Egress, EgressHook, EgressProtocol, EgressRequest, GitAuth, GitService,
    forge::{self, strings},
    rules,
};
use crate::connections::mints::Minted;
use crate::error::Error;
use connectrpc::ConnectError;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::sync::LazyLock;
use zeroize::Zeroizing;

/// The region and the service account ID (`svc_<ULID>`) a secret names.
fn account(secret: &str) -> ToolResult<(&str, String)> {
    let ulid = |part: &str| part.len() == 26 && part.bytes().all(|b| b.is_ascii_alphanumeric());
    let mut parts = secret
        .strip_prefix("ent_svc_")
        .unwrap_or_default()
        .split('_');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(region), Some(account), Some(key), Some(_))
            if !region.is_empty()
                && region.bytes().all(|b| b.is_ascii_lowercase())
                && ulid(account)
                && ulid(key) =>
        {
            Ok((region, format!("svc_{account}")))
        }
        _ => Err(ConnectError::failed_precondition(
            "The Entire service account secret is malformed",
        )),
    }
}
/// The Core API base of the secret's region.
pub fn core(access: &Access) -> ToolResult<String> {
    let (region, _) = account(access.secret("secret")?)?;
    Ok(access
        .endpoints
        .get("entire_api", &format!("https://{region}.console.entire.io"))
        .trim_end_matches('/')
        .to_owned())
}

/// Exchanges the connection's secret for a service account token (`connections::mints` caches
/// it until five minutes before it expires).
pub async fn service_token(access: &Access) -> ToolResult<Minted> {
    let secret = access.secret("secret")?;
    let (region, resource) = account(secret)?;
    let auth = format!("https://{region}.auth.entire.io");
    let url = access.url("entire_auth", &auth, &["oauth", "token"])?;
    let form = Zeroizing::new(
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair(
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            )
            .append_pair(
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            )
            .append_pair("subject_token", secret)
            .append_pair("audience", &auth)
            .append_pair("resource", &resource)
            .finish(),
    );
    let response = access
        .http
        .json(
            access
                .http
                .client
                .post(url)
                .basic_auth("entire-cli", Some(""))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(form.as_str().to_owned()),
        )
        .await
        .map_err(|_| {
            ConnectError::failed_precondition(
                "Entire refused the service account secret; it may have expired or been deleted",
            )
        })?;
    let token = response.0["access_token"]
        .as_str()
        .ok_or_else(|| ConnectError::unknown("Entire returned no service account token"))?;
    let lifetime = response.0["expires_in"].as_i64().unwrap_or(0);
    Ok(Minted::bearer(
        token,
        Some(chrono::Utc::now() + chrono::TimeDelta::seconds(lifetime)),
    ))
}

pub struct Entire(Egress);
static HOOK: LazyLock<Entire> = LazyLock::new(|| Entire(definition()));
pub fn hook() -> &'static Entire {
    &HOOK
}

fn definition() -> Egress {
    Egress {
        protocols: vec![EgressProtocol::Git {
            // The connection's cluster host; this one only describes the provider.
            upstream: "https://<cluster>.entire.io",
            auth: GitAuth::Bearer,
            replicas: true,
            services: vec![GitService::UploadPack, GitService::ReceivePack],
            deny_delete: true,
            deny_force: true,
            deny_default_branch: false,
        }],
        config_schema: json!({
            "type": "object",
            "properties": {
                "repositories": {
                    "type": "array",
                    "title": "Repositories",
                    "items": {"type": "string", "pattern": "^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$"},
                    "uniqueItems": true,
                    "x-tilde-options": "entire.list_repositories",
                    "description": "project/repository. Empty: every repository the service account can reach"
                },
                "push": forge::push_schema("Name the default branch (e.g. main) to let pushes update it."),
            },
            "additionalProperties": false
        }),
        config_version: 1,
        setup_queries: vec!["entire.list_repositories"],
    }
}

#[async_trait::async_trait]
impl EgressHook for Entire {
    fn definition(&self) -> &Egress {
        &self.0
    }
    fn check(&self, request: &EgressRequest<'_>, config: &Value) -> Decision {
        let decision = rules::check(request);
        if decision != Decision::Allow {
            return decision;
        }
        let EgressRequest::Git {
            repository,
            service,
            updates,
            ..
        } = request
        else {
            return decision;
        };
        // Repositories hosted on Entire live under `et/`; GitHub mirrors (`gh/`) are not served.
        let name = repository
            .strip_prefix("et/")
            .filter(|name| name.split('/').count() == 2);
        let repositories = strings(config.get("repositories"));
        forge::check_git(
            name.is_some_and(|name| {
                repositories.is_empty() || repositories.iter().any(|r| r.eq_ignore_ascii_case(name))
            }),
            repository,
            &strings(config.pointer("/push/branches")),
            *service,
            updates,
        )
    }
    fn upstream(&self, _protocol: &EgressProtocol, access: &Access) -> Result<String, Error> {
        let cluster = crate::connections::model::optional(&access.values, "cluster")
            .filter(|host| cluster_host(host))
            .ok_or_else(|| Error::Invalid("The connection names no Entire cluster host".into()))?;
        Ok(format!("https://{cluster}"))
    }
    fn git_remotes(&self, upstream: &str) -> Vec<String> {
        let host = upstream.trim_start_matches("https://");
        vec![format!("entire://{host}/"), format!("{upstream}/")]
    }
    async fn setup_query(
        &self,
        access: &Access,
        name: &str,
        _input: Value,
    ) -> Result<Value, Error> {
        if name != "entire.list_repositories" {
            return Err(Error::Invalid(format!("Unknown setup query {name}")));
        }
        let invalid = |e: ConnectError| Error::Invalid(e.message.unwrap_or_default());
        let minted = crate::connections::mints::mint(
            access,
            "entire",
            "service_account",
            &Default::default(),
            "setup",
        )
        .await
        .map_err(invalid)?;
        let core = core(access).map_err(invalid)?;
        let mut options = vec![];
        let mut page = String::new();
        // At most ten pages of a hundred.
        for _ in 0..10 {
            let mut request = access
                .http
                .client
                .get(format!("{core}/api/v1/repos"))
                .query(&[("pageSize", "100"), ("pageToken", page.as_str())]);
            for (header, value) in minted.headers() {
                request = request.header(header, value.expose_secret());
            }
            let response = access.http.json(request).await?;
            options.extend(
                response.0["repos"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|repo| repo["full_name"].as_str())
                    .map(|v| json!({"value": v, "label": v})),
            );
            page = response.0["nextPageToken"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            if page.is_empty() {
                break;
            }
        }
        Ok(json!({ "options": options }))
    }
}

/// A cluster host as Entire names them, e.g. `aws-us-east-2.entire.io` (Entire is moving its
/// hosts to entire.com).
pub fn cluster_host(host: &str) -> bool {
    [".entire.io", ".entire.com"].iter().any(|suffix| {
        host.strip_suffix(suffix).is_some_and(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
    })
}
