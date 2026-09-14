//! Ship queued frames to the gateway in order and apply what it answers. Liveness
//! travels on its own short cadence so a slow event batch never makes a healthy
//! replica look dead, and the Watch stream is bounded so a half-open connection
//! is noticed within one ping interval.
use super::*;
use secrecy::ExposeSecret;
use std::collections::HashMap;
use std::time::Duration;
const BATCH: usize = 512;
/// Between heartbeats. Three fit inside the gateway's liveness window.
const HEARTBEAT: Duration = Duration::from_secs(3);
/// The gateway pings every ten seconds; silence past this means the stream is gone.
const WATCH_IDLE: Duration = Duration::from_secs(25);
impl Runtime {
    pub fn healthy(&self) -> bool {
        self.state.serving.load(Ordering::Acquire)
            && self.state.agent_healthy.load(Ordering::Acquire)
            && self.gateway_healthy()
    }
    /// Ask the agent process whether it is ready, off every other path.
    pub async fn probe_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! { _=shutdown.changed()=>return, _=tick.tick()=>{} }
            let probe = crate::proto::tilde::agent_host::v1::HealthzRequest::default();
            let ready = match crate::chat::runtime::client(
                &self.local_endpoint,
                self.host_key.expose_secret(),
                "Healthz",
                &probe,
            ) {
                Ok(client) => {
                    matches!(tokio::time::timeout(Duration::from_secs(3), client.healthz(probe)).await, Ok(Ok(value)) if value.view().ready)
                }
                Err(_) => false,
            };
            self.state.agent_healthy.store(ready, Ordering::Release);
        }
    }
    /// Tell the gateway this replica is alive, independently of the event queue.
    pub async fn heartbeat_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(HEARTBEAT);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! { _=shutdown.changed()=>return, _=tick.tick()=>{} }
            let began = std::time::Instant::now();
            let heartbeat = control::Heartbeat {
                sample_id: Uuid::new_v4().to_string(),
                ready: self.state.serving.load(Ordering::Acquire),
                agent_ready: self.state.agent_healthy.load(Ordering::Acquire),
                latency_ms: 0,
                ..Default::default()
            };
            let frame = control::Upstream {
                frame: Some(heartbeat.into()),
                ..Default::default()
            };
            match tokio::time::timeout(
                HEARTBEAT * 2,
                self.gateway.publish(self.instance_id, vec![frame]),
            )
            .await
            {
                Ok(Ok(response)) => {
                    self.state.gateway_ok.store(now(), Ordering::Release);
                    self.apply_publish(response, &HashMap::new()).await;
                    tracing::trace!(agent_id=%self.agent_id, latency_ms=began.elapsed().as_millis() as i64, "Heartbeat acknowledged");
                }
                _ => tracing::warn!(agent_id=%self.agent_id, "Heartbeat did not reach the gateway"),
            }
        }
    }
    /// Drain the outbox into Publish calls. Failures keep frames in order and retry.
    pub async fn shipper(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        loop {
            tokio::select! {
                _=shutdown.changed()=>return,
                _=self.state.outbox_notify.notified()=>{}
            }
            loop {
                let frames: Vec<control::Upstream> = {
                    let mut outbox = self.state.outbox.lock().unwrap_or_else(|e| e.into_inner());
                    let take = outbox.len().min(BATCH);
                    outbox.drain(..take).collect()
                };
                if frames.is_empty() {
                    break;
                }
                let threads: HashMap<String, Uuid> = frames
                    .iter()
                    .filter_map(|f| match &f.frame {
                        Some(control::upstream::Frame::Event(event)) => event
                            .event
                            .as_option()
                            .map(|e| (e.id.clone(), id(&e.thread_id).unwrap_or_default())),
                        _ => None,
                    })
                    .collect();
                match self.gateway.publish(self.instance_id, frames.clone()).await {
                    Ok(response) => {
                        self.state.gateway_ok.store(now(), Ordering::Release);
                        self.apply_publish(response, &threads).await;
                    }
                    Err(_) => {
                        tracing::warn!(agent_id=%self.agent_id, frames=frames.len(), "Gateway publish failed; retrying");
                        {
                            let mut outbox =
                                self.state.outbox.lock().unwrap_or_else(|e| e.into_inner());
                            for frame in frames.into_iter().rev() {
                                outbox.push_front(frame);
                            }
                        }
                        tokio::select! {_=shutdown.changed()=>return,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
                    }
                }
            }
        }
    }
    /// Lost leases fail local work; a rejected event means this replica's cache and
    /// the record disagree, so the thread is dropped and re-hydrated on next use
    /// rather than kept as a diverged source for every later event.
    async fn apply_publish(
        &self,
        response: control::PublishResponse,
        threads: &HashMap<String, Uuid>,
    ) {
        for lease in response.lost {
            let _ = self.apply_lease(lease).await;
        }
        let mut diverged = std::collections::BTreeSet::new();
        for event in response.rejected_event_ids {
            tracing::error!(agent_id=%self.agent_id, event_id=%event, "Gateway rejected an event; local state has diverged from the projection");
            if let Some(thread) = threads.get(&event).filter(|t| !t.is_nil()) {
                diverged.insert(*thread);
            }
        }
        for thread in diverged {
            self.resync(thread).await;
        }
    }
    /// Periodic cache maintenance: idle threads leave memory and release their leases.
    pub async fn housekeeping(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! { _=shutdown.changed()=>return, _=tick.tick()=>{} }
            self.evict_idle().await;
        }
    }
    /// Upload attachment bytes separately so a slow object store never delays events.
    pub async fn attachment_worker(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        loop {
            tokio::select! {
                _=shutdown.changed()=>return,
                _=tick.tick()=>{}
            }
            if self.transfer_attachments().await.is_err() {
                tracing::warn!(agent_id=%self.agent_id, "Attachment transfer to the gateway will retry");
            }
        }
    }
    /// Consume one Watch stream until it ends or falls silent.
    pub(crate) async fn consume(
        &self,
        node: &super::super::sidecar::Node,
        stream: &mut gateway::WatchStream,
    ) -> Result<()> {
        loop {
            let message = tokio::time::timeout(WATCH_IDLE, stream.message())
                .await
                .map_err(|_| ChatError::Transport)?
                .map_err(|_| ChatError::Transport)?;
            let Some(message) = message else {
                return Ok(());
            };
            let frame: control::WatchResponse = message.to_owned_message();
            match frame.frame {
                Some(control::watch_response::Frame::Snapshot(snapshot)) => {
                    if let Some(configuration) = snapshot.configuration.into_option() {
                        self.configure(configuration);
                    }
                    for lease in snapshot.leases {
                        self.apply_lease(lease).await?;
                    }
                }
                Some(control::watch_response::Frame::Configuration(configuration)) => {
                    self.configure(*configuration)
                }
                Some(control::watch_response::Frame::Lease(lease)) => {
                    self.apply_lease(*lease).await?
                }
                Some(control::watch_response::Frame::Directive(directive)) => {
                    // Watch resends unacked directives on every reconnect; run each once.
                    let key = id(&directive.id).unwrap_or_default();
                    let fresh = self
                        .state
                        .directives_in_flight
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(key);
                    if !fresh {
                        continue;
                    }
                    let node = node.clone();
                    let state = self.state.clone();
                    tokio::spawn(async move {
                        node.directive(*directive).await;
                        state
                            .directives_in_flight
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(&key);
                    });
                }
                Some(control::watch_response::Frame::Ping(_)) | None => {}
            }
        }
    }
}
