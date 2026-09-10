//! PostgreSQL notifications invalidate durable reads; they never carry event data.
//! Subscribe before reading, drain committed state, then wait. Reconnection also
//! invalidates readers because NOTIFY is not a durable queue.
use sqlx::{PgPool, postgres::PgListener};
use tokio::sync::{OnceCell, watch};

#[derive(Default)]
pub struct Notifications {
    receiver: OnceCell<watch::Receiver<()>>,
}

impl Notifications {
    /// One listener per service/channel, shared by all local subscribers.
    pub async fn subscribe(
        &self,
        pool: &PgPool,
        channel: &'static str,
    ) -> Result<watch::Receiver<()>, sqlx::Error> {
        let receiver = self.receiver.get_or_try_init(|| async {
            // LISTEN owns a connection for its lifetime; do not exhaust the query pool.
            let listener_pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .connect_lazy_with(pool.connect_options().as_ref().clone());
            let mut listener = PgListener::connect_with(&listener_pool).await?;
            listener.listen(channel).await?;
            let (sender, receiver) = watch::channel(());
            let pool = pool.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = sender.closed() => break,
                        _ = pool.close_event() => break,
                        result = listener.try_recv() => {
                            // try_recv reconnects and restores LISTEN before returning None.
                            // On errors also wake readers; their durable read can report failure.
                            sender.send_replace(());
                            if result.is_err() {
                                // Explicitly recreate on other errors too. Waking only before
                                // reconnect could miss commits made during the disconnect.
                                drop(listener);
                                loop {
                                    tokio::select! {
                                        _ = sender.closed() => return,
                                        _ = pool.close_event() => return,
                                        _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
                                    }
                                    let reconnect = async {
                                        let mut next = PgListener::connect_with(&listener_pool).await?;
                                        next.listen(channel).await?;
                                        Ok::<_, sqlx::Error>(next)
                                    };
                                    tokio::select! {
                                        _ = sender.closed() => return,
                                        _ = pool.close_event() => return,
                                        result = reconnect => if let Ok(next) = result {
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
            Ok::<_, sqlx::Error>(receiver)
        }).await?;
        Ok(receiver.clone())
    }
}
