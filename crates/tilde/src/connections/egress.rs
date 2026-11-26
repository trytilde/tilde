//! Egress: what a connection's provider lets a sandbox reach through the gateway with the
//! connection's credential, which never enters the sandbox.
//!
//! A provider declares the protocols its upstream speaks (`Egress`): HTTP with deny-by-default
//! allow and deny rules (deny wins), and git smart-HTTP with the services it allows and which ref
//! updates it refuses. Upstreams are fixed by the provider or read from the connection (a
//! self-managed instance, a regional cluster), never by a grant. A grant (a sandbox
//! blueprint's, `sandboxes::egress`) carries a config typed by the provider's JSON Schema that can
//! only narrow what the rules allow. The provider's hook (`EgressHook`) checks each request
//! against the rules and that config, and names the narrowest credential the grant needs.
//!
//! Definitions are built-in catalog code (`hook`), like mint strategies: registered providers
//! cannot declare egress yet. Besides the git forges, the API providers agents read from
//! sandboxes declare HTTP egress (`catalog::apis`: Linear, Sentry, Jira, PagerDuty, Datadog,
//! Grafana, Prometheus, New Relic, Honeycomb, Terraform; `catalog::aws::egress`: AWS, re-signed
//! with SigV4). How the credential travels is the connection type's mint (`mints`): a bearer
//! token, the provider's own headers, Basic, or SigV4 signing credentials.
use crate::chat::providers::Access;
use crate::connections::mints::MintScope;
use crate::error::Error;
use serde_json::Value;

pub struct Egress {
    pub protocols: Vec<EgressProtocol>,
    /// JSON Schema of a grant's config. `x-tilde-options: "<setup query>"` on a property fills
    /// its choices from that query.
    pub config_schema: Value,
    /// Bumped when the schema changes incompatibly; grants record the version they were saved at.
    pub config_version: u32,
    pub setup_queries: Vec<&'static str>,
}

pub enum EgressProtocol {
    /// Rules are `METHOD /path` patterns (`*` any method): `*` matches one path segment, `**`
    /// the rest of the path, `{name}` one segment the hook reads by name.
    Http {
        upstream: &'static str,
        allow: Vec<&'static str>,
        deny: Vec<&'static str>,
        /// Environment for the sandbox's shells: `{url}` is the grant's gateway URL, `{token}`
        /// the sandbox's egress token.
        wiring: Vec<EnvTemplate>,
    },
    /// Git smart-HTTP. Paths are `<repository>[.git]/info/refs`, `.../git-upload-pack` and
    /// `.../git-receive-pack` under the upstream.
    Git {
        upstream: &'static str,
        auth: GitAuth,
        /// Upstream answers with redirects to replica nodes under its own host: the gateway
        /// follows those itself, with the credential, instead of handing them to the client.
        replicas: bool,
        services: Vec<GitService>,
        deny_delete: bool,
        deny_force: bool,
        deny_default_branch: bool,
    },
}
impl EgressProtocol {
    pub fn upstream(&self) -> &'static str {
        match self {
            Self::Http { upstream, .. } | Self::Git { upstream, .. } => upstream,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Http { .. } => "http",
            Self::Git { .. } => "git",
        }
    }
}

/// How a git upstream takes the minted token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitAuth {
    /// Basic, the token as the password of this user (`x-access-token` on GitHub, `oauth2` on
    /// GitLab).
    Basic(&'static str),
    Bearer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitService {
    /// Fetch and clone.
    UploadPack,
    /// Push.
    ReceivePack,
}
impl GitService {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "git-upload-pack" => Some(Self::UploadPack),
            "git-receive-pack" => Some(Self::ReceivePack),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UploadPack => "git-upload-pack",
            Self::ReceivePack => "git-receive-pack",
        }
    }
}

pub struct EnvTemplate {
    pub name: &'static str,
    pub value: &'static str,
}

/// A ref update of a push: `old` all zeros creates the ref, `new` all zeros deletes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefUpdate {
    pub old: String,
    pub new: String,
    pub name: String,
}
pub const ZERO_SHA: &str = "0000000000000000000000000000000000000000";
impl RefUpdate {
    pub fn is_delete(&self) -> bool {
        self.new == ZERO_SHA
    }
    pub fn is_create(&self) -> bool {
        self.old == ZERO_SHA
    }
}

/// One request a sandbox makes through a grant.
pub enum EgressRequest<'a> {
    Http {
        protocol: &'a EgressProtocol,
        method: &'a str,
        /// The upstream path, starting with `/`, without the query.
        path: &'a str,
        query: Option<&'a str>,
        headers: &'a http::HeaderMap,
        /// The whole request body (bounded by the gateway).
        body: &'a [u8],
    },
    Git {
        protocol: &'a EgressProtocol,
        /// The repository path under the upstream, without `.git`: `owner/repo`,
        /// `group/subgroup/project`, `et/project/repo`.
        repository: &'a str,
        service: GitService,
        /// The ref updates a push's body starts with; empty for discovery and fetches.
        updates: &'a [RefUpdate],
        /// Updates proven fast-forward by the pack the push carries (indexes into `updates`).
        fast_forward: &'a [bool],
        /// The repository's default branch (`refs/heads/main`) as upstream advertises it; `None`
        /// when it could not be learned, which refuses every push under `deny_default_branch`.
        default_branch: Option<&'a str>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny(String),
}
impl Decision {
    pub fn deny(reason: impl Into<String>) -> Self {
        Self::Deny(reason.into())
    }
}

/// A provider's egress behaviour. The defaults apply the manifest rules alone and mint the
/// connection's unscoped credential.
#[async_trait::async_trait]
pub trait EgressHook: Send + Sync {
    fn definition(&self) -> &Egress;
    fn check(&self, request: &EgressRequest<'_>, _config: &Value) -> Decision {
        rules::check(request)
    }
    fn scope(&self, _config: &Value) -> MintScope {
        MintScope::default()
    }
    /// The protocol's upstream for this connection: the provider's fixed one unless the
    /// connection names its own (a self-managed instance, a regional cluster).
    fn upstream(&self, protocol: &EgressProtocol, _access: &Access) -> Result<String, Error> {
        Ok(protocol.upstream().to_owned())
    }
    /// Where an allowed HTTP request goes, given the connection's upstream: there, unless the
    /// provider addresses requests by what they carry (AWS: the service and region they are
    /// signed for).
    fn route(&self, upstream: String, _request: &EgressRequest<'_>) -> Result<String, Decision> {
        Ok(upstream)
    }
    /// The remote URL prefixes a sandbox's git rewrites to the route, given the git upstream.
    fn git_remotes(&self, upstream: &str) -> Vec<String> {
        vec![format!("{upstream}/")]
    }
    /// Choices for a config field (`x-tilde-options`), from the connection's upstream.
    async fn setup_query(&self, access: &Access, name: &str, input: Value) -> Result<Value, Error>;
}

/// The built-in provider's egress hook.
pub fn hook(provider: &str) -> Option<&'static dyn EgressHook> {
    match provider {
        "github" => Some(crate::connections::catalog::github::egress::hook()),
        "gitlab" => Some(crate::connections::catalog::gitlab::hook()),
        "entire" => Some(crate::connections::catalog::entire::hook()),
        "aws" => Some(crate::connections::catalog::aws::egress::hook()),
        other => crate::connections::catalog::apis::hook(other).map(|h| h as &dyn EgressHook),
    }
}

/// A grant's config, checked against the provider's schema. Keys the schema leaves out are
/// filled with its defaults, so the stored config says exactly what the grant allows.
pub fn validate_config(egress: &Egress, config: &Value) -> Result<Value, Error> {
    let validator = jsonschema::validator_for(&egress.config_schema)
        .map_err(|_| Error::Invalid("The provider's egress config schema is invalid".into()))?;
    if let Some(error) = validator.iter_errors(config).next() {
        return Err(Error::Invalid(format!(
            "Invalid egress config at {}: {error}",
            match error.instance_path().as_str() {
                "" => "the root",
                path => path,
            }
        )));
    }
    Ok(with_defaults(&egress.config_schema, config.clone()))
}
fn with_defaults(schema: &Value, mut value: Value) -> Value {
    if let (Some(properties), Some(object)) = (
        schema.get("properties").and_then(Value::as_object),
        value.as_object_mut(),
    ) {
        for (key, property) in properties {
            match object.get_mut(key) {
                Some(child) => *child = with_defaults(property, child.take()),
                None if property.get("default").is_some() => {
                    object.insert(key.clone(), property["default"].clone());
                }
                None if property.get("type") == Some(&Value::from("object")) => {
                    let child = with_defaults(property, Value::Object(Default::default()));
                    object.insert(key.clone(), child);
                }
                None => {}
            }
        }
    }
    value
}

/// The access key ID, region and service of a SigV4 `Authorization` header
/// (`AWS4-HMAC-SHA256 Credential=<key>/<date>/<region>/<service>/aws4_request, ...`).
pub fn sigv4(authorization: &str) -> Option<(&str, &str, &str)> {
    let (scheme, rest) = authorization.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("aws4-hmac-sha256") {
        return None;
    }
    let scope = rest
        .split(',')
        .find_map(|part| part.trim().strip_prefix("Credential="))?;
    let mut parts = scope.split('/');
    let key = parts.next()?;
    let (_, region, service) = (parts.next()?, parts.next()?, parts.next()?);
    Some((key, region, service))
}

/// What git forges' grants share: narrowing to repositories and to the branches pushes update.
pub mod forge {
    use super::*;
    use serde_json::json;

    pub fn strings(value: Option<&Value>) -> Vec<&str> {
        value
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }
    /// The `push` property of a forge grant's config schema.
    pub fn push_schema(note: &str) -> Value {
        json!({
            "type": "object",
            "title": "Push",
            "properties": {
                "branches": {
                    "type": "array",
                    "title": "Branches",
                    "items": {"type": "string", "pattern": "^[A-Za-z0-9_./*-]+$"},
                    "default": ["factory/*"],
                    "description": format!("Branches pushes may create and update (`*` matches within a path segment). Empty: no pushes. {note}")
                }
            },
            "additionalProperties": false
        })
    }
    /// `*` matches any run of characters except `/`.
    pub fn glob(pattern: &str, name: &str) -> bool {
        match pattern.split_once('*') {
            None => pattern == name,
            Some((prefix, rest)) => {
                let Some(tail) = name.strip_prefix(prefix) else {
                    return false;
                };
                (0..=tail.find('/').unwrap_or(tail.len()))
                    .filter(|&i| tail.is_char_boundary(i))
                    .any(|i| glob(rest, &tail[i..]))
            }
        }
    }
    /// A git request against a grant: the repository must be granted, a push needs granted
    /// branches (`branches` are globs; empty: no push) and may only update those.
    pub fn check_git(
        granted: bool,
        repository: &str,
        branches: &[&str],
        service: GitService,
        updates: &[RefUpdate],
    ) -> Decision {
        if !granted {
            return Decision::deny(format!("{repository} is not granted"));
        }
        if service == GitService::ReceivePack && branches.is_empty() {
            return Decision::deny("Pushing is not granted");
        }
        for update in updates {
            match update.name.strip_prefix("refs/heads/") {
                Some(branch) if branches.iter().any(|pattern| glob(pattern, branch)) => {}
                _ => {
                    return Decision::deny(format!("Pushing to {} is not granted", update.name));
                }
            }
        }
        Decision::Allow
    }
}

pub mod rules {
    use super::*;

    /// The manifest rules: an HTTP request must match an allow rule and no deny rule; a git
    /// request must use an allowed service, and a push may not delete refs when the protocol
    /// denies it, nor rewrite them when it denies force (an update the pack does not prove a
    /// fast-forward).
    pub fn check(request: &EgressRequest<'_>) -> Decision {
        match request {
            EgressRequest::Http {
                protocol: EgressProtocol::Http { allow, deny, .. },
                method,
                path,
                ..
            } => {
                if deny
                    .iter()
                    .any(|rule| matches(rule, method, path).is_some())
                {
                    return Decision::deny(format!("{method} {path} is denied"));
                }
                if allow
                    .iter()
                    .any(|rule| matches(rule, method, path).is_some())
                {
                    return Decision::Allow;
                }
                Decision::deny(format!("{method} {path} is not allowed"))
            }
            EgressRequest::Git {
                protocol:
                    EgressProtocol::Git {
                        services,
                        deny_delete,
                        deny_force,
                        deny_default_branch,
                        ..
                    },
                service,
                updates,
                fast_forward,
                default_branch,
                ..
            } => {
                if !services.contains(service) {
                    return Decision::deny(format!("{} is not allowed", service.as_str()));
                }
                for (i, update) in updates.iter().enumerate() {
                    if *deny_default_branch && default_branch.is_none_or(|d| d == update.name) {
                        return Decision::deny(format!(
                            "Pushing to the default branch is denied ({})",
                            default_branch.unwrap_or("unknown")
                        ));
                    }
                    if update.is_delete() {
                        if *deny_delete {
                            return Decision::deny(format!("Deleting {} is denied", update.name));
                        }
                    } else if *deny_force
                        && !update.is_create()
                        && !fast_forward.get(i).copied().unwrap_or(false)
                    {
                        return Decision::deny(format!(
                            "Updating {} is not a fast-forward; force pushes are denied",
                            update.name
                        ));
                    }
                }
                Decision::Allow
            }
            _ => Decision::deny("The request does not match the grant's protocol"),
        }
    }

    /// The `{name}` segments of `path` if `rule` matches the method and path.
    pub fn matches<'p>(rule: &str, method: &str, path: &'p str) -> Option<Vec<(String, &'p str)>> {
        let (rule_method, rule_path) = rule.split_once(' ')?;
        if rule_method != "*" && !rule_method.eq_ignore_ascii_case(method) {
            return None;
        }
        // Empty, dot and encoded-slash or dot segments could reach another resource upstream. One
        // trailing slash names the same resource (Sentry's paths end with one).
        let rest = path.strip_prefix('/')?;
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        let segments = || rest.split('/').filter(|_| !rest.is_empty());
        if segments().any(|segment| {
            let lower = segment.to_ascii_lowercase();
            segment.is_empty()
                || segment.trim_matches('.').is_empty()
                || lower.contains("%2f")
                || lower.contains("%2e")
                || lower.contains("%5c")
                || segment.contains('\\')
        }) {
            return None;
        }
        let mut captures = vec![];
        let mut segments = segments();
        for part in rule_path.trim_start_matches('/').split('/') {
            if part == "**" {
                return Some(captures);
            }
            let segment = segments.next()?;
            if let Some(name) = part.strip_prefix('{').and_then(|p| p.strip_suffix('}')) {
                captures.push((name.to_owned(), segment));
            } else if part != "*" && part != segment {
                return None;
            }
        }
        segments.next().is_none().then_some(captures)
    }
}

/// What a GraphQL API's grant checks: the operations a request body carries.
pub mod graphql {
    use super::Decision;

    #[derive(Debug, PartialEq, Eq)]
    pub struct Operation {
        /// `query`, `mutation` or `subscription`.
        pub kind: String,
        /// The top-level fields it selects (aliases resolved); `...` for a fragment spread, which
        /// could select anything.
        pub fields: Vec<String>,
    }

    /// A GraphQL request (`{"query": "..."}`) may only query, and mutate with the fields in
    /// `mutations`. Batches, subscriptions and anything unparseable are refused.
    pub fn check(body: &[u8], mutations: &[&str]) -> Decision {
        let Some(operations) = operations(body) else {
            return Decision::deny("Not a GraphQL request");
        };
        for operation in &operations {
            match operation.kind.as_str() {
                "query" => {}
                "mutation" => {
                    if let Some(field) = operation
                        .fields
                        .iter()
                        .find(|f| !mutations.contains(&f.as_str()))
                    {
                        return Decision::deny(format!("The mutation {field} is not granted"));
                    }
                }
                kind => return Decision::deny(format!("GraphQL {kind}s are not allowed")),
            }
        }
        Decision::Allow
    }

    /// The operations of a GraphQL request body; `None` when it is not one request with a
    /// parseable document.
    pub fn operations(body: &[u8]) -> Option<Vec<Operation>> {
        let request: serde_json::Value = serde_json::from_slice(body).ok()?;
        let tokens = tokens(request.get("query")?.as_str()?)?;
        let mut tokens = tokens.into_iter().peekable();
        let mut operations = vec![];
        while let Some(token) = tokens.next() {
            let kind = match token {
                Token::Punct(b'{') => "query",
                Token::Name(name @ ("query" | "mutation" | "subscription" | "fragment")) => {
                    // The name, variable definitions and directives, up to the selection set.
                    loop {
                        match tokens.next()? {
                            Token::Punct(b'{') => break,
                            Token::Punct(b'(') => skip(&mut tokens, b'(', b')')?,
                            _ => {}
                        }
                    }
                    name
                }
                _ => return None,
            };
            if kind == "fragment" {
                skip(&mut tokens, b'{', b'}')?;
                continue;
            }
            operations.push(Operation {
                kind: kind.into(),
                fields: selection(&mut tokens)?,
            });
        }
        (!operations.is_empty()).then_some(operations)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Token<'a> {
        Name(&'a str),
        /// Punctuation; `.` stands for a spread (`...`).
        Punct(u8),
    }
    type Tokens<'a> = std::iter::Peekable<std::vec::IntoIter<Token<'a>>>;

    /// The top-level fields of a selection set whose `{` was read.
    fn selection(tokens: &mut Tokens<'_>) -> Option<Vec<String>> {
        let mut fields = vec![];
        loop {
            match tokens.next()? {
                Token::Punct(b'}') => return Some(fields),
                Token::Punct(b'(') => skip(tokens, b'(', b')')?,
                Token::Punct(b'{') => skip(tokens, b'{', b'}')?,
                // A directive's name is not a field.
                Token::Punct(b'@') => {
                    tokens.next()?;
                }
                Token::Punct(b'.') => {
                    fields.push("...".into());
                    // `...Name` or `... on Type`; an inline fragment's selection is skipped.
                    if let Some(Token::Name("on")) = tokens.peek() {
                        tokens.next();
                        tokens.next()?;
                    } else if let Some(Token::Name(_)) = tokens.peek() {
                        tokens.next();
                    }
                }
                Token::Name(name) => {
                    if tokens.peek() == Some(&Token::Punct(b':')) {
                        tokens.next();
                        match tokens.next()? {
                            Token::Name(field) => fields.push(field.into()),
                            _ => return None,
                        }
                    } else {
                        fields.push(name.into());
                    }
                }
                _ => return None,
            }
        }
    }
    /// Past the `close` matching an `open` that was read.
    fn skip(tokens: &mut Tokens<'_>, open: u8, close: u8) -> Option<()> {
        let mut depth = 1;
        while depth > 0 {
            match tokens.next()? {
                Token::Punct(c) if c == open => depth += 1,
                Token::Punct(c) if c == close => depth -= 1,
                _ => {}
            }
        }
        Some(())
    }
    /// Names and punctuation; strings, numbers, commas and comments are dropped.
    fn tokens(source: &str) -> Option<Vec<Token<'_>>> {
        let bytes = source.as_bytes();
        let mut out = vec![];
        let mut i = 0;
        while i < bytes.len() {
            let rest = &bytes[i..];
            match bytes[i] {
                b' ' | b'\t' | b'\n' | b'\r' | b',' => i += 1,
                b'#' => {
                    while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                        i += 1;
                    }
                }
                b'"' if rest.starts_with(b"\"\"\"") => {
                    i += 3;
                    loop {
                        let rest = bytes.get(i..)?;
                        if rest.is_empty() {
                            return None;
                        } else if rest.starts_with(b"\\\"\"\"") {
                            i += 4;
                        } else if rest.starts_with(b"\"\"\"") {
                            i += 3;
                            break;
                        } else {
                            i += 1;
                        }
                    }
                }
                b'"' => {
                    i += 1;
                    loop {
                        match bytes.get(i)? {
                            b'\\' => i += 2,
                            b'"' => {
                                i += 1;
                                break;
                            }
                            b'\n' | b'\r' => return None,
                            _ => i += 1,
                        }
                    }
                }
                b'.' if rest.starts_with(b"...") => {
                    out.push(Token::Punct(b'.'));
                    i += 3;
                }
                c if c.is_ascii_alphabetic() || c == b'_' => {
                    let start = i;
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
                    {
                        i += 1;
                    }
                    out.push(Token::Name(&source[start..i]));
                }
                c if c.is_ascii_digit() || c == b'-' => {
                    i += 1;
                    while i < bytes.len()
                        && (bytes[i].is_ascii_alphanumeric() || b".+-".contains(&bytes[i]))
                    {
                        i += 1;
                    }
                }
                c if b"{}()[]:=@$!|&".contains(&c) => {
                    out.push(Token::Punct(c));
                    i += 1;
                }
                _ => return None,
            }
        }
        Some(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn body(query: &str) -> Vec<u8> {
            serde_json::json!({ "query": query })
                .to_string()
                .into_bytes()
        }

        // The parser is what stands between a sandbox and an ungranted mutation, so its edges
        // are checked here: aliases, arguments holding braces and strings, comments, fragments.
        #[test]
        fn mutations_are_read_through_aliases_arguments_and_fragments() {
            let granted = ["issueCreate", "commentCreate"];
            let allowed = [
                "{ issues(first: 5) { nodes { id title } } }",
                "query Q($id: String!) { issue(id: $id) { id } }",
                "mutation M($input: IssueCreateInput!) { issueCreate(input: $input) { success } }",
                "mutation { a: commentCreate(input: {body: \"} mutation { apiKeyCreate\"}) { success } }",
                "# mutation { teamDelete }\nquery { viewer { id } }",
                "fragment F on Issue { id } query { issue(id: \"x\") { ...F } }",
            ];
            for query in allowed {
                assert_eq!(check(&body(query), &granted), Decision::Allow, "{query}");
            }
            let denied = [
                "mutation { issueCreate(input: {}) { success } teamDelete(id: \"x\") { success } }",
                "mutation { issueCreate: teamDelete(id: \"x\") { success } }",
                "mutation { ...Everything }",
                "subscription { issueUpdates { id } }",
                "{ issues { nodes { id } } } mutation { userDelete { success } }",
                "mutation { issueCreate(input: {title: \"unterminated) { success } }",
            ];
            for query in denied {
                assert_ne!(check(&body(query), &granted), Decision::Allow, "{query}");
            }
            assert_ne!(
                check(b"[{\"query\":\"{ viewer { id } }\"}]", &granted),
                Decision::Allow
            );
        }
    }
}
