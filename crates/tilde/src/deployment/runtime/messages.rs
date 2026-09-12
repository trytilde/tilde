//! Streamed agent messages and activity views over the in-memory thread.
use super::*;
impl Runtime {
    pub async fn begin_message(
        &self,
        s: &Scope,
        key: Uuid,
        recipients: &[String],
        reply: Option<&str>,
    ) -> Result<()> {
        let shared = self.load(s.thread_id).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        self.ensure_capacity()?;
        for target in recipients {
            if !t
                .thread
                .participants
                .iter()
                .any(|p| p.id == *target && p.active)
            {
                return Err(ChatError::NotFound);
            }
        }
        if let Some(reply) = reply
            && t.message(id(reply)?).is_none()
        {
            return Err(ChatError::NotFound);
        }
        if let Some(old) = t.message(key) {
            return if old.participant_id == s.participant_id.to_string()
                && old.status == "streaming"
            {
                Ok(())
            } else {
                Err(ChatError::Conflict)
            };
        }
        let message = types::Message {
            id: key.to_string(),
            thread_id: s.thread_id.to_string(),
            participant_id: s.participant_id.to_string(),
            status: "streaming".into(),
            addressed_participant_ids: recipients.to_vec(),
            in_reply_to_message_id: reply.map(str::to_owned),
            format: "text".into(),
            created_at: chat::audit::timestamp(chrono::Utc::now()).into(),
            ..Default::default()
        };
        t.upsert_message(message.clone());
        self.index_message(key, s.thread_id);
        self.emit(&mut t, "message.started", message.into(), Some(s))?;
        drop(t);
        self.changed(s.thread_id);
        Ok(())
    }
    pub async fn append_message(&self, thread: Uuid, key: Uuid, delta: &str) -> Result<()> {
        let shared = self.local(thread).ok_or(ChatError::NotFound)?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        self.ensure_capacity()?;
        let mut message = t.message(key).cloned().ok_or(ChatError::NotFound)?;
        if message.status != "streaming" {
            return Err(ChatError::Conflict);
        }
        message.text.push_str(delta);
        if message.text.len() > 1024 * 1024 {
            return Err(ChatError::Invalid("Message is too large".into()));
        }
        let chunk = types::Activity {
            kind: "message.delta".into(),
            entity_id: key.to_string(),
            participant_id: message.participant_id.clone(),
            text_delta: delta.into(),
            detail: Some(
                types::MessageChunk {
                    message_id: key.to_string(),
                    text_delta: delta.into(),
                    ..Default::default()
                }
                .into(),
            ),
            ..Default::default()
        };
        t.upsert_message(message);
        self.emit(&mut t, "message.delta", chunk.into(), None)?;
        drop(t);
        self.changed(thread);
        Ok(())
    }
    pub async fn attach_message(&self, thread: Uuid, key: Uuid, ids: &[String]) -> Result<()> {
        let shared = self.local(thread).ok_or(ChatError::NotFound)?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let mut message = t.message(key).cloned().ok_or(ChatError::NotFound)?;
        if message.status != "streaming" {
            return Err(ChatError::Conflict);
        }
        for attachment in ids {
            message.attachments.push(
                t.attachments
                    .get(&id(attachment)?)
                    .map(|a| a.metadata.clone())
                    .ok_or(ChatError::NotFound)?,
            );
        }
        t.upsert_message(message.clone());
        self.emit(&mut t, "message.attachments", message.into(), None)?;
        drop(t);
        self.changed(thread);
        Ok(())
    }
    pub async fn finish_message(
        &self,
        thread: Uuid,
        key: Uuid,
        status: &str,
    ) -> Result<types::Message> {
        let shared = self.local(thread).ok_or(ChatError::NotFound)?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let mut message = t.message(key).cloned().ok_or(ChatError::NotFound)?;
        if message.status != "streaming" {
            return if message.status == status {
                Ok(message)
            } else {
                Err(ChatError::Conflict)
            };
        }
        message.status = status.into();
        t.upsert_message(message.clone());
        self.emit(
            &mut t,
            if status == "complete" {
                "message.completed"
            } else {
                "message.aborted"
            },
            message.clone().into(),
            None,
        )?;
        if status == "complete" {
            self.route_message_in(&mut t, &message)?;
        }
        drop(t);
        self.changed(thread);
        Ok(message)
    }
    pub async fn abort_message(&self, key: Uuid, thread: Uuid) -> Result<()> {
        self.finish_message(thread, key, "aborted")
            .await
            .map(|_| ())
    }
    pub async fn steering_message(
        &self,
        thread: Uuid,
        key: Uuid,
    ) -> Result<Option<types::Message>> {
        match self.message(key).await {
            Ok(message) if message.thread_id == thread.to_string() => Ok(Some(message)),
            Ok(_) | Err(ChatError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }
    /// Retained activity for a thread, in owner order.
    pub(crate) async fn activities(&self, thread: Uuid) -> Result<Vec<types::Activity>> {
        let shared = self.load(thread).await?;
        let t = shared.lock().await;
        Ok(t.events.iter().cloned().map(event_activity).collect())
    }
    pub async fn activity_page(
        &self,
        thread: Uuid,
        after: i64,
        limit: u32,
    ) -> Result<chat::ActivityPage> {
        let size = if limit == 0 { 100 } else { limit.min(100) } as usize;
        let mut events: Vec<types::Activity> = self
            .activities(thread)
            .await?
            .into_iter()
            .filter(|a| a.sequence > after)
            .collect();
        let has_more = events.len() > size;
        events.truncate(size);
        Ok(chat::ActivityPage {
            next_sequence: events.last().map(|a| a.sequence).unwrap_or(after),
            events,
            has_more,
        })
    }
}
pub(crate) fn event_activity(e: types::RuntimeEvent) -> types::Activity {
    let mut activity = types::Activity {
        event_id: e.id.clone(),
        origin_agent_id: if e.origin_agent_id.is_empty() {
            e.agent_id.clone()
        } else {
            e.origin_agent_id.clone()
        },
        origin_instance_id: e.origin_instance_id.clone(),
        origin_sequence: e.origin_sequence,
        sequence: e.origin_sequence,
        kind: e.kind,
        created_at: chat::audit::timestamp(
            chrono::DateTime::from_timestamp_millis(e.created_at).unwrap_or_default(),
        )
        .into(),
        ..Default::default()
    };
    match e.state {
        Some(State::Activity(value)) => {
            activity.entity_id = value.entity_id;
            activity.text_delta = value.text_delta;
            activity.participant_id = value.participant_id;
            activity.invocation_id = value.invocation_id;
            activity.detail = value.detail;
        }
        Some(State::Message(value)) => {
            activity.entity_id = value.id.clone();
            activity.participant_id = value.participant_id.clone();
            activity.detail = Some((*value).into());
        }
        Some(State::Participant(value)) => {
            activity.entity_id = value.id.clone();
            activity.detail = Some((*value).into());
        }
        Some(State::ToolCall(value)) => {
            activity.entity_id = value.id.clone();
            activity.detail = Some((*value).into());
        }
        Some(State::Typing(value)) => {
            activity.entity_id = value.participant_id.clone();
            activity.detail = Some((*value).into());
        }
        Some(State::Run(value)) => activity.entity_id = value.id,
        Some(State::Invocation(value)) => {
            activity.entity_id = value.id.clone();
            activity.invocation_id = value.id;
            activity.participant_id = value.participant_id;
        }
        Some(State::Goal(value)) => activity.entity_id = value.id,
        Some(State::Task(value)) => activity.entity_id = value.id,
        _ => activity.entity_id = e.id,
    }
    activity
}
