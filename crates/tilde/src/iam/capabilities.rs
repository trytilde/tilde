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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reach {
    #[default]
    None,
    Any,
    Only {
        ids: Vec<String>,
    },
}
impl Reach {
    pub fn permits(&self, id: &str) -> bool {
        match self {
            Self::None => false,
            Self::Any => true,
            Self::Only { ids } => ids.iter().any(|v| v == id),
        }
    }
    fn subset_of(&self, ceiling: &Self) -> bool {
        match self {
            Self::None => true,
            Self::Any => matches!(ceiling, Self::Any),
            Self::Only { ids } => ids.iter().all(|id| ceiling.permits(id)),
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Capabilities(pub BTreeMap<Capability, Reach>);
impl Capabilities {
    pub fn grants(&self, action: Capability) -> bool {
        match self.0.get(&action) {
            Some(Reach::Any) => true,
            Some(Reach::Only { ids }) => !ids.is_empty(),
            _ => false,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.0.keys().all(|action| !self.grants(*action))
    }

    pub fn permits(&self, action: Capability, id: &str) -> bool {
        self.0.get(&action).is_some_and(|scope| scope.permits(id))
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
                    let other = ceiling.0.get(action).unwrap_or(&Reach::None);
                    let narrowed = match (scope, other) {
                        (Reach::None, _) | (_, Reach::None) => Reach::None,
                        (Reach::Any, other) => other.clone(),
                        (scope, Reach::Any) => scope.clone(),
                        (Reach::Only { ids }, other) => Reach::Only {
                            ids: ids.iter().filter(|id| other.permits(id)).cloned().collect(),
                        },
                    };
                    (*action, narrowed)
                })
                .collect(),
        )
    }
    pub fn subset_of(&self, ceiling: &Self) -> bool {
        self.0
            .iter()
            .all(|(action, scope)| scope.subset_of(ceiling.0.get(action).unwrap_or(&Reach::None)))
    }
    pub fn validate(&self) -> Result<(), Error> {
        let mut total = 0;
        for (action, scope) in &self.0 {
            if let Reach::Only { ids } = scope {
                if matches!(
                    action,
                    Capability::AgentsCreate
                        | Capability::ThreadRead
                        | Capability::WorkRead
                        | Capability::WorkWrite
                        | Capability::RunUpdate
                ) {
                    return Err(Error::Invalid(
                        "This capability supports only none or any within the invocation scope"
                            .into(),
                    ));
                }
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
    pub fn from_wire(value: types::Capabilities) -> Result<Self, Error> {
        let mut map = BTreeMap::new();
        for (name, scope) in value.grants {
            let action = serde_json::from_value(serde_json::Value::String(name))
                .map_err(|_| Error::Invalid("Unknown capability".into()))?;
            let reach = match scope.mode.as_str() {
                "none" if scope.ids.is_empty() => Reach::None,
                "any" if scope.ids.is_empty() => Reach::Any,
                "only" => Reach::Only { ids: scope.ids },
                _ => return Err(Error::Invalid("Invalid capability scope".into())),
            };
            map.insert(action, reach);
        }
        let result = Self(map);
        result.validate()?;
        Ok(result)
    }
    pub fn wire(&self) -> types::Capabilities {
        types::Capabilities {
            grants: self
                .0
                .iter()
                .map(|(action, scope)| {
                    let name = serde_json::to_value(action)
                        .expect("capability name")
                        .as_str()
                        .unwrap()
                        .to_owned();
                    let (mode, ids) = match scope {
                        Reach::None => ("none", vec![]),
                        Reach::Any => ("any", vec![]),
                        Reach::Only { ids } => ("only", ids.clone()),
                    };
                    (
                        name,
                        types::CapabilityScope {
                            mode: mode.into(),
                            ids,
                            ..Default::default()
                        },
                    )
                })
                .collect(),
            ..Default::default()
        }
    }
}
