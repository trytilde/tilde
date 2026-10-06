//! The sandbox process's service on the agent runtime listener (authenticated by its enrollment
//! and session tokens, never invocation tokens) and the blueprint management service.
use super::{Sandboxes, db, digest};
use crate::error::Error;
use crate::proto::tilde::{management::v1 as management, sandbox::v1 as wire};
use crate::services::tilde::{
    management::v1::SandboxBlueprintService, sandbox::v1::SandboxService,
};
use crate::tools::{Caller, Owner, Tools};
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use opentelemetry::{
    KeyValue,
    trace::{FutureExt, SpanKind},
};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use zeroize::Zeroize;

const NOTIFY_CHANNEL: &str = "tilde_sandbox_calls";

/// Served on the agent runtime listener.
pub fn router(tools: Tools) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Runtime(tools))),
        4 * 1024 * 1024,
    )
}
pub fn management_router(sandboxes: Sandboxes) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Management(sandboxes))),
        256 * 1024,
    )
}

fn bearer(ctx: &RequestContext) -> Result<&str, ConnectError> {
    ctx.headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| ConnectError::unauthenticated("Sandbox token required"))
}
/// A Connect frame. The session token and environment are plaintext only inside it: the
/// framework encodes it and then drops it, which clears them.
struct Frame(wire::ConnectResponse);
impl Encodable<wire::ConnectResponse> for Frame {
    fn encode(&self, codec: connectrpc::CodecFormat) -> Result<buffa::bytes::Bytes, ConnectError> {
        Encodable::<wire::ConnectResponse>::encode(&self.0, codec)
    }
}
impl Drop for Frame {
    fn drop(&mut self) {
        if let Some(wire::connect_response::Frame::Registered(registered)) = &mut self.0.frame {
            if let Some(token) = &mut registered.session_token {
                token.zeroize();
            }
            for var in &mut registered.env {
                var.value.zeroize();
            }
        }
    }
}

struct Runtime(Tools);
impl Runtime {
    fn sandboxes(&self) -> &Sandboxes {
        &self.0.sandboxes
    }
    async fn session(&self, ctx: &RequestContext) -> Result<db::SessionRow, ConnectError> {
        db::sandbox_authenticate_opt(
            &self
                .sandboxes()
                .connections
                .pool
                .get()
                .await
                .map_err(Error::from)?,
            &digest(bearer(ctx)?),
        )
        .await
        .map_err(Error::from)?
        .ok_or_else(|| ConnectError::unauthenticated("Invalid sandbox token"))
    }
}
impl SandboxService for Runtime {
    /// An enrollment token is exchanged once for a session; a session token reconnects.
    async fn connect(
        &self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, wire::ConnectRequest>,
    ) -> ServiceResult<
        connectrpc::ServiceStream<impl Encodable<wire::ConnectResponse> + Send + use<>>,
    > {
        let sandboxes = self.sandboxes().clone();
        let pool = sandboxes.connections.pool.clone();
        let presented = digest(bearer(&ctx)?);
        let session = crate::connections::model::random_secret();
        let session_hash = digest(session.expose_secret());
        let client = pool.get().await.map_err(Error::from)?;
        let (sandbox, blueprint, session_hash, issued) =
            match db::sandbox_enroll_opt(&client, &presented, &session_hash)
                .await
                .map_err(Error::from)?
            {
                Some(row) => (row.id, row.blueprint_id, session_hash, Some(session)),
                None => {
                    let row = db::sandbox_authenticate_opt(&client, &presented)
                        .await
                        .map_err(Error::from)?
                        .ok_or_else(|| ConnectError::unauthenticated("Invalid sandbox token"))?;
                    db::sandbox_connected_execute(&client, row.id)
                        .await
                        .map_err(Error::from)?;
                    (row.id, row.blueprint_id, presented, None)
                }
            };
        drop(client);
        let env = sandboxes.env(blueprint).await?;
        let mut changes = sandboxes
            .calls
            .subscribe(&pool, NOTIFY_CHANNEL)
            .await
            .map_err(Error::from)?;
        changes.borrow_and_update();
        let registered = wire::SandboxRegistered {
            sandbox_id: sandbox.to_string(),
            session_token: issued.map(|t| t.expose_secret().to_owned()),
            env: env
                .into_iter()
                .map(|(name, value)| wire::EnvVar {
                    name,
                    value: value.expose_secret().to_owned(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        Response::stream_ok(async_stream::try_stream! {
            yield Frame(wire::ConnectResponse { frame: Some(registered.into()), ..Default::default() });
            loop {
                let client = pool.get().await.map_err(Error::from)?;
                // A newer process's enrollment, or a terminated sandbox, ends the stream at its
                // next turn.
                if db::sandbox_authenticate_opt(&client, &session_hash).await.map_err(Error::from)?.map(|r| r.id) != Some(sandbox) { break; }
                db::sandbox_connected_execute(&client, sandbox).await.map_err(Error::from)?;
                let calls = db::call_claim_all(&client, sandbox).await.map_err(Error::from)?;
                drop(client);
                for call in calls {
                    yield Frame(wire::ConnectResponse { frame: Some(wire::SandboxOperation {
                        id: call.id.to_string(), name: call.operation, input_json: call.input_json, ..Default::default()
                    }.into()), ..Default::default() });
                }
                tokio::select! {
                    changed = changes.changed() => { if changed.is_err() { break; } changes.borrow_and_update(); }
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {
                        yield Frame(wire::ConnectResponse { frame: Some(wire::SandboxPing::default().into()), ..Default::default() });
                    }
                }
            }
        })
    }
    async fn respond<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::RespondRequest>,
    ) -> ServiceResult<impl Encodable<wire::RespondResponse> + Send + use<'a>> {
        let sandbox = self.session(&ctx).await?.id;
        let request = request.to_owned_message();
        let (status, output, error) = match request.outcome {
            // Too large to hand to the agent: the call fails with that rather than timing out.
            Some(wire::respond_request::Outcome::OutputJson(output))
                if output.len() > crate::tools::MAX_OUTPUT =>
            {
                (
                    "failed",
                    String::new(),
                    crate::tools::OUTPUT_TOO_LARGE.into(),
                )
            }
            Some(wire::respond_request::Outcome::OutputJson(output)) => {
                if !serde_json::from_str::<Value>(&output).is_ok_and(|v| v.is_object()) {
                    return Err(ConnectError::invalid_argument(
                        "Output must be a JSON object",
                    ));
                }
                ("completed", output, String::new())
            }
            Some(wire::respond_request::Outcome::Error(error)) if !error.is_empty() => {
                ("failed", String::new(), error.chars().take(2048).collect())
            }
            _ => {
                return Err(ConnectError::invalid_argument(
                    "An output or an error is required",
                ));
            }
        };
        if db::call_finish_execute(
            &self
                .sandboxes()
                .connections
                .pool
                .get()
                .await
                .map_err(Error::from)?,
            crate::chat::id(&request.id)?,
            sandbox,
            status,
            &output,
            &error,
        )
        .await
        .map_err(Error::from)?
            == 0
        {
            return Err(ConnectError::not_found(
                "No delivered operation with this ID",
            ));
        }
        Response::ok(wire::RespondResponse::default())
    }
    async fn list_tools<'a>(
        &'a self,
        ctx: RequestContext,
        _request: ServiceRequest<'_, wire::ListToolsRequest>,
    ) -> ServiceResult<impl Encodable<wire::ListToolsResponse> + Send + use<'a>> {
        let session = self.session(&ctx).await?;
        let tools = self.0.load(Owner::Blueprint(session.blueprint_id)).await?;
        Response::ok(wire::ListToolsResponse {
            tools: tools
                .into_iter()
                .map(|(definition, _, _)| definition)
                .collect(),
            ..Default::default()
        })
    }
    /// A process inside the sandbox calls one of the blueprint's tools. The call is an
    /// `execute_tool` span of the agent's invocation: inside the operation it runs under, else in
    /// the sandbox's latest invocation.
    async fn invoke_tool<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::InvokeToolRequest>,
    ) -> ServiceResult<impl Encodable<wire::InvokeToolResponse> + Send + use<'a>> {
        let session = self.session(&ctx).await?;
        let request = request.to_owned_message();
        if request.input_json.len() > 64 * 1024 {
            return Err(ConnectError::invalid_argument(
                "Tool input is at most 64 KiB",
            ));
        }
        let input: Value = match request.input_json.as_str() {
            "" => Value::Object(Default::default()),
            json => serde_json::from_str(json)
                .map_err(|_| ConnectError::invalid_argument("Invalid tool input JSON"))?,
        };
        let operation = request
            .operation_id
            .as_deref()
            .map(crate::chat::id)
            .transpose()?;
        let scope = db::trace_scope_opt(
            &self
                .sandboxes()
                .connections
                .pool
                .get()
                .await
                .map_err(Error::from)?,
            session.id,
            operation,
        )
        .await
        .map_err(Error::from)?;
        let call = Uuid::new_v4();
        let cx = span(
            &request.name,
            call,
            session.id,
            scope.as_ref(),
            &request.input_json,
        );
        let _end = crate::telemetry::tracing::context::EndOnDrop(cx.clone());
        let result = async {
            let (definition, callee, _) = self
                .0
                .load(Owner::Blueprint(session.blueprint_id))
                .await?
                .into_iter()
                .find(|(definition, _, _)| definition.name == request.name)
                .ok_or_else(|| ConnectError::not_found("The sandbox has no such tool"))?;
            crate::tools::validate(&definition, &input)?;
            // Tool hosts are told what the sandbox was launched for, never a traced invocation:
            // a shared sandbox's latest invocation may be another agent's.
            self.0
                .run(
                    callee,
                    Caller {
                        call_id: call,
                        agent: session.agent_id,
                        thread: session.thread_id,
                        sandbox: Some(session.id),
                    },
                    input,
                )
                .await
        }
        .with_context(cx.clone())
        .await;
        crate::chat::tools::finish(&cx, &result);
        Response::ok(wire::InvokeToolResponse {
            output_json: result?.to_string(),
            ..Default::default()
        })
    }
}

/// An `execute_tool` span as the agent's own tool calls have, child of the operation's trace
/// context when the call ran under one.
fn span(
    name: &str,
    call: Uuid,
    sandbox: Uuid,
    scope: Option<&db::TraceScopeRow>,
    input_json: &str,
) -> opentelemetry::Context {
    let parent = scope
        .filter(|s| !s.traceparent.is_empty())
        .map(|s| crate::telemetry::tracing::context::restore(&s.traceparent, &s.tracestate))
        .unwrap_or_default();
    let mut attributes = vec![
        KeyValue::new("gen_ai.operation.name", "execute_tool"),
        KeyValue::new("gen_ai.tool.name", name.to_owned()),
        KeyValue::new("gen_ai.tool.call.id", call.to_string()),
        KeyValue::new("tilde.tool.provider_id", "sandbox"),
        KeyValue::new("tilde.sandbox.id", sandbox.to_string()),
        KeyValue::new(
            "tilde.observation.input",
            crate::chat::tools::bounded(input_json),
        ),
    ];
    if let Some(scope) = scope {
        for (key, value) in [
            ("tilde.invocation.id", scope.invocation_id),
            ("tilde.agent.id", scope.agent_id),
            ("tilde.run.id", scope.run_id),
            ("tilde.thread.id", scope.thread_id),
        ] {
            attributes.push(KeyValue::new(key, value.to_string()));
        }
    }
    crate::telemetry::tracing::context::start(
        format!("execute_tool {name}"),
        SpanKind::Internal,
        &parent,
        attributes,
    )
}

fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Invalid UUID".into()))
}
fn reuse_text(reuse: buffa::EnumValue<management::SandboxReuse>) -> Result<&'static str, Error> {
    use management::SandboxReuse as R;
    match reuse.as_known() {
        Some(R::Unspecified | R::Thread) => Ok("thread"),
        Some(R::Agent) => Ok("agent"),
        Some(R::AgentIdentity) => Ok("agent_identity"),
        Some(R::Global) => Ok("global"),
        Some(R::GlobalIdentity) => Ok("global_identity"),
        None => Err(Error::Invalid("Unknown sandbox reuse mode".into())),
    }
}
fn reuse_wire(reuse: &str) -> management::SandboxReuse {
    use management::SandboxReuse as R;
    match reuse {
        "agent" => R::Agent,
        "agent_identity" => R::AgentIdentity,
        "global" => R::Global,
        "global_identity" => R::GlobalIdentity,
        _ => R::Thread,
    }
}
fn timings(t: management::SandboxTimings) -> super::Timings {
    super::Timings {
        sleep_after: t.sleep_after_seconds,
        terminate_after: t.terminate_after_seconds,
        connect_timeout: t.connect_timeout_seconds,
    }
}
fn blueprint_wire(b: super::Blueprint) -> management::SandboxBlueprint {
    management::SandboxBlueprint {
        id: b.id.to_string(),
        name: b.name,
        connection_id: b.connection_id.to_string(),
        template: b.template,
        reuse: reuse_wire(&b.reuse).into(),
        env_names: b.env_names,
        agent_ids: b.agent_ids.iter().map(Uuid::to_string).collect(),
        timings: management::SandboxTimings {
            sleep_after_seconds: b.timings.sleep_after,
            terminate_after_seconds: b.timings.terminate_after,
            connect_timeout_seconds: b.timings.connect_timeout,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}
fn agent_sandbox_wire(row: db::AgentSandboxRow) -> management::AgentSandbox {
    management::AgentSandbox {
        agent_id: row.agent_id.to_string(),
        blueprint_id: row.blueprint_id.to_string(),
        tool_source_id: row.source_id.map(|s| s.to_string()).unwrap_or_default(),
        ..Default::default()
    }
}
fn sandbox_wire(row: db::SandboxListRow) -> management::Sandbox {
    use management::SandboxStatus as S;
    management::Sandbox {
        id: row.id.to_string(),
        blueprint_id: row.blueprint_id.to_string(),
        reuse: reuse_wire(&row.reuse).into(),
        agent_id: row.agent_id.map(|a| a.to_string()),
        thread_id: row.thread_id.map(|t| t.to_string()),
        identity_id: row.identity_id.map(|i| i.to_string()),
        status: match row.status.as_str() {
            "running" => S::Running,
            "sleeping" => S::Sleeping,
            "failed" => S::Failed,
            _ => S::Starting,
        }
        .into(),
        provider_sandbox_id: row.provider_sandbox_id,
        connected: row.connected,
        error: row.error,
        last_used_at: crate::chat::audit::timestamp(row.last_used_at).into(),
        created_at: crate::chat::audit::timestamp(row.created_at).into(),
        ..Default::default()
    }
}

struct Management(Sandboxes);
impl SandboxBlueprintService for Management {
    async fn list_sandbox_blueprints<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListSandboxBlueprintsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListSandboxBlueprintsResponse> + Send + use<'a>>
    {
        let request = request.to_owned_message();
        let search = crate::rpc::search(request.search.as_deref())?;
        Response::ok(management::ListSandboxBlueprintsResponse {
            blueprints: self
                .0
                .blueprints(search)
                .await?
                .into_iter()
                .map(blueprint_wire)
                .collect(),
            ..Default::default()
        })
    }
    async fn get_sandbox_blueprint<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetSandboxBlueprintRequest>,
    ) -> ServiceResult<impl Encodable<management::GetSandboxBlueprintResponse> + Send + use<'a>>
    {
        let blueprint = self
            .0
            .blueprint(id(&request.to_owned_message().id)?)
            .await?;
        Response::ok(management::GetSandboxBlueprintResponse {
            blueprint: blueprint_wire(blueprint).into(),
            ..Default::default()
        })
    }
    async fn create_sandbox_blueprint<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::CreateSandboxBlueprintRequest>,
    ) -> ServiceResult<impl Encodable<management::CreateSandboxBlueprintResponse> + Send + use<'a>>
    {
        let r = request.to_owned_message();
        let blueprint = self
            .0
            .create_blueprint(
                &r.name,
                id(&r.connection_id)?,
                &r.template,
                reuse_text(r.reuse)?,
                r.timings
                    .into_option()
                    .map(timings)
                    .unwrap_or(super::Timings {
                        sleep_after: 0,
                        terminate_after: 0,
                        connect_timeout: 0,
                    }),
            )
            .await?;
        Response::ok(management::CreateSandboxBlueprintResponse {
            blueprint: blueprint_wire(blueprint).into(),
            ..Default::default()
        })
    }
    async fn update_sandbox_blueprint<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::UpdateSandboxBlueprintRequest>,
    ) -> ServiceResult<impl Encodable<management::UpdateSandboxBlueprintResponse> + Send + use<'a>>
    {
        let r = request.to_owned_message();
        let blueprint = self
            .0
            .update_blueprint(
                id(&r.id)?,
                r.name.as_deref(),
                r.connection_id.as_deref().map(id).transpose()?,
                r.template.as_deref(),
                r.reuse.map(reuse_text).transpose()?,
                r.timings.into_option().map(timings),
            )
            .await?;
        Response::ok(management::UpdateSandboxBlueprintResponse {
            blueprint: blueprint_wire(blueprint).into(),
            ..Default::default()
        })
    }
    async fn delete_sandbox_blueprint<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteSandboxBlueprintRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteSandboxBlueprintResponse> + Send + use<'a>>
    {
        self.0
            .delete_blueprint(id(&request.to_owned_message().id)?)
            .await?;
        Response::ok(management::DeleteSandboxBlueprintResponse::default())
    }
    async fn set_sandbox_env_var<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetSandboxEnvVarRequest>,
    ) -> ServiceResult<impl Encodable<management::SetSandboxEnvVarResponse> + Send + use<'a>> {
        let mut r = request.to_owned_message();
        let value = SecretString::from(std::mem::take(&mut r.value));
        let blueprint = self
            .0
            .set_env(id(&r.blueprint_id)?, &r.name, &value)
            .await?;
        Response::ok(management::SetSandboxEnvVarResponse {
            blueprint: blueprint_wire(blueprint).into(),
            ..Default::default()
        })
    }
    async fn delete_sandbox_env_var<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteSandboxEnvVarRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteSandboxEnvVarResponse> + Send + use<'a>>
    {
        let r = request.to_owned_message();
        let blueprint = self.0.delete_env(id(&r.blueprint_id)?, &r.name).await?;
        Response::ok(management::DeleteSandboxEnvVarResponse {
            blueprint: blueprint_wire(blueprint).into(),
            ..Default::default()
        })
    }
    async fn list_sandboxes<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListSandboxesRequest>,
    ) -> ServiceResult<impl Encodable<management::ListSandboxesResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let sandboxes = self
            .0
            .list(
                r.blueprint_id.as_deref().map(id).transpose()?,
                r.agent_id.as_deref().map(id).transpose()?,
            )
            .await?;
        Response::ok(management::ListSandboxesResponse {
            sandboxes: sandboxes.into_iter().map(sandbox_wire).collect(),
            ..Default::default()
        })
    }
    async fn terminate_sandbox<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::TerminateSandboxRequest>,
    ) -> ServiceResult<impl Encodable<management::TerminateSandboxResponse> + Send + use<'a>> {
        self.0
            .terminate(id(&request.to_owned_message().id)?)
            .await?;
        Response::ok(management::TerminateSandboxResponse::default())
    }
    async fn get_agent_sandbox<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetAgentSandboxRequest>,
    ) -> ServiceResult<impl Encodable<management::GetAgentSandboxResponse> + Send + use<'a>> {
        let agent = id(&request.to_owned_message().agent_id)?;
        Response::ok(management::GetAgentSandboxResponse {
            sandbox: self
                .0
                .agent_sandbox(agent)
                .await?
                .map(agent_sandbox_wire)
                .into(),
            ..Default::default()
        })
    }
    async fn set_agent_sandbox<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetAgentSandboxRequest>,
    ) -> ServiceResult<impl Encodable<management::SetAgentSandboxResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let row = self
            .0
            .set_agent_sandbox(id(&r.agent_id)?, id(&r.blueprint_id)?)
            .await?;
        Response::ok(management::SetAgentSandboxResponse {
            sandbox: agent_sandbox_wire(row).into(),
            ..Default::default()
        })
    }
    async fn remove_agent_sandbox<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RemoveAgentSandboxRequest>,
    ) -> ServiceResult<impl Encodable<management::RemoveAgentSandboxResponse> + Send + use<'a>>
    {
        self.0
            .remove_agent_sandbox(id(&request.to_owned_message().agent_id)?)
            .await?;
        Response::ok(management::RemoveAgentSandboxResponse::default())
    }
}
