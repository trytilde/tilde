use super::*;
impl Runtime {
    pub async fn goals(&self, s: &Scope) -> Result<Vec<types::Goal>> {
        #[derive(Deserialize)]
        struct Row {
            id: String,
            payload: String,
        }
        self.client
            .query::<Row>(
                "SELECT id,payload FROM goals WHERE thread_id=? AND agent_id=? ORDER BY id",
                vec![json!(s.thread_id), json!(s.agent_id)],
            )
            .await?
            .into_iter()
            .map(|r| self.open(id(&r.id)?, "goal", &r.payload))
            .collect()
    }
    pub async fn create_goal(&self, s: &Scope, r: chat::CreateGoal) -> Result<types::Goal> {
        text(&r.objective)?;
        let key = id(&r.id)?;
        let _guard = self.mutation.lock().await;
        if let Some(old) = self.goals(s).await?.into_iter().find(|g| g.id == r.id) {
            return if old.objective == r.objective {
                Ok(old)
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
        self.commit(
            s.thread_id,
            vec![
                statement(
                    "INSERT INTO goals(id,thread_id,agent_id,status,payload) VALUES(?,?,?,?,?)",
                    vec![
                        json!(key),
                        json!(s.thread_id),
                        json!(s.agent_id),
                        json!(goal.status),
                        json!(self.seal(key, "goal", &goal)?),
                    ],
                ),
                self.event_scoped(s, "goal.updated", goal.clone().into())?,
            ],
        )
        .await?;
        Ok(goal)
    }
    pub async fn update_goal(&self, s: &Scope, r: chat::UpdateGoal) -> Result<types::Goal> {
        if !["active", "completed", "failed", "canceled"].contains(&r.status.as_str()) {
            return Err(ChatError::Invalid("Invalid goal status".into()));
        }
        let _guard = self.mutation.lock().await;
        let mut goal = self
            .goals(s)
            .await?
            .into_iter()
            .find(|g| g.id == r.id)
            .ok_or(ChatError::NotFound)?;
        if goal.status != "active" && goal.status != r.status {
            return Err(ChatError::Conflict);
        }
        goal.status = r.status;
        let key = id(&goal.id)?;
        self.commit(
            s.thread_id,
            vec![
                statement(
                    "UPDATE goals SET status=?,payload=? WHERE id=?",
                    vec![
                        json!(goal.status),
                        json!(self.seal(key, "goal", &goal)?),
                        json!(key),
                    ],
                ),
                self.event_scoped(s, "goal.updated", goal.clone().into())?,
            ],
        )
        .await?;
        Ok(goal)
    }
    pub async fn tasks(&self, s: &Scope) -> Result<Vec<types::Task>> {
        #[derive(Deserialize)]
        struct Row {
            id: String,
            payload: String,
        }
        self.client
            .query::<Row>(
                "SELECT id,payload FROM tasks WHERE thread_id=? AND agent_id=? ORDER BY id",
                vec![json!(s.thread_id), json!(s.agent_id)],
            )
            .await?
            .into_iter()
            .map(|r| self.open(id(&r.id)?, "task", &r.payload))
            .collect()
    }
    pub async fn create_task(&self, s: &Scope, mut r: chat::CreateTask) -> Result<types::Task> {
        text(&r.title)?;
        let key = id(&r.id)?;
        let _guard = self.mutation.lock().await;
        r.dependency_ids.sort();
        if r.dependency_ids.len() > 100
            || r.dependency_ids.windows(2).any(|p| p[0] == p[1])
            || r.dependency_ids.contains(&r.id)
        {
            return Err(ChatError::Invalid("Invalid task dependencies".into()));
        }
        let tasks = self.tasks(s).await?;
        for dep in &r.dependency_ids {
            if !tasks.iter().any(|t| t.id == *dep) {
                return Err(ChatError::NotFound);
            }
        }
        if let Some(goal) = &r.goal_id
            && !self.goals(s).await?.iter().any(|g| g.id == *goal)
        {
            return Err(ChatError::NotFound);
        }
        if let Some(old) = tasks.into_iter().find(|t| t.id == r.id) {
            return if old.title == r.title
                && old.goal_id == r.goal_id
                && old.dependency_ids == r.dependency_ids
            {
                Ok(old)
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
        self.commit(
            s.thread_id,
            vec![
                statement(
                    "INSERT INTO tasks(id,thread_id,agent_id,status,payload) VALUES(?,?,?,?,?)",
                    vec![
                        json!(key),
                        json!(s.thread_id),
                        json!(s.agent_id),
                        json!(task.status),
                        json!(self.seal(key, "task", &task)?),
                    ],
                ),
                self.event_scoped(s, "task.updated", task.clone().into())?,
            ],
        )
        .await?;
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
        let _guard = self.mutation.lock().await;
        let tasks = self.tasks(s).await?;
        let mut task = tasks
            .iter()
            .find(|t| t.id == r.id)
            .cloned()
            .ok_or(ChatError::NotFound)?;
        if ["completed", "failed", "canceled"].contains(&task.status.as_str())
            && task.status != r.status
        {
            return Err(ChatError::Conflict);
        }
        if ["working", "completed"].contains(&r.status.as_str())
            && task.dependency_ids.iter().any(|dep| {
                !tasks
                    .iter()
                    .any(|t| t.id == *dep && t.status == "completed")
            })
        {
            return Err(ChatError::Conflict);
        }
        task.status = r.status;
        task.blocked_reason = r.blocked_reason;
        let key = id(&task.id)?;
        self.commit(
            s.thread_id,
            vec![
                statement(
                    "UPDATE tasks SET status=?,payload=? WHERE id=?",
                    vec![
                        json!(task.status),
                        json!(self.seal(key, "task", &task)?),
                        json!(key),
                    ],
                ),
                self.event_scoped(s, "task.updated", task.clone().into())?,
            ],
        )
        .await?;
        Ok(task)
    }
    pub async fn cache_converted_messages(
        &self,
        s: &Scope,
        values: Vec<chat::ConvertedMessage>,
    ) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let mut writes = vec![];
        for value in values {
            let message = self.message(id(&value.message_id)?).await?;
            if message.thread_id != s.thread_id.to_string() || message.status != "complete" {
                return Err(ChatError::Conflict);
            }
            if value.message_json.len() > 1024 * 1024
                || serde_json::from_str::<Value>(&value.message_json).is_err()
            {
                return Err(ChatError::Invalid("Invalid converted message JSON".into()));
            }
            let key = Uuid::new_v5(&s.agent_id, value.message_id.as_bytes());
            let state = types::ConvertedMessageState {
                message_id: value.message_id.clone(),
                message_json: value.message_json.clone(),
                ..Default::default()
            };
            writes.push(statement("INSERT INTO converted_messages(id,thread_id,agent_id,message_id,representation) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET representation=excluded.representation",vec![json!(key),json!(s.thread_id),json!(s.agent_id),json!(value.message_id),json!(self.seal(key,"converted_message",&state)?)]));
            writes.push(self.event_scoped(s, "message.converted", state.into())?);
        }
        self.commit(s.thread_id, writes).await
    }
    pub async fn hydrate_converted_messages(
        &self,
        agent: Uuid,
        thread: Uuid,
        messages: &[String],
    ) -> Result<Vec<chat::ConvertedMessage>> {
        #[derive(Deserialize)]
        struct Row {
            id: String,
            message_id: String,
            representation: String,
        }
        let rows=self.client.query::<Row>("SELECT id,message_id,representation FROM converted_messages WHERE thread_id=? AND agent_id=?",vec![json!(thread),json!(agent)]).await?;
        rows.into_iter()
            .filter(|r| messages.contains(&r.message_id))
            .map(|r| {
                let state: types::ConvertedMessageState =
                    self.open(id(&r.id)?, "converted_message", &r.representation)?;
                Ok(chat::ConvertedMessage {
                    message_id: state.message_id,
                    message_json: state.message_json,
                })
            })
            .collect()
    }
    pub async fn typing(&self, thread: Uuid, participant: Uuid, typing: bool) -> Result<()> {
        if !self
            .thread(thread)
            .await?
            .participants
            .iter()
            .any(|p| p.id == participant.to_string() && p.active)
        {
            return Err(ChatError::NotFound);
        }
        let state = types::Typing {
            participant_id: participant.to_string(),
            typing,
            expires_at: crate::chat::audit::timestamp(
                chrono::Utc::now() + chrono::Duration::seconds(10),
            )
            .into(),
            ..Default::default()
        };
        self.commit(thread,vec![statement("INSERT INTO typing(id,thread_id,participant_id,expires_at) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET expires_at=excluded.expires_at",vec![json!(participant),json!(thread),json!(participant),json!(if typing{now()+10_000}else{now()})]),self.event(thread,"typing.updated",state.into())?]).await
    }
    pub async fn report_tool_call(&self, s: &Scope, tool: types::ToolCall) -> Result<()> {
        let key = id(&tool.id)?;
        if tool.name.is_empty()
            || !["running", "completed", "failed", "aborted"].contains(&tool.status.as_str())
        {
            return Err(ChatError::Invalid("Invalid tool report".into()));
        }
        let _guard = self.mutation.lock().await;
        #[derive(Deserialize)]
        struct Row {
            thread_id: String,
            invocation_id: String,
            payload: String,
        }
        let old = self
            .client
            .query::<Row>(
                "SELECT thread_id,invocation_id,payload FROM tool_calls WHERE id=?",
                vec![json!(key)],
            )
            .await?
            .pop();
        if let Some(row) = old {
            if row.thread_id != s.thread_id.to_string() || row.invocation_id != s.id.to_string() {
                return Err(ChatError::Denied);
            }
            let previous: types::ToolCall = self.open(key, "tool_call", &row.payload)?;
            if previous.name != tool.name || previous.provider_id != tool.provider_id {
                return Err(ChatError::Conflict);
            }
            if previous.status != "running" {
                return if previous == tool {
                    Ok(())
                } else {
                    Err(ChatError::Conflict)
                };
            }
        }
        self.commit(s.thread_id,vec![statement("INSERT INTO tool_calls(id,thread_id,invocation_id,status,payload) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,payload=excluded.payload",vec![json!(key),json!(s.thread_id),json!(s.id),json!(tool.status),json!(self.seal(key,"tool_call",&tool)?)]),self.event_scoped(s,"tool.updated",tool.into())?]).await
    }
}
