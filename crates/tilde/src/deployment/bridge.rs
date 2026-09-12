//! Gateway coordination connects participant-specific agent clusters. Shared
//! history is copied only into retained conversations, never retired ones.
use super::{Deployments, corrosion::client::statement, runtime::Runtime};
use crate::{
    chat::{ChatError, Result},
    error::Error,
    proto::tilde::types::v1 as types,
};
use secrecy::ExposeSecret;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;
impl Deployments {
    pub async fn queue_bridges(&self) -> std::result::Result<bool, Error> {
        let rows = sqlx::query_file!("../../queries/deployment/bridge_pending.sql")
            .fetch_all(&self.pool)
            .await?;
        let more = rows.len() == 50;
        for row in rows {
            let mut tx = self.pool.begin().await?;
            if sqlx::query_file!(
                "../../queries/deployment/bridge_receipt.sql",
                row.thread_id,
                row.sequence,
                row.agent_id
            )
            .fetch_optional(&mut *tx)
            .await?
            .is_none()
            {
                continue;
            }
            let page = self
                .chat()
                .activity_page(row.thread_id, row.sequence - 1, 1)
                .await?;
            let Some(activity) = page.events.into_iter().next() else {
                continue;
            };
            if activity.origin_agent_id != row.agent_id.to_string() {
                sqlx::query_file!(
                    "../../queries/deployment/bridge_placement.sql",
                    row.thread_id,
                    row.agent_id
                )
                .execute(&mut *tx)
                .await?;
                let snapshot = types::BridgeSnapshot {
                    thread: self.chat().thread(row.thread_id).await?.into(),
                    activities: vec![activity],
                    sequence: row.sequence,
                    ..Default::default()
                };
                let id = Uuid::new_v5(
                    &row.thread_id,
                    format!("bridge:{}:{}", row.agent_id, row.sequence).as_bytes(),
                );
                let payload = self.seal_record(id, "bridge", &snapshot)?;
                sqlx::query_file!(
                    "../../queries/deployment/outbox_insert.sql",
                    id,
                    row.agent_id,
                    "bridge",
                    payload
                )
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
        }
        Ok(more)
    }
    pub async fn deliver_bridge(
        &self,
        agent: Uuid,
        mut snapshot: types::BridgeSnapshot,
    ) -> std::result::Result<(), Error> {
        let thread = super::id(&snapshot.thread.id)?;
        let settings = self.get(agent).await?;
        if settings.mode.as_known() != Some(types::DeploymentMode::Sidecar) {
            return Ok(());
        }
        if sqlx::query_file!(
            "../../queries/deployment/project/location.sql",
            thread,
            agent
        )
        .fetch_optional(&self.pool)
        .await?
        .is_some_and(|r| r.storage != "corrosion")
        {
            return Ok(());
        }
        let node = sqlx::query_file!("../../queries/deployment/project/node_available.sql", agent)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Invalid("Target sidecar unavailable".into()))?;
        let record = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_one(&self.pool)
            .await?;
        let secret = self.open_secrets(
            agent,
            record.encrypted_secrets.as_deref().ok_or(Error::Denied)?,
        )?;
        let peer = super::corrosion::Client::new(
            &format!("{}/corrosion", node.agent_ingress_url.trim_end_matches('/')),
            secret.api_token.clone(),
        )?;
        if peer
            .query::<super::runtime::IdRow>(
                "SELECT id FROM threads WHERE id=?",
                vec![json!(thread)],
            )
            .await?
            .is_empty()
        {
            snapshot.thread = self.chat().thread(thread).await?.into();
            let mut before = None;
            loop {
                let page = self.chat().message_page(thread, before, 100).await?;
                snapshot.messages.extend(page.messages);
                if page.next_page_token.is_empty() {
                    break;
                }
                before = Some(super::id(&page.next_page_token)?);
            }
            snapshot.activities.clear();
            let mut after = 0;
            loop {
                let page = self.chat().activity_page(thread, after, 100).await?;
                after = page.next_sequence;
                snapshot.activities.extend(page.events);
                if !page.has_more {
                    break;
                }
            }
        }
        let response = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Denied)?
            .post(format!(
                "{}/bridge",
                node.agent_ingress_url.trim_end_matches('/')
            ))
            .bearer_auth(secret.api_token.expose_secret())
            .json(&snapshot)
            .send()
            .await
            .map_err(|_| Error::Invalid("Bridge peer unavailable".into()))?;
        if !response.status().is_success() {
            return Err(Error::Invalid(
                "Bridge peer did not accept shared history".into(),
            ));
        }
        Ok(())
    }
    pub async fn bridge_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let notifications = crate::database::notifications::Notifications::default();
        let mut changed = match notifications
            .subscribe(&self.pool, "tilde_chat_activity")
            .await
        {
            Ok(v) => v,
            Err(_) => return,
        };
        changed.mark_changed();
        let mut retry = None;
        loop {
            tokio::select! {_=shutdown.changed()=>return,_=changed.changed()=>{},_=async{if let Some(at)=retry{tokio::time::sleep_until(at).await}else{std::future::pending().await}}=>{}}
            changed.borrow_and_update();
            retry = None;
            match self.queue_bridges().await {
                Ok(true) => changed.mark_changed(),
                Ok(false) => {}
                Err(_) => {
                    tracing::warn!("Conversation bridge will retry");
                    retry = Some(tokio::time::Instant::now() + Duration::from_secs(1));
                }
            }
        }
    }
}
impl Runtime {
    pub(crate) async fn apply_bridge(&self, snapshot: types::BridgeSnapshot) -> Result<()> {
        let thread = crate::chat::id(&snapshot.thread.id)?;
        if !snapshot
            .thread
            .participants
            .iter()
            .any(|p| p.agent_id.as_deref() == Some(&self.agent_id.to_string()))
        {
            return Err(ChatError::Denied);
        }
        let mut writes = vec![];
        for p in &snapshot.thread.participants {
            if let Some(user) = &p.user_id {
                writes.push(statement(
                    "INSERT INTO users(id,name) VALUES(?,?) ON CONFLICT(id) DO NOTHING",
                    vec![json!(user), json!(p.name)],
                ));
            }
            writes.push(self.participant_insert(thread, p)?);
        }
        // A bootstrap includes old history, but only the triggering activity
        // may produce a new invocation. Replaying history must not replay work.
        let routed = snapshot
            .activities
            .iter()
            .filter(|a| a.sequence == snapshot.sequence)
            .filter_map(|a| match &a.detail {
                Some(types::activity::Detail::Message(m)) => Some((**m).clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut incoming = snapshot.messages;
        for activity in &snapshot.activities {
            if let Some(types::activity::Detail::Message(message)) = &activity.detail {
                incoming.push((**message).clone());
            }
        }
        for message in &incoming {
            let id = crate::chat::id(&message.id)?;
            writes.push(self.message_insert(message)?);
            writes.push(statement("INSERT INTO bridge_message_versions(id,message_id,thread_id,sequence,payload) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(format!("{id}:{}",snapshot.sequence)),json!(id),json!(thread),json!(snapshot.sequence),json!(self.seal(id,"message",message)?)]));
        }
        for mut activity in snapshot.activities {
            if activity.event_id.is_empty() {
                activity.event_id =
                    Uuid::new_v5(&thread, &activity.sequence.to_be_bytes()).to_string();
            }
            let id = crate::chat::id(&activity.event_id)?;
            let origin = if activity.origin_instance_id.is_empty() {
                Uuid::nil().to_string()
            } else {
                activity.origin_instance_id.clone()
            };
            let origin_agent = if activity.origin_agent_id.is_empty() {
                snapshot.thread.primary_agent_id.clone()
            } else {
                activity.origin_agent_id.clone()
            };
            let sequence = if activity.origin_sequence == 0 {
                activity.sequence
            } else {
                activity.origin_sequence
            };
            activity.origin_instance_id = origin.clone();
            activity.origin_agent_id = origin_agent.clone();
            activity.origin_sequence = sequence;
            let created = activity.created_at.seconds * 1000
                + i64::from(activity.created_at.nanos) / 1_000_000;
            let event = types::RuntimeEvent {
                id: id.to_string(),
                thread_id: thread.to_string(),
                agent_id: self.agent_id.to_string(),
                origin_instance_id: origin.clone(),
                origin_agent_id: origin_agent.clone(),
                origin_sequence: sequence,
                created_at: created,
                kind: activity.kind.clone(),
                archive_origin: true,
                state: Some(activity.into()),
                ..Default::default()
            };
            writes.push(statement("INSERT INTO events(id,thread_id,agent_id,origin_instance_id,origin_agent_id,origin_sequence,created_at,kind,payload) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(id),json!(thread),json!(self.agent_id),json!(origin),json!(origin_agent),json!(sequence),json!(created),json!(event.kind),json!(self.seal(id,"event",&event)?)]));
        }
        writes.push(statement("INSERT INTO threads(id,title,primary_agent_id,last_activity_at,payload) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(thread),json!(snapshot.thread.title),json!(snapshot.thread.primary_agent_id),json!(super::runtime::now()),json!(self.seal(thread,"thread",&*snapshot.thread)?)]));
        self.commit(thread, writes).await?;
        for message in routed {
            if message.status == "complete" {
                self.route_message(&message).await?;
            }
        }
        Ok(())
    }
}

impl Deployments {
    pub(crate) async fn start_retained_run(
        &self,
        request: &crate::chat::StartRun,
    ) -> std::result::Result<Option<types::Run>, Error> {
        let agent = super::id(&request.agent_id)?;
        let thread = super::id(&request.thread_id)?;
        if self.get(agent).await?.mode.as_known() != Some(types::DeploymentMode::Sidecar) {
            return Ok(None);
        }
        let location = sqlx::query_file!(
            "../../queries/deployment/project/location.sql",
            thread,
            agent
        )
        .fetch_optional(&self.pool)
        .await?;
        if location.as_ref().is_some_and(|r| r.storage == "postgres") {
            return Ok(None);
        }
        if location.as_ref().is_some_and(|r| r.storage == "retiring") {
            return Err(Error::Conflict);
        }
        let peer = self
            .peer(agent)
            .await?
            .ok_or_else(|| Error::Invalid("Target sidecar unavailable".into()))?;
        if !peer.exists(thread).await? {
            let mut snapshot = types::BridgeSnapshot {
                thread: self.chat().thread(thread).await?.into(),
                ..Default::default()
            };
            let mut before = None;
            loop {
                let page = self.chat().message_page(thread, before, 100).await?;
                snapshot.messages.extend(page.messages);
                if page.next_page_token.is_empty() {
                    break;
                }
                before = Some(super::id(&page.next_page_token)?);
            }
            peer.apply_bridge(snapshot).await?;
        }
        sqlx::query_file!(
            "../../queries/deployment/bridge_placement.sql",
            thread,
            agent
        )
        .execute(&self.pool)
        .await?;
        Ok(Some(
            peer.start_run(crate::chat::StartRun {
                thread_id: request.thread_id.clone(),
                agent_id: request.agent_id.clone(),
                objective: request.objective.clone(),
                goal_id: request.goal_id.clone(),
                idempotency_key: request.idempotency_key.clone(),
            })
            .await?,
        ))
    }
}
