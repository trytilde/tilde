//! Routines prompt an agent when one trigger fires: a five-field UTC cron schedule, or one signal
//! type of one signal-capable connection. Every firing starts a run in a new thread; the prompt and
//! thread title are `{{ key }}` templates rendered against the signal's context (`signals::context`)
//! or, for a cron routine, `scheduled_at`. `routine_runs` records each firing once per fire key, so
//! a redelivered webhook or a second replica claiming the same due time starts nothing.
//!
//! Cron routines are claimed and rescheduled in one transaction before they fire, which makes a
//! firing at most once: a crash between the commit and the run loses that firing rather than
//! repeating it. Missed times are not backfilled; an overdue routine fires once and moves to its
//! next time after now.
pub mod db;
pub mod rpc;

use crate::chat::{Chat, ChatError, CreateThread, StartRun};
use crate::connections::model::Capability;
use crate::database::Pool;
use crate::error::Error;
use crate::signals::{self, Signal};
use chrono::{DateTime, Utc};
use std::time::Duration;
use uuid::Uuid;

const TICK: Duration = Duration::from_secs(15);
const DUE_BATCH: i64 = 25;
/// A run objective holds at most 16384 bytes (`chat::text`): the rendered prompt, then the
/// signal's summary and data.
const PROMPT_BYTES: usize = 8000;
const SIGNAL_DATA_BYTES: usize = 7000;
const TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub enum Trigger {
    Cron {
        schedule: String,
    },
    Signal {
        connection_id: Uuid,
        signal_type: String,
    },
}
pub struct Routine {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub name: String,
    pub prompt: String,
    pub thread_title: String,
    pub enabled: bool,
    pub trigger: Trigger,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_thread_id: Option<Uuid>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
/// The editable fields of a routine, shared by create and update.
pub struct RoutineInput {
    pub name: String,
    pub prompt: String,
    /// Empty titles each thread with the routine's name.
    pub thread_title: String,
    pub enabled: bool,
    pub trigger: Trigger,
}

/// The next time a five-field cron expression matches strictly after `after`, in UTC.
pub fn next_run(schedule: &str, after: DateTime<Utc>) -> Result<DateTime<Utc>, Error> {
    let invalid = || Error::Invalid("Schedule must be a five-field cron expression".into());
    if schedule.split_whitespace().count() != 5 || schedule.contains('@') {
        return Err(invalid());
    }
    schedule
        .parse::<croner::Cron>()
        .map_err(|_| invalid())?
        .find_next_occurrence(&after, false)
        .map_err(|_| invalid())
}

#[derive(Clone)]
pub struct Routines {
    pool: Pool,
    chat: Chat,
}
impl Routines {
    pub fn new(pool: Pool, chat: Chat) -> Self {
        Self { pool, chat }
    }
    pub async fn list(&self, agent: Uuid) -> Result<Vec<Routine>, Error> {
        let rows = db::routine_list_all(&self.pool.get().await?, Some(agent), None).await?;
        Ok(rows.into_iter().map(routine_model).collect())
    }
    pub async fn get(&self, id: Uuid) -> Result<Routine, Error> {
        db::routine_list_all(&self.pool.get().await?, None, Some(id))
            .await?
            .into_iter()
            .next()
            .map(routine_model)
            .ok_or_else(|| Error::Invalid("Routine not found".into()))
    }
    pub async fn create(&self, agent: Uuid, input: RoutineInput) -> Result<Routine, Error> {
        let next = self.validate(&input).await?;
        let client = self.pool.get().await?;
        if crate::connections::db::agent_exists_opt(&client, agent)
            .await?
            .is_none()
        {
            return Err(Error::NotFound);
        }
        let id = Uuid::new_v4();
        let (schedule, connection, signal_type) = columns(&input.trigger);
        db::routine_insert_execute(
            &client,
            id,
            agent,
            input.name.trim(),
            &input.prompt,
            input.thread_title.trim(),
            input.enabled,
            schedule,
            connection,
            signal_type,
            next,
        )
        .await?;
        drop(client);
        self.get(id).await
    }
    /// Replaces every editable field; a cron routine's next time is recomputed from now.
    pub async fn update(&self, id: Uuid, input: RoutineInput) -> Result<Routine, Error> {
        let next = self.validate(&input).await?;
        let (schedule, connection, signal_type) = columns(&input.trigger);
        let updated = db::routine_update_execute(
            &self.pool.get().await?,
            id,
            input.name.trim(),
            &input.prompt,
            input.thread_title.trim(),
            input.enabled,
            schedule,
            connection,
            signal_type,
            next,
        )
        .await?;
        if updated == 0 {
            return Err(Error::Invalid("Routine not found".into()));
        }
        self.get(id).await
    }
    pub async fn delete(&self, id: Uuid) -> Result<(), Error> {
        db::routine_delete_execute(&self.pool.get().await?, id).await?;
        Ok(())
    }
    /// The signal source of a signal-capable connection: its types and template variables.
    pub async fn signal_source(
        &self,
        connection: Uuid,
    ) -> Result<&'static dyn signals::Source, Error> {
        let row = crate::connections::db::connection_get_opt(&self.pool.get().await?, connection)
            .await?
            .ok_or_else(|| Error::Invalid("Connection not found".into()))?;
        if !row.capable(Capability::Signal) {
            return Err(Error::Invalid(
                "This connection type does not emit signals".into(),
            ));
        }
        signals::source(&row.provider_id, &row.type_id)
            .ok_or_else(|| Error::Invalid("This connection type does not emit signals".into()))
    }
    /// Checks the input and returns the next run time it should be stored with.
    async fn validate(&self, input: &RoutineInput) -> Result<Option<DateTime<Utc>>, Error> {
        let name = input.name.trim();
        if name.is_empty() || name.chars().count() > 200 {
            return Err(Error::Invalid("Name must contain 1-200 characters".into()));
        }
        // Bytes, not characters: the prompt and a signal's data must fit one run objective.
        if input.prompt.trim().is_empty() || input.prompt.len() > PROMPT_BYTES {
            return Err(Error::Invalid("Prompt must contain 1-8000 bytes".into()));
        }
        if input.thread_title.chars().count() > 500 {
            return Err(Error::Invalid(
                "Thread title is limited to 500 characters".into(),
            ));
        }
        match &input.trigger {
            Trigger::Cron { schedule } => {
                let next = next_run(schedule, Utc::now())?;
                Ok(input.enabled.then_some(next))
            }
            Trigger::Signal {
                connection_id,
                signal_type,
            } => {
                if !self
                    .signal_source(*connection_id)
                    .await?
                    .types()
                    .iter()
                    .any(|t| t.id == *signal_type)
                {
                    return Err(Error::Invalid(
                        "This connection does not emit that signal type".into(),
                    ));
                }
                Ok(None)
            }
        }
    }

    /// Fires every enabled routine waiting on one of these verified signals. Failures are
    /// recorded on the routine's run; the provider's delivery is acknowledged regardless.
    pub async fn signal(&self, connection: Uuid, slug: &str, batch: Vec<Signal>) {
        for signal in batch {
            let matches = async {
                Ok::<_, Error>(
                    db::routine_signal_matches_all(
                        &self.pool.get().await?,
                        connection,
                        &signal.signal_type,
                    )
                    .await?,
                )
            }
            .await;
            let matches = match matches {
                Ok(matches) => matches,
                Err(error) => {
                    tracing::warn!(%error, %connection, "Routine signal lookup failed");
                    continue;
                }
            };
            let context = signals::context(&signal);
            for r in matches {
                let objective = format!(
                    "{}\n\nSignal `{}` from connection `{slug}`: {}\n```json\n{}\n```",
                    cut(signals::render(&r.prompt, &context), PROMPT_BYTES),
                    signal.signal_type,
                    signal.summary,
                    cut(
                        serde_json::to_string_pretty(&signal.data).unwrap_or_default(),
                        SIGNAL_DATA_BYTES
                    ),
                );
                let title = title(&r.thread_title, &r.name, &context);
                let key = format!("{}:{}", signal.signal_type, signal.event_id);
                self.fire(r.id, r.agent_id, title, &key, objective).await;
            }
        }
    }
    /// Claims and reschedules due cron routines in one transaction, then fires them.
    pub async fn tick(&self) -> Result<(), Error> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let due = db::routine_due_all(&tx, DUE_BATCH).await?;
        let now = Utc::now();
        for r in &due {
            let next = r.schedule.as_deref().and_then(|s| next_run(s, now).ok());
            db::routine_reschedule_execute(&tx, r.id, next).await?;
        }
        tx.commit().await?;
        drop(client);
        futures::future::join_all(due.into_iter().map(|r| async move {
            let scheduled = r.next_run_at.unwrap_or(now).to_rfc3339();
            let context = serde_json::json!({ "scheduled_at": scheduled });
            let objective = cut(signals::render(&r.prompt, &context), PROMPT_BYTES);
            let title = title(&r.thread_title, &r.name, &context);
            self.fire(
                r.id,
                r.agent_id,
                title,
                &format!("cron:{scheduled}"),
                objective,
            )
            .await
        }))
        .await;
        Ok(())
    }
    pub async fn worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    if let Err(error) = self.tick().await {
                        tracing::warn!(%error, "Routine schedule tick failed");
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { break; }
                }
            }
        }
    }
    /// Starts the agent's run in a new thread once per fire key and records the outcome.
    async fn fire(&self, routine: Uuid, agent: Uuid, title: String, key: &str, objective: String) {
        let run = Uuid::new_v4();
        let claimed = async {
            Ok::<_, Error>(db::run_claim_opt(&self.pool.get().await?, run, routine, key).await?)
        }
        .await;
        match claimed {
            Ok(Some(_)) => {}
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(%error, %routine, "Routine run could not be recorded");
                return;
            }
        }
        let mut thread = None;
        let started: Result<(), ChatError> = async {
            let created = self
                .chat
                .create_thread(CreateThread {
                    title,
                    participants: vec![crate::proto::tilde::types::v1::ParticipantRef {
                        agent_id: Some(agent.to_string()),
                        ..Default::default()
                    }],
                    primary_agent_id: agent.to_string(),
                })
                .await?;
            thread = Uuid::parse_str(&created.id).ok();
            self.chat
                .start_run(StartRun {
                    thread_id: created.id,
                    agent_id: agent.to_string(),
                    objective,
                    goal_id: None,
                    idempotency_key: format!("routine:{run}"),
                })
                .await?;
            Ok(())
        }
        .await;
        let error = started.err().map(|error| {
            tracing::warn!(%error, %routine, "Routine run could not start");
            error.to_string()
        });
        let recorded = async {
            Ok::<_, Error>(
                db::run_finish_execute(&self.pool.get().await?, run, thread, error.as_deref())
                    .await?,
            )
        }
        .await;
        if let Err(error) = recorded {
            tracing::warn!(%error, %routine, "Routine run outcome could not be recorded");
        }
    }
}

fn columns(trigger: &Trigger) -> (Option<&str>, Option<Uuid>, Option<&str>) {
    match trigger {
        Trigger::Cron { schedule } => (Some(schedule.trim()), None, None),
        Trigger::Signal {
            connection_id,
            signal_type,
        } => (None, Some(*connection_id), Some(signal_type)),
    }
}
fn routine_model(r: db::RoutineRow) -> Routine {
    let trigger = match (r.schedule, r.connection_id, r.signal_type) {
        (Some(schedule), _, _) => Trigger::Cron { schedule },
        // The table's check constraint guarantees one trigger or the other.
        (None, connection, signal_type) => Trigger::Signal {
            connection_id: connection.unwrap_or_default(),
            signal_type: signal_type.unwrap_or_default(),
        },
    };
    Routine {
        id: r.id,
        agent_id: r.agent_id,
        name: r.name,
        prompt: r.prompt,
        thread_title: r.thread_title,
        enabled: r.enabled,
        trigger,
        next_run_at: r.next_run_at,
        last_run_at: r.last_run_at,
        last_thread_id: r.last_thread_id,
        last_error: r.last_error,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }
}
/// The rendered thread title, else the routine's name, at most [`TITLE_CHARS`] characters.
fn title(template: &str, name: &str, context: &serde_json::Value) -> String {
    let rendered = signals::render(template, context);
    let title = match rendered.trim() {
        "" => name,
        rendered => rendered,
    };
    title.chars().take(TITLE_CHARS).collect()
}
/// Cuts text to at most `bytes`, marking the cut.
fn cut(mut text: String, bytes: usize) -> String {
    if text.len() > bytes {
        let mut end = bytes;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n… (truncated)");
    }
    text
}
