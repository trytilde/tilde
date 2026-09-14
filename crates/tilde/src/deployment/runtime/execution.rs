//! Runs, invocations and commands for the replica holding a thread, plus the
//! executor that drives the local agent process. The lease epoch tells the agent
//! process a fresh invocation from a stale one after the lease changed hands.
use super::*;
use crate::proto::tilde::agent_host::v1 as host;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use store::Command;
use types::agent_command::Action;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Claims {
    iss: String,
    aud: String,
    sub: Uuid,
    invocation_id: Uuid,
    run_id: Uuid,
    thread_id: Uuid,
    participant_id: Uuid,
    capabilities: crate::iam::capabilities::Capabilities,
    iat: i64,
    exp: i64,
    lease_epoch: u64,
    agent_generation: i64,
}
impl Runtime {
    pub(crate) fn agent_participant(&self, t: &ThreadState) -> Result<Uuid> {
        t.thread
            .participants
            .iter()
            .find(|p| p.agent_id.as_deref() == Some(&self.agent_id.to_string()) && p.active)
            .map(|p| id(&p.id))
            .ok_or(ChatError::NotFound)?
    }
    pub async fn start_run(&self, r: chat::StartRun) -> Result<types::Run> {
        text(&r.objective)?;
        text(&r.idempotency_key)?;
        let thread = id(&r.thread_id)?;
        if id(&r.agent_id)? != self.agent_id {
            return Err(ChatError::Denied);
        }
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let participant = self.agent_participant(&t)?;
        let run_id = Uuid::new_v5(
            &thread,
            format!("{}:{}", r.agent_id, r.idempotency_key).as_bytes(),
        );
        if let Some(old) = t.runs.get(&run_id) {
            return if old.objective == r.objective {
                Ok(self.run_view(&t, old))
            } else {
                Err(ChatError::Conflict)
            };
        }
        if t.active_invocation(self.agent_id).is_some() {
            return Err(ChatError::Conflict);
        }
        let mut run = types::Run {
            id: run_id.to_string(),
            thread_id: r.thread_id,
            agent_id: r.agent_id,
            objective: r.objective,
            status: "active".into(),
            goal_id: r.goal_id,
            ..Default::default()
        };
        self.begin_invocation(&mut t, &mut run, participant)?;
        drop(t);
        self.changed(thread);
        Ok(run)
    }
    fn run_view(&self, t: &ThreadState, run: &types::Run) -> types::Run {
        let mut run = run.clone();
        if let Ok(key) = id(&run.invocation_id)
            && let Some(v) = t.invocations.get(&key)
        {
            run.invocation_status = v.status.clone();
        }
        run
    }
    /// Record a pending invocation and its invoke command; the executor picks it up.
    pub(crate) fn begin_invocation(
        &self,
        t: &mut ThreadState,
        run: &mut types::Run,
        participant: Uuid,
    ) -> Result<()> {
        let thread = id(&run.thread_id)?;
        let epoch = t.lease.epoch;
        tracing::debug!(agent_id=%self.agent_id, instance_id=%self.instance_id, thread_id=%thread, run_id=%run.id, epoch, "Invocation begins");
        if !self.owns(t) {
            return Err(ChatError::Denied);
        }
        let run_id = id(&run.id)?;
        let invocation_id = Uuid::new_v5(
            &run_id,
            format!("{}:{}:{}", self.instance_id, epoch, run.invocation_id).as_bytes(),
        );
        run.invocation_id = invocation_id.to_string();
        run.invocation_status = "pending".into();
        let invocation = types::InvocationState {
            id: invocation_id.to_string(),
            thread_id: run.thread_id.clone(),
            run_id: run.id.clone(),
            agent_id: self.agent_id.to_string(),
            participant_id: participant.to_string(),
            owner_instance_id: self.instance_id.to_string(),
            generation: epoch,
            status: "pending".into(),
            traceparent: crate::telemetry::context::capture().0,
            tracestate: crate::telemetry::context::capture().1,
            objective: run.objective.clone(),
            ..Default::default()
        };
        let command = types::AgentCommand {
            id: Uuid::new_v4().to_string(),
            thread_id: run.thread_id.clone(),
            participant_id: participant.to_string(),
            agent_id: self.agent_id.to_string(),
            owner_instance_id: self.instance_id.to_string(),
            generation: epoch,
            created_at: now(),
            action: Some(
                types::InvokeCommand {
                    invocation_id: invocation_id.to_string(),
                    run_id: run.id.clone(),
                    objective: run.objective.clone(),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        };
        let attempt = Uuid::new_v5(&id(&command.id)?, &epoch.to_be_bytes());
        t.invocations.insert(invocation_id, invocation.clone());
        t.push_command(Command {
            attempt_id: attempt,
            command,
            acked_at: None,
            finished_at: None,
            failure: String::new(),
        });
        t.runs.insert(run_id, run.clone());
        self.state
            .run_threads
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(run_id, thread);
        self.state
            .invocation_threads
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(invocation_id, thread);
        self.emit(t, "run.created", run.clone().into(), None)?;
        self.emit(t, "invocation.pending", invocation.into(), None)?;
        self.state
            .work
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back((thread, attempt));
        self.state.work_notify.notify_one();
        Ok(())
    }
    pub(crate) fn run_thread(&self, run: Uuid) -> Option<Uuid> {
        self.state
            .run_threads
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&run)
            .copied()
    }
    pub(crate) fn invocation_thread(&self, invocation: Uuid) -> Option<Uuid> {
        self.state
            .invocation_threads
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&invocation)
            .copied()
    }
    pub async fn run(&self, key: Uuid) -> Result<types::Run> {
        let thread = self.run_thread(key).ok_or(ChatError::NotFound)?;
        let shared = self.local(thread).ok_or(ChatError::NotFound)?;
        let t = shared.lock().await;
        let run = t.runs.get(&key).ok_or(ChatError::NotFound)?;
        Ok(self.run_view(&t, run))
    }
    pub async fn invocation(&self, key: Uuid) -> Result<types::InvocationState> {
        let thread = self.invocation_thread(key).ok_or(ChatError::NotFound)?;
        let shared = self.local(thread).ok_or(ChatError::NotFound)?;
        let t = shared.lock().await;
        t.invocations.get(&key).cloned().ok_or(ChatError::NotFound)
    }
    pub async fn resume_run(&self, key: Uuid) -> Result<types::Run> {
        let thread = self.run_thread(key).ok_or(ChatError::NotFound)?;
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let mut run = t.runs.get(&key).cloned().ok_or(ChatError::NotFound)?;
        if !["waiting", "failed"].contains(&run.status.as_str()) {
            return Err(ChatError::Conflict);
        }
        let participant = self.agent_participant(&t)?;
        let previous = run.invocation_id.clone();
        run.status = "active".into();
        self.begin_invocation(&mut t, &mut run, participant)?;
        let generation = t.lease.epoch;
        let mut carried = vec![];
        for command in t
            .commands
            .iter_mut()
            .filter(|c| c.acked_at.is_none() && c.finished_at.is_none())
        {
            if let Some(Action::Steer(input)) = &command.command.action
                && input.invocation_id == previous
            {
                let mut next = command.command.clone();
                if let Some(Action::Steer(input)) = &mut next.action {
                    input.invocation_id = run.invocation_id.clone();
                    next.id =
                        Uuid::new_v5(&id(&run.invocation_id)?, id(&input.input_id)?.as_bytes())
                            .to_string();
                }
                next.created_at = now();
                command.finished_at = Some(now());
                carried.push(next);
            }
        }
        for next in carried {
            let attempt = Uuid::new_v5(&id(&next.id)?, &generation.to_be_bytes());
            t.push_command(Command {
                attempt_id: attempt,
                command: next,
                acked_at: None,
                finished_at: None,
                failure: String::new(),
            });
        }
        drop(t);
        self.changed(thread);
        Ok(run)
    }
    pub async fn set_run_status(&self, s: &Scope, status: &str) -> Result<()> {
        if !["waiting", "completed", "failed", "canceled"].contains(&status) {
            return Err(ChatError::Invalid("Invalid run status".into()));
        }
        let shared = self.load(s.thread_id).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let mut run = t.runs.get(&s.run_id).cloned().ok_or(ChatError::NotFound)?;
        run.status = status.into();
        t.runs.insert(s.run_id, run.clone());
        self.emit(&mut t, "run.updated", run.into(), None)?;
        drop(t);
        self.changed(s.thread_id);
        Ok(())
    }
    pub async fn steer_invocation(&self, r: chat::SteerInvocation) -> Result<()> {
        text(&r.text)?;
        let invocation = id(&r.invocation_id)?;
        let thread = self
            .invocation_thread(invocation)
            .ok_or(ChatError::NotFound)?;
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let v = t
            .invocations
            .get(&invocation)
            .cloned()
            .ok_or(ChatError::NotFound)?;
        if !["pending", "running"].contains(&v.status.as_str()) {
            return Err(ChatError::Conflict);
        }
        let command = types::AgentCommand {
            id: Uuid::new_v5(&invocation, id(&r.input_id)?.as_bytes()).to_string(),
            thread_id: v.thread_id.clone(),
            participant_id: v.participant_id,
            agent_id: v.agent_id,
            owner_instance_id: v.owner_instance_id,
            generation: v.generation,
            created_at: now(),
            action: Some(
                types::SteerCommand {
                    invocation_id: r.invocation_id,
                    input_id: r.input_id,
                    text: r.text,
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        };
        if t.command(&command.id).is_none() {
            let attempt = Uuid::new_v5(&id(&command.id)?, &command.generation.to_be_bytes());
            t.push_command(Command {
                attempt_id: attempt,
                command,
                acked_at: None,
                finished_at: None,
                failure: String::new(),
            });
        }
        drop(t);
        self.changed(thread);
        Ok(())
    }
    pub async fn cancel_invocation(&self, key: Uuid) -> Result<()> {
        let thread = self.invocation_thread(key).ok_or(ChatError::NotFound)?;
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let mut v = t
            .invocations
            .get(&key)
            .cloned()
            .ok_or(ChatError::NotFound)?;
        if !matches!(v.status.as_str(), "pending" | "running") {
            return Ok(());
        }
        v.status = "canceled".into();
        v.ended_at = now();
        let run_id = id(&v.run_id)?;
        let mut run = t.runs.get(&run_id).cloned().ok_or(ChatError::NotFound)?;
        run.status = "canceled".into();
        run.invocation_status = "canceled".into();
        t.invocations.insert(key, v.clone());
        t.runs.insert(run_id, run.clone());
        self.emit(&mut t, "invocation.ended", v.into(), None)?;
        self.emit(&mut t, "run.updated", run.into(), None)?;
        drop(t);
        self.changed(thread);
        Ok(())
    }
    /// Addressed completed messages start a run or steer the active invocation.
    pub(crate) fn route_message_in(
        &self,
        t: &mut ThreadState,
        message: &types::Message,
    ) -> Result<()> {
        if self.paused() || message.status != "complete" {
            return Ok(());
        }
        let sender = t
            .thread
            .participants
            .iter()
            .find(|p| p.id == message.participant_id)
            .cloned()
            .ok_or(ChatError::NotFound)?;
        let Some(target) = t
            .thread
            .participants
            .iter()
            .find(|p| p.active && p.agent_id.as_deref() == Some(&self.agent_id.to_string()))
            .cloned()
        else {
            return Ok(());
        };
        let addressed = message.addressed_participant_ids.contains(&target.id)
            || (message.addressed_participant_ids.is_empty()
                && sender.user_id.is_some()
                && t.thread.primary_agent_id == self.agent_id.to_string());
        if !addressed {
            return Ok(());
        }
        let receipt = Uuid::new_v5(&self.agent_id, message.id.as_bytes());
        if !t.dispatched.insert(receipt) {
            return Ok(());
        }
        let objective = if message.text.is_empty() {
            "New message with attachments.".to_owned()
        } else {
            message.text.clone()
        };
        let thread = id(&message.thread_id)?;
        if let Some(active) = t.active_invocation(self.agent_id).cloned() {
            let command = types::AgentCommand {
                id: Uuid::new_v5(&id(&active.id)?, id(&message.id)?.as_bytes()).to_string(),
                thread_id: message.thread_id.clone(),
                participant_id: target.id.clone(),
                agent_id: self.agent_id.to_string(),
                owner_instance_id: active.owner_instance_id,
                generation: active.generation,
                created_at: now(),
                action: Some(
                    types::SteerCommand {
                        invocation_id: active.id,
                        input_id: message.id.clone(),
                        text: objective,
                        message: message.clone().into(),
                        ..Default::default()
                    }
                    .into(),
                ),
                ..Default::default()
            };
            let attempt = Uuid::new_v5(&id(&command.id)?, &command.generation.to_be_bytes());
            t.push_command(Command {
                attempt_id: attempt,
                command,
                acked_at: None,
                finished_at: None,
                failure: String::new(),
            });
            return Ok(());
        }
        let run_id = Uuid::new_v5(
            &thread,
            format!("{}:{}", self.agent_id, message.id).as_bytes(),
        );
        let mut run = types::Run {
            id: run_id.to_string(),
            thread_id: message.thread_id.clone(),
            agent_id: self.agent_id.to_string(),
            objective,
            status: "active".into(),
            ..Default::default()
        };
        t.run_meta.insert(
            run_id,
            store::RunMeta {
                source_identity_id: if message.delivery.is_set() {
                    sender.user_id.clone().unwrap_or_default()
                } else {
                    String::new()
                },
                channel_origin: message.delivery.is_set() && sender.user_id.is_some(),
            },
        );
        self.begin_invocation(t, &mut run, id(&target.id)?)
    }
    pub async fn issue_token(&self, v: &types::InvocationState) -> Result<SecretString> {
        let configuration = self.configuration()?;
        let capabilities = crate::iam::capabilities::Capabilities::from_wire(
            configuration
                .agent
                .capabilities
                .clone()
                .into_option()
                .unwrap_or_default(),
        )
        .map_err(|_| ChatError::Denied)?;
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            iss: "tilde:invocation".into(),
            aud: "tilde:agent-api".into(),
            sub: self.agent_id,
            invocation_id: id(&v.id)?,
            run_id: id(&v.run_id)?,
            thread_id: id(&v.thread_id)?,
            participant_id: id(&v.participant_id)?,
            capabilities,
            iat: now,
            exp: now + 900,
            lease_epoch: v.generation,
            agent_generation: configuration.agent_generation,
        };
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(self.signing_key.expose_secret().as_bytes()),
        )
        .map(SecretString::from)
        .map_err(|_| ChatError::Transport)
    }
    fn decode(&self, token: &str, leeway: u64) -> Result<Claims> {
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_issuer(&["tilde:invocation"]);
        validation.set_audience(&["tilde:agent-api"]);
        validation.leeway = leeway;
        let claims = jsonwebtoken::decode::<Claims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(self.signing_key.expose_secret().as_bytes()),
            &validation,
        )
        .map_err(|_| ChatError::Denied)?
        .claims;
        if claims.sub != self.agent_id {
            return Err(ChatError::Denied);
        }
        Ok(claims)
    }
    pub async fn scope(&self, token: &str) -> Result<Scope> {
        let claims = self.decode(token, 0)?;
        let shared = self.local(claims.thread_id).ok_or(ChatError::Denied)?;
        let t = shared.lock().await;
        let v = t
            .invocations
            .get(&claims.invocation_id)
            .ok_or(ChatError::Denied)?;
        let configuration = self.configuration()?;
        let refused = [
            (
                "agent_generation",
                claims.agent_generation != configuration.agent_generation,
            ),
            ("status", v.status != "running"),
            ("expired", v.lease_expires_at < now()),
            ("thread", v.thread_id != claims.thread_id.to_string()),
            ("run", v.run_id != claims.run_id.to_string()),
            ("epoch", t.lease.epoch != claims.lease_epoch),
            ("lease", !self.owns(&t)),
            ("paused", configuration.agent.paused),
        ]
        .into_iter()
        .find(|(_, failed)| *failed)
        .map(|(reason, _)| reason);
        if let Some(reason) = refused {
            tracing::debug!(agent_id=%self.agent_id, invocation_id=%claims.invocation_id, reason, "Invocation capability refused");
            return Err(ChatError::Denied);
        }
        let meta = t.run_meta.get(&claims.run_id).cloned().unwrap_or_default();
        if meta.channel_origin {
            let connection = configuration
                .connections
                .iter()
                .find(|c| c.id == t.thread.channel.connection_id && c.status == "ready")
                .ok_or(ChatError::Denied)?;
            let allowed = match connection.access_mode.as_known() {
                Some(types::ChannelAccessMode::Public) => true,
                Some(types::ChannelAccessMode::Private) => connection
                    .identities
                    .iter()
                    .any(|i| i.id == meta.source_identity_id && i.verified && i.allowed),
                _ => false,
            };
            if !allowed {
                return Err(ChatError::Denied);
            }
        }
        Ok(Scope {
            capabilities: claims.capabilities,
            id: claims.invocation_id,
            run_id: claims.run_id,
            thread_id: claims.thread_id,
            agent_id: claims.sub,
            participant_id: id(&v.participant_id)?,
        })
    }
    pub async fn renew_token(&self, token: &str) -> Result<SecretString> {
        let scope = self.scope(token).await?;
        let mut claims = self.decode(token, 0)?;
        let current = crate::iam::capabilities::Capabilities::from_wire(
            self.configuration()?
                .agent
                .capabilities
                .clone()
                .into_option()
                .unwrap_or_default(),
        )
        .map_err(|_| ChatError::Denied)?;
        claims.capabilities = scope.capabilities.intersect(&current);
        claims.iat = chrono::Utc::now().timestamp();
        claims.exp = claims.iat + 900;
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(self.signing_key.expose_secret().as_bytes()),
        )
        .map(SecretString::from)
        .map_err(|_| ChatError::Transport)
    }
    pub async fn verified_claims(
        &self,
        token: &str,
        trace: bool,
    ) -> Result<crate::iam::tokens::Claims> {
        let claims = self.decode(token, if trace { 300 } else { 0 })?;
        if trace {
            let invocation = self.invocation(claims.invocation_id).await?;
            if invocation.thread_id != claims.thread_id.to_string()
                || invocation.run_id != claims.run_id.to_string()
            {
                return Err(ChatError::Denied);
            }
            let live = invocation.status == "running"
                && invocation.lease_expires_at > now()
                && claims.exp > chrono::Utc::now().timestamp();
            let terminal = ["stopped", "failed", "canceled"].contains(&invocation.status.as_str())
                && invocation.ended_at > 0
                && invocation.ended_at + 300_000 > now()
                && invocation.ended_at <= claims.exp * 1000;
            if !live && !terminal {
                return Err(ChatError::Denied);
            }
        } else {
            self.scope(token).await?;
        }
        Ok(crate::iam::tokens::Claims {
            iss: claims.iss,
            aud: claims.aud,
            sub: claims.sub,
            invocation_id: claims.invocation_id,
            run_id: claims.run_id,
            thread_id: claims.thread_id,
            capabilities: claims.capabilities,
            iat: claims.iat,
            exp: claims.exp,
        })
    }
    pub fn verify_token_signature(&self, token: &str) -> Result<()> {
        self.decode(token, 0).map(|_| ())
    }
    pub fn token_thread(&self, token: &str) -> Result<Uuid> {
        Ok(self.decode(token, 300)?.thread_id)
    }
    /// Drive pending invoke commands for threads this replica owns.
    pub async fn worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tasks = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _=shutdown.changed()=>{tasks.abort_all();return;},
                _=tasks.join_next(),if !tasks.is_empty()=>{},
                _=self.state.work_notify.notified()=>{},
                _=tokio::time::sleep(Duration::from_secs(5))=>{},
            }
            let pending: Vec<(Uuid, Uuid)> = self
                .state
                .work
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .drain(..)
                .collect();
            for (thread, attempt) in pending {
                let runtime = self.clone();
                tasks.spawn(async move {
                    if runtime.execute_command(thread, attempt).await.is_err() {
                        tracing::warn!(thread_id=%thread, "Sidecar command execution failed");
                    }
                });
            }
        }
    }
    async fn execute_command(&self, thread: Uuid, attempt: Uuid) -> Result<()> {
        let shared = self.local(thread).ok_or(ChatError::NotFound)?;
        let command = {
            let t = shared.lock().await;
            let Some(entry) = t.commands.iter().find(|c| c.attempt_id == attempt) else {
                return Ok(());
            };
            if entry.finished_at.is_some()
                || !self.owns(&t)
                || t.lease.epoch != entry.command.generation
            {
                return Ok(());
            }
            entry.command.clone()
        };
        let result = match command.action.clone() {
            Some(Action::Invoke(invoke)) => {
                self.execute_invocation(&shared, attempt, &command, *invoke)
                    .await
            }
            _ => Err(ChatError::Invalid("Only invoke commands execute".into())),
        };
        let mut t = shared.lock().await;
        if let Some(entry) = t.commands.iter_mut().find(|c| c.attempt_id == attempt) {
            entry.finished_at = Some(now());
            if result.is_err() {
                entry.failure = "agent_execution_failed".into();
            }
        }
        if result.is_err() && self.owns(&t) {
            let activity = types::Activity {
                kind: "command.failed".into(),
                entity_id: command.id.clone(),
                participant_id: command.participant_id.clone(),
                text_delta: "agent_execution_failed".into(),
                ..Default::default()
            };
            self.emit(&mut t, "command.failed", activity.into(), None)?;
        }
        drop(t);
        self.changed(thread);
        result
    }
    async fn execute_invocation(
        &self,
        shared: &Shared,
        attempt: Uuid,
        command: &types::AgentCommand,
        invoke: types::InvokeCommand,
    ) -> Result<()> {
        use opentelemetry::trace::{FutureExt, TraceContextExt};
        let key = id(&invoke.invocation_id)?;
        let thread = id(&command.thread_id)?;
        let (mut v, tracing_enabled) = {
            let t = shared.lock().await;
            (
                t.invocations
                    .get(&key)
                    .cloned()
                    .ok_or(ChatError::NotFound)?,
                self.configuration()?.tracing_enabled,
            )
        };
        if v.status != "pending" {
            return Ok(());
        }
        let cx = if tracing_enabled {
            crate::telemetry::context::start(
                "tilde.invocation",
                opentelemetry::trace::SpanKind::Client,
                &crate::telemetry::context::restore(&v.traceparent, &v.tracestate),
                vec![
                    opentelemetry::KeyValue::new("tilde.agent.id", self.agent_id.to_string()),
                    opentelemetry::KeyValue::new("tilde.thread.id", v.thread_id.clone()),
                    opentelemetry::KeyValue::new("tilde.invocation.id", v.id.clone()),
                    opentelemetry::KeyValue::new("tilde.run.id", v.run_id.clone()),
                ],
            )
        } else {
            opentelemetry::Context::new()
        };
        let _end = crate::telemetry::context::EndOnDrop(cx.clone());
        {
            let _guard = cx.clone().attach();
            let parent = crate::telemetry::context::capture();
            v.traceparent = parent.0;
            v.tracestate = parent.1;
        }
        v.status = "running".into();
        v.lease_expires_at = now() + 30_000;
        {
            let mut t = shared.lock().await;
            t.invocations.insert(key, v.clone());
            self.emit(&mut t, "invocation.running", v.clone().into(), None)?;
        }
        self.changed(thread);
        let mut pending = Vec::new();
        let result = async {
            let capability = self.issue_token(&v).await?;
            let configuration = self.configuration()?;
            let request = host::InvokeRequest {
                command_id: command.id.clone(),
                assignment_generation: v.generation,
                owner_instance_id: self.instance_id.to_string(),
                agent_generation: configuration.agent_generation,
                invocation_id: v.id.clone(),
                run_id: v.run_id.clone(),
                thread_id: v.thread_id.clone(),
                agent_id: self.agent_id.to_string(),
                objective: invoke.objective,
                callback_url: self.callback_url.clone(),
                capability: capability.expose_secret().into(),
                messages: self.messages(thread, 100).await?,
                thread: self.thread(thread).await?.into(),
                cached_messages: self
                    .hydrate_converted_messages(self.agent_id, thread, &[])
                    .await?
                    .into_iter()
                    .map(|c| crate::proto::tilde::runtime::v1::CachedAgentRepresentation { message_id: c.message_id, message_json: c.message_json, ..Default::default() })
                    .collect(),
                ..Default::default()
            };
            let mut stream = tokio::time::timeout(Duration::from_secs(5), crate::chat::runtime::client(&self.local_endpoint, self.host_key.expose_secret(), "Invoke", &request)?.invoke(request))
                .await
                .map_err(|_| ChatError::Transport)?
                .map_err(|_| ChatError::Transport)?;
            let mut acknowledged = false;
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _=tick.tick()=>{
                        // The lease is only renewed while the gateway still hears this replica.
                        if !self.gateway_healthy() || self.paused() { return Err(ChatError::Denied); }
                        let mut t=shared.lock().await;
                        if !self.owns(&t) || t.lease.epoch!=v.generation { return Err(ChatError::Denied); }
                        if let Some(current)=t.invocations.get_mut(&key) { if current.status!="running" { return Err(ChatError::Denied); } current.lease_expires_at=now()+30_000; }
                    },
                    message=stream.message()=>{
                        let Some(message)=message.map_err(|_|ChatError::Transport)? else { break; };
                        let message=message.view();
                        pending.extend(message.pending_input_ids.iter().map(|id| (*id).to_owned()));
                        if message.accepted_command_id==command.id && !acknowledged {
                            acknowledged=true;
                            let mut t=shared.lock().await;
                            if let Some(entry)=t.commands.iter_mut().find(|c| c.attempt_id==attempt) { entry.acked_at=Some(now()); }
                        }
                        if !message.reasoning_delta.is_empty() {
                            let activity=types::Activity{kind:"reasoning.delta".into(),entity_id:v.id.clone(),participant_id:v.participant_id.clone(),invocation_id:v.id.clone(),text_delta:message.reasoning_delta.into(),..Default::default()};
                            let mut t=shared.lock().await;
                            self.emit(&mut t,"reasoning.delta",activity.into(),None)?;
                            drop(t);
                            self.changed(thread);
                        }
                    }
                }
            }
            if !acknowledged {
                return Err(ChatError::Transport);
            }
            Ok(())
        }
        .with_context(cx.clone())
        .await;
        if result.is_err() {
            cx.span()
                .set_status(opentelemetry::trace::Status::error("Invocation failed"));
        }
        self.finish_invocation(
            shared,
            key,
            if result.is_ok() { "stopped" } else { "failed" },
            &pending,
        )
        .await?;
        result
    }
    pub(crate) async fn finish_invocation(
        &self,
        shared: &Shared,
        key: Uuid,
        status: &str,
        pending: &[String],
    ) -> Result<()> {
        let mut t = shared.lock().await;
        let mut v = t
            .invocations
            .get(&key)
            .cloned()
            .ok_or(ChatError::NotFound)?;
        if v.status == "canceled" || !matches!(v.status.as_str(), "pending" | "running") {
            return Ok(());
        }
        if !self.owns(&t) || t.lease.epoch != v.generation {
            // The lease moved on; the new holder's state is canonical.
            v.status = "failed".into();
            v.ended_at = now();
            t.invocations.insert(key, v);
            return Ok(());
        }
        v.status = status.into();
        v.ended_at = now();
        let run_id = id(&v.run_id)?;
        let mut run = t.runs.get(&run_id).cloned().ok_or(ChatError::NotFound)?;
        let suspending = run.status == "suspending";
        run.invocation_status = status.into();
        if run.status == "active" || suspending {
            run.status = if status == "stopped" {
                "waiting"
            } else {
                status
            }
            .into();
        }
        t.invocations.insert(key, v.clone());
        t.runs.insert(run_id, run.clone());
        self.emit(&mut t, "invocation.ended", v.clone().into(), None)?;
        self.emit(&mut t, "run.updated", run.clone().into(), None)?;
        if status == "stopped" {
            let mut text = Vec::new();
            for command in t.commands.iter_mut().filter(|c| c.finished_at.is_none()) {
                if let Some(Action::Steer(steer)) = &command.command.action
                    && steer.invocation_id == v.id
                    && (pending.contains(&steer.input_id) || command.acked_at.is_none())
                {
                    text.push(steer.text.clone());
                    if suspending {
                        command.acked_at = None;
                    } else {
                        command.finished_at = Some(now());
                    }
                }
            }
            if !text.is_empty() && !suspending {
                let participant = id(&v.participant_id)?;
                let next_id = Uuid::new_v5(&key, b"unconsumed-input");
                let mut next = types::Run {
                    id: next_id.to_string(),
                    thread_id: v.thread_id.clone(),
                    agent_id: v.agent_id.clone(),
                    objective: text.join("\n"),
                    status: "active".into(),
                    ..Default::default()
                };
                self.begin_invocation(&mut t, &mut next, participant)?;
            }
        }
        drop(t);
        self.changed(id(&v.thread_id)?);
        Ok(())
    }
    /// A lease change from the gateway. Losing the lease fails local work; gaining
    /// one for a thread not cached here (recovery moved it) hydrates it, which
    /// restarts any interrupted run.
    pub(crate) async fn apply_lease(&self, lease: control::ThreadLease) -> Result<()> {
        let thread = id(&lease.thread_id)?;
        let mine = lease.held && lease.holder_instance_id == self.instance_id.to_string();
        let Some(shared) = self.local(thread) else {
            if mine {
                let runtime = self.clone();
                tokio::spawn(async move {
                    if runtime.load(thread).await.is_err() {
                        tracing::warn!(agent_id=%runtime.agent_id, thread_id=%thread, "Could not hydrate a thread handed to this replica");
                    }
                });
            }
            return Ok(());
        };
        let mut t = shared.lock().await;
        if mine {
            if t.lease.holder != Some(self.instance_id) {
                t.lease = Lease {
                    holder: Some(self.instance_id),
                    holder_url: String::new(),
                    epoch: self.state.lease_epochs.fetch_add(1, Ordering::AcqRel) + 1,
                };
                self.forget_held_elsewhere(thread);
            }
            return Ok(());
        }
        if t.lease.holder != Some(self.instance_id) && !lease.held {
            return Ok(());
        }
        t.lease.holder = lease
            .held
            .then(|| id(&lease.holder_instance_id).ok())
            .flatten();
        t.lease.holder_url = lease.holder_public_url.clone();
        if lease.held {
            self.note_held_elsewhere(thread, lease.holder_public_url);
        }
        let stale: Vec<Uuid> = t
            .invocations
            .iter()
            .filter(|(_, v)| matches!(v.status.as_str(), "pending" | "running"))
            .map(|(k, _)| *k)
            .collect();
        for key in stale {
            if let Some(v) = t.invocations.get_mut(&key) {
                v.status = "failed".into();
                v.ended_at = now();
            }
        }
        for command in t.commands.iter_mut().filter(|c| c.finished_at.is_none()) {
            command.finished_at = Some(now());
            command.failure = "lease_changed".into();
        }
        drop(t);
        self.changed(thread);
        Ok(())
    }
    /// Unacknowledged steering for one invocation as (command id, input) pairs.
    pub(crate) async fn pending_steering(
        &self,
        thread: Uuid,
        invocation: Uuid,
    ) -> Result<Vec<(String, types::SteerCommand)>> {
        let shared = self.local(thread).ok_or(ChatError::NotFound)?;
        let t = shared.lock().await;
        let mut out = vec![];
        for command in t
            .commands
            .iter()
            .filter(|c| c.acked_at.is_none() && c.finished_at.is_none())
        {
            if let Some(Action::Steer(input)) = &command.command.action
                && input.invocation_id == invocation.to_string()
            {
                out.push((command.command.id.clone(), (**input).clone()));
            }
        }
        Ok(out)
    }
    pub async fn suspend_invocation(&self, invocation: Uuid) -> Result<()> {
        let thread = self
            .invocation_thread(invocation)
            .ok_or(ChatError::NotFound)?;
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let v = t
            .invocations
            .get(&invocation)
            .cloned()
            .ok_or(ChatError::NotFound)?;
        if v.status != "running" {
            return Err(ChatError::Conflict);
        }
        let run_id = id(&v.run_id)?;
        let mut run = t.runs.get(&run_id).cloned().ok_or(ChatError::NotFound)?;
        run.status = "suspending".into();
        t.runs.insert(run_id, run.clone());
        self.emit(&mut t, "run.updated", run.into(), None)?;
        drop(t);
        self.changed(thread);
        Ok(())
    }
    /// Returns true when the acknowledgement was recorded or already existed.
    pub(crate) async fn acknowledge_control(
        &self,
        thread: Uuid,
        invocation: Uuid,
        key: Uuid,
    ) -> Result<bool> {
        let shared = self.local(thread).ok_or(ChatError::NotFound)?;
        let mut t = shared.lock().await;
        if t.control_receipts.contains(&key) {
            return Ok(true);
        }
        let Some(command) = t
            .commands
            .iter_mut()
            .find(|c| c.command.id == key.to_string() && c.finished_at.is_none())
        else {
            return Ok(false);
        };
        let at = now();
        command.acked_at = Some(at);
        command.finished_at = Some(at);
        if let Some(Action::Steer(input)) = &command.command.action {
            let input = id(&input.input_id)?;
            t.dispatched
                .insert(Uuid::new_v5(&self.agent_id, input.to_string().as_bytes()));
        }
        t.control_receipts.insert(key);
        let activity = types::Activity {
            kind: "command.acknowledged".into(),
            entity_id: key.to_string(),
            invocation_id: invocation.to_string(),
            ..Default::default()
        };
        self.emit(&mut t, "command.acknowledged", activity.into(), None)?;
        drop(t);
        self.changed(thread);
        Ok(true)
    }
}
