//! Warm cache for gateway-mode invocations. A sidecar keeps the threads it executes in memory;
//! the gateway serves runtime RPCs from Postgres. This narrows that gap for what an agent asks
//! right after it is woken: which channel tools it may call (and the credentials behind them),
//! and the first page of its history. Priming runs in the background the moment an invocation
//! is claimed, so those reads are already done when the agent's first RPC arrives.
//!
//! Correctness first. The channel catalog changes only through connections, assignments and
//! identity grants, all of which fire `tilde_sidecar_configuration`; history changes through
//! thread activity, which fires `tilde_chat_activity`, and local writes forget their thread
//! directly. Either notification drops the whole respective map, and a short TTL bounds
//! staleness if a listener is ever down. Entries are keyed to the invocation and dropped when
//! it ends. No secrets live here: decrypted credentials stay with
//! [`crate::connections::service::Connections`], which keeps its own bounded,
//! notification-invalidated cache.
//!
//! ponytail: notifications are payload-less, so one change anywhere drops every entry of that
//! kind; carry thread ids in the payload if churn ever shows up in traces.
use super::{Chat, MessagePage};
use crate::database::Pool;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(60);
pub type ChannelRows = Arc<Vec<crate::chat::db::AgentChannelsRow>>;
type Cache<T> = RwLock<HashMap<(Uuid, Uuid), Entry<T>>>;
struct Entry<T> {
    value: T,
    until: Instant,
}
impl<T: Clone> Entry<T> {
    fn live(&self) -> Option<T> {
        (self.until > Instant::now()).then(|| self.value.clone())
    }
}
#[derive(Default)]
pub struct Warm {
    /// Assigned ready channel connections and the thread's binding, per (agent, thread).
    channels: Cache<ChannelRows>,
    /// The first history page as one invocation sees it, per (thread, invocation).
    history: Cache<Arc<MessagePage>>,
    notifications: crate::database::notifications::Notifications,
    activity: crate::database::notifications::Notifications,
}
impl Warm {
    pub fn channels(&self, agent: Uuid, thread: Uuid) -> Option<ChannelRows> {
        self.channels
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(agent, thread))?
            .live()
    }
    pub fn store_channels(&self, agent: Uuid, thread: Uuid, rows: ChannelRows) {
        self.channels
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                (agent, thread),
                Entry {
                    value: rows,
                    until: Instant::now() + TTL,
                },
            );
    }
    pub fn history(&self, thread: Uuid, invocation: Uuid) -> Option<Arc<MessagePage>> {
        self.history
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(thread, invocation))?
            .live()
    }
    pub fn store_history(&self, thread: Uuid, invocation: Uuid, page: Arc<MessagePage>) {
        self.history
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                (thread, invocation),
                Entry {
                    value: page,
                    until: Instant::now() + TTL,
                },
            );
    }
    /// A write to the thread from this process: its pages are stale for every invocation.
    pub fn forget_history(&self, thread: Uuid) {
        self.history
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|(t, _), _| *t != thread);
    }
    /// The invocation ended; nothing will ask on its behalf again.
    pub fn forget_invocation(&self, agent: Uuid, thread: Uuid, invocation: Uuid) {
        self.history
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&(thread, invocation));
        self.channels
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&(agent, thread));
    }
    /// Drop everything, for example after a change made outside this process's write paths.
    pub fn clear(&self) {
        self.channels
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.history
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
    fn sweep(&self) {
        let now = Instant::now();
        self.channels
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|_, e| e.until > now);
        self.history
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|_, e| e.until > now);
    }
    /// Drop caches on the notifications that can invalidate them; sweep expired entries.
    pub async fn worker(
        self: Arc<Self>,
        pool: Pool,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        let idle = || tokio::sync::watch::channel(()).1;
        let mut configuration = self
            .notifications
            .subscribe(&pool, "tilde_sidecar_configuration")
            .await
            .unwrap_or_else(|_| idle());
        let mut activity = self
            .activity
            .subscribe(&pool, "tilde_chat_activity")
            .await
            .unwrap_or_else(|_| idle());
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = tick.tick() => self.sweep(),
                changed = configuration.changed() => {
                    if changed.is_err() { configuration = idle(); }
                    self.channels.write().unwrap_or_else(|e| e.into_inner()).clear();
                }
                changed = activity.changed() => {
                    if changed.is_err() { activity = idle(); }
                    self.history.write().unwrap_or_else(|e| e.into_inner()).clear();
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { break; }
                }
            }
        }
    }
}
impl Chat {
    /// Runs in the background as soon as an invocation is claimed: the channel catalog for
    /// this agent and thread, and the decrypted credentials of every connection in it. The
    /// history page is cached by the wake itself, which loads it for the invoke request.
    pub async fn prime(self, agent: Uuid, thread: Uuid) {
        let Some(channels) = self.channels.clone() else {
            return;
        };
        match channels.catalog(agent, thread).await {
            Ok(rows) => {
                for id in rows.iter().filter_map(|r| r.id) {
                    if let Err(error) = channels.connections.resolve(id).await {
                        tracing::debug!(connection=%id, %error, "Credential warm-up skipped");
                    }
                }
            }
            Err(error) => {
                tracing::debug!(%agent, %thread, %error, "Channel catalog warm-up skipped")
            }
        }
    }
}
