//! Prompts declared in agent code. `tilde deploy` discovers them (framework agent objects,
//! YAML configs, `definePrompt`) and registers them with the deployment; the engine keeps every
//! distinct content as an immutable version and records which deployments shipped it. Inference
//! calls are linked to versions off the request path ([`matching`]): by the static text the
//! gateway finds in the request body, or by the `x-tilde-prompt` stamp the SDK sends for
//! dynamic prompts. Nothing here is edited from the UI: the management surface is a read-only
//! history with usage per version.
pub mod db;
pub mod matching;
pub mod rpc;
use crate::proto::tilde::{management::v1 as management, types::v1 as types};
use crate::{database::Pool, error::Error};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

const MAX_TEMPLATE_BYTES: usize = 256 * 1024;
const MAX_CONFIG_BYTES: usize = 64 * 1024;
const MAX_SECTIONS: usize = 64;
const MAX_ORIGIN_BYTES: usize = 512;

#[derive(Clone)]
pub struct Prompts {
    pool: Pool,
}
/// How a template is written; decides its variables and how calls are matched to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Text without variables.
    Plain,
    /// `{{var}}` and `{{> section}}` partials.
    Mustache,
    /// `{var}`; `{{` and `}}` are literal braces.
    Braces,
    /// A function: the template is its source and calls reach it only by stamp.
    Dynamic,
}
impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Mustache => "mustache",
            Self::Braces => "braces",
            Self::Dynamic => "dynamic",
        }
    }
    pub fn parse(value: &str) -> Self {
        match value {
            "mustache" => Self::Mustache,
            "braces" => Self::Braces,
            "dynamic" => Self::Dynamic,
            _ => Self::Plain,
        }
    }
    fn from_wire(value: types::PromptFormat) -> Result<Self, Error> {
        match value {
            types::PromptFormat::PROMPT_FORMAT_PLAIN => Ok(Self::Plain),
            types::PromptFormat::PROMPT_FORMAT_MUSTACHE => Ok(Self::Mustache),
            types::PromptFormat::PROMPT_FORMAT_BRACES => Ok(Self::Braces),
            types::PromptFormat::PROMPT_FORMAT_DYNAMIC => Ok(Self::Dynamic),
            _ => Err(Error::Invalid("Prompt format is required".into())),
        }
    }
    pub fn wire(self) -> types::PromptFormat {
        match self {
            Self::Plain => types::PromptFormat::PROMPT_FORMAT_PLAIN,
            Self::Mustache => types::PromptFormat::PROMPT_FORMAT_MUSTACHE,
            Self::Braces => types::PromptFormat::PROMPT_FORMAT_BRACES,
            Self::Dynamic => types::PromptFormat::PROMPT_FORMAT_DYNAMIC,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub hash: Vec<u8>,
    pub content: String,
}
#[derive(Debug, Clone)]
pub struct PromptVersion {
    pub id: Uuid,
    pub prompt_id: Uuid,
    pub number: i32,
    pub hash: Vec<u8>,
    pub template: String,
    pub config: String,
    pub variables: Vec<String>,
    pub format: Format,
    pub origin: String,
    pub deployment_id: Option<Uuid>,
    pub commit_sha: Option<String>,
    pub created_at: DateTime<Utc>,
    pub sections: Vec<Section>,
}
#[derive(Debug, Clone)]
pub struct Prompt {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub latest: Option<PromptVersion>,
}
#[derive(Debug, Clone)]
pub struct Usage {
    pub version_id: Uuid,
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_micros: Option<i64>,
    pub average_latency_ms: i64,
    pub last_used_at: DateTime<Utc>,
}
/// A declared prompt that passed validation, ready to store inside a deployment's transaction.
#[derive(Debug, Clone)]
pub struct Declared {
    pub name: String,
    pub format: Format,
    pub template: String,
    pub sections: BTreeMap<String, String>,
    pub config: String,
    pub hash: Vec<u8>,
    pub origin: String,
    pub variables: Vec<String>,
}

/// One rule shared with the SDK: template, then each section by name, then config, each
/// field framed by a label and NUL so no boundary shift produces the same digest.
pub fn content_hash(template: &str, sections: &BTreeMap<String, String>, config: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"template\0");
    hasher.update(template.as_bytes());
    hasher.update(b"\0");
    for (name, content) in sections {
        hasher.update(b"section\0");
        hasher.update(name.as_bytes());
        hasher.update(b"\0");
        hasher.update(content.as_bytes());
        hasher.update(b"\0");
    }
    hasher.update(b"config\0");
    hasher.update(config.as_bytes());
    hasher.finalize().into()
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'))
}
/// `{{name}}` placeholders and `{{> name}}` partial references, in order of appearance.
pub fn placeholders(text: &str) -> (Vec<String>, Vec<String>) {
    let mut variables = Vec::new();
    let mut partials = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        let inner = after[..end].trim();
        if let Some(partial) = inner.strip_prefix('>') {
            let partial = partial.trim();
            if valid_name(partial) && !partials.iter().any(|p| p == partial) {
                partials.push(partial.to_owned());
            }
        } else if !inner.is_empty()
            && inner
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && !variables.iter().any(|v| v == inner)
        {
            variables.push(inner.to_owned());
        }
        rest = &after[end + 2..];
    }
    (variables, partials)
}
fn brace_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
/// Splits a `{var}` template into literal text and variable names. `{{` and `}}` are escaped
/// braces; a brace pair around anything that is not a name stays literal text.
pub(crate) fn brace_parts(text: &str) -> Vec<Result<String, String>> {
    let mut parts: Vec<Result<String, String>> = Vec::new();
    let mut literal = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(['{', '}']) {
        literal.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            literal.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        if tail.starts_with('{')
            && let Some(end) = tail.find('}')
            && brace_name(&tail[1..end])
        {
            parts.push(Ok(std::mem::take(&mut literal)));
            parts.push(Err(tail[1..end].to_owned()));
            rest = &tail[end + 1..];
            continue;
        }
        literal.push_str(&tail[..1]);
        rest = &tail[1..];
    }
    literal.push_str(rest);
    parts.push(Ok(literal));
    parts
}

impl Declared {
    /// Validate one declaration by its format and check the SDK's hash against the content.
    pub fn new(p: management::DeclaredPrompt) -> Result<Self, Error> {
        let name = p.name;
        let invalid = |message: &str| Error::Invalid(format!("Prompt {name}: {message}"));
        if !valid_name(&name) {
            return Err(Error::Invalid(format!(
                "Prompt name {name} must use letters, digits, '.', '_', '-' or '/' (1-128 characters)"
            )));
        }
        let format = Format::from_wire(p.format.as_known().unwrap_or_default())
            .map_err(|_| invalid("format is required"))?;
        if p.template.is_empty() || p.template.len() > MAX_TEMPLATE_BYTES {
            return Err(invalid("template must be 1-262144 bytes"));
        }
        if p.origin.len() > MAX_ORIGIN_BYTES {
            return Err(invalid("origin is too long"));
        }
        if p.sections.len() > MAX_SECTIONS {
            return Err(invalid("at most 64 sections"));
        }
        let mut sections = BTreeMap::new();
        for s in p.sections {
            if !valid_name(&s.name) || s.content.len() > MAX_TEMPLATE_BYTES {
                return Err(invalid("invalid section"));
            }
            if sections.insert(s.name, s.content).is_some() {
                return Err(invalid("duplicate section"));
            }
        }
        if format != Format::Mustache && !sections.is_empty() {
            return Err(invalid("only mustache templates include sections"));
        }
        // The SDK serialises an absent config as `{}`; the hash covers that text.
        let config = if p.config.is_empty() {
            "{}".to_owned()
        } else {
            p.config
        };
        if config.len() > MAX_CONFIG_BYTES
            || !matches!(
                serde_json::from_str::<serde_json::Value>(&config),
                Ok(serde_json::Value::Object(_))
            )
        {
            return Err(invalid("config must be a JSON object"));
        }
        let hash = hex::decode(&p.hash)
            .ok()
            .filter(|h| h.len() == 32)
            .ok_or_else(|| invalid("hash must be 64 hex characters"))?;
        if content_hash(&p.template, &sections, &config) != hash.as_slice() {
            return Err(invalid("hash does not match the declared content"));
        }
        let variables = match format {
            Format::Plain | Format::Dynamic => vec![],
            Format::Braces => {
                let mut variables: Vec<String> = Vec::new();
                for part in brace_parts(&p.template) {
                    if let Err(v) = part
                        && !variables.contains(&v)
                    {
                        variables.push(v);
                    }
                }
                variables
            }
            Format::Mustache => {
                let (mut variables, partials) = placeholders(&p.template);
                for partial in &partials {
                    if !sections.contains_key(partial) {
                        return Err(invalid(&format!(
                            "includes an undeclared section {{{{> {partial}}}}}"
                        )));
                    }
                }
                for content in sections.values() {
                    let (inner, nested) = placeholders(content);
                    if !nested.is_empty() {
                        return Err(invalid("sections cannot include other sections"));
                    }
                    for v in inner {
                        if !variables.contains(&v) {
                            variables.push(v);
                        }
                    }
                }
                variables
            }
        };
        Ok(Self {
            name,
            format,
            template: p.template,
            sections,
            config,
            hash,
            origin: p.origin,
            variables,
        })
    }
}
/// Store a deployment's prompts inside the caller's transaction: each distinct content is a
/// version once per prompt, and the deployment links every version it shipped.
pub async fn register(
    tx: &tokio_postgres::Transaction<'_>,
    agent: Uuid,
    deployment: Uuid,
    prompts: &[Declared],
) -> Result<(), Error> {
    for p in prompts {
        let prompt = db::prompt_upsert_one(tx, Uuid::new_v4(), agent, &p.name).await?;
        db::prompt_lock_one(tx, prompt).await?;
        let version = match db::version_by_hash_opt(tx, prompt, &p.hash).await? {
            Some(existing) => existing,
            None => {
                let version = db::version_insert_one(
                    tx,
                    Uuid::new_v4(),
                    prompt,
                    &p.hash,
                    &p.template,
                    &p.config,
                    &p.variables,
                    p.format.as_str(),
                    &p.origin,
                    deployment,
                )
                .await?;
                let names: Vec<&str> = p.sections.keys().map(String::as_str).collect();
                let hashes: Vec<Vec<u8>> = p
                    .sections
                    .values()
                    .map(|c| Sha256::digest(c.as_bytes()).to_vec())
                    .collect();
                let contents: Vec<&str> = p.sections.values().map(String::as_str).collect();
                db::sections_insert_execute(tx, version, &names, &hashes, &contents).await?;
                version
            }
        };
        db::deployment_link_execute(tx, deployment, version).await?;
    }
    Ok(())
}

impl Prompts {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
    async fn versions(&self, ids: &[Uuid]) -> Result<Vec<PromptVersion>, Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let db = self.pool.get().await?;
        let rows = db::versions_by_ids_all(&db, ids).await?;
        let sections = db::sections_list_all(&db, ids).await?;
        Ok(rows
            .into_iter()
            .map(|v| PromptVersion {
                sections: sections
                    .iter()
                    .filter(|s| s.version_id == v.id)
                    .map(|s| Section {
                        name: s.name.clone(),
                        hash: s.hash.clone(),
                        content: s.content.clone(),
                    })
                    .collect(),
                id: v.id,
                prompt_id: v.prompt_id,
                number: v.number,
                hash: v.hash,
                template: v.template,
                config: v.config,
                variables: v.variables,
                format: Format::parse(&v.format),
                origin: v.origin,
                deployment_id: v.deployment_id,
                commit_sha: v.commit_sha,
                created_at: v.created_at,
            })
            .collect())
    }
    pub async fn list(&self, agent: Uuid) -> Result<Vec<Prompt>, Error> {
        let rows = db::prompts_list_all(&self.pool.get().await?, agent).await?;
        let latest: Vec<Uuid> = rows.iter().filter_map(|r| r.latest_id).collect();
        let versions = self.versions(&latest).await?;
        Ok(rows
            .into_iter()
            .map(|r| Prompt {
                latest: r
                    .latest_id
                    .and_then(|id| versions.iter().find(|v| v.id == id).cloned()),
                id: r.id,
                agent_id: r.agent_id,
                name: r.name,
                created_at: r.created_at,
            })
            .collect())
    }
    /// The prompt with every version, newest first, and the inference usage each collected.
    pub async fn get(&self, id: Uuid) -> Result<(Prompt, Vec<PromptVersion>, Vec<Usage>), Error> {
        let db = self.pool.get().await?;
        let row = db::prompt_get_opt(&db, id).await?.ok_or(Error::NotFound)?;
        let ids: Vec<Uuid> = db::versions_list_all(&db, id)
            .await?
            .into_iter()
            .map(|v| v.id)
            .collect();
        let usage = db::usage_all(&db, id)
            .await?
            .into_iter()
            .map(|u| Usage {
                version_id: u.version_id,
                requests: u.requests,
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
                cost_micros: u.cost_micros,
                average_latency_ms: u.average_latency_ms,
                last_used_at: u.last_used_at,
            })
            .collect();
        drop(db);
        let versions = self.versions(&ids).await?;
        let prompt = Prompt {
            id: row.id,
            agent_id: row.agent_id,
            name: row.name,
            created_at: row.created_at,
            latest: versions.first().cloned(),
        };
        Ok((prompt, versions, usage))
    }
    /// The prompt versions a deployment shipped, with each prompt's id and name.
    pub async fn for_deployment(
        &self,
        deployment: Uuid,
    ) -> Result<Vec<(Uuid, String, PromptVersion)>, Error> {
        let rows = db::deployment_versions_all(&self.pool.get().await?, deployment).await?;
        let ids: Vec<Uuid> = rows.iter().map(|r| r.version_id).collect();
        let versions = self.versions(&ids).await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let version = versions.iter().find(|v| v.id == r.version_id)?.clone();
                Some((r.prompt_id, r.name, version))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hash_is_shared_with_the_sdk() {
        // The SDK test asserts the same vector; change both or neither.
        let sections = BTreeMap::from([("rules".to_owned(), "Be brief.".to_owned())]);
        assert_eq!(
            hex::encode(content_hash(
                "Hello {{name}}\n{{> rules}}",
                &sections,
                r#"{"model":"claude-sonnet-5","temperature":0.2}"#
            )),
            "985e00ff1447c634a4fbe5dca489e677fbdf175148e3a9933e05c884ebf9e691"
        );
    }
    #[test]
    fn placeholders_separate_variables_from_partials() {
        let (variables, partials) = placeholders("{{ a }} {{> rules}} {{b}} {{a}} {{bad name}}");
        assert_eq!(variables, vec!["a", "b"]);
        assert_eq!(partials, vec!["rules"]);
    }
}
