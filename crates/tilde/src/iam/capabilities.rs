//! Closed, typed capability map. Missing grants deny; a token can only narrow stored grants.
use crate::error::Error;
use crate::proto::tilde::types::v1 as types;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    #[serde(rename = "agents.read")]
    AgentsRead,
    #[serde(rename = "agents.create")]
    AgentsCreate,
    #[serde(rename = "agents.update")]
    AgentsUpdate,
    #[serde(rename = "agents.delete")]
    AgentsDelete,
    #[serde(rename = "agents.invoke")]
    AgentsInvoke,
    #[serde(rename = "agents.grant_capabilities")]
    AgentsGrant,
    #[serde(rename = "thread.read")]
    ThreadRead,
    #[serde(rename = "work.read")]
    WorkRead,
    #[serde(rename = "work.write")]
    WorkWrite,
    #[serde(rename = "run.update")]
    RunUpdate,
    #[serde(rename = "tools.invoke")]
    ToolsInvoke,
}
impl Capability {
    /// Targeted actions select agents or tools; all other actions are yes/no permissions.
    pub fn is_targeted(self) -> bool {
        !matches!(
            self,
            Self::AgentsCreate
                | Self::ThreadRead
                | Self::WorkRead
                | Self::WorkWrite
                | Self::RunUpdate
        )
    }

    fn denied(self) -> Reach {
        if self.is_targeted() {
            Reach::None
        } else {
            Reach::No
        }
    }
}

/// The public permission vocabulary, shared by storage, signed claims and RPCs.
/// `No`/`Yes` apply to binary actions; `None`/`All`/`Selected` apply to targeted actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reach {
    No,
    Yes,
    None,
    All,
    Selected { ids: Vec<String> },
}
impl Reach {
    fn supports(&self, action: Capability) -> bool {
        match self {
            Self::No | Self::Yes => !action.is_targeted(),
            Self::None | Self::All | Self::Selected { .. } => action.is_targeted(),
        }
    }

    pub fn permits(&self, id: &str) -> bool {
        match self {
            Self::No | Self::None => false,
            Self::Yes | Self::All => true,
            Self::Selected { ids } => ids.iter().any(|v| v == id),
        }
    }
    fn subset_of(&self, ceiling: &Self) -> bool {
        match self {
            Self::No | Self::None => true,
            Self::Yes => matches!(ceiling, Self::Yes),
            Self::All => matches!(ceiling, Self::All),
            Self::Selected { ids } => ids.iter().all(|id| ceiling.permits(id)),
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Capabilities(pub BTreeMap<Capability, Reach>);
impl Capabilities {
    pub fn grants(&self, action: Capability) -> bool {
        match self.0.get(&action).filter(|scope| scope.supports(action)) {
            Some(Reach::Yes | Reach::All) => true,
            Some(Reach::Selected { ids }) => !ids.is_empty(),
            _ => false,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.0.keys().all(|action| !self.grants(*action))
    }

    pub fn permits(&self, action: Capability, id: &str) -> bool {
        self.0
            .get(&action)
            .is_some_and(|scope| scope.supports(action) && scope.permits(id))
    }
    pub fn require(&self, action: Capability, id: &str) -> Result<(), connectrpc::ConnectError> {
        if self.permits(action, id) {
            Ok(())
        } else {
            Err(connectrpc::ConnectError::permission_denied(
                "Capability does not permit this operation",
            ))
        }
    }
    /// Renewal never acquires grants that were absent from the original invocation credential.
    pub fn intersect(&self, ceiling: &Self) -> Self {
        Self(
            self.0
                .iter()
                .map(|(action, scope)| {
                    let denied = action.denied();
                    let other = ceiling.0.get(action).unwrap_or(&denied);
                    let narrowed = if !scope.supports(*action) || !other.supports(*action) {
                        denied
                    } else {
                        match (scope, other) {
                            (Reach::Yes, Reach::Yes) => Reach::Yes,
                            (Reach::All, other) => other.clone(),
                            (scope, Reach::All) => scope.clone(),
                            (Reach::Selected { ids }, Reach::Selected { .. }) => Reach::Selected {
                                ids: ids.iter().filter(|id| other.permits(id)).cloned().collect(),
                            },
                            _ => denied,
                        }
                    };
                    (*action, narrowed)
                })
                .collect(),
        )
    }
    pub fn subset_of(&self, ceiling: &Self) -> bool {
        self.0.iter().all(|(action, scope)| {
            let denied = action.denied();
            let other = ceiling.0.get(action).unwrap_or(&denied);
            scope.supports(*action) && other.supports(*action) && scope.subset_of(other)
        })
    }
    pub fn validate(&self) -> Result<(), Error> {
        let mut total = 0;
        for (action, scope) in &self.0 {
            if !scope.supports(*action) {
                return Err(Error::Invalid(if action.is_targeted() {
                    "This capability supports none, all or selected targets".into()
                } else {
                    "This capability supports no or yes within the invocation scope".into()
                }));
            }
            if let Reach::Selected { ids } = scope {
                total += ids.len();
                for id in ids {
                    if id.is_empty()
                        || id.len() > 128
                        || (*action != Capability::ToolsInvoke
                            && uuid::Uuid::parse_str(id).is_err())
                    {
                        return Err(Error::Invalid("Invalid capability target ID".into()));
                    }
                }
            }
        }
        if total > 100 {
            return Err(Error::Invalid(
                "At most 100 capability targets are allowed".into(),
            ));
        }
        Ok(())
    }
    /// Decode the action-specific contract. Unknown enum values and invalid target lists fail closed.
    pub fn from_wire(value: types::Capabilities) -> Result<Self, Error> {
        use types::{BinaryPermission as Binary, TargetSelection as Selection};
        let mut map = BTreeMap::new();
        for (action, permission) in [
            (Capability::AgentsCreate, value.agents_create),
            (Capability::ThreadRead, value.thread_read),
            (Capability::WorkRead, value.work_read),
            (Capability::WorkWrite, value.work_write),
            (Capability::RunUpdate, value.run_update),
        ] {
            let reach = match permission.as_known() {
                Some(Binary::Unspecified) => continue,
                Some(Binary::No) => Reach::No,
                Some(Binary::Yes) => Reach::Yes,
                None => {
                    return Err(Error::Invalid(
                        "Unknown binary permission; choose No or Yes".into(),
                    ));
                }
            };
            map.insert(action, reach);
        }
        for (action, permission) in [
            (Capability::AgentsRead, value.agents_read),
            (Capability::AgentsUpdate, value.agents_update),
            (Capability::AgentsDelete, value.agents_delete),
            (Capability::AgentsInvoke, value.agents_invoke),
            (Capability::AgentsGrant, value.agents_grant_capabilities),
            (Capability::ToolsInvoke, value.tools_invoke),
        ] {
            let Some(permission) = permission.into_option() else {
                continue;
            };
            let reach =
                match permission.mode.as_known() {
                    Some(Selection::None) if permission.ids.is_empty() => Reach::None,
                    Some(Selection::All) if permission.ids.is_empty() => Reach::All,
                    Some(Selection::Selected) => Reach::Selected {
                        ids: permission.ids,
                    },
                    _ => return Err(Error::Invalid(
                        "Choose None, All or Selected; target IDs are allowed only with Selected"
                            .into(),
                    )),
                };
            map.insert(action, reach);
        }
        let result = Self(map);
        result.validate()?;
        Ok(result)
    }

    pub fn wire(&self) -> types::Capabilities {
        use types::{BinaryPermission as Binary, TargetSelection as Selection};
        let binary = |action| -> buffa::EnumValue<Binary> {
            match self.0.get(&action) {
                None => Binary::Unspecified.into(),
                Some(Reach::No) => Binary::No.into(),
                Some(Reach::Yes) => Binary::Yes.into(),
                // Preserve invalidity if an unchecked in-memory grant reaches the boundary.
                _ => (-1).into(),
            }
        };
        let target = |action| {
            let Some(reach) = self.0.get(&action) else {
                return Default::default();
            };
            let (mode, ids) = match reach {
                Reach::None => (Selection::None, vec![]),
                Reach::All => (Selection::All, vec![]),
                Reach::Selected { ids } => (Selection::Selected, ids.clone()),
                _ => (Selection::Unspecified, vec![]),
            };
            types::TargetPermission {
                mode: mode.into(),
                ids,
                ..Default::default()
            }
            .into()
        };
        types::Capabilities {
            agents_read: target(Capability::AgentsRead),
            agents_create: binary(Capability::AgentsCreate),
            agents_update: target(Capability::AgentsUpdate),
            agents_delete: target(Capability::AgentsDelete),
            agents_invoke: target(Capability::AgentsInvoke),
            agents_grant_capabilities: target(Capability::AgentsGrant),
            thread_read: binary(Capability::ThreadRead),
            work_read: binary(Capability::WorkRead),
            work_write: binary(Capability::WorkWrite),
            run_update: binary(Capability::RunUpdate),
            tools_invoke: target(Capability::ToolsInvoke),
            ..Default::default()
        }
    }
}
