//! Management surface for an agent's routines.
use super::{Routine, RoutineInput, Routines, Trigger};
use crate::error::Error;
use crate::proto::tilde::management::v1 as management;
use crate::services::tilde::management::v1::RoutineService;
use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use management::{create_routine_request, routine, update_routine_request};
use std::sync::Arc;
use uuid::Uuid;

struct Rpc(Routines);
pub fn router(routines: Routines) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(routines))),
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
fn trigger_model(
    cron: Option<&management::CronTrigger>,
    signal: Option<&management::SignalTrigger>,
) -> Result<Trigger, Error> {
    match (cron, signal) {
        (Some(cron), None) => Ok(Trigger::Cron {
            schedule: cron.schedule.clone(),
        }),
        (None, Some(signal)) => Ok(Trigger::Signal {
            connection_id: id(&signal.connection_id)?,
            signal_type: signal.signal_type.clone(),
        }),
        _ => Err(Error::Invalid(
            "A routine needs a cron or a signal trigger".into(),
        )),
    }
}
fn routine_wire(r: Routine) -> management::Routine {
    management::Routine {
        id: r.id.to_string(),
        agent_id: r.agent_id.to_string(),
        name: r.name,
        prompt: r.prompt,
        thread_title: r.thread_title,
        enabled: r.enabled,
        trigger: Some(match r.trigger {
            Trigger::Cron { schedule } => {
                routine::Trigger::Cron(Box::new(management::CronTrigger {
                    schedule,
                    ..Default::default()
                }))
            }
            Trigger::Signal {
                connection_id,
                signal_type,
            } => routine::Trigger::Signal(Box::new(management::SignalTrigger {
                connection_id: connection_id.to_string(),
                signal_type,
                ..Default::default()
            })),
        }),
        next_run_at: r.next_run_at.map(timestamp).into(),
        last_run_at: r.last_run_at.map(timestamp).into(),
        last_thread_id: r.last_thread_id.map(|t| t.to_string()),
        last_error: r.last_error,
        created_at: timestamp(r.created_at).into(),
        updated_at: timestamp(r.updated_at).into(),
        ..Default::default()
    }
}
impl RoutineService for Rpc {
    async fn list_routines<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListRoutinesRequest>,
    ) -> ServiceResult<impl Encodable<management::ListRoutinesResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let routines = self.0.list(id(&request.agent_id)?).await?;
        Response::ok(management::ListRoutinesResponse {
            routines: routines.into_iter().map(routine_wire).collect(),
            ..Default::default()
        })
    }
    async fn create_routine<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::CreateRoutineRequest>,
    ) -> ServiceResult<impl Encodable<management::CreateRoutineResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let trigger = match &request.trigger {
            Some(create_routine_request::Trigger::Cron(cron)) => trigger_model(Some(cron), None),
            Some(create_routine_request::Trigger::Signal(signal)) => {
                trigger_model(None, Some(signal))
            }
            None => trigger_model(None, None),
        }?;
        let routine = self
            .0
            .create(
                id(&request.agent_id)?,
                RoutineInput {
                    name: request.name,
                    prompt: request.prompt,
                    thread_title: request.thread_title,
                    enabled: request.enabled,
                    trigger,
                },
            )
            .await?;
        Response::ok(management::CreateRoutineResponse {
            routine: routine_wire(routine).into(),
            ..Default::default()
        })
    }
    async fn update_routine<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::UpdateRoutineRequest>,
    ) -> ServiceResult<impl Encodable<management::UpdateRoutineResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let trigger = match &request.trigger {
            Some(update_routine_request::Trigger::Cron(cron)) => trigger_model(Some(cron), None),
            Some(update_routine_request::Trigger::Signal(signal)) => {
                trigger_model(None, Some(signal))
            }
            None => trigger_model(None, None),
        }?;
        let routine = self
            .0
            .update(
                id(&request.id)?,
                RoutineInput {
                    name: request.name,
                    prompt: request.prompt,
                    thread_title: request.thread_title,
                    enabled: request.enabled,
                    trigger,
                },
            )
            .await?;
        Response::ok(management::UpdateRoutineResponse {
            routine: routine_wire(routine).into(),
            ..Default::default()
        })
    }
    async fn delete_routine<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteRoutineRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteRoutineResponse> + Send + use<'a>> {
        self.0.delete(id(&request.to_owned_message().id)?).await?;
        Response::ok(management::DeleteRoutineResponse::default())
    }
    async fn list_signal_types<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListSignalTypesRequest>,
    ) -> ServiceResult<impl Encodable<management::ListSignalTypesResponse> + Send + use<'a>> {
        let source = self
            .0
            .signal_source(id(&request.to_owned_message().connection_id)?)
            .await?;
        Response::ok(management::ListSignalTypesResponse {
            signal_types: source
                .types()
                .into_iter()
                .map(|t| management::SignalType {
                    id: t.id,
                    name: t.name,
                    description: t.description,
                    default_thread_title: t.title,
                    ..Default::default()
                })
                .collect(),
            variables: crate::signals::COMMON
                .iter()
                .chain(source.variables())
                .map(|v| management::SignalVariable {
                    key: v.key.into(),
                    description: v.description.into(),
                    example: v.example.into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
}
