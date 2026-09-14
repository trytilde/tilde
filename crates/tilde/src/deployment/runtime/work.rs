//! Goals, tasks, conversions, typing and tool audit for the owning replica.
use super::*;
impl Runtime {
    pub async fn goals(&self, s: &Scope) -> Result<Vec<types::Goal>> {
        let shared = self.load(s.thread_id).await?;
        let t = shared.lock().await;
        Ok(t.goals.values().cloned().collect())
    }
    pub async fn create_goal(&self, s: &Scope, r: chat::CreateGoal) -> Result<types::Goal> {
        text(&r.objective)?;
        let key = id(&r.id)?;
        let shared = self.load(s.thread_id).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        if let Some(old) = t.goals.get(&key) {
            return if old.objective == r.objective {
                Ok(old.clone())
            } else {
                Err(ChatError::Conflict)
            };
        }
        let goal = types::Goal {
            id: r.id,
            objective: r.objective,
            status: "active".into(),
            ..Default::default()
        };
        t.goals.insert(key, goal.clone());
        self.emit(&mut t, "goal.updated", goal.clone().into(), Some(s))?;
        drop(t);
        self.changed(s.thread_id);
        Ok(goal)
    }
    pub async fn update_goal(&self, s: &Scope, r: chat::UpdateGoal) -> Result<types::Goal> {
        if !["active", "completed", "failed", "canceled"].contains(&r.status.as_str()) {
            return Err(ChatError::Invalid("Invalid goal status".into()));
        }
        let key = id(&r.id)?;
        let shared = self.load(s.thread_id).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let mut goal = t.goals.get(&key).cloned().ok_or(ChatError::NotFound)?;
        if goal.status != "active" && goal.status != r.status {
            return Err(ChatError::Conflict);
        }
        goal.status = r.status;
        t.goals.insert(key, goal.clone());
        self.emit(&mut t, "goal.updated", goal.clone().into(), Some(s))?;
        drop(t);
        self.changed(s.thread_id);
        Ok(goal)
    }
    pub async fn tasks(&self, s: &Scope) -> Result<Vec<types::Task>> {
        let shared = self.load(s.thread_id).await?;
        let t = shared.lock().await;
        Ok(t.tasks.values().cloned().collect())
    }
    pub async fn create_task(&self, s: &Scope, mut r: chat::CreateTask) -> Result<types::Task> {
        text(&r.title)?;
        let key = id(&r.id)?;
        r.dependency_ids.sort();
        if r.dependency_ids.len() > 100
            || r.dependency_ids.windows(2).any(|p| p[0] == p[1])
            || r.dependency_ids.contains(&r.id)
        {
            return Err(ChatError::Invalid("Invalid task dependencies".into()));
        }
        let shared = self.load(s.thread_id).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        for dep in &r.dependency_ids {
            if !t.tasks.contains_key(&id(dep)?) {
                return Err(ChatError::NotFound);
            }
        }
        if let Some(goal) = &r.goal_id
            && !t.goals.contains_key(&id(goal)?)
        {
            return Err(ChatError::NotFound);
        }
        if let Some(old) = t.tasks.get(&key) {
            return if old.title == r.title
                && old.goal_id == r.goal_id
                && old.dependency_ids == r.dependency_ids
            {
                Ok(old.clone())
            } else {
                Err(ChatError::Conflict)
            };
        }
        let task = types::Task {
            id: r.id,
            title: r.title,
            status: "pending".into(),
            goal_id: r.goal_id,
            dependency_ids: r.dependency_ids,
            ..Default::default()
        };
        t.tasks.insert(key, task.clone());
        self.emit(&mut t, "task.updated", task.clone().into(), Some(s))?;
        drop(t);
        self.changed(s.thread_id);
        Ok(task)
    }
    pub async fn update_task(&self, s: &Scope, r: chat::UpdateTask) -> Result<types::Task> {
        if ![
            "pending",
            "working",
            "blocked",
            "completed",
            "failed",
            "canceled",
        ]
        .contains(&r.status.as_str())
            || r.blocked_reason.len() > 16384
        {
            return Err(ChatError::Invalid("Invalid task update".into()));
        }
        let key = id(&r.id)?;
        let shared = self.load(s.thread_id).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        let mut task = t.tasks.get(&key).cloned().ok_or(ChatError::NotFound)?;
        if ["completed", "failed", "canceled"].contains(&task.status.as_str())
            && task.status != r.status
        {
            return Err(ChatError::Conflict);
        }
        if ["working", "completed"].contains(&r.status.as_str())
            && task.dependency_ids.iter().any(|dep| {
                !t.tasks
                    .values()
                    .any(|d| d.id == *dep && d.status == "completed")
            })
        {
            return Err(ChatError::Conflict);
        }
        task.status = r.status;
        task.blocked_reason = r.blocked_reason;
        t.tasks.insert(key, task.clone());
        self.emit(&mut t, "task.updated", task.clone().into(), Some(s))?;
        drop(t);
        self.changed(s.thread_id);
        Ok(task)
    }
    pub async fn cache_converted_messages(
        &self,
        s: &Scope,
        values: Vec<chat::ConvertedMessage>,
    ) -> Result<()> {
        let shared = self.load(s.thread_id).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        for value in values {
            let key = id(&value.message_id)?;
            let message = t.message(key).ok_or(ChatError::NotFound)?;
            if message.status != "complete" {
                return Err(ChatError::Conflict);
            }
            if value.message_json.len() > 1024 * 1024
                || serde_json::from_str::<serde_json::Value>(&value.message_json).is_err()
            {
                return Err(ChatError::Invalid("Invalid converted message JSON".into()));
            }
            let state = types::ConvertedMessageState {
                message_id: value.message_id.clone(),
                message_json: value.message_json.clone(),
                ..Default::default()
            };
            t.converted.insert(key, value.message_json);
            self.emit(&mut t, "message.converted", state.into(), Some(s))?;
        }
        Ok(())
    }
    pub async fn hydrate_converted_messages(
        &self,
        agent: Uuid,
        thread: Uuid,
        messages: &[String],
    ) -> Result<Vec<chat::ConvertedMessage>> {
        if agent != self.agent_id {
            return Err(ChatError::Denied);
        }
        let shared = self.load(thread).await?;
        let t = shared.lock().await;
        Ok(t.converted
            .iter()
            .filter(|(key, _)| messages.is_empty() || messages.contains(&key.to_string()))
            .map(|(key, json)| chat::ConvertedMessage {
                message_id: key.to_string(),
                message_json: json.clone(),
            })
            .collect())
    }
    pub async fn typing(&self, thread: Uuid, participant: Uuid, typing: bool) -> Result<()> {
        let shared = self.load(thread).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        if !t
            .thread
            .participants
            .iter()
            .any(|p| p.id == participant.to_string() && p.active)
        {
            return Err(ChatError::NotFound);
        }
        let state = types::Typing {
            participant_id: participant.to_string(),
            typing,
            expires_at: chat::audit::timestamp(chrono::Utc::now() + chrono::Duration::seconds(10))
                .into(),
            ..Default::default()
        };
        t.typing
            .insert(participant, if typing { now() + 10_000 } else { now() });
        self.emit(&mut t, "typing.updated", state.into(), None)?;
        drop(t);
        self.changed(thread);
        Ok(())
    }
    pub async fn report_tool_call(&self, s: &Scope, tool: types::ToolCall) -> Result<()> {
        let key = id(&tool.id)?;
        if tool.name.is_empty()
            || !["running", "completed", "failed", "aborted"].contains(&tool.status.as_str())
        {
            return Err(ChatError::Invalid("Invalid tool report".into()));
        }
        let shared = self.load(s.thread_id).await?;
        let mut t = shared.lock().await;
        self.require_owner(&t)?;
        if let Some(previous) = t.tool_calls.get(&key) {
            if previous.name != tool.name || previous.provider_id != tool.provider_id {
                return Err(ChatError::Conflict);
            }
            if previous.status != "running" {
                return if *previous == tool {
                    Ok(())
                } else {
                    Err(ChatError::Conflict)
                };
            }
        }
        t.tool_calls.insert(key, tool.clone());
        self.emit(&mut t, "tool.updated", tool.into(), Some(s))?;
        drop(t);
        self.changed(s.thread_id);
        Ok(())
    }
}
