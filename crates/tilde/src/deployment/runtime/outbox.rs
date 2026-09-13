//! Ship queued frames to the gateway in order and apply what it answers:
//! acknowledgements, claim outcomes, and fences for stale generations.
use super::*;
use control::upstream::Frame;
use secrecy::ExposeSecret;
use std::time::Duration;
const BATCH: usize = 256;
impl Runtime {
    async fn heartbeat(&self) -> control::Heartbeat {
        let began = std::time::Instant::now();
        let probe = crate::proto::tilde::agent_host::v1::HealthzRequest::default();
        let agent_ready = match crate::chat::runtime::client(
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
        self.state
            .agent_healthy
            .store(agent_ready, Ordering::Release);
        control::Heartbeat {
            sample_id: Uuid::new_v4().to_string(),
            ready: self.state.serving.load(Ordering::Acquire),
            agent_ready,
            latency_ms: began.elapsed().as_millis().min(i32::MAX as u128) as i32,
            ..Default::default()
        }
    }
    pub fn healthy(&self) -> bool {
        self.state.serving.load(Ordering::Acquire)
            && self.state.agent_healthy.load(Ordering::Acquire)
            && self.gateway_healthy()
    }
    /// Drain the outbox into Publish calls. Failures keep frames in order and retry.
    pub async fn shipper(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut evict = tokio::time::interval(Duration::from_secs(60));
        loop {
            let mut heartbeat = None;
            tokio::select! {
                _=shutdown.changed()=>return,
                _=tick.tick()=>{ heartbeat=Some(self.heartbeat().await); }
                _=evict.tick()=>{ self.evict_idle().await; continue; }
                _=self.state.outbox_notify.notified()=>{}
            }
            let mut frames: Vec<control::Upstream> = {
                let mut outbox = self.state.outbox.lock().unwrap_or_else(|e| e.into_inner());
                let take = outbox.len().min(BATCH);
                outbox.drain(..take).collect()
            };
            if let Some(heartbeat) = heartbeat {
                frames.insert(
                    0,
                    control::Upstream {
                        frame: Some(heartbeat.into()),
                        ..Default::default()
                    },
                );
            }
            if frames.is_empty() {
                continue;
            }
            match self.gateway.publish(self.instance_id, frames.clone()).await {
                Ok(response) => {
                    self.state.gateway_ok.store(now(), Ordering::Release);
                    self.apply_publish(response).await;
                    if !self
                        .state
                        .outbox
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .is_empty()
                    {
                        self.state.outbox_notify.notify_one();
                    }
                }
                Err(_) => {
                    tracing::warn!(agent_id=%self.agent_id, "Gateway publish failed; retrying");
                    {
                        let mut outbox =
                            self.state.outbox.lock().unwrap_or_else(|e| e.into_inner());
                        for frame in frames.into_iter().rev() {
                            if !matches!(frame.frame, Some(Frame::Heartbeat(_))) {
                                outbox.push_front(frame);
                            }
                        }
                    }
                    tokio::select! {_=shutdown.changed()=>return,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
                    self.state.outbox_notify.notify_one();
                }
            }
        }
    }
    async fn apply_publish(&self, response: control::PublishResponse) {
        for fence in response.fences {
            if let Ok(thread) = id(&fence.thread_id) {
                let _ = self.fence(thread, fence.generation).await;
            }
        }
        for event in response.rejected_event_ids {
            tracing::error!(agent_id=%self.agent_id, event_id=%event, "Gateway rejected an event; local state has diverged from the projection");
        }
    }
    /// Upload attachment bytes separately so a slow object store never delays heartbeats.
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
    /// Consume one Watch stream until it ends.
    pub(crate) async fn consume(
        &self,
        node: &super::super::sidecar::Node,
        stream: &mut gateway::WatchStream,
    ) -> Result<()> {
        while let Some(message) = stream.message().await.map_err(|_| ChatError::Transport)? {
            let frame: control::WatchResponse = message.to_owned_message();
            match frame.frame {
                Some(control::watch_response::Frame::Snapshot(snapshot)) => {
                    if let Some(configuration) = snapshot.configuration.into_option() {
                        self.configure(configuration);
                    }
                    for assignment in snapshot.assignments {
                        self.apply_assignment(assignment).await?;
                    }
                }
                Some(control::watch_response::Frame::Configuration(configuration)) => {
                    self.configure(*configuration)
                }
                Some(control::watch_response::Frame::Assignment(assignment)) => {
                    self.apply_assignment(*assignment).await?
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
        Ok(())
    }
}
