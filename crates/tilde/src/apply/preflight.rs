//! Preflight: every check an apply can make before its transaction, collected rather than
//! stopping at the first, so one run lists everything a person must do. It writes no registry
//! state; the one write is preparing a deployment, which stores its skill files by content hash,
//! and that is skipped once a permission is missing. It resolves the connections the transaction
//! then uses.
use super::{Apply, Connection, Refs, Required};
use crate::deployment::Prepared;
use crate::error::Error;
use crate::proto::tilde::management::v1 as m;
use m::RequiredActionCode as Code;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

pub(super) struct Checked {
    pub refs: Refs,
    pub required: Vec<Required>,
    /// Each agent's validated deployment, by agent key.
    pub deployments: HashMap<String, Prepared>,
}

/// What a spec uses a connection for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Use {
    Inference,
    Tools(Vec<String>),
    Sandbox,
    Egress(String),
    Signal(String),
}
struct Usage<'a> {
    reference: &'a m::ConnectionRef,
    subject: String,
    usage: Use,
    /// The resource it belongs to: an agent (by key) or a blueprint (by name). Only Tilde Cloud
    /// reads it (teams), so open-source Tilde allows it unread.
    #[allow(dead_code)]
    owner: Owner<'a>,
    /// The team the connection resolves in: its resource's own (Tilde Cloud). None: the
    /// organization, or a server without teams.
    team: Option<Uuid>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Owner<'a> {
    Agent(&'a str),
    Blueprint(&'a str),
}

struct Found(Vec<Required>);
impl Found {
    fn push(
        &mut self,
        code: Code,
        subject: impl Into<String>,
        message: impl Into<String>,
        url: String,
    ) {
        self.0.push(Required {
            code,
            subject: subject.into(),
            message: message.into(),
            url,
        });
    }
    fn invalid(&mut self, subject: impl Into<String>, message: impl Into<String>) {
        self.push(Code::Invalid, subject, message, String::new());
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The closest of `options` to `name`, if it is plausibly a typo of it.
pub(crate) fn nearest<'a>(
    name: &str,
    options: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let distance = |a: &str, b: &str| {
        let b: Vec<char> = b.chars().collect();
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut previous = row[0];
            row[0] = i + 1;
            for (j, cb) in b.iter().enumerate() {
                let current = row[j + 1];
                row[j + 1] = (previous + usize::from(ca != *cb))
                    .min(row[j] + 1)
                    .min(current + 1);
                previous = current;
            }
        }
        row[b.len()]
    };
    let limit = (name.chars().count() / 3).max(2);
    options
        .into_iter()
        .map(|option| (distance(name, option), option))
        .filter(|(d, _)| *d <= limit)
        .min()
        .map(|(_, option)| option)
}

/// Spec shape: keys, duplicates, and blueprints declared differently by two agents.
fn shape(request: &m::ApplyRequest, found: &mut Found) {
    let mut keys = HashSet::new();
    let mut blueprints: HashMap<&str, (&str, &m::BlueprintSpec)> = HashMap::new();
    if request.agents.is_empty() && request.users.is_empty() && request.groups.is_empty() {
        found.invalid("spec", "The spec declares no agents or people");
    }
    for agent in &request.agents {
        let key = agent.key.as_str();
        if !valid_key(key) {
            found.invalid(
                format!("agent {key}"),
                "An agent key is 1-64 lowercase letters, digits and dashes",
            );
        }
        if !keys.insert(key) {
            found.invalid(format!("agent {key}"), "Two agents have this key");
        }
        let mut aliases = HashSet::new();
        let mut alias_connections = HashMap::new();
        for alias in agent.inference.iter().filter(|a| !a.delete) {
            let subject = format!("{key}/inference/{}", alias.alias);
            if let Err(error) = crate::connections::model::validate_alias(&alias.alias) {
                found.invalid(&subject, error.to_string());
            }
            if !aliases.insert(alias.alias.as_str()) {
                found.invalid(&subject, "The agent declares this alias twice");
            }
            if let Some(other) =
                alias_connections.insert(alias.connection.slug.as_str(), &alias.alias)
            {
                found.invalid(
                    &subject,
                    format!("Alias {other} uses the same connection; a connection has one alias per agent"),
                );
            }
        }
        let mut sources = HashSet::new();
        for source in &agent.tools {
            if !sources.insert(source.connection.slug.as_str()) {
                found.invalid(
                    format!("{key}/tools/{}", source.connection.slug),
                    "The agent declares this tool source twice",
                );
            }
        }
        let mut routines = HashSet::new();
        for routine in &agent.routines {
            if !routines.insert(routine.name.trim()) {
                found.invalid(
                    format!("{key}/routines/{}", routine.name),
                    "The agent declares this routine twice",
                );
            }
        }
        if let Some(blueprint) = agent.sandbox.as_option() {
            match blueprints.get(blueprint.name.as_str()) {
                Some((other, declared)) if *declared != blueprint => found.invalid(
                    format!("sandbox {}", blueprint.name),
                    format!("Agents {other} and {key} declare this blueprint differently"),
                ),
                Some(_) => {}
                None => {
                    blueprints.insert(&blueprint.name, (key, blueprint));
                    let mut slugs = HashSet::new();
                    let mut connections = HashSet::new();
                    for egress in blueprint.egress.iter().filter(|e| !e.delete) {
                        let subject = format!("sandbox {}/egress/{}", blueprint.name, egress.slug);
                        if !slugs.insert(egress.slug.as_str()) {
                            found
                                .invalid(&subject, "The blueprint declares this egress slug twice");
                        }
                        if !connections.insert(egress.connection.slug.as_str()) {
                            found.invalid(&subject, "Another egress grant uses this connection");
                        }
                    }
                }
            }
        }
    }
}

/// Every connection use of the request, live entries only.
fn usages(request: &m::ApplyRequest) -> Vec<Usage<'_>> {
    let mut out = Vec::new();
    let mut seen_blueprints = HashSet::new();
    for agent in &request.agents {
        let key = &agent.key;
        for alias in agent.inference.iter().filter(|a| !a.delete) {
            out.push(Usage {
                reference: &alias.connection,
                subject: format!("{key}/inference/{}", alias.alias),
                usage: Use::Inference,
                owner: Owner::Agent(key),
                team: None,
            });
        }
        // A deleted source still names its connection, which must resolve to find the source.
        for source in &agent.tools {
            out.push(Usage {
                reference: &source.connection,
                subject: format!("{key}/tools/{}", source.connection.slug),
                usage: Use::Tools(if source.delete {
                    Vec::new()
                } else {
                    source.tools.clone()
                }),
                owner: Owner::Agent(key),
                team: None,
            });
        }
        if let Some(blueprint) = agent.sandbox.as_option()
            && seen_blueprints.insert(&blueprint.name)
        {
            out.push(Usage {
                reference: &blueprint.provider,
                subject: format!("sandbox {}", blueprint.name),
                usage: Use::Sandbox,
                owner: Owner::Blueprint(&blueprint.name),
                team: None,
            });
            for egress in blueprint.egress.iter().filter(|e| !e.delete) {
                out.push(Usage {
                    reference: &egress.connection,
                    subject: format!("sandbox {}/egress/{}", blueprint.name, egress.slug),
                    usage: Use::Egress(egress.config_json.clone()),
                    owner: Owner::Blueprint(&blueprint.name),
                    team: None,
                });
            }
        }
        for routine in agent.routines.iter().filter(|r| !r.delete) {
            if let Some(m::routine_trigger_spec::Kind::Signal(signal)) =
                routine.trigger.as_option().and_then(|t| t.kind.as_ref())
            {
                out.push(Usage {
                    reference: &signal.connection,
                    subject: format!("{key}/routines/{}", routine.name),
                    usage: Use::Signal(signal.signal_type.clone()),
                    owner: Owner::Agent(key),
                    team: None,
                });
            }
        }
    }
    out
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

pub(super) async fn check(
    apply: &Apply,
    request: &m::ApplyRequest,
) -> Result<Checked, connectrpc::ConnectError> {
    let mut found = Found(Vec::new());
    shape(request, &mut found);
    let pool = &apply.tools.connections.pool;
    // The agents the specs upsert, by key: found by key, else by a fixed ID.
    let mut existing = HashMap::new();
    for agent in &request.agents {
        let id = agent.id.as_deref().and_then(|id| Uuid::parse_str(id).ok());
        let Some(row) = crate::agent::db::apply_get_opt(&pool.get().await?, &agent.key, id).await?
        else {
            continue;
        };
        if let Some(other) = row.key.as_deref().filter(|k| *k != agent.key) {
            found.invalid(
                format!("agent {}", agent.key),
                format!(
                    "Agent {} already has key {other}; an ID cannot move it to another key",
                    row.id
                ),
            );
            continue;
        }
        existing.insert(agent.key.as_str(), row);
    }
    // Users, groups and access grants are Tilde Cloud's; other servers refuse a spec with any.
    #[allow(unused_mut, unused_assignments)]
    let mut people_refused = !request.users.is_empty()
        || !request.groups.is_empty()
        || request
            .agents
            .iter()
            .any(|a| !a.users.is_empty() || !a.groups.is_empty() || !a.access.is_empty());

    if people_refused {
        found.invalid("people", "Users, groups and access grants need Tilde Cloud");
    }

    // Each resource resolves its connections and secrets in its own team.
    #[allow(unused_mut)]
    let mut owners: HashMap<Owner, Option<Uuid>> = HashMap::new();
    #[allow(unused_mut)]
    let mut usages = usages(request);
    let blueprint_team = |name: &str| owners.get(&Owner::Blueprint(name)).copied().flatten();

    // Connections: each slug resolved once per team, then checked for every use.
    let mut refs = Refs::default();
    let mut by_slug: BTreeMap<(Option<Uuid>, &str), Vec<&Usage>> = BTreeMap::new();
    for usage in &usages {
        by_slug
            .entry((usage.team, usage.reference.slug.as_str()))
            .or_default()
            .push(usage);
    }
    for ((_owning, slug), uses) in by_slug {
        let subject = &uses[0].subject;
        let Some((provider, name)) = slug.split_once('/') else {
            found.invalid(
                subject,
                format!("Connection {slug} is not a provider/name slug"),
            );
            continue;
        };
        let wanted_type = uses.iter().find_map(|u| u.reference.r#type.clone());
        let create_url = apply.link(&format!(
            "/registry/tools/catalog/{}?name={}{}",
            encode(provider),
            encode(name),
            wanted_type
                .as_deref()
                .map(|t| format!("&type={}", encode(t)))
                .unwrap_or_default()
        ));
        let rows =
            crate::connections::db::connection_by_slug_all(&pool.get().await?, provider, name)
                .await?;
        let Some(row) = rows.into_iter().next() else {
            found.push(
                Code::ConnectionMissing,
                subject,
                format!("Connection {slug} does not exist; create it, named {name}"),
                create_url,
            );
            continue;
        };
        if let Some(wanted) = &wanted_type
            && *wanted != row.type_id
        {
            found.push(
                Code::ConnectionWrongType,
                subject,
                format!(
                    "Connection {slug} is a {} connection, not {wanted}",
                    row.type_id
                ),
                create_url,
            );
            continue;
        }
        if row.status != "ready" {
            found.push(
                Code::ConnectionNotReady,
                subject,
                format!(
                    "Connection {slug} is {}; reconnect it",
                    row.status.replace('_', " ")
                ),
                apply.link(&format!(
                    "/registry/tools/connections?search={}",
                    encode(name)
                )),
            );
            continue;
        }
        let connection = Connection {
            id: row.id,
            slug: slug.to_owned(),
            provider_id: row.provider_id.clone(),
            type_id: row.type_id.clone(),
        };
        let catalog_url = apply.link(&format!("/registry/tools/catalog/{}", encode(provider)));
        let details = crate::connections::db::connection_get_opt(&pool.get().await?, row.id)
            .await?
            .ok_or(Error::Missing("Connection"))?;
        for usage in &uses {
            let subject = &usage.subject;
            let missing = |what: &str| format!("Connection {slug} ({}) cannot {what}", row.type_id);
            match &usage.usage {
                Use::Inference if !details.inference_capable => found.push(
                    Code::CapabilityMissing,
                    subject,
                    missing("serve inference"),
                    catalog_url.clone(),
                ),
                Use::Inference => {}
                Use::Sandbox
                    if crate::sandboxes::launch::Provider::of(&row.provider_id).is_none() =>
                {
                    found.push(
                        Code::CapabilityMissing,
                        subject,
                        missing("launch sandboxes (use E2B or Modal)"),
                        catalog_url.clone(),
                    )
                }
                Use::Sandbox => {}
                Use::Egress(config) => match apply.tools.sandboxes.egress_hook(row.id).await {
                    Err(_) => found.push(
                        Code::CapabilityMissing,
                        subject,
                        missing("grant sandbox egress"),
                        catalog_url.clone(),
                    ),
                    Ok(hook) => {
                        let config = serde_json::from_str(config)
                            .map_err(|_| Error::Invalid("The egress config is not JSON".into()))
                            .and_then(|config| {
                                crate::connections::egress::validate_config(
                                    hook.definition(),
                                    &config,
                                )
                            });
                        if let Err(error) = config {
                            found.invalid(subject, error.to_string());
                        }
                    }
                },
                Use::Signal(signal_type) => {
                    match crate::signals::source(&row.provider_id, &row.type_id) {
                        Some(source) if details.signal_capable => {
                            let types = source.types();
                            if !types.iter().any(|t| t.id == *signal_type) {
                                let hint =
                                    nearest(signal_type, types.iter().map(|t| t.id.as_str()))
                                        .map(|n| format!("; did you mean {n}?"))
                                        .unwrap_or_default();
                                found.push(
                                    Code::SignalNotOffered,
                                    subject,
                                    format!(
                                        "Connection {slug} emits no {signal_type} signal{hint}"
                                    ),
                                    catalog_url.clone(),
                                );
                            }
                        }
                        _ => found.push(
                            Code::CapabilityMissing,
                            subject,
                            missing("trigger routines"),
                            catalog_url.clone(),
                        ),
                    }
                }
                Use::Tools(_) if !details.tool_capable => found.push(
                    Code::CapabilityMissing,
                    subject,
                    missing("serve tools"),
                    catalog_url.clone(),
                ),
                Use::Tools(names) => {
                    let offered = apply.tools.provider_tools(row.id);
                    let offered: Vec<String> = match offered.await {
                        Ok(tools) => tools.into_iter().map(|t| t.name).collect(),
                        Err(error) => {
                            found.push(
                                Code::ToolNotOffered,
                                subject,
                                format!("The tools of {slug} could not be listed: {error}"),
                                catalog_url.clone(),
                            );
                            continue;
                        }
                    };
                    for name in names.iter().filter(|n| *n != "*" && !offered.contains(n)) {
                        let hint = nearest(name, offered.iter().map(String::as_str))
                            .map(|n| format!("; did you mean {n}?"))
                            .unwrap_or_default();
                        found.push(
                            Code::ToolNotOffered,
                            subject,
                            format!("Connection {slug} offers no tool {name}{hint}"),
                            catalog_url.clone(),
                        );
                    }
                    refs.offered.insert(slug.to_owned(), offered);
                }
            }
        }
        if refs
            .connections
            .get(slug)
            .is_some_and(|other| other.id != connection.id)
        {
            found.invalid(
                subject,
                format!("Connection {slug} is a different connection in each team using it; give them distinct names"),
            );
            continue;
        }
        refs.connections.insert(slug.to_owned(), connection);
    }

    // An agent's connection carries one alias, so a new alias must not displace another one on
    // the same connection; that alias goes only by its own `delete`.
    for agent in &request.agents {
        let Some(row) = existing.get(agent.key.as_str()) else {
            continue;
        };
        let aliases = crate::inference::db::agent_aliases_all(&pool.get().await?, row.id).await?;
        for spec in agent.inference.iter().filter(|a| !a.delete) {
            let Some(connection) = refs.connections.get(&spec.connection.slug) else {
                continue;
            };
            let displaced = aliases.iter().find(|a| {
                a.connection_id == connection.id
                    && a.alias != spec.alias
                    && !agent
                        .inference
                        .iter()
                        .any(|d| d.delete && d.alias == a.alias)
            });
            if let Some(other) = displaced {
                found.invalid(
                    format!("{}/inference/{}", agent.key, spec.alias),
                    format!(
                        "Connection {} already has alias {} on this agent; declare that alias with delete: true to replace it",
                        connection.slug, other.alias
                    ),
                );
            }
        }
    }

    // Secrets blueprint environments reference: a person sets each one in the app.
    let mut seen = HashSet::new();
    for blueprint in request.agents.iter().filter_map(|a| a.sandbox.as_option()) {
        if !seen.insert(&blueprint.name) {
            continue;
        }
        let mut names: Vec<&String> = blueprint.env.keys().collect();
        names.sort();
        for variable in names {
            let value = &blueprint.env[variable];
            let Some(m::env_value::Value::Secret(reference)) = &value.value else {
                continue;
            };
            let name = reference.name.as_str();
            let subject = format!("sandbox {}/env/{variable}", blueprint.name);
            let key = (blueprint.name.clone(), name.to_owned());
            if value.delete || refs.secrets.contains_key(&key) {
                continue;
            }
            let _owning = blueprint_team(&blueprint.name);
            if !crate::secrets::valid_name(name) {
                found.invalid(&subject, format!("{name} is not a valid secret name"));
                continue;
            }
            let secret = crate::secrets::db::secret_by_name_opt(&pool.get().await?, name).await?;
            let link = format!("/connections/secrets?add={}", encode(name));
            match secret {
                Some(id) => {
                    refs.secrets.insert(key, id);
                }
                None => found.push(
                    Code::SecretMissing,
                    subject,
                    format!("Secret {name} does not exist; add it in Connections → Secrets"),
                    apply.link(&link),
                ),
            }
        }
    }

    // Routines and deployments are validated as their services validate them.
    let denied = found.0.iter().any(|r| r.code == Code::PermissionDenied);
    let mut deployments = HashMap::new();
    for agent in &request.agents {
        for routine in agent.routines.iter().filter(|r| !r.delete) {
            if let Err(error) = check_routine(routine) {
                found.invalid(
                    format!("{}/routines/{}", agent.key, routine.name),
                    error.to_string(),
                );
            }
        }
        if let Some(spec) = agent.deployment.as_option().filter(|_| !denied) {
            let subject = format!("{}/deployment", agent.key);
            if spec.external_id.trim().is_empty() {
                found.invalid(&subject, "A deployment needs an external ID");
                continue;
            }
            let registration = crate::deployment::apply::registration(spec, request);
            match apply.deployments.prepare(registration).await {
                Ok(prepared) => {
                    deployments.insert(agent.key.clone(), prepared);
                }
                Err(Error::Invalid(message)) => found.invalid(&subject, message),
                Err(other) => return Err(other.into()),
            }
        }
    }
    Ok(Checked {
        refs,
        required: found.0,
        deployments,
    })
}

/// A routine's own fields and cron schedule; its signal is checked with its connection.
fn check_routine(spec: &m::RoutineSpec) -> Result<(), Error> {
    use crate::routines::{RoutineInput, Trigger};
    let trigger = match spec.trigger.as_option().and_then(|t| t.kind.as_ref()) {
        Some(m::routine_trigger_spec::Kind::Cron(cron)) => Trigger::Cron {
            schedule: cron.schedule.clone(),
        },
        Some(m::routine_trigger_spec::Kind::Signal(signal)) => Trigger::Signal {
            connection_id: Uuid::nil(),
            signal_type: signal.signal_type.clone(),
            filters: signal
                .filters
                .iter()
                .map(|f| crate::routines::Filter {
                    path: f.key.clone(),
                    values: f.values.clone(),
                })
                .collect(),
        },
        None => {
            return Err(Error::Invalid(
                "A routine needs a cron or a signal trigger".into(),
            ));
        }
    };
    crate::routines::check(&RoutineInput {
        name: spec.name.clone(),
        prompt: spec.prompt.clone(),
        thread_title: spec.thread_title.clone(),
        enabled: spec.enabled.unwrap_or(false),
        trigger,
    })
    .map(|_| ())
}
