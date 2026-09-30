//! Management surface for inference spend and budgets.
use super::budgets::{self, Action, Period, Scope};
use crate::database::Pool;
use crate::error::Error;
use crate::proto::tilde::management::v1 as management;
use crate::services::tilde::management::v1::InferenceService;
use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use std::sync::Arc;
use uuid::Uuid;

struct Rpc(Pool);
/// Mount `InferenceService` for the management listener; every RPC reads or writes Postgres.
pub fn router(pool: Pool) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(pool))),
        256 * 1024,
    )
}
fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Invalid UUID".into()))
}
fn timestamp(date: chrono::DateTime<chrono::Utc>) -> buffa_types::google::protobuf::Timestamp {
    buffa_types::google::protobuf::Timestamp {
        seconds: date.timestamp(),
        nanos: date.timestamp_subsec_nanos() as i32,
        ..Default::default()
    }
}
fn scope_model(value: buffa::EnumValue<management::BudgetScope>) -> Result<Scope, Error> {
    if value == management::BudgetScope::Agent {
        Ok(Scope::Agent)
    } else if value == management::BudgetScope::Identity {
        Ok(Scope::Identity)
    } else {
        Err(Error::Invalid("Budget scope required".into()))
    }
}
fn period_model(value: buffa::EnumValue<management::BudgetPeriod>) -> Result<Period, Error> {
    if value == management::BudgetPeriod::Day {
        Ok(Period::Day)
    } else if value == management::BudgetPeriod::Month {
        Ok(Period::Month)
    } else if value == management::BudgetPeriod::Total {
        Ok(Period::Total)
    } else {
        Err(Error::Invalid("Budget period required".into()))
    }
}
fn action_model(value: buffa::EnumValue<management::BudgetAction>) -> Result<Action, Error> {
    if value == management::BudgetAction::Block {
        Ok(Action::Block)
    } else if value == management::BudgetAction::Flag {
        Ok(Action::Flag)
    } else {
        Err(Error::Invalid("Budget action required".into()))
    }
}
fn budget_wire(b: budgets::Budget) -> management::Budget {
    management::Budget {
        id: b.id.to_string(),
        scope: match b.scope {
            Scope::Agent => management::BudgetScope::Agent,
            Scope::Identity => management::BudgetScope::Identity,
        }
        .into(),
        scope_id: b.scope_id.to_string(),
        period: match b.period {
            Period::Day => management::BudgetPeriod::Day,
            Period::Month => management::BudgetPeriod::Month,
            Period::Total => management::BudgetPeriod::Total,
        }
        .into(),
        limit_micros: b.limit_micros,
        action: match b.action {
            Action::Block => management::BudgetAction::Block,
            Action::Flag => management::BudgetAction::Flag,
        }
        .into(),
        spent_micros: b.spent_micros,
        exhausted_until: b.exhausted_until.map(timestamp).into(),
        connection_id: b.connection_id.map(|c| c.to_string()),
        ..Default::default()
    }
}
impl InferenceService for Rpc {
    /// Per-connection totals for the window, the daily series and the agent's all-time spend.
    async fn get_usage<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::GetUsageRequest>,
    ) -> ServiceResult<impl Encodable<management::GetUsageResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let since = request
            .since
            .map(|t| {
                chrono::DateTime::from_timestamp(t.seconds, t.nanos.max(0) as u32)
                    .unwrap_or_default()
            })
            .unwrap_or_else(|| {
                use chrono::Datelike;
                let now = chrono::Utc::now();
                now.date_naive()
                    .with_day(1)
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|d| d.and_utc())
                    .unwrap_or(now)
            });
        let until = request
            .until
            .map(|t| {
                chrono::DateTime::from_timestamp(t.seconds, t.nanos.max(0) as u32)
                    .unwrap_or_default()
            })
            .unwrap_or_else(chrono::Utc::now);
        let agent = id(&request.agent_id)?;
        let total = super::db::usage_total_one(&self.0.get().await.map_err(Error::from)?, agent)
            .await
            .map_err(Error::from)?;
        let rows = super::db::usage_by_connection_all(
            &self.0.get().await.map_err(Error::from)?,
            agent,
            since,
            until,
        )
        .await
        .map_err(Error::from)?;
        let bucket = match request.bucket.as_str() {
            "" | "day" => "day",
            "hour" => "hour",
            _ => return Err(Error::Invalid("Bucket must be day or hour".into()).into()),
        };
        let series = super::db::usage_series_all(
            &self.0.get().await.map_err(Error::from)?,
            agent,
            since,
            until,
            bucket,
        )
        .await
        .map_err(Error::from)?;
        Response::ok(management::GetUsageResponse {
            total_cost_micros: total,
            series: series
                .into_iter()
                .map(|r| management::UsagePoint {
                    bucket_start: timestamp(r.bucket).into(),
                    connection_id: r.connection_id.to_string(),
                    requests: r.requests,
                    cost_micros: r.cost_micros,
                    ..Default::default()
                })
                .collect(),
            connections: rows
                .into_iter()
                .map(|r| management::ConnectionUsage {
                    connection_id: r.connection_id.to_string(),
                    requests: r.requests,
                    input_tokens: r.input_tokens,
                    output_tokens: r.output_tokens,
                    cost_micros: r.cost_micros,
                    unpriced_requests: r.unpriced_requests,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    /// Budgets, optionally filtered by scope and scope id.
    async fn list_budgets<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ListBudgetsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListBudgetsResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let scope = request.scope.map(scope_model).transpose()?;
        let scope_id = request.scope_id.as_deref().map(id).transpose()?;
        Response::ok(management::ListBudgetsResponse {
            budgets: budgets::list(&self.0, scope, scope_id)
                .await?
                .into_iter()
                .map(budget_wire)
                .collect(),
            ..Default::default()
        })
    }
    /// Create or replace the budget for (scope, scope id, period, connection); settles at once.
    async fn set_budget<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::SetBudgetRequest>,
    ) -> ServiceResult<impl Encodable<management::SetBudgetResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let budget = budgets::set(
            &self.0,
            scope_model(request.scope)?,
            id(&request.scope_id)?,
            request.connection_id.as_deref().map(id).transpose()?,
            period_model(request.period)?,
            request.limit_micros,
            action_model(request.action)?,
        )
        .await?;
        Response::ok(management::SetBudgetResponse {
            budget: budget_wire(budget).into(),
            ..Default::default()
        })
    }
    /// Remove a budget; its scope is unblocked at the next token renewal.
    async fn delete_budget<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::DeleteBudgetRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteBudgetResponse> + Send + use<'a>> {
        budgets::delete(&self.0, id(request.id)?).await?;
        Response::ok(management::DeleteBudgetResponse::default())
    }
}
