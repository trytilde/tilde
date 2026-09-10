//! ConnectRPC agent dispatch, durable pending invocations, steering, and cancellation.
//!
//! One invocation per thread/agent can be active; other threads and agents run independently.
//! New input becomes durable steering. Inputs acknowledged by the host but not consumed by
//! the reasoning loop carry into another invocation. A new message after execution ends
//! starts a new run; explicit resume continues an existing waiting or failed objective.
//! Pending work survives restarts. Expired active leases fail for explicit resume rather
//! than automatically replaying potentially completed external effects.
use crate::chat as application;
use crate::proto::tilde::agent_host::v1 as host;
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
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", thread)
            .fetch_one(&mut *tx)
            .await?;
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
            sqlx::query_file!(
                "../../queries/chat/invocation_create.sql",
                invocation,
                run,
                thread,
                agent,
                crate::telemetry::context::capture().0,
                crate::telemetry::context::capture().1
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
        let old = self.run(run).await?;
        let thread = id(&old.thread_id)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", thread)
            .fetch_one(&mut *tx)
            .await?;
        let row = sqlx::query_file!("../../queries/chat/run_resume.sql", run)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ChatError::Conflict)?;
        let invocation = Uuid::new_v4();
        sqlx::query_file!(
            "../../queries/chat/invocation_create.sql",
            invocation,
            run,
            row.thread_id,
            row.agent_id,
            crate::telemetry::context::capture().0,
            crate::telemetry::context::capture().1
        )
        .execute(&mut *tx)
        .await?;
        activity(&mut tx, thread, "invocation.pending", invocation, "").await?;
        tx.commit().await?;
        self.run(run).await
    }
    async fn route_pending(&self) -> Result<bool> {
        let messages = sqlx::query_file!("../../queries/chat/route_pending.sql")
            .fetch_all(&self.pool)
            .await?;
        let more = messages.len() == 50;
        for message in messages {
            let mut tx = self.pool.begin().await?;
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
                    "../../queries/chat/invocation_create.sql",
                    invocation,
                    run,
                    message.thread_id,
                    message.agent_id,
                    message.traceparent,
                    message.tracestate
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
        let r = sqlx::query_file!("../../queries/chat/run_get.sql", run)
            .fetch_optional(&self.pool)
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
    /// Persist steering first; the worker retries until the current host acknowledges it.
    pub async fn steer_invocation(&self, r: application::SteerInvocation) -> Result<()> {
        text(&r.text)?;
        let invocation = id(&r.invocation_id)?;
        let input = id(&r.input_id)?;
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ChatError::NotFound)?;
        if !["pending", "running"].contains(&row.status.as_str()) {
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
        let existing = sqlx::query_file!("../../queries/chat/input_get.sql", invocation, input)
            .fetch_one(&mut *tx)
            .await?;
        if existing.text != r.text {
            return Err(ChatError::Conflict);
        }
        tx.commit().await?;
        Ok(())
    }
    /// Revoke callbacks immediately and persist cancellation; the runtime receives a signed Cancel RPC.
    pub async fn cancel_invocation(&self, invocation: Uuid) -> Result<()> {
        let row = sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ChatError::NotFound)?;
        self.finish(invocation, "canceled").await?;
        if row.status == "running" {
            let endpoint = row.endpoint_url.ok_or(ChatError::Transport)?;
            let key = self.signing_key(row.agent_id, &row.webhook_signing_key)?;
            let request = host::CancelRequest {
                invocation_id: invocation.to_string(),
                ..Default::default()
            };
            tokio::time::timeout(
                Duration::from_secs(5),
                client(&endpoint, key.expose_secret(), "Cancel", &request)?.cancel(request),
            )
            .await
            .map_err(|_| ChatError::Transport)?
            .map_err(|_| ChatError::Transport)?;
        }
        Ok(())
    }
    pub(crate) async fn finish(&self, invocation: Uuid, status: &str) -> Result<()> {
        let existing = sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
            .fetch_one(&self.pool)
            .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", existing.thread_id)
            .fetch_one(&mut *tx)
            .await?;
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
            if status == "stopped" {
                let inputs = sqlx::query_file!("../../queries/chat/inputs_pending.sql", invocation)
                    .fetch_all(&mut *tx)
                    .await?;
                if !inputs.is_empty() {
                    let state = sqlx::query_file!("../../queries/chat/run_state.sql", row.run_id)
                        .fetch_one(&mut *tx)
                        .await?;
                    let next_run = if state.status == "waiting" {
                        sqlx::query_file!("../../queries/chat/run_resume.sql", row.run_id)
                            .fetch_one(&mut *tx)
                            .await?;
                        row.run_id
                    } else {
                        let run = Uuid::new_v4();
                        let objective = inputs
                            .iter()
                            .map(|v| v.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let key = format!("pending:{invocation}");
                        sqlx::query_file!(
                            "../../queries/chat/run_create.sql",
                            run,
                            state.thread_id,
                            state.agent_id,
                            objective,
                            None::<Uuid>,
                            key
                        )
                        .fetch_one(&mut *tx)
                        .await?;
                        run
                    };
                    let next = Uuid::new_v4();
                    sqlx::query_file!(
                        "../../queries/chat/invocation_create.sql",
                        next,
                        next_run,
                        state.thread_id,
                        state.agent_id,
                        crate::telemetry::context::capture().0,
                        crate::telemetry::context::capture().1
                    )
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query_file!("../../queries/chat/input_move.sql", invocation, next)
                        .execute(&mut *tx)
                        .await?;
                    activity(&mut tx, row.thread_id, "invocation.pending", next, "").await?;
                }
            }
        }
        tx.commit().await?;
        Ok(())
    }
    async fn execute(&self, invocation: Uuid) -> Result<()> {
        let Some(scope) = sqlx::query_file!("../../queries/chat/invocation_claim.sql", invocation)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(());
        };
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
            sqlx::query_file!("../../queries/tracing/execution_context.sql",invocation,crate::telemetry::context::capture().0).execute(&self.pool).await?;
            let capability = self.tokens.issue(scope.agent_id, invocation, scope.thread_id, scope.run_id).await?;
            let row=sqlx::query_file!("../../queries/chat/invocation_endpoint.sql",invocation).fetch_one(&self.pool).await?;
            let endpoint=row.endpoint_url.ok_or(ChatError::Transport)?;let key=self.signing_key(row.agent_id,&row.webhook_signing_key)?;
            let messages=self.messages(scope.thread_id,100).await?;
            let cached=self.hydrate_converted_messages(scope.agent_id,scope.thread_id,&messages.iter().map(|m|m.id.clone()).collect::<Vec<_>>()).await?;
            let request=host::InvokeRequest{invocation_id:invocation.to_string(),run_id:scope.run_id.to_string(),thread_id:scope.thread_id.to_string(),agent_id:scope.agent_id.to_string(),objective:row.objective,callback_url:self.callback_url.clone(),capability:capability.expose_secret().to_owned(),messages, cached_messages:cached.into_iter().map(|c|runtime_pb::CachedAgentRepresentation{message_id:c.message_id,message_json:c.message_json,..Default::default()}).collect(),thread:self.thread(scope.thread_id).await?.into(),..Default::default()};
            let mut stream=tokio::time::timeout(Duration::from_secs(10),client(&endpoint,key.expose_secret(),"Invoke",&request)?.invoke(request)).await.map_err(|_|ChatError::Transport)?.map_err(|_|ChatError::Transport)?;
            let mut inputs = self.input_notifications.subscribe(&self.pool, "tilde_chat_input").await?;
            inputs.mark_changed();
            let mut input_retry = None;
            let mut heartbeat=tokio::time::interval(Duration::from_secs(2));
            loop {
                tokio::select! {
                    message=stream.message()=>{
                        let Some(message)=message.map_err(|_|ChatError::Transport)? else{break;};
                        let view=message.view();
                        for pending in &view.pending_input_ids {
                            sqlx::query_file!("../../queries/chat/input_unaccept.sql",invocation,id(pending)?).execute(&self.pool).await?;
                        }
                        if !view.reasoning_delta.is_empty() {
                            let mut tx=self.pool.begin().await?;activity(&mut tx,scope.thread_id,"reasoning.delta",invocation,view.reasoning_delta).await?;tx.commit().await?;
                        }
                    },
                    _=heartbeat.tick()=>{
                        if sqlx::query_file!("../../queries/chat/invocation_heartbeat.sql",invocation).execute(&self.pool).await?.rows_affected()==0 {return Ok(());}
                    },
                    _ = async {
                        if let Some(deadline) = input_retry { tokio::time::sleep_until(deadline).await; }
                        else { let _ = inputs.changed().await; }
                    } => {
                        inputs.borrow_and_update();
                        input_retry = None;
                        let pending = sqlx::query_file!("../../queries/chat/inputs_pending.sql",invocation).fetch_all(&self.pool).await?;
                        if pending.len() > 8 { inputs.mark_changed(); }
                        for input in pending.into_iter().take(8) {
                            let request=host::SteerRequest{invocation_id:invocation.to_string(),input_id:input.id.to_string(),text:input.text,message:self.steering_message(scope.thread_id,input.id).await?.into(),..Default::default()};
                            let sent=tokio::time::timeout(Duration::from_secs(1),client(&endpoint,key.expose_secret(),"Steer",&request)?.steer(request)).await;
                            if matches!(sent,Ok(Ok(_))) {sqlx::query_file!("../../queries/chat/input_accept.sql",invocation,input.id).execute(&self.pool).await?;}
                            else { input_retry = Some(tokio::time::Instant::now() + Duration::from_secs(1)); }
                        }
                    }
                }
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
    async fn expire_invocations(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query_file!("../../queries/chat/invocation_expire.sql")
            .fetch_all(&mut *tx)
            .await?;
        for row in rows {
            sqlx::query_file!("../../queries/chat/run_status.sql", row.run_id, "failed")
                .execute(&mut *tx)
                .await?;
            activity(&mut tx, row.thread_id, "invocation.ended", row.id, "failed").await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// Supervise bounded, independent invocations. Expired execution is failed, never blindly replayed.
    pub async fn worker(self, shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut shutdown = shutdown;
        let mut workers = tokio::task::JoinSet::new();
        let mut changed = loop {
            tokio::select! {
                _ = shutdown.changed() => return,
                result = self.work_notifications.subscribe(&self.pool, "tilde_chat_work") => {
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
                    .fetch_all(&self.pool)
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
