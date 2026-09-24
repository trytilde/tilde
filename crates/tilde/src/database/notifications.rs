//! PostgreSQL notifications invalidate durable reads; they never carry event data.
//! Subscribe before reading, drain committed state, then wait. Reconnection also
//! invalidates readers because NOTIFY is not a durable queue.
use super::{DbError, Pool};
use futures::StreamExt;
use tokio::sync::{OnceCell, watch};
use tokio_postgres::AsyncMessage;

#[derive(Default)]
pub struct Notifications {
    receiver: OnceCell<watch::Receiver<()>>,
}

impl Notifications {
    /// One listener per service/channel, shared by all local subscribers.
    pub async fn subscribe(
        &self,
        pool: &Pool,
        channel: &'static str,
    ) -> Result<watch::Receiver<()>, DbError> {
        let receiver = self
            .receiver
            .get_or_try_init(|| async {
                // LISTEN owns a connection for its lifetime; do not borrow one from the query pool.
                let config = pool.config();
                let mut listener = Listener::connect(&config, channel).await?;
                let (sender, receiver) = watch::channel(());
                let pool = pool.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = sender.closed() => break,
                            _ = pool.close_event() => break,
                            result = listener.next() => {
                                // Wake readers on every notification and on errors alike; a
                                // durable read can report the failure itself.
                                sender.send_replace(());
                                if result.is_err() {
                                    // Recreate the connection. Waking only before reconnect
                                    // could miss commits made during the disconnect.
                                    loop {
                                        tokio::select! {
                                            _ = sender.closed() => return,
                                            _ = pool.close_event() => return,
                                            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
                                        }
                                        tokio::select! {
                                            _ = sender.closed() => return,
                                            _ = pool.close_event() => return,
                                            result = Listener::connect(&config, channel) => if let Ok(next) = result {
                                                listener = next;
                                                sender.send_replace(());
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
                Ok::<_, DbError>(receiver)
            })
            .await?;
        Ok(receiver.clone())
    }
}

/// A dedicated connection, continuously polled even while LISTEN is being sent.
/// Only invalidation signals cross the bounded channel; event data stays in Postgres.
struct Listener {
    client: tokio_postgres::Client,
    messages: tokio::sync::mpsc::Receiver<()>,
    driver: tokio::task::JoinHandle<()>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.driver.abort();
    }
}
impl Listener {
    async fn connect(config: &tokio_postgres::Config, channel: &str) -> Result<Self, DbError> {
        let (client, mut connection) = config.connect(super::tls()?).await?;
        let (sender, messages) = tokio::sync::mpsc::channel(1);
        let driver = tokio::spawn(async move {
            let mut stream = futures::stream::poll_fn(move |cx| connection.poll_message(cx));
            while let Some(message) = stream.next().await {
                match message {
                    Ok(AsyncMessage::Notification(_)) => {
                        let _ = sender.try_send(());
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            // Dropping sender wakes the consumer on connection loss.
        });
        let listener = Self {
            client,
            messages,
            driver,
        };
        let channel = channel.replace('"', "\"\"");
        listener
            .client
            .batch_execute(&format!("LISTEN \"{channel}\""))
            .await?;
        Ok(listener)
    }
    async fn next(&mut self) -> Result<(), DbError> {
        self.messages
            .recv()
            .await
            .ok_or_else(|| DbError::decode("listener connection closed"))
    }
}
