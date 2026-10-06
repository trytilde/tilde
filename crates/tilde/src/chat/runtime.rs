//! ConnectRPC dispatch and API-owned per-agent message concurrency.
//! Queued inputs become fresh invocations; interrupt revokes old callbacks and sends Cancel.
//! Hosts only receive invocation snapshots and cancellation signals, never a steering queue.
use crate::chat as application;
use crate::chat::{Chat, ChatError, Result, activity, id, text};
use crate::proto::tilde::agent_host::v1 as host;
use crate::proto::tilde::provider::tilde::v1 as ingress_pb;
use crate::proto::tilde::runtime::v1 as runtime_pb;
use crate::proto::tilde::types::v1 as types;

use opentelemetry::{
    KeyValue,
    trace::{FutureExt, SpanKind, TraceContextExt},
};
use secrecy::ExposeSecret;

use std::time::Duration;
use uuid::Uuid;

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
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        super::access::lock_thread_route(&tx, thread).await?;
        crate::chat::db::agent_available_opt(&tx, agent)
            .await?
            .ok_or(ChatError::Conflict)?;
        crate::chat::db::thread_lock_one(&tx, thread).await?;
        if !crate::chat::access::db::run_allowed_one(&tx, agent, thread, None::<Uuid>)
            .await?
            .allowed
        {
            return Err(ChatError::Denied);
        }
        let participant =
            crate::chat::db::participant_find_opt(&tx, thread, None::<Uuid>, Some(agent))
                .await?
                .ok_or(ChatError::NotFound)?;
        if !participant.active {
            return Err(ChatError::NotFound);
        }
        let inserted = crate::chat::db::run_create_opt(
            &tx,
            run,
            thread,
            agent,
            &(r.objective),
            goal,
            &(r.idempotency_key),
        )
        .await?;
        let actual = if inserted.is_some() {
            if crate::chat::db::invocation_active_opt(&tx, thread, agent)
                .await?
                .is_some()
            {
                return Err(ChatError::Conflict);
            }
            let deployment = super::pin_deployment(&tx, thread, agent).await?;
            if self.deployments.is_some() && deployment.is_none() {
                return Err(ChatError::Invalid(
                    "No live, ready deployment is available".into(),
                ));
            }
            crate::chat::db::invocation_create_execute(
                &tx,
                invocation,
                run,
                thread,
                agent,
                &(crate::telemetry::tracing::context::capture().0),
                &(crate::telemetry::tracing::context::capture().1),
                deployment,
            )
            .await?;
            activity(
                self.pg()?,
                &tx,
                thread,
                "invocation.pending",
                invocation,
                "",
            )
            .await?;
            run
        } else {
            let existing =
                crate::chat::db::run_replay_one(&tx, thread, agent, &(r.idempotency_key)).await?;
            if existing.objective != r.objective || existing.goal_id != goal {
                return Err(ChatError::Conflict);
            }
            existing.id
        };
        tx.commit().await?;
        drop(tx_client);
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
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        super::access::lock_thread_route(&tx, thread).await?;
        crate::chat::db::agent_available_opt(&tx, agent)
            .await?
            .ok_or(ChatError::Conflict)?;
        crate::chat::db::thread_lock_one(&tx, thread).await?;
        let row = crate::chat::db::run_resume_opt(&tx, run)
            .await?
            .ok_or(ChatError::Conflict)?;
        let invocation = Uuid::new_v4();
        let deployment = super::pin_deployment(&tx, row.thread_id, row.agent_id).await?;
        crate::chat::db::invocation_create_execute(
            &tx,
            invocation,
            run,
            row.thread_id,
            row.agent_id,
            &(crate::telemetry::tracing::context::capture().0),
            &(crate::telemetry::tracing::context::capture().1),
            deployment,
        )
        .await?;
        crate::chat::db::input_move_execute(&tx, id(&old.invocation_id)?, invocation).await?;
        activity(
            self.pg()?,
            &tx,
            thread,
            "invocation.pending",
            invocation,
            "",
        )
        .await?;
        tx.commit().await?;
        drop(tx_client);
        self.run(run).await
    }
    async fn route_pending(&self) -> Result<bool> {
        let messages = crate::chat::db::route_pending_all(&self.pg()?.get().await?).await?;
        let more = messages.len() == 50;
        for message in messages {
            let mut tx_client = self.pg()?.get().await?;
            let tx = tx_client.transaction().await?;
            super::access::lock_thread_route(&tx, message.thread_id).await?;
            if !crate::chat::access::db::message_allowed_one(
                &tx,
                message.agent_id,
                message.thread_id,
                message.source_identity_id,
            )
            .await?
            .allowed
            {
                continue;
            }
            let Some(agent) = crate::chat::db::agent_available_opt(&tx, message.agent_id).await?
            else {
                continue;
            };
            crate::chat::db::thread_lock_one(&tx, message.thread_id).await?;
            let deployment =
                super::pin_deployment(&tx, message.thread_id, message.agent_id).await?;
            if deployment.is_none() {
                continue;
            }
            let sidecar = if let Some(deployment) = deployment {
                crate::deployment::db::deployment_get_opt(&tx, deployment, message.agent_id)
                    .await?
                    .is_some_and(|d| d.target == "sidecar")
            } else {
                false
            };
            if sidecar {
                tx.commit().await?;
                continue; // The relay worker delivers the same canonical message to its replica.
            }
            if crate::chat::db::route_claim_execute(&tx, message.id, message.agent_id).await? == 0 {
                continue;
            }
            if let Some(active) =
                crate::chat::db::invocation_active_opt(&tx, message.thread_id, message.agent_id)
                    .await?
            {
                crate::chat::db::input_create_execute(&tx, active.id, message.id, &(message.text))
                    .await?;
                match agent.concurrency_policy.as_str() {
                    "interrupt" => {
                        crate::chat::db::invocation_finish_one(&tx, active.id, "canceled").await?;
                        crate::chat::db::run_status_execute(&tx, active.run_id, "canceled").await?;
                        activity(
                            self.pg()?,
                            &tx,
                            message.thread_id,
                            "invocation.ended",
                            active.id,
                            "canceled",
                        )
                        .await?;
                        super::agent_lifecycle::requeue_inputs(
                            self.pg()?,
                            &tx,
                            active.id,
                            active.run_id,
                        )
                        .await?;
                    }
                    "queue_and_batch" if active.status == "pending" => {
                        crate::chat::db::invocation_batch_execute(&tx, active.id, &(message.text))
                            .await?;
                        crate::chat::db::invocation_cutoff_execute(
                            &tx,
                            active.id,
                            Some(message.id),
                        )
                        .await?;
                        crate::chat::db::input_accept_execute(&tx, active.id, message.id).await?;
                    }
                    _ => {}
                }
                activity(
                    self.pg()?,
                    &tx,
                    message.thread_id,
                    "queue.updated",
                    message.id,
                    "",
                )
                .await?;
            } else {
                let run = Uuid::new_v4();
                let invocation = Uuid::new_v4();
                let key = message.id.to_string();
                crate::chat::db::run_create_one(
                    &tx,
                    run,
                    message.thread_id,
                    message.agent_id,
                    &(message.text),
                    None::<Uuid>,
                    &(key),
                )
                .await?;
                crate::chat::access::db::run_source_execute(
                    &tx,
                    run,
                    message.source_identity_id,
                    message.channel_origin,
                )
                .await?;
                let deployment =
                    super::pin_deployment(&tx, message.thread_id, message.agent_id).await?;
                crate::chat::db::invocation_create_execute(
                    &tx,
                    invocation,
                    run,
                    message.thread_id,
                    message.agent_id,
                    &(message.traceparent),
                    &(message.tracestate),
                    deployment,
                )
                .await?;
                crate::chat::db::invocation_cutoff_execute(&tx, invocation, Some(message.id))
                    .await?;
                activity(
                    self.pg()?,
                    &tx,
                    message.thread_id,
                    "invocation.pending",
                    invocation,
                    "",
                )
                .await?;
            }
            tx.commit().await?;
            drop(tx_client);
        }
        Ok(more)
    }
    /// Return durable work and the latest invocation independently.
    pub async fn run(&self, run: Uuid) -> Result<types::Run> {
        if let Some(local) = self.local() {
            return local.run(run).await;
        }
        let r = crate::chat::db::run_get_opt(&self.pg()?.get().await?, run)
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
        let target = crate::chat::db::invocation_endpoint_opt(&self.pg()?.get().await?, invocation)
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
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        let row = crate::chat::db::invocation_endpoint_opt(&tx, invocation)
            .await?
            .ok_or(ChatError::NotFound)?;
        super::access::lock_thread_route(&tx, row.thread_id).await?;
        let agent = crate::chat::db::agent_available_opt(&tx, row.agent_id)
            .await?
            .ok_or(ChatError::Conflict)?;
        crate::chat::db::thread_lock_one(&tx, row.thread_id).await?;
        if let Some(existing) = crate::chat::db::input_get_opt(&tx, invocation, input).await? {
            return if existing.text == r.text {
                Ok(())
            } else {
                Err(ChatError::Conflict)
            };
        }
        let current = crate::chat::db::invocation_endpoint_one(&tx, invocation).await?;
        if !["pending", "running"].contains(&current.status.as_str()) {
            return Err(ChatError::Conflict);
        }
        crate::chat::db::input_create_execute(&tx, invocation, input, &(r.text)).await?;
        if agent.concurrency_policy == "interrupt" {
            crate::chat::db::invocation_finish_one(&tx, invocation, "canceled").await?;
            crate::chat::db::run_status_execute(&tx, row.run_id, "canceled").await?;
            activity(
                self.pg()?,
                &tx,
                row.thread_id,
                "invocation.ended",
                invocation,
                "canceled",
            )
            .await?;
            super::agent_lifecycle::requeue_inputs(self.pg()?, &tx, invocation, row.run_id).await?;
        } else if agent.concurrency_policy == "queue_and_batch" && current.status == "pending" {
            let event = crate::chat::db::input_get_one(&tx, invocation, input).await?;
            crate::chat::db::invocation_batch_execute(&tx, invocation, &(r.text)).await?;
            crate::chat::db::invocation_cutoff_execute(
                &tx,
                invocation,
                event.history_through_message_id,
            )
            .await?;
            crate::chat::db::input_accept_execute(&tx, invocation, input).await?;
        }
        activity(self.pg()?, &tx, row.thread_id, "queue.updated", input, "").await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    /// Queue `text` as an input of a live invocation without applying the agent's concurrency
    /// policy: the invocation reads it, or it starts a fresh invocation once this one ends. For
    /// deliveries that must never cancel the invocation, such as background tool results, which
    /// are gateway-only like the tools that produce them. Conflict when the invocation has ended.
    pub(crate) async fn queue_input(
        &self,
        invocation: Uuid,
        input: Uuid,
        text: &str,
    ) -> Result<()> {
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        let row = crate::chat::db::invocation_endpoint_opt(&tx, invocation)
            .await?
            .ok_or(ChatError::NotFound)?;
        super::access::lock_thread_route(&tx, row.thread_id).await?;
        crate::chat::db::thread_lock_one(&tx, row.thread_id).await?;
        if let Some(existing) = crate::chat::db::input_get_opt(&tx, invocation, input).await? {
            return if existing.text == text {
                Ok(())
            } else {
                Err(ChatError::Conflict)
            };
        }
        let current = crate::chat::db::invocation_endpoint_one(&tx, invocation).await?;
        if !["pending", "running"].contains(&current.status.as_str()) {
            return Err(ChatError::Conflict);
        }
        crate::chat::db::input_create_execute(&tx, invocation, input, text).await?;
        activity(self.pg()?, &tx, row.thread_id, "queue.updated", input, "").await?;
        tx.commit().await?;
        Ok(())
    }
    /// Revoke callbacks immediately and persist cancellation; the execution observes cancellation on its control stream.
    pub async fn cancel_invocation(&self, invocation: Uuid) -> Result<()> {
        if let Some(local) = self.local() {
            return local.cancel_invocation(invocation).await;
        }
        let target = crate::chat::db::invocation_endpoint_opt(&self.pg()?.get().await?, invocation)
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
            .is_sidecar(agent, thread.ok_or(ChatError::NotFound)?)
            .await
            .map_err(|_| ChatError::Transport)?
        {
            return Ok(None);
        }
        let token = deployments
            .issue_ingress_token(agent, thread.map(|t| t.to_string()))
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
        let rows =
            crate::deployment::db::thread_sidecar_agents_all(&self.pg()?.get().await?, thread)
                .await?;
        for row in rows {
            if self
                .deployments
                .as_ref()
                .unwrap()
                .is_sidecar(row.agent_id, thread)
                .await
                .map_err(|_| ChatError::Transport)?
            {
                return Ok(Some(row.agent_id));
            }
        }
        Ok(None)
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
        let existing =
            crate::chat::db::invocation_endpoint_one(&self.pg()?.get().await?, invocation).await?;
        self.warm
            .forget_invocation(existing.agent_id, existing.thread_id, invocation);
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, existing.thread_id).await?;
        let suspending = crate::chat::db::run_state_one(&tx, existing.run_id)
            .await?
            .status
            == "suspending";
        if let Some(row) = crate::chat::db::invocation_finish_opt(&tx, invocation, status).await? {
            let run_status = match status {
                "stopped" => "waiting",
                "canceled" => "canceled",
                _ => "failed",
            };
            crate::chat::db::run_status_execute(&tx, row.run_id, run_status).await?;
            activity(
                self.pg()?,
                &tx,
                row.thread_id,
                "invocation.ended",
                invocation,
                status,
            )
            .await?;
            if !suspending {
                super::agent_lifecycle::requeue_inputs(self.pg()?, &tx, invocation, row.run_id)
                    .await?;
            }
        }
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    async fn execute(&self, invocation: Uuid) -> Result<()> {
        let Some(target) =
            crate::chat::db::invocation_endpoint_opt(&self.pg()?.get().await?, invocation).await?
        else {
            return Ok(());
        };
        let mut claim_client = self.pg()?.get().await?;
        let claim = claim_client.transaction().await?;
        super::access::lock_thread_route(&claim, target.thread_id).await?;
        let Some(scope) = crate::chat::db::invocation_claim_opt(&claim, invocation).await? else {
            return Ok(());
        };
        claim.commit().await?;
        drop(claim_client);
        // Off the wake path: the agent's first tool and credential reads are already in memory.
        tokio::spawn(self.clone().prime(scope.agent_id, scope.thread_id));
        let parent =
            crate::telemetry::tracing::context::restore(&scope.traceparent, &scope.tracestate);
        let cx = crate::telemetry::tracing::context::start(
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
        let _end = crate::telemetry::tracing::context::EndOnDrop(cx.clone());
        let result = async {
            crate::telemetry::tracing::db::execution_context_execute(
                &self.pg()?.get().await?,
                invocation,
                &(crate::telemetry::tracing::context::capture().0),
            )
            .await?;
            let capability = self
                .tokens
                .issue(scope.agent_id, invocation, scope.thread_id, scope.run_id)
                .await?;
            let row =
                crate::chat::db::invocation_endpoint_one(&self.pg()?.get().await?, invocation)
                    .await?;
            let page = self
                .invocation_message_page(scope.thread_id, None, 100, Some(invocation))
                .await?;
            let messages = page.messages;
            let cached = page.cached_messages;
            // Pushed so the agent updates its local skills by version instead of listing them.
            // Without it (the lookup failed) the SDK lists them itself; an empty list would
            // instead remove every skill it holds.
            let state = match self
                .skills()?
                .for_agent(scope.agent_id, Some(invocation))
                .await
            {
                Ok(skills) => Some(host::InvocationState {
                    skills: skills
                        .into_iter()
                        .map(crate::skills::rpc::summary_wire)
                        .collect(),
                    ..Default::default()
                }),
                Err(error) => {
                    tracing::warn!(%invocation, %error, "invocation skills not pushed");
                    None
                }
            };
            let mut request = host::InvokeRequest {
                agent_generation: scope.generation,
                invocation_id: invocation.to_string(),
                run_id: scope.run_id.to_string(),
                thread_id: scope.thread_id.to_string(),
                agent_id: scope.agent_id.to_string(),
                objective: row.objective,
                callback_url: self.callback_url.clone(),
                capability: capability.expose_secret().to_owned(),
                messages,
                cached_messages: cached
                    .into_iter()
                    .map(|c| runtime_pb::CachedAgentRepresentation {
                        message_id: c.message_id,
                        message_json: c.message_json,
                        ..Default::default()
                    })
                    .collect(),
                thread: self.thread(scope.thread_id).await?.into(),
                deployment_id: row.deployment_id.map(|d| d.to_string()).unwrap_or_default(),
                traceparent: crate::telemetry::tracing::context::capture().0,
                tracestate: crate::telemetry::tracing::context::capture().1,
                state: state.into(),
                ..Default::default()
            };
            // An agent with a sandbox runs only once its sandbox is up and connected. Starting
            // one can outlast the invocation's lease, which is renewed meanwhile; an invocation
            // canceled in the meantime is not woken.
            if let Some(tools) = &self.tools {
                let ensure = tools.sandboxes.ensure(
                    scope.agent_id,
                    scope.thread_id,
                    scope.run_id,
                    invocation,
                );
                tokio::pin!(ensure);
                let mut renew = tokio::time::interval(Duration::from_secs(10));
                loop {
                    tokio::select! {
                        result = &mut ensure => {
                            result.map_err(|error| ChatError::Invalid(error.to_string()))?;
                            break;
                        }
                        _ = renew.tick() => self.renew_lease(invocation).await?,
                    }
                }
                let status =
                    crate::chat::db::invocation_status_opt(&self.pg()?.get().await?, invocation)
                        .await?
                        .map(|r| r.status);
                if status.as_deref() != Some("running") {
                    return Ok(());
                }
            }
            self.wake(
                scope.agent_id,
                scope.thread_id,
                invocation,
                row.deployment_id,
                row.target.as_deref(),
                row.target_reference.as_deref(),
                &mut request,
            )
            .await?;
            let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
            loop {
                // The host holds its own connection to the gateway and renews the lease
                // through it; this task only waits for the invocation to end.
                heartbeat.tick().await;
                let status =
                    crate::chat::db::invocation_status_opt(&self.pg()?.get().await?, invocation)
                        .await?
                        .map(|r| r.status);
                if !matches!(status.as_deref(), Some("running")) {
                    return Ok(());
                }
            }
        }
        .with_context(cx.clone())
        .await;
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
    /// in receives the wake as a frame; Lambda is invoked asynchronously.
    #[allow(clippy::too_many_arguments)]
    async fn wake(
        &self,
        agent: Uuid,
        thread: Uuid,
        invocation: Uuid,
        deployment: Option<Uuid>,
        target: Option<&str>,
        reference: Option<&str>,
        request: &mut host::InvokeRequest,
    ) -> Result<()> {
        if let (Some(deployments), Some(id)) = (&self.deployments, deployment)
            && !deployments
                .deployment(agent, id)
                .await
                .map_err(|_| ChatError::Transport)?
                .routable
        {
            return Err(ChatError::Invalid(
                "The pinned deployment has no live, ready connection".into(),
            ));
        }
        if target == Some("lambda") {
            let reference = reference.ok_or(ChatError::Transport)?;
            let payload = serde_json::to_vec(&*request).map_err(|_| ChatError::Transport)?;
            crate::deployment::wake::lambda(reference, &payload)
                .await
                .map_err(|_| ChatError::Transport)?;
            return Ok(());
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
                return Ok(());
            }
            tracing::warn!(agent_id=%agent, instance_id=%instance, invocation_id=%invocation, "Connected instance did not accept the wake");
            // The invocation fails; discard the wake so a reconnect cannot execute it later.
            if queued {
                let _ = deployments.acknowledge_wake(agent, command).await;
            }
            request.command_id.clear();
        }
        Err(ChatError::Transport)
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
                crate::chat::db::invocation_heartbeat_execute(&self.pg()?.get().await?, invocation)
                    .await?;
            }
            Some(Event::ReasoningDelta(delta)) => {
                if !delta.is_empty() {
                    let mut tx_client = self.pg()?.get().await?;
                    let tx = tx_client.transaction().await?;
                    activity(
                        self.pg()?,
                        &tx,
                        thread,
                        "reasoning.delta",
                        invocation,
                        &delta,
                    )
                    .await?;
                    tx.commit().await?;
                    drop(tx_client);
                }
                crate::chat::db::invocation_heartbeat_execute(&self.pg()?.get().await?, invocation)
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
        crate::chat::db::invocation_heartbeat_execute(&self.pg()?.get().await?, invocation).await?;
        Ok(())
    }
    async fn expire_invocations(&self) -> Result<()> {
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        let rows = crate::chat::db::invocation_expire_all(&tx).await?;
        for row in rows {
            crate::chat::db::run_status_execute(&tx, row.run_id, "failed").await?;
            activity(
                self.pg()?,
                &tx,
                row.thread_id,
                "invocation.ended",
                row.id,
                "failed",
            )
            .await?;
            super::agent_lifecycle::requeue_inputs(self.pg()?, &tx, row.id, row.run_id).await?;
        }
        tx.commit().await?;
        drop(tx_client);
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
        tokio::spawn(self.warm.clone().worker(pool.clone(), shutdown.clone()));
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
                    if let Some(tools) = &self.tools {
                        let _ = tools.hosts.expire().await;
                    }
                    let _ = async { crate::chat::access::db::cleanup_execute(&pool.get().await?).await }.await;
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
                match async { crate::chat::db::invocations_pending_all(&pool.get().await?).await }
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
