//! Origin-aware replay receipts tolerate out-of-order delivery. Contiguous
//! positions compress into ranges; a late event fills a gap instead of being
//! skipped by a high-water mark. Postgres positions serve the same cursor at the gateway.
use super::*;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, pin::Pin};
pub type ActivityStream = Pin<Box<dyn Stream<Item = Result<(types::Activity, String)>> + Send>>;
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    postgres: i64,
    origins: BTreeMap<String, Vec<[i64; 2]>>,
}
impl Cursor {
    fn decode(value: &str) -> Result<Self> {
        if value.is_empty() {
            return Ok(Self::default());
        }
        if value.len() > 64 * 1024 {
            return Err(ChatError::Invalid("Cursor is too large".into()));
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| ChatError::Invalid("Invalid cursor".into()))?;
        let cursor: Self = serde_json::from_slice(&bytes)
            .map_err(|_| ChatError::Invalid("Invalid cursor".into()))?;
        if cursor.postgres < 0
            || cursor
                .origins
                .values()
                .any(|ranges| ranges.iter().any(|r| r[0] < 1 || r[1] < r[0]))
        {
            return Err(ChatError::Invalid("Invalid cursor".into()));
        }
        Ok(cursor)
    }
    fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(self).expect("typed cursor"))
    }
    fn seen(&self, key: &str, sequence: i64) -> bool {
        self.origins
            .get(key)
            .is_some_and(|ranges| ranges.iter().any(|r| sequence >= r[0] && sequence <= r[1]))
    }
    fn remember(&mut self, key: String, sequence: i64) {
        let ranges = self.origins.entry(key).or_default();
        ranges.push([sequence, sequence]);
        ranges.sort_unstable();
        let mut merged: Vec<[i64; 2]> = vec![];
        for range in ranges.drain(..) {
            if let Some(last) = merged.last_mut()
                && range[0] <= last[1].saturating_add(1)
            {
                last[1] = last[1].max(range[1]);
                continue;
            }
            merged.push(range);
        }
        *ranges = merged;
    }
    fn accept(&mut self, event: &types::Activity) -> bool {
        if event.origin_instance_id.is_empty() {
            return true;
        }
        let key = format!("{}/{}", event.origin_agent_id, event.origin_instance_id);
        if self.seen(&key, event.origin_sequence) {
            return false;
        }
        self.remember(key, event.origin_sequence);
        true
    }
}
impl Chat {
    pub async fn list_ingress_threads(
        &self,
        agent: Option<Uuid>,
        after: &str,
        limit: u32,
    ) -> Result<(Vec<types::Thread>, String)> {
        let size = if limit == 0 { 30 } else { limit.min(100) };
        if let Some(local) = self.local() {
            if agent.is_some_and(|id| id != local.agent_id) {
                return Err(ChatError::Denied);
            }
            return local.list_threads(after, size as usize).await;
        }
        let cursor = if after.is_empty() {
            None
        } else {
            Some(id(after)?)
        };
        let mut rows = sqlx::query_file!(
            "../../queries/chat/threads_list.sql",
            agent,
            cursor,
            i64::from(size) + 1
        )
        .fetch_all(self.pg()?)
        .await?;
        let more = rows.len() > size as usize;
        if more {
            rows.pop();
        }
        let next = if more {
            rows.last().map(|r| r.id.to_string()).unwrap_or_default()
        } else {
            String::new()
        };
        let mut threads = vec![];
        for r in rows {
            threads.push(self.thread(r.id).await?);
        }
        Ok((threads, next))
    }
    pub async fn ingress_activity(
        &self,
        thread: Uuid,
        after: &str,
        limit: u32,
    ) -> Result<(Vec<types::Activity>, String, bool)> {
        let mut cursor = Cursor::decode(after)?;
        let size = if limit == 0 { 100 } else { limit.min(100) } as usize;
        if let Some(local) = self.local() {
            let mut events = vec![];
            let mut more = false;
            for event in local.activities(thread).await? {
                if events.len() == size {
                    more = true;
                    break;
                }
                if cursor.accept(&event) {
                    events.push(event);
                }
            }
            return Ok((events, cursor.encode(), more));
        }
        let page = self
            .activity_page(thread, cursor.postgres, size as u32)
            .await?;
        cursor.postgres = page.next_sequence;
        let events = page
            .events
            .into_iter()
            .filter(|event| cursor.accept(event))
            .collect();
        Ok((events, cursor.encode(), page.has_more))
    }
    pub async fn watch_ingress(&self, thread: Uuid, after: &str) -> Result<ActivityStream> {
        self.thread(thread).await?;
        if let Some(local) = self.local() {
            let local = local.clone();
            let mut cursor = Cursor::decode(after)?;
            let mut changes = local.changes();
            return Ok(Box::pin(async_stream::try_stream! {
                loop {
                    for event in local.activities(thread).await? {
                        if cursor.accept(&event) { yield (event, cursor.encode()); }
                    }
                    loop {
                        match changes.recv().await {
                            Ok(changed) if changed == thread => break,
                            Ok(_) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => break,
                            Err(_) => return,
                        }
                    }
                }
            }));
        }
        let chat = self.clone();
        let mut cursor = Cursor::decode(after)?;
        let mut changed = self
            .activity_notifications
            .subscribe(self.pg()?, "tilde_chat_activity")
            .await?;
        Ok(Box::pin(async_stream::try_stream! {loop{
            changed.borrow_and_update();let page=chat.activity_page(thread,cursor.postgres,100).await?;
            for event in page.events{cursor.postgres=event.sequence;if cursor.accept(&event){yield(event,cursor.encode());}}
            if page.has_more{continue;}
            if changed.changed().await.is_err(){break;}
        }}))
    }
}
