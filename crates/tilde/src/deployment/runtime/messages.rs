use super::*;
impl Runtime {
    pub async fn begin_message(
        &self,
        s: &Scope,
        key: Uuid,
        recipients: &[String],
        reply: Option<&str>,
    ) -> Result<()> {
        let roster = self.thread(s.thread_id).await?;
        for recipient in recipients {
            if !roster
                .participants
                .iter()
                .any(|p| p.id == *recipient && p.active)
            {
                return Err(ChatError::NotFound);
            }
        }
        if let Some(reply) = reply
            && self.message(id(reply)?).await?.thread_id != s.thread_id.to_string()
        {
            return Err(ChatError::NotFound);
        }
        let message = types::Message {
            id: key.to_string(),
            thread_id: s.thread_id.to_string(),
            participant_id: s.participant_id.to_string(),
            status: "streaming".into(),
            format: "text".into(),
            addressed_participant_ids: recipients.to_vec(),
            in_reply_to_message_id: reply.map(str::to_owned),
            created_at: crate::chat::audit::timestamp(chrono::Utc::now()).into(),
            ..Default::default()
        };
        self.commit(
            s.thread_id,
            vec![
                self.message_insert(&message)?,
                self.event_scoped(s, "message.started", message.into())?,
            ],
        )
        .await
    }
    pub async fn append_message(&self, thread: Uuid, key: Uuid, delta: &str) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let mut message = self.message(key).await?;
        if message.thread_id != thread.to_string() || message.status != "streaming" {
            return Err(ChatError::Conflict);
        }
        message.text.push_str(delta);
        if message.text.len() > 1024 * 1024 {
            return Err(ChatError::Invalid("Message is too large".into()));
        }
        self.commit(
            thread,
            vec![
                self.message_insert(&message)?,
                self.event(thread, "message.delta", message.into())?,
            ],
        )
        .await
    }
    pub async fn attach_message(&self, thread: Uuid, key: Uuid, ids: &[String]) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let mut message = self.message(key).await?;
        if message.thread_id != thread.to_string() || message.status != "streaming" {
            return Err(ChatError::Conflict);
        }
        for attachment in ids {
            message
                .attachments
                .push(self.attachment_metadata(thread, id(attachment)?).await?);
        }
        self.commit(
            thread,
            vec![
                self.message_insert(&message)?,
                self.event(thread, "message.attachments", message.into())?,
            ],
        )
        .await
    }
    pub async fn finish_message(
        &self,
        thread: Uuid,
        key: Uuid,
        status: &str,
    ) -> Result<types::Message> {
        let _guard = self.mutation.lock().await;
        let mut message = self.message(key).await?;
        if message.thread_id != thread.to_string() {
            return Err(ChatError::Denied);
        }
        if message.status != "streaming" {
            return if message.status == status {
                Ok(message)
            } else {
                Err(ChatError::Conflict)
            };
        }
        message.status = status.into();
        self.commit(
            thread,
            vec![
                self.message_insert(&message)?,
                self.event(
                    thread,
                    if status == "complete" {
                        "message.completed"
                    } else {
                        "message.aborted"
                    },
                    message.clone().into(),
                )?,
            ],
        )
        .await?;
        drop(_guard);
        if status == "complete" {
            self.route_message(&message).await?;
        }
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
    // Sidecar native subscriptions use origin-aware cursors. This projection also
    // supports the shared domain API for a single-origin page.
    pub async fn activity_page(
        &self,
        thread: Uuid,
        after: i64,
        limit: u32,
    ) -> Result<chat::ActivityPage> {
        let size = if limit == 0 { 100 } else { limit.min(100) };
        let mut rows=self.client.query::<EventRow>("SELECT * FROM events WHERE thread_id=? AND origin_sequence>? ORDER BY origin_sequence,id LIMIT ?",vec![json!(thread),json!(after),json!(size+1)]).await?;
        let has_more = rows.len() > size as usize;
        if has_more {
            rows.pop();
        }
        let mut events = vec![];
        for row in rows {
            let event: types::RuntimeEvent = self.open(id(&row.id)?, "event", &row.payload)?;
            events.push(event_activity(event));
        }
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
        created_at: crate::chat::audit::timestamp(
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
