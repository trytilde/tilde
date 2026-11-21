//! `ApplyService`: proto in, report out. Authorization is part of preflight, so a caller missing
//! a permission gets it listed as a required action with everything else.
use super::{Action, Apply, Outcome};
use crate::proto::tilde::management::v1 as m;
use crate::services::tilde::management::v1::ApplyService;
use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::ExposeSecret;
use std::sync::Arc;

pub fn router(apply: Apply) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(apply)),
        // Specs carry declared prompts and inline skill files.
        32 * 1024 * 1024,
    )
}

impl ApplyService for Apply {
    async fn apply<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, m::ApplyRequest>,
    ) -> ServiceResult<impl Encodable<m::ApplyResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let report = Apply::apply(self, &request).await?;
        Response::ok(m::ApplyResponse {
            outcome: match report.outcome {
                Outcome::Planned => m::ApplyOutcome::Planned,
                Outcome::Applied => m::ApplyOutcome::Applied,
                Outcome::Blocked => m::ApplyOutcome::Blocked,
            }
            .into(),
            changes: report
                .changes
                .into_iter()
                .map(|c| m::Change {
                    kind: c.kind.into(),
                    key: c.key,
                    action: match c.action {
                        Action::Create => m::ChangeAction::Create,
                        Action::Update => m::ChangeAction::Update,
                        Action::Unchanged => m::ChangeAction::Unchanged,
                        Action::Remove => m::ChangeAction::Remove,
                    }
                    .into(),
                    fields: c
                        .fields
                        .into_iter()
                        .map(|f| m::FieldChange {
                            field: f.name,
                            before: f.before,
                            after: f.after,
                            sealed: f.sealed,
                            ..Default::default()
                        })
                        .collect(),
                    note: c.note,
                    ..Default::default()
                })
                .collect(),
            required_actions: report
                .required
                .into_iter()
                .map(|r| m::RequiredAction {
                    code: r.code.into(),
                    subject: r.subject,
                    message: r.message,
                    url: r.url,
                    ..Default::default()
                })
                .collect(),
            agents: report
                .agents
                .into_iter()
                .map(|a| {
                    let (deployment_id, deployment_token) = a
                        .deployment
                        .map(|(id, token)| (id.to_string(), token.expose_secret().to_owned()))
                        .unwrap_or_default();
                    m::AppliedAgent {
                        key: a.key,
                        id: a.id.to_string(),
                        deployment_id,
                        deployment_token,
                        ..Default::default()
                    }
                })
                .collect(),
            ..Default::default()
        })
    }
}
