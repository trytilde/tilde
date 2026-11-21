//! Declarative apply: agent specs written in code (`@trytilde/iac`), sent by `tilde deploy` to
//! `ApplyService.Apply`, become registry state in three steps.
//!
//! 1. Preflight (`preflight`) reads only. It validates the specs, resolves every referenced
//!    connection and checks it is ready, of the declared type and capable of each use, and
//!    returns every action a person must take first. Any action blocks the whole apply.
//! 2. One transaction for every agent of the request. Plain per-domain functions
//!    (`<domain>::apply`) upsert each resource by its key, whoever created it, in dependency
//!    order: blueprints, then per agent its fields, inference aliases, tool sources, sandbox,
//!    routines and deployment. Each returns the `Change`s it made.
//! 3. Commit, or for a dry run roll back: a plan runs exactly the writes an apply would.
//!
//! Nothing is removed because a spec omits it; an entry with `delete` removes its resource. A
//! resource's own settings converge (a tool source's tool list, a blueprint's environment).
mod preflight;
pub mod rpc;

use crate::agent::Agents;
use crate::error::Error;
use crate::proto::tilde::management::v1 as m;
use secrecy::SecretString;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Create,
    Update,
    Unchanged,
    Remove,
}
/// One field of a change. A sealed field's values are secret and never reported.
#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub before: String,
    pub after: String,
    pub sealed: bool,
}
/// What an apply did, or a plan would do, to one resource.
#[derive(Debug)]
pub struct Change {
    pub kind: &'static str,
    pub key: String,
    pub action: Action,
    pub fields: Vec<Field>,
    pub note: String,
}
impl Change {
    fn new(kind: &'static str, key: String, action: Action) -> Self {
        Self {
            kind,
            key,
            action,
            fields: Vec::new(),
            note: String::new(),
        }
    }
    pub fn create(kind: &'static str, key: impl Into<String>) -> Self {
        Self::new(kind, key.into(), Action::Create)
    }
    pub fn remove(kind: &'static str, key: impl Into<String>) -> Self {
        Self::new(kind, key.into(), Action::Remove)
    }
    /// An existing resource: unchanged until a field differs.
    pub fn existing(kind: &'static str, key: impl Into<String>) -> Self {
        Self::new(kind, key.into(), Action::Unchanged)
    }
    /// Records the field when it differs (always, on a create).
    pub fn field(&mut self, name: impl Into<String>, before: impl ToString, after: impl ToString) {
        let (before, after) = (before.to_string(), after.to_string());
        if self.action == Action::Create || before != after {
            self.push(name.into(), before, after, false);
        }
    }
    /// A secret field: only whether it changed is recorded.
    pub fn sealed(&mut self, name: impl Into<String>, changed: bool) {
        if self.action == Action::Create || changed {
            self.push(name.into(), String::new(), String::new(), true);
        }
    }
    fn push(&mut self, name: String, before: String, after: String, sealed: bool) {
        if self.action == Action::Unchanged {
            self.action = Action::Update;
        }
        self.fields.push(Field {
            name,
            before,
            after,
            sealed,
        });
    }
    pub fn changed(&self) -> bool {
        self.action != Action::Unchanged
    }
}

/// Something a person must do before the apply can run, and where to do it.
#[derive(Debug)]
pub struct Required {
    pub code: m::RequiredActionCode,
    pub subject: String,
    pub message: String,
    pub url: String,
}

/// A connection a spec references, as preflight resolved it.
#[derive(Debug, Clone)]
pub struct Connection {
    pub id: Uuid,
    pub slug: String,
    pub provider_id: String,
    pub type_id: String,
}
/// The references of one apply, all resolved and checked by preflight.
#[derive(Default)]
pub struct Refs {
    connections: HashMap<String, Connection>,
    /// The tools each tool source's connection offers, by slug.
    offered: HashMap<String, Vec<String>>,
    /// Secrets blueprint environments reference, by blueprint and secret name.
    secrets: HashMap<(String, String), Uuid>,
}
impl Refs {
    pub fn connection(&self, reference: &m::ConnectionRef) -> Result<&Connection, Error> {
        self.connections.get(&reference.slug).ok_or_else(|| {
            Error::Invalid(format!("Connection {} was not resolved", reference.slug))
        })
    }
    pub fn secret(&self, blueprint: &str, name: &str) -> Result<Uuid, Error> {
        self.secrets
            .get(&(blueprint.to_owned(), name.to_owned()))
            .copied()
            .ok_or_else(|| Error::Invalid(format!("Secret {name} was not resolved")))
    }
    pub fn slug_of(&self, id: Uuid) -> Option<&str> {
        self.connections
            .values()
            .find(|c| c.id == id)
            .map(|c| c.slug.as_str())
    }
    pub fn offered(&self, reference: &m::ConnectionRef) -> &[String] {
        self.offered.get(&reference.slug).map_or(&[], Vec::as_slice)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Planned,
    Applied,
    Blocked,
}
/// An agent the apply wrote, with the deployment it registered, if any.
pub struct Applied {
    pub key: String,
    pub id: Uuid,
    pub deployment: Option<(Uuid, SecretString)>,
}
pub struct Report {
    pub outcome: Outcome,
    pub changes: Vec<Change>,
    pub required: Vec<Required>,
    pub agents: Vec<Applied>,
}

#[derive(Clone)]
pub struct Apply {
    agents: Agents,
    tools: crate::tools::Tools,
    deployments: crate::deployment::Deployments,
    /// The app's origin required-action links start with; empty leaves them relative.
    app_url: String,
}
impl Apply {
    pub fn new(
        agents: Agents,
        tools: crate::tools::Tools,
        deployments: crate::deployment::Deployments,
        app_url: String,
    ) -> Self {
        Self {
            agents,
            tools,
            deployments,
            app_url: app_url.trim_end_matches('/').to_owned(),
        }
    }
    fn link(&self, path: &str) -> String {
        format!("{}{path}", self.app_url)
    }

    pub async fn apply(
        &self,
        request: &m::ApplyRequest,
    ) -> Result<Report, connectrpc::ConnectError> {
        let checked = preflight::check(self, request).await?;
        if !checked.required.is_empty() {
            return Ok(Report {
                outcome: Outcome::Blocked,
                changes: Vec::new(),
                required: checked.required,
                agents: Vec::new(),
            });
        }
        let refs = &checked.refs;
        let mut prepared = checked.deployments;
        let agents = self.agents.clone();
        let crypto = &self.tools.connections.crypto;
        let mut client = self.tools.connections.pool.get().await?;
        let tx = client.transaction().await.map_err(Error::from)?;
        let mut changes = Vec::new();
        let mut blueprints = HashMap::new();
        for spec in request.agents.iter().filter_map(|a| a.sandbox.as_option()) {
            if blueprints.contains_key(&spec.name) {
                continue;
            }
            let (id, blueprint_changes) =
                crate::sandboxes::apply::blueprint(&tx, crypto, refs, spec).await?;
            changes.extend(blueprint_changes);
            blueprints.insert(spec.name.clone(), id);
        }
        let mut applied = Vec::new();
        for spec in &request.agents {
            let (agent, change) = crate::agent::apply::agent(&tx, &agents, spec).await?;
            changes.push(change);
            let key = spec.key.as_str();
            changes.extend(
                crate::inference::apply::aliases(&tx, agent, key, refs, &spec.inference).await?,
            );
            changes.extend(crate::tools::apply::sources(&tx, agent, key, refs, &spec.tools).await?);
            if let Some(blueprint) = spec.sandbox.as_option() {
                changes.push(
                    crate::sandboxes::apply::agent_sandbox(
                        &tx,
                        agent,
                        key,
                        &blueprint.name,
                        blueprints[&blueprint.name],
                    )
                    .await?,
                );
            }
            changes.extend(
                crate::routines::apply::routines(&tx, agent, key, refs, &spec.routines).await?,
            );
            let mut deployment = None;
            if let Some(input) = prepared.remove(key) {
                let (change, registered) = self.deployments.apply(&tx, agent, key, input).await?;
                changes.push(change);
                deployment = registered;
            }
            applied.push(Applied {
                key: spec.key.clone(),
                id: agent,
                deployment,
            });
        }
        let outcome = if request.dry_run {
            tx.rollback().await.map_err(Error::from)?;
            applied.clear();
            Outcome::Planned
        } else {
            tx.commit().await.map_err(Error::from)?;
            Outcome::Applied
        };
        Ok(Report {
            outcome,
            changes,
            required: Vec::new(),
            agents: applied,
        })
    }
}
