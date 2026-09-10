// SPDX-License-Identifier: Apache-2.0

use crate::bounded_channel::{BoundedSender, SendError};
use flume::r#async::SendFut;

// #[derive(Clone)]
// pub enum OTLPPayload {
//     Traces(Vec<ResourceSpans>),
//     Metrics(Vec<ResourceMetrics>),
// }

#[derive(Clone)]
pub struct OTLPOutput<T> {
    tx: BoundedSender<T>,
    pub validator: Option<std::sync::Arc<dyn Fn(&T) -> Result<(), u16> + Send + Sync>>,
}

impl<T> OTLPOutput<T> {
    pub fn new(tx: BoundedSender<T>) -> Self {
        Self {
            tx,
            validator: None,
        }
    }

    /// Validate an entire decoded request before batching can split it.
    pub fn with_validator(
        mut self,
        validator: impl Fn(&T) -> Result<(), u16> + Send + Sync + 'static,
    ) -> Self {
        self.validator = Some(std::sync::Arc::new(validator));
        self
    }

    pub async fn send(&self, events: T) -> Result<(), SendError> {
        self.tx.send(events).await
    }

    pub fn send_async(&self, events: T) -> SendFut<'_, T> {
        self.tx.send_async(events)
    }
}
