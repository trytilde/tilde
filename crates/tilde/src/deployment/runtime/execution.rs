use super::*;
use crate::proto::tilde::agent_host::v1 as host;
use futures::StreamExt;
use std::time::Duration;
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
    capabilities: crate::iam::capabilities::Capabilities,
    iat: i64,
    exp: i64,
    assignment_generation: u64,
    agent_generation: i64,
}
#[derive(Deserialize)]
struct AssignmentRow {
    thread_id: String,
    participant_id: String,
    agent_id: String,
    owner_instance_id: String,
    generation: i64,
    stopped: i64,
}
#[derive(Deserialize)]
pub struct CommandRow {
    pub attempt_id: String,
    pub id: String,
    pub thread_id: String,
    pub participant_id: String,
    pub agent_id: String,
    pub owner_instance_id: String,
    pub generation: i64,
    pub kind: String,
    pub created_at: i64,
    pub acked_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub payload: String,
    pub failure: String,
}
impl Runtime {
    pub(crate) fn assignment_insert(&self, a: &types::ParticipantAssignment) -> Value {
        statement(
            "INSERT INTO assignments(id,thread_id,participant_id,agent_id,owner_instance_id,generation,stopped) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO NOTHING",
            vec![
                json!(format!("{}:{}", a.participant_id, a.generation)),
                json!(a.thread_id),
                json!(a.participant_id),
                json!(a.agent_id),
                json!(a.owner_instance_id),
                json!(a.generation),
                json!(i32::from(a.stopped)),
            ],
        )
    }
    pub async fn assignment(
        &self,
        thread: Uuid,
        participant: Uuid,
    ) -> Result<types::ParticipantAssignment> {
        let row = self
            .client
            .query::<AssignmentRow>(
                "SELECT * FROM assignments WHERE participant_id=? ORDER BY generation DESC LIMIT 1",
                vec![json!(participant)],
            )
            .await?
            .pop();
        Ok(match row {
            Some(r) => types::ParticipantAssignment {
                thread_id: r.thread_id,
                participant_id: r.participant_id,
                agent_id: r.agent_id,
                owner_instance_id: r.owner_instance_id,
                generation: r.generation as u64,
                stopped: r.stopped != 0,
                ..Default::default()
            },
            None => types::ParticipantAssignment {
                thread_id: thread.to_string(),
                participant_id: participant.to_string(),
                agent_id: self.agent_id.to_string(),
                owner_instance_id: self.instance_id.to_string(),
                generation: 1,
                ..Default::default()
            },
        })
    }
    pub async fn start_run(&self, r: chat::StartRun) -> Result<types::Run> {
        text(&r.objective)?;
        text(&r.idempotency_key)?;
        let thread = id(&r.thread_id)?;
        if id(&r.agent_id)? != self.agent_id {
            return Err(ChatError::Denied);
        }
        let participant = self
            .thread(thread)
            .await?
            .participants
            .into_iter()
            .find(|p| p.agent_id.as_deref() == Some(&r.agent_id) && p.active)
            .ok_or(ChatError::NotFound)?;
        let _guard = self.mutation.lock().await;
        let run_id = Uuid::new_v5(
            &thread,
            format!("{}:{}", r.agent_id, r.idempotency_key).as_bytes(),
        );
        match self.run(run_id).await {
            Ok(old) => {
                return if old.objective == r.objective {
                    Ok(old)
                } else {
                    Err(ChatError::Conflict)
                };
            }
            Err(ChatError::NotFound) => {}
            Err(e) => return Err(e),
        }
        let active=self.client.query::<IdRow>("SELECT id FROM invocations WHERE thread_id=? AND agent_id=? AND status IN ('pending','running')",vec![json!(thread),json!(self.agent_id)]).await?;
        if !active.is_empty() {
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
        let writes = self
            .invocation_writes(&mut run, id(&participant.id)?, &r.idempotency_key)
            .await?;
        self.commit(thread, writes).await?;
        Ok(run)
    }
    async fn invocation_writes(
        &self,
        run: &mut types::Run,
        participant: Uuid,
        key: &str,
    ) -> Result<Vec<Value>> {
        let thread = id(&run.thread_id)?;
        let assignment = self.assignment(thread, participant).await?;
        if assignment.stopped {
            return Err(ChatError::Conflict);
        }
        let invocation_id = Uuid::new_v5(
            &id(&run.id)?,
            format!("{}:{}", assignment.generation, run.invocation_id).as_bytes(),
        );
        run.invocation_id = invocation_id.to_string();
        run.invocation_status = "pending".into();
        let invocation = types::InvocationState {
            id: invocation_id.to_string(),
            thread_id: run.thread_id.clone(),
            run_id: run.id.clone(),
            agent_id: self.agent_id.to_string(),
            participant_id: participant.to_string(),
            owner_instance_id: assignment.owner_instance_id.clone(),
            generation: assignment.generation,
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
            owner_instance_id: assignment.owner_instance_id.clone(),
            generation: assignment.generation,
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
        Ok(vec![
            statement(
                "INSERT INTO assignments(id,thread_id,participant_id,agent_id,owner_instance_id,generation,stopped) VALUES(?,?,?,?,?,?,0) ON CONFLICT(id) DO NOTHING",
                vec![
                    json!(format!("{participant}:{}", assignment.generation)),
                    json!(thread),
                    json!(participant),
                    json!(self.agent_id),
                    json!(assignment.owner_instance_id),
                    json!(assignment.generation),
                ],
            ),
            statement(
                "INSERT INTO runs(id,thread_id,agent_id,participant_id,status,idempotency_key,payload) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,payload=excluded.payload",
                vec![
                    json!(run.id),
                    json!(thread),
                    json!(self.agent_id),
                    json!(participant),
                    json!(run.status),
                    json!(key),
                    json!(self.seal(id(&run.id)?, "run", run)?),
                ],
            ),
            self.invocation_insert(&invocation)?,
            self.command_insert(&command)?,
            self.event(thread, "participant.assigned", assignment.into())?,
            self.event(thread, "run.created", run.clone().into())?,
            self.event(thread, "invocation.pending", invocation.into())?,
        ])
    }
    pub(crate) fn invocation_insert(&self, v: &types::InvocationState) -> Result<Value> {
        Ok(statement(
            "INSERT INTO invocations(id,thread_id,run_id,agent_id,participant_id,owner_instance_id,generation,status,lease_expires_at,payload) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,lease_expires_at=excluded.lease_expires_at,payload=excluded.payload",
            vec![
                json!(v.id),
                json!(v.thread_id),
                json!(v.run_id),
                json!(v.agent_id),
                json!(v.participant_id),
                json!(v.owner_instance_id),
                json!(v.generation),
                json!(v.status),
                json!(v.lease_expires_at),
                json!(self.seal(id(&v.id)?, "invocation", v)?),
            ],
        ))
    }
    pub(crate) fn command_insert(&self, c: &types::AgentCommand) -> Result<Value> {
        let kind = match &c.action {
            Some(Action::Invoke(_)) => "invoke",
            Some(Action::Steer(_)) => "steer",
            _ => return Err(ChatError::Invalid("Command requires an action".into())),
        };
        let attempt = Uuid::new_v5(&id(&c.id)?, &c.generation.to_be_bytes());
        Ok(statement(
            "INSERT INTO commands(attempt_id,id,thread_id,participant_id,agent_id,owner_instance_id,generation,kind,created_at,payload) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(attempt_id) DO NOTHING",
            vec![
                json!(attempt),
                json!(c.id),
                json!(c.thread_id),
                json!(c.participant_id),
                json!(c.agent_id),
                json!(c.owner_instance_id),
                json!(c.generation),
                json!(kind),
                json!(c.created_at),
                json!(self.seal(id(&c.id)?, "command", c)?),
            ],
        ))
    }
    pub async fn run(&self, key: Uuid) -> Result<types::Run> {
        let row = self
            .client
            .query::<Payload>("SELECT payload FROM runs WHERE id=?", vec![json!(key)])
            .await?
            .pop()
            .ok_or(ChatError::NotFound)?;
        let mut run: types::Run = self.open(key, "run", &row.payload)?;
        if !run.invocation_id.is_empty() {
            run.invocation_status = self.invocation(id(&run.invocation_id)?).await?.status;
        }
        Ok(run)
    }
    pub async fn invocation(&self, key: Uuid) -> Result<types::InvocationState> {
        #[derive(Deserialize)]
        struct Row {
            payload: String,
            status: String,
            lease_expires_at: i64,
        }
        let row = self
            .client
            .query::<Row>(
                "SELECT payload,status,lease_expires_at FROM invocations WHERE id=?",
                vec![json!(key)],
            )
            .await?
            .pop()
            .ok_or(ChatError::NotFound)?;
        let mut invocation: types::InvocationState = self.open(key, "invocation", &row.payload)?;
        invocation.status = row.status;
        invocation.lease_expires_at = row.lease_expires_at;
        Ok(invocation)
    }
    pub async fn resume_run(&self, key: Uuid) -> Result<types::Run> {
        let mut run = self.run(key).await?;
        if !["waiting", "failed"].contains(&run.status.as_str()) {
            return Err(ChatError::Conflict);
        }
        let participant = self
            .thread(id(&run.thread_id)?)
            .await?
            .participants
            .into_iter()
            .find(|p| p.agent_id.as_deref() == Some(&run.agent_id) && p.active)
            .ok_or(ChatError::NotFound)?;
        let _guard = self.mutation.lock().await;
        let previous = run.invocation_id.clone();
        run.status = "active".into();
        let mut writes = self
            .invocation_writes(&mut run, id(&participant.id)?, &key.to_string())
            .await?;
        for row in self
            .client
            .query::<CommandRow>(
                "SELECT * FROM commands WHERE thread_id=? AND kind='steer' AND acked_at IS NULL",
                vec![json!(run.thread_id)],
            )
            .await?
        {
            let mut command: types::AgentCommand =
                self.open(id(&row.id)?, "command", &row.payload)?;
            if let Some(Action::Steer(input)) = &mut command.action
                && input.invocation_id == previous
            {
                input.invocation_id = run.invocation_id.clone();
                command.id =
                    Uuid::new_v5(&id(&run.invocation_id)?, id(&input.input_id)?.as_bytes())
                        .to_string();
                command.created_at = now();
                writes.push(self.command_insert(&command)?);
                writes.push(statement(
                    "UPDATE commands SET finished_at=? WHERE attempt_id=?",
                    vec![json!(now()), json!(row.attempt_id)],
                ));
            }
        }
        self.commit(id(&run.thread_id)?, writes).await?;
        Ok(run)
    }
    pub async fn set_run_status(&self, s: &Scope, status: &str) -> Result<()> {
        if !["waiting", "completed", "failed", "canceled"].contains(&status) {
            return Err(ChatError::Invalid("Invalid run status".into()));
        }
        let _guard = self.mutation.lock().await;
        let mut run = self.run(s.run_id).await?;
        run.status = status.into();
        self.commit(
            s.thread_id,
            vec![
                statement(
                    "UPDATE runs SET status=?,payload=? WHERE id=?",
                    vec![
                        json!(status),
                        json!(self.seal(s.run_id, "run", &run)?),
                        json!(s.run_id),
                    ],
                ),
                self.event(s.thread_id, "run.updated", run.into())?,
            ],
        )
        .await
    }
    pub async fn steer_invocation(&self, r: chat::SteerInvocation) -> Result<()> {
        text(&r.text)?;
        let invocation = self.invocation(id(&r.invocation_id)?).await?;
        if !["pending", "running"].contains(&invocation.status.as_str()) {
            return Err(ChatError::Conflict);
        }
        let command = types::AgentCommand {
            id: Uuid::new_v5(&id(&r.invocation_id)?, id(&r.input_id)?.as_bytes()).to_string(),
            thread_id: invocation.thread_id.clone(),
            participant_id: invocation.participant_id,
            agent_id: invocation.agent_id,
            owner_instance_id: invocation.owner_instance_id,
            generation: invocation.generation,
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
        self.commit(
            id(&invocation.thread_id)?,
            vec![self.command_insert(&command)?],
        )
        .await
    }
    pub async fn cancel_invocation(&self, key: Uuid) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let mut invocation = self.invocation(key).await?;
        if !matches!(invocation.status.as_str(), "pending" | "running") {
            return Ok(());
        }
        invocation.status = "canceled".into();
        invocation.ended_at = now();
        let mut run = self.run(id(&invocation.run_id)?).await?;
        run.status = "canceled".into();
        run.invocation_status = "canceled".into();
        let thread = id(&invocation.thread_id)?;
        self.commit(
            thread,
            vec![
                self.invocation_insert(&invocation)?,
                statement(
                    "UPDATE runs SET status=?,payload=? WHERE id=?",
                    vec![
                        json!(run.status),
                        json!(self.seal(id(&run.id)?, "run", &run)?),
                        json!(run.id),
                    ],
                ),
                self.event(thread, "invocation.ended", invocation.into())?,
                self.event(thread, "run.updated", run.into())?,
            ],
        )
        .await
    }
    pub async fn route_message(&self, message: &types::Message) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let writes = self.route_message_writes(message).await?;
        if writes.is_empty() {
            return Ok(());
        }
        self.commit(id(&message.thread_id)?, writes).await
    }
    pub(crate) async fn route_message_writes(
        &self,
        message: &types::Message,
    ) -> Result<Vec<Value>> {
        if self.configuration().await?.agent.paused {
            return Ok(vec![]);
        }
        let thread = self.thread(id(&message.thread_id)?).await?;
        self.route_message_in(message, &thread).await
    }
    pub(crate) async fn route_message_in(
        &self,
        message: &types::Message,
        thread: &types::Thread,
    ) -> Result<Vec<Value>> {
        if self.configuration().await?.agent.paused {
            return Ok(vec![]);
        }
        let sender = thread
            .participants
            .iter()
            .find(|p| p.id == message.participant_id)
            .ok_or(ChatError::NotFound)?;
        let Some(target) = thread
            .participants
            .iter()
            .find(|p| p.active && p.agent_id.as_deref() == Some(&self.agent_id.to_string()))
        else {
            return Ok(vec![]);
        };
        let addressed = message.addressed_participant_ids.contains(&target.id)
            || (message.addressed_participant_ids.is_empty()
                && sender.user_id.is_some()
                && thread.primary_agent_id == self.agent_id.to_string());
        if !addressed {
            return Ok(vec![]);
        }
        let receipt = Uuid::new_v5(&self.agent_id, message.id.as_bytes());
        if !self
            .client
            .query::<IdRow>(
                "SELECT id FROM message_dispatch WHERE id=?",
                vec![json!(receipt)],
            )
            .await?
            .is_empty()
        {
            return Ok(vec![]);
        }
        let active=self.client.query::<IdRow>("SELECT id FROM invocations WHERE thread_id=? AND agent_id=? AND status IN ('pending','running') LIMIT 1",vec![json!(message.thread_id),json!(self.agent_id)]).await?.pop();
        let objective = if message.text.is_empty() {
            "New message with attachments.".into()
        } else {
            message.text.clone()
        };
        let mut writes = if let Some(active) = active {
            let invocation = self.invocation(id(&active.id)?).await?;
            let command = types::AgentCommand {
                id: Uuid::new_v5(&id(&active.id)?, id(&message.id)?.as_bytes()).to_string(),
                thread_id: message.thread_id.clone(),
                participant_id: target.id.clone(),
                agent_id: self.agent_id.to_string(),
                owner_instance_id: invocation.owner_instance_id,
                generation: invocation.generation,
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
            vec![self.command_insert(&command)?]
        } else {
            let run_id = Uuid::new_v5(
                &id(&message.thread_id)?,
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
            self.invocation_writes(&mut run, id(&target.id)?, &message.id)
                .await?
        };
        if message.delivery.is_set() && sender.user_id.is_some() {
            let run_id = Uuid::new_v5(
                &id(&message.thread_id)?,
                format!("{}:{}", self.agent_id, message.id).as_bytes(),
            );
            writes.push(statement(
                "UPDATE runs SET source_identity_id=?,channel_origin=1 WHERE id=?",
                vec![
                    json!(sender.user_id.as_deref().unwrap_or("")),
                    json!(run_id),
                ],
            ));
        }
        writes.push(statement("INSERT INTO message_dispatch(id,thread_id,message_id,agent_id) VALUES(?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(receipt),json!(message.thread_id),json!(message.id),json!(self.agent_id)]));
        Ok(writes)
    }
    pub async fn issue_token(&self, v: &types::InvocationState) -> Result<SecretString> {
        let configuration = self.configuration().await?;
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
            capabilities,
            iat: now,
            exp: now + 900,
            assignment_generation: v.generation,
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
    pub async fn scope(&self, token: &str) -> Result<Scope> {
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_issuer(&["tilde:invocation"]);
        validation.set_audience(&["tilde:agent-api"]);
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
        let v = self.invocation(claims.invocation_id).await?;
        let assignment = self
            .assignment(claims.thread_id, id(&v.participant_id)?)
            .await?;
        let configuration = self.configuration().await?;
        if claims.agent_generation != configuration.agent_generation
            || v.status != "running"
            || v.lease_expires_at < now()
            || v.thread_id != claims.thread_id.to_string()
            || v.run_id != claims.run_id.to_string()
            || assignment.generation != claims.assignment_generation
            || assignment.stopped
            || configuration.agent.paused
        {
            return Err(ChatError::Denied);
        }
        #[derive(Deserialize)]
        struct Origin {
            source_identity_id: String,
            channel_origin: i64,
        }
        let origin = self
            .client
            .query::<Origin>(
                "SELECT source_identity_id,channel_origin FROM runs WHERE id=?",
                vec![json!(v.run_id)],
            )
            .await?
            .pop()
            .ok_or(ChatError::Denied)?;
        if origin.channel_origin != 0 {
            let thread = self.thread(claims.thread_id).await?;
            let connection = configuration
                .connections
                .iter()
                .find(|c| c.id == thread.channel.connection_id && c.status == "ready")
                .ok_or(ChatError::Denied)?;
            let allowed = match connection.access_mode.as_known() {
                Some(types::ChannelAccessMode::Public) => true,
                Some(types::ChannelAccessMode::Private) => connection
                    .identities
                    .iter()
                    .any(|i| i.id == origin.source_identity_id && i.verified && i.allowed),
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
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_issuer(&["tilde:invocation"]);
        validation.set_audience(&["tilde:agent-api"]);
        let mut claims = jsonwebtoken::decode::<Claims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(self.signing_key.expose_secret().as_bytes()),
            &validation,
        )
        .map_err(|_| ChatError::Denied)?
        .claims;
        let current = crate::iam::capabilities::Capabilities::from_wire(
            self.configuration()
                .await?
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
    pub async fn worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tasks = tokio::task::JoinSet::new();
        let mut accepted = std::collections::BTreeSet::new();
        loop {
            let subscription=self.client.subscribe::<CommandRow>("SELECT * FROM commands WHERE kind='invoke' AND owner_instance_id=? AND acked_at IS NULL AND finished_at IS NULL",vec![json!(self.instance_id)]).await;
            if let Ok(mut stream) = subscription {
                loop {
                    tokio::select! {
                        _=shutdown.changed()=>{tasks.abort_all();return;},
                        done=tasks.join_next(),if !tasks.is_empty()=>{if let Some(Ok(key))=done {accepted.remove(&key);}},
                        value=stream.next()=>match value{
                            Some(Ok(row)) if !row.deleted=>{
                                if accepted.insert(row.value.attempt_id.clone()) {let runtime=self.clone();tasks.spawn(async move {let key=row.value.attempt_id.clone();if runtime.execute_command(row.value).await.is_err(){tracing::warn!("Sidecar command execution failed");}key});}
                            },Some(Ok(_))=>{},_=>break,
                        }
                    }
                }
            }
            tokio::select! {_=shutdown.changed()=>{tasks.abort_all();return;},_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        }
    }
    async fn execute_command(&self, row: CommandRow) -> Result<()> {
        let command: types::AgentCommand = self.open(id(&row.id)?, "command", &row.payload)?;
        let assignment = self
            .assignment(id(&row.thread_id)?, id(&row.participant_id)?)
            .await?;
        if assignment.owner_instance_id != self.instance_id.to_string()
            || assignment.generation != command.generation
            || assignment.stopped
        {
            return Ok(());
        }
        let result = async {
            match command.action {
                Some(Action::Invoke(invoke)) => self.execute_invocation(&row, *invoke).await,
                Some(_) => Err(ChatError::Invalid(
                    "Control commands are consumed by the execution".into(),
                )),
                None => Err(ChatError::Invalid("Missing command action".into())),
            }
        }
        .await;
        let mut writes = vec![statement(
            "UPDATE commands SET finished_at=?,failure=? WHERE attempt_id=?",
            vec![
                json!(now()),
                json!(if result.is_err() {
                    "agent_execution_failed"
                } else {
                    ""
                }),
                json!(row.attempt_id),
            ],
        )];
        if result.is_err() {
            let activity = types::Activity {
                kind: "command.failed".into(),
                entity_id: row.id.clone(),
                participant_id: row.participant_id.clone(),
                text_delta: "agent_execution_failed".into(),
                ..Default::default()
            };
            writes.push(self.event(id(&row.thread_id)?, "command.failed", activity.into())?);
        }
        self.commit(id(&row.thread_id)?, writes).await?;
        result
    }
    async fn ack(&self, row: &CommandRow) -> Result<()> {
        self.client
            .transaction(vec![statement(
                "UPDATE commands SET acked_at=? WHERE attempt_id=? AND acked_at IS NULL",
                vec![json!(now()), json!(row.attempt_id)],
            )])
            .await?;
        Ok(())
    }
    async fn execute_invocation(
        &self,
        row: &CommandRow,
        invoke: types::InvokeCommand,
    ) -> Result<()> {
        let key = id(&invoke.invocation_id)?;
        let mut v = self.invocation(key).await?;
        if v.status != "pending" {
            return Ok(());
        }
        use opentelemetry::trace::{FutureExt, TraceContextExt};
        let cx = if self.configuration().await?.tracing_enabled {
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
        self.commit(
            id(&v.thread_id)?,
            vec![
                self.invocation_insert(&v)?,
                self.event(id(&v.thread_id)?, "invocation.running", v.clone().into())?,
            ],
        )
        .await?;
        let mut pending = Vec::new();
        let result=async {
            let capability=self.issue_token(&v).await?;let configuration=self.configuration().await?;
            let request=host::InvokeRequest{command_id:row.id.clone(),assignment_generation:v.generation,owner_instance_id:self.instance_id.to_string(),agent_generation:configuration.agent_generation,invocation_id:v.id.clone(),run_id:v.run_id.clone(),thread_id:v.thread_id.clone(),agent_id:self.agent_id.to_string(),objective:invoke.objective,callback_url:self.callback_url.clone(),capability:capability.expose_secret().into(),messages:self.messages(id(&v.thread_id)?,100).await?,thread:self.thread(id(&v.thread_id)?).await?.into(),cached_messages:vec![],..Default::default()};
            let mut stream=tokio::time::timeout(Duration::from_secs(5),crate::chat::runtime::client(&self.local_endpoint,self.host_key.expose_secret(),"Invoke",&request)?.invoke(request)).await.map_err(|_|ChatError::Transport)?.map_err(|_|ChatError::Transport)?;
            let mut acknowledged=false;
            let mut tick=tokio::time::interval(Duration::from_secs(5));
            loop {tokio::select!{
                _=tick.tick()=>{
                    self.scope(capability.expose_secret()).await?;
                    let assignment=self.assignment(id(&v.thread_id)?,id(&v.participant_id)?).await?;
                    if assignment.generation!=v.generation||assignment.owner_instance_id!=self.instance_id.to_string()||assignment.stopped{return Err(ChatError::Denied);}
                    self.client.transaction(vec![statement("UPDATE invocations SET lease_expires_at=? WHERE id=? AND status='running'",vec![json!(now()+30_000),json!(v.id)])]).await?;
                },
                message=stream.message()=>{
                    let Some(message)=message.map_err(|_|ChatError::Transport)?else{break;};let message=message.view();
                    pending.extend(message.pending_input_ids.iter().map(|id| (*id).to_owned()));
                    if message.accepted_command_id==row.id{self.ack(row).await?;acknowledged=true;}
                    if !message.reasoning_delta.is_empty(){let activity=types::Activity{kind:"reasoning.delta".into(),entity_id:v.id.clone(),participant_id:v.participant_id.clone(),invocation_id:v.id.clone(),text_delta:message.reasoning_delta.into(),..Default::default()};self.commit(id(&v.thread_id)?,vec![self.event(id(&v.thread_id)?,"reasoning.delta",activity.into())?]).await?;}
                }
            }}if !acknowledged{return Err(ChatError::Transport);}Ok(())
        }.with_context(cx.clone()).await;
        if result.is_err() {
            cx.span()
                .set_status(opentelemetry::trace::Status::error("Invocation failed"));
        }
        self.finish_invocation(
            key,
            if result.is_ok() { "stopped" } else { "failed" },
            &pending,
        )
        .await?;
        result
    }
    pub(crate) async fn finish_invocation(
        &self,
        key: Uuid,
        status: &str,
        pending: &[String],
    ) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let mut v = self.invocation(key).await?;
        if v.status == "canceled" {
            return Ok(());
        }
        v.status = status.into();
        v.ended_at = now();
        let mut run = self.run(id(&v.run_id)?).await?;
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
        let thread = id(&v.thread_id)?;
        let assignment = self.assignment(thread, id(&v.participant_id)?).await?;
        if assignment.generation != v.generation
            || assignment.owner_instance_id != self.instance_id.to_string()
            || assignment.stopped
        {
            return Ok(());
        }
        let mut writes = vec![
            self.invocation_insert(&v)?,
            statement(
                "UPDATE runs SET status=?,payload=? WHERE id=?",
                vec![
                    json!(run.status),
                    json!(self.seal(id(&run.id)?, "run", &run)?),
                    json!(run.id),
                ],
            ),
            self.event(thread, "invocation.ended", v.clone().into())?,
            self.event(thread, "run.updated", run.into())?,
        ];
        if status == "stopped" {
            let mut text = Vec::new();
            for row in self.client.query::<CommandRow>("SELECT * FROM commands WHERE thread_id=? AND kind='steer' ORDER BY created_at,id",vec![json!(thread)]).await? {
                let command: types::AgentCommand = self.open(id(&row.id)?,"command",&row.payload)?;
                if let Some(Action::Steer(steer)) = command.action
                    && steer.invocation_id==v.id && (pending.contains(&steer.input_id) || row.acked_at.is_none()) {
                    text.push(steer.text);
                    if suspending {writes.push(statement("UPDATE commands SET acked_at=NULL,finished_at=NULL WHERE attempt_id=?",vec![json!(row.attempt_id)]));}
                }
            }
            if !text.is_empty() && !suspending {
                let mut next = types::Run {
                    id: Uuid::new_v5(&key, b"unconsumed-input").to_string(),
                    thread_id: v.thread_id.clone(),
                    agent_id: v.agent_id.clone(),
                    objective: text.join("\n"),
                    status: "active".into(),
                    ..Default::default()
                };
                writes.extend(
                    self.invocation_writes(
                        &mut next,
                        id(&v.participant_id)?,
                        &format!("unconsumed:{key}"),
                    )
                    .await?,
                );
            }
        }
        writes.push(statement("INSERT INTO write_guards(id,reason) SELECT NULL,'stale execution' WHERE EXISTS(SELECT 1 FROM assignments WHERE thread_id=? AND participant_id=? AND generation>?)",vec![json!(thread),json!(v.participant_id),json!(v.generation)]));
        self.commit(thread, writes).await
    }
}
impl Runtime {
    pub async fn verified_claims(
        &self,
        token: &str,
        trace: bool,
    ) -> Result<crate::iam::tokens::Claims> {
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_issuer(&["tilde:invocation"]);
        validation.set_audience(&["tilde:agent-api"]);
        if trace {
            validation.leeway = 300;
        }
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
}
impl Runtime {
    pub fn verify_token_signature(&self, token: &str) -> Result<()> {
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_issuer(&["tilde:invocation"]);
        validation.set_audience(&["tilde:agent-api"]);
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
        Ok(())
    }
}
impl Runtime {
    pub async fn configuration_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        loop {
            if let Ok(mut changes) = self
                .client
                .subscribe::<Payload>(
                    "SELECT payload FROM configuration WHERE agent_id=?",
                    vec![json!(self.agent_id)],
                )
                .await
            {
                loop {
                    tokio::select! {_=shutdown.changed()=>return,change=changes.next()=>match change{
                        Some(Ok(change)) if !change.deleted=>{
                            if self.configuration().await.is_ok_and(|c|!c.agent.paused)
                                && let Ok(rows)=self.client.query::<IdRow>("SELECT m.id FROM messages m WHERE m.status='complete' AND NOT EXISTS(SELECT 1 FROM message_dispatch d WHERE d.message_id=m.id AND d.agent_id=?)",vec![json!(self.agent_id)]).await{for row in rows{if let Ok(message)=self.message(id(&row.id).unwrap_or_default()).await{let _=self.route_message(&message).await;}}}
                        },Some(Ok(_))=>{},_=>break,
                    }}
                }
            }
            tokio::select! {_=shutdown.changed()=>return,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        }
    }
}
