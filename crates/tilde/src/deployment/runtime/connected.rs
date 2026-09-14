//! Agent processes that dial the sidecar over the run protocol. The sidecar is
//! gateway-shaped toward its local agent: the agent registers an instance, receives
//! wakes as frames and reports acceptance, reasoning and completion back. Nothing
//! here touches the gateway; a connected agent is local state of one runtime.
use super::*;
use crate::proto::tilde::agent_host::v1 as host;
use crate::proto::tilde::run::v1 as run;
use std::sync::Mutex as StdMutex;
use tokio::sync::mpsc;

/// Silence past this and the agent process is treated as gone.
const LOCAL_LIVENESS_MS: i64 = 10_000;
struct Local {
    wakes: mpsc::Sender<host::InvokeRequest>,
    last_seen: i64,
}
/// Where one execution hears from the agent that took its wake.
struct Waiter {
    events: mpsc::Sender<run::report_request::Event>,
}
#[derive(Default)]
pub(crate) struct Connected {
    instances: StdMutex<HashMap<Uuid, Local>>,
    waiters: StdMutex<HashMap<Uuid, Waiter>>,
}
pub(crate) struct LocalWake {
    /// The agent process that took the wake; the execution ends when it stops heartbeating.
    pub instance: Uuid,
    pub events: mpsc::Receiver<run::report_request::Event>,
}
impl Runtime {
    /// Register an agent process; the receiver yields wakes for it to run.
    pub(crate) fn attach_local(&self, instance: Uuid) -> mpsc::Receiver<host::InvokeRequest> {
        let (wakes, receiver) = mpsc::channel(64);
        self.state
            .connected
            .instances
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                instance,
                Local {
                    wakes,
                    last_seen: now(),
                },
            );
        self.state.agent_healthy.store(true, Ordering::Release);
        receiver
    }
    pub(crate) fn detach_local(&self, instance: Uuid) {
        self.state
            .connected
            .instances
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&instance);
    }
    pub(crate) fn local_heartbeat(&self, instance: Uuid, ready: bool) -> bool {
        let mut instances = self
            .state
            .connected
            .instances
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match instances.get_mut(&instance) {
            Some(local) => {
                local.last_seen = if ready { now() } else { 0 };
                true
            }
            None => false,
        }
    }
    /// Whether one agent process is still heartbeating.
    pub(crate) fn local_instance_connected(&self, instance: Uuid) -> bool {
        self.state
            .connected
            .instances
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&instance)
            .is_some_and(|local| now() - local.last_seen < LOCAL_LIVENESS_MS)
    }
    /// Whether an agent process has heartbeated recently.
    pub(crate) fn local_agent_connected(&self) -> bool {
        self.state
            .connected
            .instances
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .any(|local| now() - local.last_seen < LOCAL_LIVENESS_MS)
    }
    /// Where reports for one invocation land, whichever way its host was woken.
    pub(crate) fn expect_reports(
        &self,
        invocation: Uuid,
    ) -> mpsc::Receiver<run::report_request::Event> {
        let (events_tx, events) = mpsc::channel(256);
        self.state
            .connected
            .waiters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(invocation, Waiter { events: events_tx });
        events
    }
    /// Hand a wake to the most recently seen live agent process, if any.
    pub(crate) fn wake_local(&self, request: host::InvokeRequest) -> Option<LocalWake> {
        let invocation = id(&request.invocation_id).ok()?;
        let (instance, sender) = {
            let instances = self
                .state
                .connected
                .instances
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            instances
                .iter()
                .filter(|(_, local)| now() - local.last_seen < LOCAL_LIVENESS_MS)
                .max_by_key(|(_, local)| local.last_seen)
                .map(|(instance, local)| (*instance, local.wakes.clone()))?
        };
        let events = self.expect_reports(invocation);
        if sender.try_send(request).is_err() {
            self.forget_waiter(invocation);
            return None;
        }
        Some(LocalWake { instance, events })
    }
    pub(crate) fn forget_waiter(&self, invocation: Uuid) {
        self.state
            .connected
            .waiters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&invocation);
    }
    /// A report from the agent process for an execution this runtime is driving.
    pub(crate) async fn report_local(
        &self,
        invocation: Uuid,
        event: run::report_request::Event,
    ) -> Result<()> {
        let events = self
            .state
            .connected
            .waiters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&invocation)
            .map(|waiter| waiter.events.clone())
            .ok_or(ChatError::NotFound)?;
        events.send(event).await.map_err(|_| ChatError::Transport)
    }
}
