//! ConnectRPC dispatch and API-owned per-agent message concurrency.
//! Queued inputs become fresh invocations; interrupt revokes old callbacks and sends Cancel.
//! Hosts only receive invocation snapshots and cancellation signals, never a steering queue.
use crate::chat as application;
use crate::proto::tilde::agent_host::v1 as host;
use crate::proto::tilde::ingress::v1 as ingress_pb;
use crate::proto::tilde::runtime::v1 as runtime_pb;
use crate::proto::tilde::types::v1 as types;
use crate::{
    chat::{Chat, ChatError, Result, activity, id, text},
    services::tilde::agent_host::v1::AgentServiceClient,
};
use connectrpc::client::{ClientConfig, HttpClient};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use opentelemetry::{
    KeyValue,
    trace::{FutureExt, SpanKind, TraceContextExt},
};
use secrecy::ExposeSecret;

use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub(crate) fn client<M: buffa::Message>(
    endpoint: &str,
    key: &str,
    method: &str,
    request: &M,
) -> Result<AgentServiceClient<HttpClient>> {
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let digest = hex::encode(Sha256::digest(request.encode_to_vec()));
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|_| ChatError::Transport)?;
    mac.update(format!("{method}.{timestamp}.{digest}").as_bytes());
    let mut headers = http::HeaderMap::new();
    headers.insert(
        "x-tilde-timestamp",
        timestamp.parse().map_err(|_| ChatError::Transport)?,
    );
    headers.insert(
        "x-tilde-signature",
        hex::encode(mac.finalize().into_bytes())
            .parse()
            .map_err(|_| ChatError::Transport)?,
    );
    crate::telemetry::context::inject(&mut headers);
    let transport = if endpoint.starts_with("https://") {
        let roots = connectrpc::rustls::RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
        );
        let tls = connectrpc::rustls::ClientConfig::builder_with_provider(Arc::new(
            connectrpc::rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| ChatError::Transport)?
        .with_root_certificates(roots)
        .with_no_client_auth();
        HttpClient::with_tls(Arc::new(tls))
    } else {
        HttpClient::plaintext_http2_only()
    };
    let config = ClientConfig::new(endpoint.parse().map_err(|_| ChatError::Transport)?)
        .with_default_headers(headers);
    Ok(AgentServiceClient::new(transport, config))
}
impl Chat {
    /// Persist one invocation per explicit objective. A duplicate request cannot start a second loop.
    pub async fn start_run(&self, r: application::StartRun) -> Result<types::Run> {
        if let Some(local) = self.local() {
            return local.start_run(r).await;
        }
        let forwarded: Option<ingress_pb::StartRunResponse> = self
            .forward_sidecar(
                id(&r.agent_id)?,
                Some(id(&r.thread_id)?),
                "StartRun",
                &ingress_pb::StartRunRequest {
                    thread_id: r.thread_id.clone(),
                    agent_id: r.agent_id.clone(),
                    objective: r.objective.clone(),
                    goal_id: r.goal_id.clone(),
                    idempotency_key: r.idempotency_key.clone(),
                    ..Default::default()
                },
            )
            .await?;
        if let Some(response) = forwarded {
            return response.run.into_option().ok_or(ChatError::Transport);
        }
        text(&r.objective)?;
        text(&r.idempotency_key)?;
        let thread = id(&r.thread_id)?;
        let agent = id(&r.agent_id)?;
        let goal = r.goal_id.as_deref().map(id).transpose()?;
        let roster = self.thread(thread).await?;
        if !roster
            .participants
            .iter()
            .any(|p| p.active && p.agent_id.as_deref() == Some(&r.agent_id))
        {
            return Err(ChatError::NotFound);
        }
        let run = Uuid::new_v4();
        let invocation = Uuid::new_v4();
        let mut tx = self.pg()?.begin().await?;
        super::access::lock_thread_route(&mut tx, thread).await?;
        sqlx::query_file!("../../queries/chat/agent_available.sql", agent)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ChatError::Conflict)?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", thread)
            .fetch_one(&mut *tx)
            .await?;
        if !sqlx::query_file!(
            "../../queries/channel_access/run_allowed.sql",
            agent,
            thread,
            None::<Uuid>
        )
        .fetch_one(&mut *tx)
        .await?
        .allowed
        {
            return Err(ChatError::Denied);
        }
        let participant = sqlx::query_file!(
            "../../queries/chat/participant_find.sql",
            thread,
            None::<Uuid>,
            Some(agent)
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ChatError::NotFound)?;
        if !participant.active {
            return Err(ChatError::NotFound);
        }
        let inserted = sqlx::query_file!(
            "../../queries/chat/run_create.sql",
            run,
            thread,
            agent,
            r.objective,
            goal,
            r.idempotency_key
        )
        .fetch_optional(&mut *tx)
        .await?;
        let actual = if inserted.is_some() {
            if sqlx::query_file!("../../queries/chat/invocation_active.sql", thread, agent)
                .fetch_optional(&mut *tx)
                .await?
                .is_some()
            {
                return Err(ChatError::Conflict);
            }
            let deployment = super::pin_deployment(&mut tx, thread, agent).await?;
            sqlx::query_file!(
                "../../queries/chat/invocation_create.sql",
                invocation,
                run,
                thread,
                agent,
                crate::telemetry::context::capture().0,
                crate::telemetry::context::capture().1,
                deployment
            )
            .execute(&mut *tx)
            .await?;
            activity(&mut tx, thread, "invocation.pending", invocation, "").await?;
            run
        } else {
            let existing = sqlx::query_file!(
                "../../queries/chat/run_replay.sql",
                thread,
                agent,
                r.idempotency_key
            )
            .fetch_one(&mut *tx)
            .await?;
            if existing.objective != r.objective || existing.goal_id != goal {
                return Err(ChatError::Conflict);
            }
            existing.id
        };
        tx.commit().await?;
        self.run(actual).await
    }
    /// Explicitly resume waiting/failed work with a new invocation identity; no ambiguous automatic goal selection.
    pub async fn resume_run(&self, run: Uuid) -> Result<types::Run> {
        if let Some(local) = self.local() {
            return local.resume_run(run).await;
        }
        let old = self.run(run).await?;
        let agent = id(&old.agent_id)?;
        let thread = id(&old.thread_id)?;
        let forwarded: Option<ingress_pb::ResumeRunResponse> = self
            .forward_sidecar(
                agent,
                Some(thread),
                "ResumeRun",
                &ingress_pb::ResumeRunRequest {
                    id: run.to_string(),
                    ..Default::default()
                },
            )
            .await?;
        if let Some(response) = forwarded {
            return response.run.into_option().ok_or(ChatError::Transport);
        }
        let mut tx = self.pg()?.begin().await?;
        super::access::lock_thread_route(&mut tx, thread).await?;
        sqlx::query_file!("../../queries/chat/agent_available.sql", agent)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ChatError::Conflict)?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", thread)
            .fetch_one(&mut *tx)
            .await?;
        let row = sqlx::query_file!("../../queries/chat/run_resume.sql", run)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ChatError::Conflict)?;
        let invocation = Uuid::new_v4();
        let deployment = super::pin_deployment(&mut tx, row.thread_id, row.agent_id).await?;
        sqlx::query_file!(
            "../../queries/chat/invocation_create.sql",
            invocation,
            run,
            row.thread_id,
            row.agent_id,
            crate::telemetry::context::capture().0,
            crate::telemetry::context::capture().1,
            deployment
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query_file!(
            "../../queries/chat/input_move.sql",
            id(&old.invocation_id)?,
            invocation
        )
        .execute(&mut *tx)
        .await?;
        activity(&mut tx, thread, "invocation.pending", invocation, "").await?;
        tx.commit().await?;
        self.run(run).await
    }
    async fn route_pending(&self) -> Result<bool> {
        let messages = sqlx::query_file!("../../queries/chat/route_pending.sql")
            .fetch_all(self.pg()?)
            .await?;
        let more = messages.len() == 50;
        for message in messages {
            let mut tx = self.pg()?.begin().await?;
            super::access::lock_thread_route(&mut tx, message.thread_id).await?;
            if !sqlx::query_file!(
                "../../queries/channel_access/message_allowed.sql",
                message.agent_id,
                message.thread_id,
                message.source_identity_id
            )
            .fetch_one(&mut *tx)
            .await?
            .allowed
            {
                continue;
            }
            let Some(agent) =
                sqlx::query_file!("../../queries/chat/agent_available.sql", message.agent_id)
                    .fetch_optional(&mut *tx)
                    .await?
            else {
                continue;
            };
            sqlx::query_file!("../../queries/chat/thread_lock.sql", message.thread_id)
                .fetch_one(&mut *tx)
                .await?;
            if sqlx::query_file!(
                "../../queries/chat/route_claim.sql",
                message.id,
                message.agent_id
            )
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 0
            {
                continue;
            }
            if let Some(active) = sqlx::query_file!(
                "../../queries/chat/invocation_active.sql",
                message.thread_id,
                message.agent_id
            )
            .fetch_optional(&mut *tx)
            .await?
            {
                sqlx::query_file!(
                    "../../queries/chat/input_create.sql",
                    active.id,
                    message.id,
                    message.text
                )
                .execute(&mut *tx)
                .await?;
                match agent.concurrency_policy.as_str() {
                    "interrupt" => {
                        sqlx::query_file!(
                            "../../queries/chat/invocation_finish.sql",
                            active.id,
                            "canceled"
                        )
                        .fetch_one(&mut *tx)
                        .await?;
                        sqlx::query_file!(
                            "../../queries/chat/run_status.sql",
                            active.run_id,
                            "canceled"
                        )
                        .execute(&mut *tx)
                        .await?;
                        activity(
                            &mut tx,
                            message.thread_id,
                            "invocation.ended",
                            active.id,
                            "canceled",
                        )
                        .await?;
                        super::agent_lifecycle::requeue_inputs(&mut tx, active.id, active.run_id)
                            .await?;
                    }
                    "queue_and_batch" if active.status == "pending" => {
                        sqlx::query_file!(
                            "../../queries/chat/invocation_batch.sql",
                            active.id,
                            message.text
                        )
                        .execute(&mut *tx)
                        .await?;
                        sqlx::query_file!(
                            "../../queries/chat/invocation_cutoff.sql",
                            active.id,
                            Some(message.id)
                        )
                        .execute(&mut *tx)
                        .await?;
                        sqlx::query_file!(
                            "../../queries/chat/input_accept.sql",
                            active.id,
                            message.id
                        )
                        .execute(&mut *tx)
                        .await?;
                    }
                    _ => {}
                }
            } else {
                let run = Uuid::new_v4();
                let invocation = Uuid::new_v4();
                let key = message.id.to_string();
                sqlx::query_file!(
                    "../../queries/chat/run_create.sql",
                    run,
                    message.thread_id,
                    message.agent_id,
                    message.text,
                    None::<Uuid>,
                    key
                )
                .fetch_one(&mut *tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/channel_access/run_source.sql",
                    run,
                    message.source_identity_id,
                    message.channel_origin
                )
                .execute(&mut *tx)
                .await?;
                let deployment =
                    super::pin_deployment(&mut tx, message.thread_id, message.agent_id).await?;
                sqlx::query_file!(
                    "../../queries/chat/invocation_create.sql",
                    invocation,
                    run,
                    message.thread_id,
                    message.agent_id,
                    message.traceparent,
                    message.tracestate,
                    deployment
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query_file!(
                    "../../queries/chat/invocation_cutoff.sql",
                    invocation,
                    Some(message.id)
                )
                .execute(&mut *tx)
                .await?;
                activity(
                    &mut tx,
                    message.thread_id,
                    "invocation.pending",
                    invocation,
                    "",
                )
                .await?;
            }
            tx.commit().await?;
        }
        Ok(more)
    }
    /// Return durable work and the latest invocation independently.
    pub async fn run(&self, run: Uuid) -> Result<types::Run> {
        if let Some(local) = self.local() {
            return local.run(run).await;
        }
        let r = sqlx::query_file!("../../queries/chat/run_get.sql", run)
            .fetch_optional(self.pg()?)
            .await?
            .ok_or(ChatError::NotFound)?;
        Ok(types::Run {
            id: r.id.to_string(),
            thread_id: r.thread_id.to_string(),
            agent_id: r.agent_id.to_string(),
            objective: r.objective,
            status: r.status,
            invocation_id: r.invocation_id.to_string(),
            invocation_status: r.invocation_status,
            goal_id: r.goal_id.map(|v| v.to_string()),
            ..Default::default()
        })
    }
    /// An explicit steering request is a new durable API event. Hosts never receive input queues.
    pub async fn steer_invocation(&self, r: application::SteerInvocation) -> Result<()> {
        if let Some(local) = self.local() {
            return local.steer_invocation(r).await;
        }
        text(&r.text)?;
        let invocation = id(&r.invocation_id)?;
        let input = id(&r.input_id)?;
        let target = sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
            .fetch_optional(self.pg()?)
            .await?
            .ok_or(ChatError::NotFound)?;
        if let Some(()) = self
            .forward_control(
                target.agent_id,
                target.thread_id,
                "SteerInvocation",
                &ingress_pb::SteerInvocationRequest {
                    invocation_id: r.invocation_id.clone(),
                    input_id: r.input_id.clone(),
                    text: r.text.clone(),
                    ..Default::default()
                },
            )
            .await?
        {
            return Ok(());
        }
        let mut tx = self.pg()?.begin().await?;
        let row = sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ChatError::NotFound)?;
        super::access::lock_thread_route(&mut tx, row.thread_id).await?;
        let agent = sqlx::query_file!("../../queries/chat/agent_available.sql", row.agent_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ChatError::Conflict)?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", row.thread_id)
            .fetch_one(&mut *tx)
            .await?;
        if let Some(existing) =
            sqlx::query_file!("../../queries/chat/input_get.sql", invocation, input)
                .fetch_optional(&mut *tx)
                .await?
        {
            return if existing.text == r.text {
                Ok(())
            } else {
                Err(ChatError::Conflict)
            };
        }
        let current = sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
            .fetch_one(&mut *tx)
            .await?;
        if !["pending", "running"].contains(&current.status.as_str()) {
            return Err(ChatError::Conflict);
        }
        sqlx::query_file!(
            "../../queries/chat/input_create.sql",
            invocation,
            input,
            r.text
        )
        .execute(&mut *tx)
        .await?;
        if agent.concurrency_policy == "interrupt" {
            sqlx::query_file!(
                "../../queries/chat/invocation_finish.sql",
                invocation,
                "canceled"
            )
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query_file!("../../queries/chat/run_status.sql", row.run_id, "canceled")
                .execute(&mut *tx)
                .await?;
            activity(
                &mut tx,
                row.thread_id,
                "invocation.ended",
                invocation,
                "canceled",
            )
            .await?;
            super::agent_lifecycle::requeue_inputs(&mut tx, invocation, row.run_id).await?;
        } else if agent.concurrency_policy == "queue_and_batch" && current.status == "pending" {
            let event = sqlx::query_file!("../../queries/chat/input_get.sql", invocation, input)
                .fetch_one(&mut *tx)
                .await?;
            sqlx::query_file!(
                "../../queries/chat/invocation_batch.sql",
                invocation,
                r.text
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query_file!(
                "../../queries/chat/invocation_cutoff.sql",
                invocation,
                event.history_through_message_id
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query_file!("../../queries/chat/input_accept.sql", invocation, input)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// Revoke callbacks immediately and persist cancellation; the execution observes cancellation on its control stream.
    pub async fn cancel_invocation(&self, invocation: Uuid) -> Result<()> {
        if let Some(local) = self.local() {
            return local.cancel_invocation(invocation).await;
        }
        let target = sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
            .fetch_optional(self.pg()?)
            .await?
            .ok_or(ChatError::NotFound)?;
        if let Some(()) = self
            .forward_control(
                target.agent_id,
                target.thread_id,
                "CancelInvocation",
                &ingress_pb::CancelInvocationRequest {
                    invocation_id: invocation.to_string(),
                    ..Default::default()
                },
            )
            .await?
        {
            return Ok(());
        }
        self.finish(invocation, "canceled").await?;
        Ok(())
    }
    /// Execute one ingress call on the sidecar replica that owns the conversation.
    /// Returns None for gateway-deployed agents so the caller continues locally.
    pub(crate) async fn forward_sidecar<
        Req: buffa::Message + serde::Serialize,
        Resp: buffa::Message + serde::de::DeserializeOwned,
    >(
        &self,
        agent: Uuid,
        thread: Option<Uuid>,
        method: &str,
        request: &Req,
    ) -> Result<Option<Resp>> {
        let Some(deployments) = &self.deployments else {
            return Ok(None);
        };
        if !deployments
            .is_sidecar(agent)
            .await
            .map_err(|_| ChatError::Transport)?
        {
            return Ok(None);
        }
        let token = deployments
            .issue_ingress_token(agent, thread.map(|t| t.to_string()), Uuid::nil())
            .await
            .map_err(|_| ChatError::Transport)?;
        let call = crate::proto::tilde::agent_event_ingress::v1::IngressCall {
            method: method.into(),
            content_type: "application/json".into(),
            body: serde_json::to_vec(request).map_err(|_| ChatError::Transport)?,
            caller_token: token.expose_secret().into(),
            ..Default::default()
        };
        let result = deployments
            .forward(agent, thread, call)
            .await
            .map_err(|e| match e {
                crate::error::Error::ChatLifecycle(e) => e,
                crate::error::Error::Invalid(message) => ChatError::Invalid(message),
                crate::error::Error::NotFound => ChatError::NotFound,
                crate::error::Error::Denied => ChatError::Denied,
                _ => ChatError::Transport,
            })?;
        if result.status != 200 {
            #[derive(serde::Deserialize)]
            struct Failure {
                code: String,
                #[serde(default)]
                message: String,
            }
            let failure: Failure =
                serde_json::from_slice(&result.body).map_err(|_| ChatError::Transport)?;
            return Err(match failure.code.as_str() {
                "not_found" => ChatError::NotFound,
                "failed_precondition" | "already_exists" => ChatError::Conflict,
                "permission_denied" | "unauthenticated" => ChatError::Denied,
                "invalid_argument" => ChatError::Invalid(failure.message),
                _ => ChatError::Transport,
            });
        }
        serde_json::from_slice(&result.body)
            .map(Some)
            .map_err(|_| ChatError::Transport)
    }
    pub(crate) async fn forward_control<Req: buffa::Message + serde::Serialize>(
        &self,
        agent: Uuid,
        thread: Uuid,
        method: &str,
        request: &Req,
    ) -> Result<Option<()>> {
        Ok(self
            .forward_sidecar::<_, ingress_pb::CancelInvocationResponse>(
                agent,
                Some(thread),
                method,
                request,
            )
            .await?
            .map(|_| ()))
    }
    /// A message posted at the gateway into a room owned by a sidecar replica.
    /// First sidecar-deployed agent participating in a thread, if any.
    pub(crate) async fn thread_sidecar_agent(&self, thread: Uuid) -> Result<Option<Uuid>> {
        if self.deployments.is_none() {
            return Ok(None);
        }
        Ok(
            sqlx::query_file!("../../queries/deployment/thread_sidecar_agents.sql", thread)
                .fetch_optional(self.pg()?)
                .await?
                .map(|row| row.agent_id),
        )
    }
    pub(crate) async fn forward_post(
        &self,
        r: &application::PostMessage,
    ) -> Result<Option<types::Message>> {
        let thread = id(&r.thread_id)?;
        let Some(agent) = self.thread_sidecar_agent(thread).await? else {
            return Ok(None);
        };
        let request = ingress_pb::PostMessageRequest {
            id: r.id.clone(),
            thread_id: r.thread_id.clone(),
            participant_id: r.participant_id.clone(),
            text: r.text.clone(),
            addressed_participant_ids: r.addressed_participant_ids.clone(),
            in_reply_to_message_id: r.in_reply_to_message_id.clone(),
            attachment_ids: r.attachment_ids.clone(),
            ..Default::default()
        };
        let response: Option<ingress_pb::PostMessageResponse> = self
            .forward_sidecar(agent, Some(thread), "PostMessage", &request)
            .await?;
        Ok(response.and_then(|r| r.message.into_option()))
    }
    pub(crate) async fn finish(&self, invocation: Uuid, status: &str) -> Result<()> {
        let existing = sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
            .fetch_one(self.pg()?)
            .await?;
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", existing.thread_id)
            .fetch_one(&mut *tx)
            .await?;
        let suspending = sqlx::query_file!("../../queries/chat/run_state.sql", existing.run_id)
            .fetch_one(&mut *tx)
            .await?
            .status
            == "suspending";
        if let Some(row) = sqlx::query_file!(
            "../../queries/chat/invocation_finish.sql",
            invocation,
            status
        )
        .fetch_optional(&mut *tx)
        .await?
        {
            let run_status = match status {
                "stopped" => "waiting",
                "canceled" => "canceled",
                _ => "failed",
            };
            sqlx::query_file!("../../queries/chat/run_status.sql", row.run_id, run_status)
                .execute(&mut *tx)
                .await?;
            activity(
                &mut tx,
                row.thread_id,
                "invocation.ended",
                invocation,
                status,
            )
            .await?;
            if !suspending {
                super::agent_lifecycle::requeue_inputs(&mut tx, invocation, row.run_id).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
    async fn execute(&self, invocation: Uuid) -> Result<()> {
        let Some(target) =
            sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
                .fetch_optional(self.pg()?)
                .await?
        else {
            return Ok(());
        };
        let mut claim = self.pg()?.begin().await?;
        super::access::lock_thread_route(&mut claim, target.thread_id).await?;
        let Some(scope) = sqlx::query_file!("../../queries/chat/invocation_claim.sql", invocation)
            .fetch_optional(&mut *claim)
            .await?
        else {
            return Ok(());
        };
        claim.commit().await?;
        let parent = crate::telemetry::context::restore(&scope.traceparent, &scope.tracestate);
        let cx = crate::telemetry::context::start(
            "tilde.invocation",
            SpanKind::Client,
            &parent,
            vec![
                KeyValue::new("tilde.invocation.id", invocation.to_string()),
                KeyValue::new("tilde.agent.id", scope.agent_id.to_string()),
                KeyValue::new("tilde.run.id", scope.run_id.to_string()),
                KeyValue::new("tilde.thread.id", scope.thread_id.to_string()),
                KeyValue::new("rpc.system.name", "connectrpc"),
                KeyValue::new("rpc.method", "Invoke"),
            ],
        );
        let _end = crate::telemetry::context::EndOnDrop(cx.clone());
        let result=async {
            sqlx::query_file!("../../queries/tracing/execution_context.sql",invocation,crate::telemetry::context::capture().0).execute(self.pg()?).await?;
            let capability = self.tokens.issue(scope.agent_id, invocation, scope.thread_id, scope.run_id).await?;
            let row=sqlx::query_file!("../../queries/chat/invocation_endpoint.sql",invocation).fetch_one(self.pg()?).await?;
            let key=self.signing_key(row.agent_id,&row.webhook_signing_key)?;
            let messages=self.invocation_message_page(scope.thread_id,None,100,Some(invocation)).await?.messages;
            let cached=self.hydrate_converted_messages(scope.agent_id,scope.thread_id,&messages.iter().map(|m|m.id.clone()).collect::<Vec<_>>()).await?;
            let mut request=host::InvokeRequest{agent_generation:scope.generation,invocation_id:invocation.to_string(),run_id:scope.run_id.to_string(),thread_id:scope.thread_id.to_string(),agent_id:scope.agent_id.to_string(),objective:row.objective,callback_url:self.callback_url.clone(),capability:capability.expose_secret().to_owned(),messages, cached_messages:cached.into_iter().map(|c|runtime_pb::CachedAgentRepresentation{message_id:c.message_id,message_json:c.message_json,..Default::default()}).collect(),thread:self.thread(scope.thread_id).await?.into(),deployment_id:row.deployment_id.map(|d|d.to_string()).unwrap_or_default(),traceparent:crate::telemetry::context::capture().0,tracestate:crate::telemetry::context::capture().1,..Default::default()};
            let woken=self.wake(scope.agent_id,scope.thread_id,invocation,row.deployment_id,row.target.as_deref(),row.target_reference.as_deref(),row.endpoint_url.as_deref(),key.expose_secret(),&mut request).await?;
            let mut heartbeat=tokio::time::interval(Duration::from_secs(2));
            match woken {
                Woken::Stream(mut stream)=>loop {
                    // A legacy host answers on the wake's response stream; a current one
                    // reports through RunService and only keeps this stream open.
                    tokio::select! {
                        message=stream.message()=>{
                            let Some(message)=message.map_err(|_|ChatError::Transport)? else{break;};
                            let view=message.view();
                            if !view.reasoning_delta.is_empty() {
                                let mut tx=self.pg()?.begin().await?;activity(&mut tx,scope.thread_id,"reasoning.delta",invocation,view.reasoning_delta).await?;tx.commit().await?;
                            }
                        },
                        _=heartbeat.tick()=>{
                            if sqlx::query_file!("../../queries/chat/invocation_heartbeat.sql",invocation).execute(self.pg()?).await?.rows_affected()==0 {return Ok(());}
                        },
                    }
                },
                Woken::Detached=>loop {
                    // The host holds its own connection to the gateway and renews the lease
                    // through it; this task only waits for the invocation to end.
                    heartbeat.tick().await;
                    let status=sqlx::query_file!("../../queries/chat/invocation_status.sql",invocation).fetch_optional(self.pg()?).await?.map(|r|r.status);
                    if !matches!(status.as_deref(),Some("running")) {return Ok(());}
                },
            } Ok(())
        }.with_context(cx.clone()).await;
        if result.is_err() {
            cx.span()
                .set_status(opentelemetry::trace::Status::error("Invocation failed"));
        }
        self.finish(
            invocation,
            if result.is_ok() { "stopped" } else { "failed" },
        )
        .with_context(cx)
        .await?;
        result
    }
    /// Start the execution on the invocation's deployment. A live instance that dialed
    /// in receives the wake as a frame; a direct endpoint is woken over HTTP; a Lambda
    /// function is invoked asynchronously. Only the HTTP path yields a response stream.
    #[allow(clippy::too_many_arguments)]
    async fn wake(
        &self,
        agent: Uuid,
        thread: Uuid,
        invocation: Uuid,
        deployment: Option<Uuid>,
        target: Option<&str>,
        reference: Option<&str>,
        endpoint: Option<&str>,
        key: &str,
        request: &mut host::InvokeRequest,
    ) -> Result<Woken> {
        if target == Some("aws_lambda") {
            let reference = reference.ok_or(ChatError::Transport)?;
            let payload = serde_json::to_vec(&*request).map_err(|_| ChatError::Transport)?;
            crate::deployment::wake::lambda(reference, &payload)
                .await
                .map_err(|_| ChatError::Transport)?;
            return Ok(Woken::Detached);
        }
        if let (Some(deployments), Some(deployment)) = (&self.deployments, deployment)
            && let Ok(Some(instance)) = deployments.connected_instance(agent, deployment).await
        {
            let command = Uuid::new_v4();
            request.command_id = command.to_string();
            let queued = deployments
                .direct_as(
                    command,
                    agent,
                    instance,
                    Some(thread),
                    crate::proto::tilde::agent_event_ingress::v1::directive::Action::Wake(
                        Box::new(request.clone()),
                    ),
                )
                .await
                .is_ok();
            if queued
                && deployments
                    .wait_directive(command, Duration::from_secs(10))
                    .await
                    .is_ok()
            {
                return Ok(Woken::Detached);
            }
            tracing::warn!(agent_id=%agent, instance_id=%instance, invocation_id=%invocation, "Connected instance did not accept the wake; falling back to its endpoint");
            // The endpoint runs it now; the queued frame must not run it again on reconnect.
            if queued {
                let _ = deployments.acknowledge_wake(agent, command).await;
            }
            request.command_id.clear();
        }
        let endpoint = endpoint.ok_or(ChatError::Transport)?;
        let stream = tokio::time::timeout(
            Duration::from_secs(10),
            client(endpoint, key, "Invoke", &*request)?.invoke(request.clone()),
        )
        .await
        .map_err(|_| ChatError::Transport)?
        .map_err(|_| ChatError::Transport)?;
        Ok(Woken::Stream(Box::new(stream)))
    }
    /// A host's account of its invocation, sent with the invocation capability.
    pub async fn report(
        &self,
        agent: Uuid,
        thread: Uuid,
        invocation: Uuid,
        event: Option<crate::proto::tilde::run::v1::report_request::Event>,
    ) -> Result<()> {
        use crate::proto::tilde::run::v1::report_request::Event;
        match event {
            Some(Event::Accepted(accepted)) => {
                if let (Some(deployments), Ok(command)) =
                    (&self.deployments, id(&accepted.command_id))
                {
                    deployments
                        .acknowledge_wake(agent, command)
                        .await
                        .map_err(|_| ChatError::Transport)?;
                }
                sqlx::query_file!("../../queries/chat/invocation_heartbeat.sql", invocation)
                    .execute(self.pg()?)
                    .await?;
            }
            Some(Event::ReasoningDelta(delta)) => {
                if !delta.is_empty() {
                    let mut tx = self.pg()?.begin().await?;
                    activity(&mut tx, thread, "reasoning.delta", invocation, &delta).await?;
                    tx.commit().await?;
                }
                sqlx::query_file!("../../queries/chat/invocation_heartbeat.sql", invocation)
                    .execute(self.pg()?)
                    .await?;
            }
            Some(Event::Stopped(stopped)) => {
                let status = if stopped.error.is_empty() {
                    "stopped"
                } else {
                    tracing::warn!(invocation_id=%invocation, error=%stopped.error, "Host reported a failed invocation");
                    "failed"
                };
                self.finish(invocation, status).await?;
            }
            None => return Err(ChatError::Invalid("Report requires an event".into())),
        }
        Ok(())
    }
    /// Extend a running invocation's lease; a no-op for anything not running.
    pub(crate) async fn renew_lease(&self, invocation: Uuid) -> Result<()> {
        if self.local().is_some() {
            return Ok(());
        }
        sqlx::query_file!("../../queries/chat/invocation_heartbeat.sql", invocation)
            .execute(self.pg()?)
            .await?;
        Ok(())
    }
    async fn expire_invocations(&self) -> Result<()> {
        let mut tx = self.pg()?.begin().await?;
        let rows = sqlx::query_file!("../../queries/chat/invocation_expire.sql")
            .fetch_all(&mut *tx)
            .await?;
        for row in rows {
            sqlx::query_file!("../../queries/chat/run_status.sql", row.run_id, "failed")
                .execute(&mut *tx)
                .await?;
            activity(&mut tx, row.thread_id, "invocation.ended", row.id, "failed").await?;
            super::agent_lifecycle::requeue_inputs(&mut tx, row.id, row.run_id).await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// Supervise bounded, independent invocations. Expired execution is failed, never blindly replayed.
    pub async fn worker(self, shutdown: tokio::sync::watch::Receiver<bool>) {
        if let Some(local) = self.local() {
            return local.as_ref().clone().worker(shutdown).await;
        }
        let Ok(pool) = self.pg() else {
            return;
        };
        let mut shutdown = shutdown;
        let mut workers = tokio::task::JoinSet::new();
        let mut changed = loop {
            tokio::select! {
                _ = shutdown.changed() => return,
                result = self.work_notifications.subscribe(pool, "tilde_chat_work") => {
                    match result {
                        Ok(changed) => break changed,
                        Err(_) => tracing::warn!("Unable to listen for agent work"),
                    }
                }
            }
            tokio::select! {
                _ = shutdown.changed() => return,
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            }
        };
        changed.mark_changed(); // Catch work committed before LISTEN, including restarts.
        let mut cleanup = tokio::time::interval(Duration::from_secs(1));
        let mut retry = None;
        loop {
            tokio::select! {
                _ = shutdown.changed() => break,
                _ = cleanup.tick() => {
                    let _ = self.expire_invocations().await;
                    let _ = self.expire_messages().await;
                    let _ = self.expire_typing().await;
                    let _ = self.expire_tool_calls().await;
                    let _ = sqlx::query_file!("../../queries/channel_access/cleanup.sql").execute(pool).await;
                    continue;
                }
                _ = workers.join_next(), if !workers.is_empty() => {}
                _ = async {
                    if let Some(deadline) = retry { tokio::time::sleep_until(deadline).await; }
                    else { let _ = changed.changed().await; }
                } => {}
            }
            changed.borrow_and_update();
            retry = None;
            match self.route_pending().await {
                Ok(true) => changed.mark_changed(), // Drain batches without another notification.
                Ok(false) => {}
                Err(_) => {
                    tracing::warn!("Unable to route pending messages");
                    retry = Some(tokio::time::Instant::now() + Duration::from_secs(1));
                }
            }
            if workers.len() < 32 {
                match sqlx::query_file!("../../queries/chat/invocations_pending.sql")
                    .fetch_all(pool)
                    .await
                {
                    Ok(rows) => {
                        for row in rows.into_iter().take(32 - workers.len()) {
                            let chat = self.clone();
                            workers.spawn(async move {
                            if chat.execute(row.id).await.is_err() {
                                tracing::warn!(invocation_id=%row.id, "Agent invocation failed");
                            }
                        });
                        }
                    }
                    Err(_) => {
                        tracing::warn!("Unable to claim agent work");
                        retry = Some(tokio::time::Instant::now() + Duration::from_secs(1));
                    }
                }
            }
        }
        workers.abort_all();
        while workers.join_next().await.is_some() {}
    }
}

/// How an execution was started, and whether this task must consume a response stream.
enum Woken {
    Stream(
        Box<
            connectrpc::client::ServerStream<
                <HttpClient as connectrpc::client::ClientTransport>::ResponseBody,
                host::__buffa::view::InvokeResponseView<'static>,
            >,
        >,
    ),
    Detached,
}
