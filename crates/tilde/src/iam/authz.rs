//! Management authorization, AWS style: a role is a named set of statements, each an action
//! on one resource or on every resource of a kind, and a principal (user, group or API key)
//! holds roles. Actions do not imply each other; a role that should read as well as deploy
//! states both. Every agent gets three system roles when it is created, reader (view), editor
//! (view, edit, share) and deployer (view, deploy, share), never edited. Installation-wide
//! roles cover every agent through the nil resource id and are assigned by administrators.
//! Administrators hold everything. API keys hold roles exactly as users do, but join no groups
//! and never administer.
//!
//! Assigning a role is bounded by reach: the caller must hold `share` on each resource the
//! role names and every action it gives there. The SQL function `iam_held` answers point
//! checks and list filters alike. Resources without roles of their own (deployments, channel
//! access, connections, logs, traces) are checked against their agent.
use super::db;
use crate::database::{GenericClient, Pool};
use crate::proto::tilde::types::v1 as types;
use connectrpc::{ConnectError, RequestContext};
use std::collections::HashMap;
use uuid::Uuid;

pub const ADMIN_GROUP: &str = "tilde_system:admin";
/// Every signed-in user. Implicit, so a role given to it means "everyone".
pub const USER_GROUP: &str = "tilde_system:user";
pub const SYSTEM_PREFIX: &str = "tilde_system:";
pub const EXTERNAL_PREFIX: &str = "external:";
pub const LOCAL_PREFIX: &str = "local:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Agent,
}
impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        (name == "agent").then_some(Self::Agent)
    }
    pub fn wire(self) -> types::ResourceKind {
        match self {
            Self::Agent => types::ResourceKind::RESOURCE_KIND_AGENT,
        }
    }
    /// The kind's vocabulary; the only place a new action is declared.
    pub fn actions(self) -> &'static [Action] {
        match self {
            Self::Agent => &[Action::View, Action::Edit, Action::Deploy, Action::Share],
        }
    }
    /// The roles every resource of the kind is created with.
    pub fn system_roles(self) -> &'static [SystemRole] {
        match self {
            Self::Agent => &[SystemRole::Reader, SystemRole::Editor, SystemRole::Deployer],
        }
    }
    pub fn action(self, name: &str) -> Option<Action> {
        self.actions().iter().copied().find(|a| a.name() == name)
    }
}

/// One thing a caller may do to a resource. Independent of each other: holding `deploy`
/// says nothing about `view`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Find the resource and read everything about it.
    View,
    /// Change it.
    Edit,
    /// Register, promote and retire deployments and issue their tokens.
    Deploy,
    /// Assign and revoke roles on it, within the caller's own reach.
    Share,
}
impl Action {
    /// The stored and wire name.
    pub fn name(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Edit => "edit",
            Self::Deploy => "deploy",
            Self::Share => "share",
        }
    }
}

/// The roles a resource is created with. Ids are `agent/<id>/<role>`, and `agents/<role>` for
/// the installation-wide ones seeded by the migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemRole {
    Reader,
    Editor,
    Deployer,
}
impl SystemRole {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Reader => "reader",
            Self::Editor => "editor",
            Self::Deployer => "deployer",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Reader => "Reader",
            Self::Editor => "Editor",
            Self::Deployer => "Deployer",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Reader => "Can view the agent, its deployments, sessions, traces and logs.",
            Self::Editor => {
                "Can change the agent's settings, connections and identity access, and share it."
            }
            Self::Deployer => {
                "Can register, promote and retire deployments, issue deployment tokens, and share deployer access."
            }
        }
    }
    pub fn actions(self) -> &'static [Action] {
        match self {
            Self::Reader => &[Action::View],
            Self::Editor => &[Action::View, Action::Edit, Action::Share],
            Self::Deployer => &[Action::View, Action::Deploy, Action::Share],
        }
    }
    pub fn id(self, resource: Resource) -> String {
        if resource.is_all() {
            format!("{}s/{}", resource.kind.as_str(), self.slug())
        } else {
            format!("{}/{}/{}", resource.kind.as_str(), resource.id, self.slug())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resource {
    pub kind: Kind,
    pub id: Uuid,
}
impl Resource {
    pub fn agent(id: Uuid) -> Self {
        Self {
            kind: Kind::Agent,
            id,
        }
    }
    /// Every resource of the kind, present and future. Stored as the nil id, which `iam_held`
    /// matches for any target. Only administrators assign roles that reach it.
    pub fn all(kind: Kind) -> Self {
        Self {
            kind,
            id: Uuid::nil(),
        }
    }
    pub fn is_all(self) -> bool {
        self.id.is_nil()
    }
}

/// Who a role is given to. Wire form is `types::Principal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grantee {
    Group(String),
    User(Uuid),
    ApiKey(Uuid),
}
impl Grantee {
    pub fn parse(value: Option<types::Principal>) -> Result<Self, ConnectError> {
        let value = value.ok_or_else(|| ConnectError::invalid_argument("Principal required"))?;
        Ok(match value.r#type.as_known() {
            Some(types::PrincipalType::User) => Self::User(super::parse_id(&value.id)?),
            Some(types::PrincipalType::ApiKey) => Self::ApiKey(super::parse_id(&value.id)?),
            Some(types::PrincipalType::Group) if !value.id.is_empty() && value.id.len() <= 200 => {
                Self::Group(value.id)
            }
            _ => return Err(ConnectError::invalid_argument("Principal type required")),
        })
    }
    pub fn wire(&self) -> types::Principal {
        let (kind, id) = match self {
            Self::Group(id) => (types::PrincipalType::Group, id.clone()),
            Self::User(id) => (types::PrincipalType::User, id.to_string()),
            Self::ApiKey(id) => (types::PrincipalType::ApiKey, id.to_string()),
        };
        types::Principal {
            r#type: kind.into(),
            id,
            ..Default::default()
        }
    }
    /// The membership table's three nullable columns; exactly one is set.
    pub fn columns(&self) -> (Option<&str>, Option<Uuid>, Option<Uuid>) {
        match self {
            Self::Group(id) => (Some(id), None, None),
            Self::User(id) => (None, Some(*id), None),
            Self::ApiKey(id) => (None, None, Some(*id)),
        }
    }
}

/// The authenticated management caller, resolved once per request by the management guard.
#[derive(Debug, Clone)]
pub struct Access {
    pub user: Option<Uuid>,
    pub api_key: Option<Uuid>,
    /// Users only; keys join no groups.
    pub groups: Vec<String>,
    /// Users only; a key is never an administrator.
    pub admin: bool,
}
impl Access {
    pub fn user(id: Uuid, mut groups: Vec<String>) -> Self {
        groups.push(USER_GROUP.into());
        Self {
            admin: groups.iter().any(|g| g == ADMIN_GROUP),
            user: Some(id),
            api_key: None,
            groups,
        }
    }
    pub fn api_key(id: Uuid) -> Self {
        Self {
            admin: false,
            user: None,
            api_key: Some(id),
            groups: Vec::new(),
        }
    }
    /// Engine-internal reads that are authorized elsewhere, such as invocation capabilities.
    pub fn system() -> Self {
        Self {
            user: None,
            api_key: None,
            groups: Vec::new(),
            admin: true,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    /// Visible but the action is not held: permission denied.
    Forbidden,
    /// Not visible: indistinguishable from a resource that does not exist.
    Hidden,
}
/// What one check found: the action asked for, and `view`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Held {
    pub held: bool,
    pub visible: bool,
}
impl Held {
    pub const ALL: Self = Self {
        held: true,
        visible: true,
    };
}
pub fn decide(found: Held) -> Decision {
    match found {
        Held { held: true, .. } => Decision::Allow,
        Held { visible: true, .. } => Decision::Forbidden,
        _ => Decision::Hidden,
    }
}

/// Database handle for point checks; placed in request extensions beside `Access`.
#[derive(Clone)]
pub struct Authz(pub(super) Pool);

fn failure(error: crate::database::DbError) -> ConnectError {
    tracing::error!(?error, "Authorization lookup failed");
    ConnectError::internal("Authorization lookup failed")
}

impl Authz {
    pub fn new(pool: Pool) -> Self {
        Self(pool)
    }
    /// One action on many resources of its kind.
    pub async fn held_many(
        &self,
        access: &Access,
        kind: Kind,
        action: Action,
        ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Held>, ConnectError> {
        if access.admin {
            return Ok(ids.iter().map(|id| (*id, Held::ALL)).collect());
        }
        let rows = db::held(
            &self.0.get().await.map_err(failure)?,
            kind,
            action,
            ids,
            access,
        )
        .await
        .map_err(failure)?;
        Ok(rows.into_iter().collect())
    }
    pub async fn held(
        &self,
        access: &Access,
        resource: Resource,
        action: Action,
    ) -> Result<Held, ConnectError> {
        Ok(self
            .held_many(access, resource.kind, action, &[resource.id])
            .await?
            .remove(&resource.id)
            .unwrap_or_default())
    }
    /// Every action of the resource's kind the caller holds.
    pub async fn actions(
        &self,
        access: &Access,
        resource: Resource,
    ) -> Result<Vec<Action>, ConnectError> {
        if access.admin {
            return Ok(resource.kind.actions().to_vec());
        }
        db::actions_held(&self.0.get().await.map_err(failure)?, resource, access)
            .await
            .map_err(failure)
    }
    pub async fn check(
        &self,
        access: &Access,
        resource: Resource,
        action: Action,
    ) -> Result<(), ConnectError> {
        match decide(self.held(access, resource, action).await?) {
            Decision::Allow => Ok(()),
            Decision::Forbidden => Err(ConnectError::permission_denied(format!(
                "Requires {} on this {}",
                action.name(),
                resource.kind.as_str()
            ))),
            Decision::Hidden => Err(ConnectError::not_found("Not found")),
        }
    }
    /// Whether the caller may assign or revoke the role: an administrator, or someone holding
    /// `share` on every resource the role names and every action it gives there. Roles that
    /// reach every resource of a kind are an administrator's to give.
    pub async fn require_reach(&self, access: &Access, role: &str) -> Result<(), ConnectError> {
        let client = self.0.get().await.map_err(failure)?;
        let statements = db::role_statements(&client, role).await.map_err(failure)?;
        if statements.is_empty() {
            return Err(ConnectError::not_found("Role not found"));
        }
        if access.admin {
            return Ok(());
        }
        for (resource, action) in statements {
            if resource.is_all() {
                return Err(ConnectError::permission_denied(
                    "Roles over every resource are assigned by administrators",
                ));
            }
            self.check(access, resource, Action::Share).await?;
            if decide(self.held(access, resource, action).await?) != Decision::Allow {
                return Err(ConnectError::permission_denied(format!(
                    "Assigning this role requires holding {} yourself",
                    action.name()
                )));
            }
        }
        Ok(())
    }
}

/// Creates the resource's system roles and, for a user, makes them its editor and deployer.
/// Runs inside the creating transaction so no resource exists without an owner. A key
/// creates only when it already edits the kind, so it needs nothing.
pub async fn create_roles(
    tx: &impl GenericClient,
    resource: Resource,
    creator: &Access,
) -> Result<(), crate::database::DbError> {
    for role in resource.kind.system_roles() {
        let id = role.id(resource);
        db::role_create(tx, &id, role.name(), role.description()).await?;
        for action in role.actions() {
            db::role_statement_put(tx, &id, resource, *action).await?;
        }
        if let Some(user) = creator.user
            && *role != SystemRole::Reader
        {
            db::role_member_put(tx, &id, &Grantee::User(user), creator).await?;
        }
    }
    Ok(())
}

pub fn caller(ctx: &RequestContext) -> Result<&Access, ConnectError> {
    ctx.extensions()
        .get::<Access>()
        .ok_or_else(|| ConnectError::unauthenticated("Management credentials required"))
}
pub fn authz(ctx: &RequestContext) -> Result<&Authz, ConnectError> {
    ctx.extensions()
        .get::<Authz>()
        .ok_or_else(|| ConnectError::unauthenticated("Management credentials required"))
}
pub fn require_admin(ctx: &RequestContext) -> Result<&Access, ConnectError> {
    let access = caller(ctx)?;
    if access.admin {
        Ok(access)
    } else {
        Err(ConnectError::permission_denied("Administrator required"))
    }
}
/// Keys never manage access, users, groups, keys or provider definitions, whatever they hold.
pub fn require_user(ctx: &RequestContext) -> Result<&Access, ConnectError> {
    let access = caller(ctx)?;
    if access.api_key.is_some() {
        Err(ConnectError::permission_denied(
            "API keys cannot perform this operation",
        ))
    } else {
        Ok(access)
    }
}
/// The one check handlers call before touching a resource.
pub async fn require(
    ctx: &RequestContext,
    resource: Resource,
    action: Action,
) -> Result<&Access, ConnectError> {
    let access = caller(ctx)?;
    authz(ctx)?.check(access, resource, action).await?;
    Ok(access)
}
/// Any signed-in user may create; they then own what they made. A key must edit the whole
/// kind, since nothing would otherwise tie what it creates to it.
pub async fn require_create(ctx: &RequestContext, kind: Kind) -> Result<&Access, ConnectError> {
    let access = caller(ctx)?;
    if access.api_key.is_some() {
        authz(ctx)?
            .check(access, Resource::all(kind), Action::Edit)
            .await
            .map_err(|_| {
                ConnectError::permission_denied(format!(
                    "Creating a {} requires edit access to all of them",
                    kind.as_str()
                ))
            })?;
    }
    Ok(access)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_roles_read_and_are_named_by_their_resource() {
        let agent = Resource::agent(Uuid::nil().max(Uuid::from_u128(7)));
        for role in Kind::Agent.system_roles() {
            assert!(role.actions().contains(&Action::View));
            assert_eq!(
                role.id(agent),
                format!("agent/{}/{}", agent.id, role.slug())
            );
            assert_eq!(
                role.id(Resource::all(Kind::Agent)),
                format!("agents/{}", role.slug())
            );
        }
        assert!(!SystemRole::Reader.actions().contains(&Action::Share));
        assert!(SystemRole::Deployer.actions().contains(&Action::Share));
        assert_eq!(Kind::Agent.action("deploy"), Some(Action::Deploy));
        assert_eq!(Kind::Agent.action("approve"), None);
    }
    #[test]
    fn held_action_allows_visible_forbids_and_nothing_is_hidden() {
        use Decision::*;
        for (found, expected) in [
            (Held::ALL, Allow),
            (
                Held {
                    held: false,
                    visible: true,
                },
                Forbidden,
            ),
            (Held::default(), Hidden),
        ] {
            assert_eq!(decide(found), expected, "{found:?}");
        }
    }
    #[test]
    fn only_users_in_the_administrators_group_administer() {
        let member = Access::user(Uuid::nil(), vec!["local:ops".into()]);
        assert!(!member.admin);
        assert!(member.groups.iter().any(|g| g == USER_GROUP));
        assert!(Access::user(Uuid::nil(), vec![ADMIN_GROUP.into()]).admin);
        let key = Access::api_key(Uuid::nil());
        assert!(!key.admin && key.groups.is_empty());
    }
}
