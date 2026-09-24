//! Spend ceilings per agent or per chat identity. A worker re-sums priced requests for every
//! budget on a timer and flips `exhausted_until`; exhaustion changes fire the configuration
//! notification. Enforcement happens where authorization already happens: token issue and
//! renewal withhold every inference connection for a blocked scope, and a sidecar learns the
//! blocked scopes with its configuration. The request path never consults a budget.
//!
//! ponytail: spend is a SUM over `inference_requests` per settle; add a rollup when the table
//! is large enough to make a 30 s sum expensive.
use crate::{database::Pool, error::Error};
use std::time::Duration;
use uuid::Uuid;

pub const SETTLE_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Agent,
    Identity,
}
impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Identity => "identity",
        }
    }
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "agent" => Ok(Self::Agent),
            "identity" => Ok(Self::Identity),
            _ => Err(Error::Invalid("Unknown budget scope".into())),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Day,
    Month,
    Total,
}
impl Period {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Month => "month",
            Self::Total => "total",
        }
    }
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "day" => Ok(Self::Day),
            "month" => Ok(Self::Month),
            "total" => Ok(Self::Total),
            _ => Err(Error::Invalid("Unknown budget period".into())),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Block,
    Flag,
}
impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Flag => "flag",
        }
    }
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "block" => Ok(Self::Block),
            "flag" => Ok(Self::Flag),
            _ => Err(Error::Invalid("Unknown budget action".into())),
        }
    }
}
pub struct Budget {
    pub id: Uuid,
    pub scope: Scope,
    pub scope_id: Uuid,
    /// Agent budgets may cap one connection; `None` caps the agent's whole spend.
    pub connection_id: Option<Uuid>,
    pub period: Period,
    pub limit_micros: i64,
    pub action: Action,
    pub spent_micros: i64,
    pub exhausted_until: Option<chrono::DateTime<chrono::Utc>>,
}
impl Budget {
    fn from_row(r: super::db::BudgetRow) -> Result<Self, Error> {
        Ok(Self {
            id: r.id,
            scope: Scope::parse(&r.scope)?,
            scope_id: r.scope_id,
            connection_id: r.connection_id,
            period: Period::parse(&r.period)?,
            limit_micros: r.limit_micros,
            action: Action::parse(&r.action)?,
            spent_micros: r.spent_micros,
            exhausted_until: r.exhausted_until,
        })
    }
}
/// Which scopes are blocked right now; what a sidecar receives with its configuration.
#[derive(Default)]
pub struct Blocked {
    pub agents: Vec<Uuid>,
    pub identities: Vec<Uuid>,
    /// `(agent, connection)` pairs whose per-connection budget is exhausted.
    pub assignments: Vec<(Uuid, Uuid)>,
}
pub async fn blocked(pool: &Pool) -> Result<Blocked, Error> {
    let mut blocked = Blocked::default();
    for row in super::db::exhausted_all(&pool.get().await?).await? {
        match (row.scope.as_str(), row.connection_id) {
            ("agent", Some(connection)) => blocked.assignments.push((row.scope_id, connection)),
            ("agent", None) => blocked.agents.push(row.scope_id),
            _ => blocked.identities.push(row.scope_id),
        }
    }
    Ok(blocked)
}
pub async fn list(
    pool: &Pool,
    scope: Option<Scope>,
    scope_id: Option<Uuid>,
) -> Result<Vec<Budget>, Error> {
    super::db::budgets_list_all(&pool.get().await?, scope.map(Scope::as_str), scope_id)
        .await?
        .into_iter()
        .map(Budget::from_row)
        .collect()
}
pub async fn set(
    pool: &Pool,
    scope: Scope,
    scope_id: Uuid,
    connection_id: Option<Uuid>,
    period: Period,
    limit_micros: i64,
    action: Action,
) -> Result<Budget, Error> {
    if connection_id.is_some() && scope != Scope::Agent {
        return Err(Error::Invalid(
            "Only agent budgets can cap one connection".into(),
        ));
    }
    if scope == Scope::Identity
        && super::db::root_identity_exists_opt(&pool.get().await?, scope_id)
            .await?
            .is_none()
    {
        return Err(Error::Invalid(
            "Identity budgets target a root identity".into(),
        ));
    }
    if limit_micros < 0 {
        return Err(Error::Invalid("Budget limit cannot be negative".into()));
    }
    let row = super::db::budget_set_one(
        &pool.get().await?,
        Uuid::new_v4(),
        scope.as_str(),
        scope_id,
        connection_id,
        period.as_str(),
        limit_micros,
        action.as_str(),
    )
    .await?;
    // Settle immediately so a tightened limit takes effect before the next tick, and return
    // the settled row rather than the freshly written one.
    super::db::budgets_settle_all(&pool.get().await?).await?;
    list(pool, Some(scope), Some(scope_id))
        .await?
        .into_iter()
        .find(|b| b.id == row.id)
        .ok_or(Error::NotFound)
}
pub async fn delete(pool: &Pool, id: Uuid) -> Result<(), Error> {
    if super::db::budget_delete_execute(&pool.get().await?, id).await? == 0 {
        return Err(Error::NotFound);
    }
    Ok(())
}
/// Re-sum every budget; exhaustion flips notify configuration readers through the trigger.
pub async fn settle(pool: &Pool) -> Result<(), Error> {
    super::db::budgets_settle_all(&pool.get().await?).await?;
    Ok(())
}
pub async fn worker(pool: Pool, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    let mut tick = tokio::time::interval(SETTLE_INTERVAL);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if let Err(error) = settle(&pool).await {
                    tracing::warn!(%error, "Inference budget settlement failed");
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { break; }
            }
        }
    }
}
