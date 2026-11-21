//! PostgreSQL notifications invalidate durable reads; they never carry event data.
//! Subscribe before reading, drain committed state, then wait. Reconnection also
//! invalidates readers because NOTIFY is not a durable queue.
//!
//! A process holds ONE listening session per server (`DATABASE_DIRECT_URL`: LISTEN needs a
//! session, which PgBouncer's transaction pooling does not give), shared by every service and
//! channel: the [`Hub`] task LISTENs on each channel as it is first subscribed and fans
//! notifications out in-process. Direct connections are scarce (PlanetScale clusters allow 25),
//! so the count must not grow with the number of services.
use super::{DbError, Pool};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::{OnceCell, mpsc, oneshot, watch};
use tokio_postgres::AsyncMessage;

/// One service's subscription to one channel.
#[derive(Default)]
pub struct Notifications {
    receiver: OnceCell<watch::Receiver<()>>,
}

impl Notifications {
    pub async fn subscribe(
        &self,
        pool: &Pool,
        channel: &'static str,
    ) -> Result<watch::Receiver<()>, DbError> {
        let receiver = self
            .receiver
            .get_or_try_init(|| subscribe(pool, channel))
            .await?;
        Ok(receiver.clone())
    }
}

struct Subscribe {
    channel: &'static str,
    reply: oneshot::Sender<Result<watch::Receiver<()>, DbError>>,
}
type Hub = mpsc::UnboundedSender<Subscribe>;

/// Hubs by server: notifications are database-wide, so pools that differ only in their
/// options (search path) share one.
fn hubs() -> &'static Mutex<HashMap<String, Hub>> {
    static HUBS: OnceLock<Mutex<HashMap<String, Hub>>> = OnceLock::new();
    HUBS.get_or_init(Mutex::default)
}
fn server(config: &tokio_postgres::Config) -> String {
    format!(
        "{:?} {:?} {:?} {:?}",
        config.get_hosts(),
        config.get_ports(),
        config.get_user(),
        config.get_dbname()
    )
}

async fn subscribe(pool: &Pool, channel: &'static str) -> Result<watch::Receiver<()>, DbError> {
    let config = pool.direct_config();
    let key = server(&config);
    // A hub ends once nobody listens; one found ending is replaced.
    for _ in 0..3 {
        let hub = hubs()
            .lock()
            .unwrap()
            .entry(key.clone())
            .or_insert_with(|| {
                let (hub, commands) = mpsc::unbounded_channel();
                tokio::spawn(run(config.clone(), key.clone(), commands));
                hub
            })
            .clone();
        let (reply, answer) = oneshot::channel();
        if hub.send(Subscribe { channel, reply }).is_ok()
            && let Ok(result) = answer.await
        {
            return result;
        }
        let mut hubs = hubs().lock().unwrap();
        if hubs.get(&key).is_some_and(|h| h.same_channel(&hub)) {
            hubs.remove(&key);
        }
    }
    Err(DbError::decode("the notification listener stopped"))
}

/// The hub: one listening session, its channels, and their local subscribers.
async fn run(
    config: Arc<tokio_postgres::Config>,
    key: String,
    mut commands: mpsc::UnboundedReceiver<Subscribe>,
) {
    let mut channels: HashMap<&'static str, watch::Sender<()>> = HashMap::new();
    let mut listener: Option<Listener> = None;
    let wake_all = |channels: &HashMap<&'static str, watch::Sender<()>>| {
        for sender in channels.values() {
            sender.send_replace(());
        }
    };
    let mut idle = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        tokio::select! {
            Some(Subscribe { channel, reply }) = commands.recv() => {
                if let Some(sender) = channels.get(channel) {
                    let _ = reply.send(Ok(sender.subscribe()));
                    continue;
                }
                if listener.is_none() {
                    match Listener::connect(&config, channels.keys().copied()).await {
                        Ok(next) => {
                            listener = Some(next);
                            wake_all(&channels);
                        }
                        Err(error) => {
                            let _ = reply.send(Err(error));
                            continue;
                        }
                    }
                }
                let listening = listener.as_ref().expect("connected above");
                match listening.listen(channel).await {
                    Ok(()) => {
                        let (sender, receiver) = watch::channel(());
                        channels.insert(channel, sender);
                        let _ = reply.send(Ok(receiver));
                    }
                    Err(error) => {
                        // Readers of the other channels catch up once it is back.
                        listener = None;
                        wake_all(&channels);
                        let _ = reply.send(Err(error));
                    }
                }
            }
            message = next(&mut listener) => match message {
                Ok(channel) => {
                    if let Some(sender) = channels.get(channel.as_str()) {
                        sender.send_replace(());
                    }
                }
                Err(_) => {
                    // Wake on loss and again on reconnect: waking only before reconnecting
                    // could miss commits made during the disconnect.
                    listener = None;
                    wake_all(&channels);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    if let Ok(next) = Listener::connect(&config, channels.keys().copied()).await {
                        listener = Some(next);
                        wake_all(&channels);
                    }
                }
            },
            _ = idle.tick() => {
                channels.retain(|_, sender| !sender.is_closed());
                if listener.is_none() && !channels.is_empty()
                    && let Ok(next) = Listener::connect(&config, channels.keys().copied()).await
                {
                    listener = Some(next);
                    wake_all(&channels);
                }
                if channels.is_empty() {
                    let mut hubs = hubs().lock().unwrap();
                    // Subscriptions sent before the hub left the map are still answered.
                    if commands.is_empty() {
                        hubs.remove(&key);
                        return;
                    }
                }
            }
        }
    }
}

/// The next notification's channel; pending without a session, which the idle tick (or the
/// next subscription) reconnects.
async fn next(listener: &mut Option<Listener>) -> Result<String, DbError> {
    match listener {
        Some(listener) => listener.next().await,
        None => std::future::pending().await,
    }
}

/// A dedicated session, continuously polled even while LISTEN is being sent.
/// Only channel names cross the channel; event data stays in Postgres.
struct Listener {
    client: tokio_postgres::Client,
    messages: mpsc::UnboundedReceiver<String>,
    driver: tokio::task::JoinHandle<()>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.driver.abort();
    }
}
impl Listener {
    async fn connect(
        config: &tokio_postgres::Config,
        channels: impl Iterator<Item = &'static str>,
    ) -> Result<Self, DbError> {
        let (client, mut connection) = config.connect(super::tls()?).await?;
        let (sender, messages) = mpsc::unbounded_channel();
        let driver = tokio::spawn(async move {
            let mut stream = futures::stream::poll_fn(move |cx| connection.poll_message(cx));
            while let Some(message) = stream.next().await {
                match message {
                    Ok(AsyncMessage::Notification(n)) => {
                        let _ = sender.send(n.channel().to_owned());
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
        for channel in channels {
            listener.listen(channel).await?;
        }
        Ok(listener)
    }
    async fn listen(&self, channel: &str) -> Result<(), DbError> {
        let channel = channel.replace('"', "\"\"");
        Ok(self
            .client
            .batch_execute(&format!("LISTEN \"{channel}\""))
            .await?)
    }
    async fn next(&mut self) -> Result<String, DbError> {
        self.messages
            .recv()
            .await
            .ok_or_else(|| DbError::decode("listener connection closed"))
    }
}
