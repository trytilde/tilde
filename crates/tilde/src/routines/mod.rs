//! Routines prompt an agent when one trigger fires: a five-field UTC cron schedule, or one signal
//! type of one signal-capable connection. Every firing starts a run in a new thread; the prompt and
//! thread title are `{{ key }}` templates rendered against the signal's context (`signals::context`)
//! or, for a cron routine, `scheduled_at`. A signal routine may also carry filters on that context
//! (`alarm.state` is `ALARM`, `issue.team.key` is `ENG` or `OPS`); it fires only when all match. `routine_runs` records each firing once per fire key, so
//! a redelivered webhook or a second replica claiming the same due time starts nothing.
//!
//! Cron routines are claimed and rescheduled in one transaction before they fire, which makes a
//! firing at most once: a crash between the commit and the run loses that firing rather than
//! repeating it. Missed times are not backfilled; an overdue routine fires once and moves to its
//! next time after now.
pub mod db;
pub mod rpc;

use crate::chat::{Chat, ChatError, CreateThread, StartRun};
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
const SUMMARY_BYTES: usize = 500;
const TITLE_CHARS: usize = 200;

pub enum Trigger {
    Cron {
        schedule: String,
    },
    Signal {
        connection_id: Uuid,
        signal_type: String,
        filters: Vec<Filter>,
    },
}
/// One condition on a signal's context: the value at `path` (dotted, as templates name it)
/// equals one of `values`. For a list, any element may; strings compare bare and other scalars
/// as their JSON text, so `true` and `42` are written as such. A missing path never matches.
#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub path: String,
    pub values: Vec<String>,
}
impl Filter {
    pub fn matches(&self, context: &serde_json::Value) -> bool {
        use serde_json::Value;
        let pointer = format!("/{}", self.path.replace('.', "/"));
        let text = |value: &Value| match value {
            Value::String(value) => Some(value.clone()),
            Value::Null | Value::Array(_) | Value::Object(_) => None,
            other => Some(other.to_string()),
        };
        let allowed = |value: &Value| text(value).is_some_and(|text| self.values.contains(&text));
        match context.pointer(&pointer) {
            Some(Value::Array(items)) => items.iter().any(allowed),
            Some(value) => allowed(value),
            None => false,
        }
    }
}
const MAX_FILTERS: usize = 20;
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
        self.read(Some(agent), None).await
    }
    pub async fn get(&self, id: Uuid) -> Result<Routine, Error> {
        self.read(None, Some(id))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Invalid("Routine not found".into()))
    }
    async fn read(&self, agent: Option<Uuid>, id: Option<Uuid>) -> Result<Vec<Routine>, Error> {
        let client = self.pool.get().await?;
        let rows = db::routine_list_all(&client, agent, id).await?;
        let ids = rows.iter().map(|r| r.id).collect::<Vec<_>>();
        let mut filters = filters_by_routine(db::filter_list_all(&client, &ids).await?);
        Ok(rows
            .into_iter()
            .map(|r| {
                let routine_filters = filters.remove(&r.id).unwrap_or_default();
                routine_model(r, routine_filters)
            })
            .collect())
    }
    pub async fn create(&self, agent: Uuid, input: RoutineInput) -> Result<Routine, Error> {
        let next = self.validate(&input).await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        if crate::connections::db::agent_exists_opt(&tx, agent)
            .await?
            .is_none()
        {
            return Err(Error::NotFound);
        }
        let id = Uuid::new_v4();
        let (schedule, connection, signal_type) = columns(&input.trigger);
        db::routine_insert_execute(
            &tx,
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
        write_filters(&tx, id, &input.trigger).await?;
        tx.commit().await?;
        drop(client);
        self.get(id).await
    }
    /// Replaces every editable field; a cron routine's next time is recomputed from now.
    pub async fn update(&self, id: Uuid, input: RoutineInput) -> Result<Routine, Error> {
        let next = self.validate(&input).await?;
        let (schedule, connection, signal_type) = columns(&input.trigger);
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let updated = db::routine_update_execute(
            &tx,
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
        write_filters(&tx, id, &input.trigger).await?;
        tx.commit().await?;
        drop(client);
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
        let row = db::signal_connection_opt(&self.pool.get().await?, connection)
            .await?
            .ok_or_else(|| {
                Error::Invalid("Connection not found, or it cannot trigger routines".into())
            })?;
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
                filters,
            } => {
                validate_filters(filters)?;
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
                let client = self.pool.get().await?;
                let matches =
                    db::routine_signal_matches_all(&client, connection, &signal.signal_type)
                        .await?;
                let ids = matches.iter().map(|r| r.id).collect::<Vec<_>>();
                let filters = filters_by_routine(db::filter_list_all(&client, &ids).await?);
                Ok::<_, Error>((matches, filters))
            }
            .await;
            let (matches, mut filters) = match matches {
                Ok(matches) => matches,
                Err(error) => {
                    tracing::warn!(%error, %connection, "Routine signal lookup failed");
                    continue;
                }
            };
            let context = signals::context(&signal);
            for r in matches {
                let routine_filters = filters.remove(&r.id).unwrap_or_default();
                if !routine_filters.iter().all(|f| f.matches(&context)) {
                    continue;
                }
                let objective = format!(
                    "{}\n\nSignal `{}` from connection `{slug}`: {}\n```json\n{}\n```",
                    cut(signals::render(&r.prompt, &context), PROMPT_BYTES),
                    signal.signal_type,
                    cut(signal.summary.clone(), SUMMARY_BYTES),
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
            ..
        } => (None, Some(*connection_id), Some(signal_type)),
    }
}
fn validate_filters(filters: &[Filter]) -> Result<(), Error> {
    if filters.len() > MAX_FILTERS {
        return Err(Error::Invalid("A routine takes at most 20 filters".into()));
    }
    for filter in filters {
        let path = filter.path.trim();
        if path.is_empty()
            || path.len() > 200
            || path.split('.').any(|segment| {
                segment.is_empty()
                    || !segment
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            })
        {
            return Err(Error::Invalid(
                "Filter paths are dotted keys such as alarm.state".into(),
            ));
        }
        if filter.values.is_empty()
            || filter.values.len() > 50
            || filter.values.iter().any(|v| v.chars().count() > 500)
        {
            return Err(Error::Invalid(
                "Each filter needs 1-50 values of at most 500 characters".into(),
            ));
        }
    }
    Ok(())
}
/// Replaces a routine's filters; a cron trigger leaves it with none.
async fn write_filters(
    tx: &crate::database::Transaction<'_>,
    routine: Uuid,
    trigger: &Trigger,
) -> Result<(), Error> {
    db::filter_clear_execute(tx, routine).await?;
    if let Trigger::Signal { filters, .. } = trigger {
        for (position, filter) in filters.iter().enumerate() {
            db::filter_insert_execute(
                tx,
                routine,
                position as i32,
                filter.path.trim(),
                &filter.values,
            )
            .await?;
        }
    }
    Ok(())
}
fn filters_by_routine(rows: Vec<db::FilterRow>) -> std::collections::HashMap<Uuid, Vec<Filter>> {
    let mut out = std::collections::HashMap::<Uuid, Vec<Filter>>::new();
    for row in rows {
        out.entry(row.routine_id).or_default().push(Filter {
            path: row.path,
            values: row.allowed,
        });
    }
    out
}
fn routine_model(r: db::RoutineRow, filters: Vec<Filter>) -> Routine {
    let trigger = match (r.schedule, r.connection_id, r.signal_type) {
        (Some(schedule), _, _) => Trigger::Cron { schedule },
        // The table's check constraint guarantees one trigger or the other.
        (None, connection, signal_type) => Trigger::Signal {
            connection_id: connection.unwrap_or_default(),
            signal_type: signal_type.unwrap_or_default(),
            filters,
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
