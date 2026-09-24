//! Best-effort in-memory audit batches. Transaction status is checked off the
//! request path: rolled-back mutations never become history. Canonical records
//! and sequence allocation remain transactional; a crash may lose queued events.
use super::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tracing::Instrument;

const CAPACITY_BYTES: usize = 32 * 1024 * 1024;
const BATCH: usize = 256;
const FLUSH_INTERVAL: Duration = Duration::from_millis(25);
const GAP_WAIT: Duration = Duration::from_secs(1);

#[derive(Clone)]
struct Queue {
    sender: mpsc::Sender<Command>,
    memory: Arc<Semaphore>,
    // Pin the allocation used as the registry key, even if pool options are replaced.
    _identity: Arc<tokio_postgres::Config>,
}
struct Pending {
    thread: Uuid,
    transaction: i64,
    entity: Uuid,
    at: DateTime<Utc>,
    event: types::Activity,
    queued: Instant,
    _memory: OwnedSemaphorePermit,
}
enum Command {
    Event(Box<Pending>),
    Flush(oneshot::Sender<()>),
}
static QUEUES: OnceLock<Mutex<HashMap<usize, Queue>>> = OnceLock::new();
fn queues() -> &'static Mutex<HashMap<usize, Queue>> {
    QUEUES.get_or_init(Mutex::default)
}
fn key(pool: &crate::database::Pool) -> usize {
    Arc::as_ptr(&pool.config()) as usize
}
fn get(pool: &crate::database::Pool) -> Queue {
    let identity = pool.config();
    let key = Arc::as_ptr(&identity) as usize;
    let mut queues = queues().lock().unwrap();
    queues
        .entry(key)
        .or_insert_with(|| {
            let (sender, receiver) = mpsc::channel(4096);
            tokio::spawn(
                run(pool.clone(), key, receiver).instrument(tracing::info_span!("audit.flush")),
            );
            Queue {
                sender,
                memory: Arc::new(Semaphore::new(CAPACITY_BYTES)),
                _identity: identity,
            }
        })
        .clone()
}

pub(crate) fn enqueue(
    pool: &crate::database::Pool,
    thread: Uuid,
    transaction: i64,
    at: DateTime<Utc>,
    event: types::Activity,
) -> Result<()> {
    let entity = id(&event.entity_id)?;
    let queue = get(pool);
    let size = event.compute_size(&mut buffa::SizeCache::default()) as usize + 1024;
    let Ok(memory) = queue
        .memory
        .clone()
        .try_acquire_many_owned(size.min(u32::MAX as usize) as u32)
    else {
        tracing::warn!(%thread, sequence=event.sequence, "Audit buffer full; dropping audit event");
        return Ok(());
    };
    if queue
        .sender
        .try_send(Command::Event(Box::new(Pending {
            thread,
            transaction,
            entity,
            at,
            event,
            queued: Instant::now(),
            _memory: memory,
        })))
        .is_err()
    {
        tracing::warn!(%thread, "Audit queue full; dropping audit event");
    }
    Ok(())
}

/// Explicit barrier for audit reads, orderly shutdown and integration tests.
/// Uncommitted events are excluded, so reading history inside a transaction cannot deadlock it.
pub async fn flush(pool: &crate::database::Pool) -> Result<()> {
    let queue = queues().lock().unwrap().get(&key(pool)).cloned();
    let Some(queue) = queue else {
        return Ok(());
    };
    let (sent, received) = oneshot::channel();
    tokio::time::timeout(Duration::from_secs(5), async {
        queue
            .sender
            .send(Command::Flush(sent))
            .await
            .map_err(|_| ChatError::Transport)?;
        received.await.map_err(|_| ChatError::Transport)
    })
    .await
    .map_err(|_| ChatError::Transport)?
}

async fn run(pool: crate::database::Pool, key: usize, mut receiver: mpsc::Receiver<Command>) {
    let mut pending = Vec::new();
    let mut barriers = Vec::new();
    let mut tick = tokio::time::interval(FLUSH_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    'worker: loop {
        if pool.is_closed() {
            break;
        }
        let force = tokio::select! {
            command = receiver.recv() => match command {
                Some(Command::Event(event)) => { pending.push(*event); pending.len() >= BATCH },
                Some(Command::Flush(done)) => { barriers.push(done); true },
                None => break,
            },
            _ = tick.tick() => true,
        };
        if !force {
            continue;
        }
        while let Ok(command) = receiver.try_recv() {
            match command {
                Command::Event(event) => pending.push(*event),
                Command::Flush(done) => barriers.push(done),
            }
        }
        if pending.is_empty() {
            for done in barriers.drain(..) {
                let _ = done.send(());
            }
            continue;
        }
        let transactions: Vec<_> = pending
            .iter()
            .map(|p| p.transaction)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let states = match async {
            crate::chat::db::audit_transactions_all(&pool.get().await?, &transactions).await
        }
        .await
        {
            Ok(rows) => rows
                .into_iter()
                .map(|r| (r.transaction_id, r.status))
                .collect::<HashMap<_, _>>(),
            Err(_) => {
                tracing::warn!("Audit transaction-status check failed; retaining queued events");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        pending.retain(|p| {
            matches!(
                states.get(&p.transaction).and_then(|s| s.as_deref()),
                Some("committed" | "in progress")
            )
        });
        let mut ready = BTreeMap::<Uuid, Vec<usize>>::new();
        for (index, event) in pending.iter().enumerate() {
            if states.get(&event.transaction).and_then(|s| s.as_deref()) == Some("committed") {
                ready.entry(event.thread).or_default().push(index);
            }
        }
        let mut handled = HashSet::new();
        for (thread, mut indexes) in ready.into_iter().take(64) {
            indexes.sort_by_key(|i| pending[*i].event.sequence);
            indexes.truncate(BATCH);
            let result = tokio::select! {
                result = write_batch(&pool, thread, &pending, &indexes) => result,
                _ = pool.close_event() => break 'worker,
            };
            match result {
                Ok(sequences) => {
                    handled.extend(sequences.into_iter().map(|sequence| (thread, sequence)))
                }
                Err(_) => {
                    tracing::warn!(%thread, "Audit batch flush failed; retaining queued events");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        pending.retain(|p| !handled.contains(&(p.thread, p.event.sequence)));
        if !pending
            .iter()
            .any(|p| states.get(&p.transaction).and_then(|s| s.as_deref()) == Some("committed"))
        {
            for done in barriers.drain(..) {
                let _ = done.send(());
            }
        }
    }
    queues().lock().unwrap().remove(&key);
}

async fn write_batch(
    pool: &crate::database::Pool,
    thread: Uuid,
    pending: &[Pending],
    indexes: &[usize],
) -> Result<Vec<i64>> {
    let mut tx_client = pool.get().await?;
    let tx = tx_client.transaction().await?;
    // Separate statement: after acquiring this lock, the next snapshot sees the
    // preceding flusher's commit, including flushers on other gateway instances.
    if !crate::chat::db::audit_writer_lock_one(&tx, thread)
        .await?
        .locked
    {
        return Ok(vec![]);
    }
    let mut sequences = Vec::new();
    let mut kinds = Vec::new();
    let mut entities = Vec::new();
    let mut deltas = Vec::new();
    let mut snapshots = Vec::new();
    let mut times = Vec::new();
    let mut skip_gaps = Vec::new();
    for index in indexes {
        let p = &pending[*index];
        sequences.push(p.event.sequence);
        kinds.push(p.event.kind.clone());
        entities.push(p.entity);
        deltas.push(p.event.text_delta.clone());
        snapshots.push(p.event.encode_to_vec());
        times.push(p.at);
        skip_gaps.push(p.queued.elapsed() >= GAP_WAIT);
    }
    let rows = crate::chat::db::audit_flush_all(
        &tx, thread, &sequences, &kinds, &entities, &deltas, &snapshots, &times, &skip_gaps,
    )
    .await?;
    tx.commit().await?;
    drop(tx_client);
    Ok(rows.into_iter().map(|r| r.sequence).collect())
}
