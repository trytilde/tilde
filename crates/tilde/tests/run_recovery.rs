//! Run-protocol heartbeats against sidecar recovery. Heartbeats that only refresh liveness no
//! longer notify recovery (each NOTIFY serialised the heartbeat's commit behind every other
//! notifying commit on the server), so a change of readiness must still wake it.
mod common;
use common::host::Gateway;
use std::time::{Duration, Instant};
use tilde::proto::tilde::run::v1 as run;
use uuid::Uuid;

/// A replica that reports itself not ready gives up a thread it holds with nothing running
/// at once: the heartbeat wakes recovery well before its interval would.
#[tokio::test]
async fn a_replica_reporting_not_ready_gives_up_its_idle_threads_at_once() {
    let f = Gateway::start(71, Default::default(), Default::default()).await;
    let lease = async |thread: Uuid, instance: Uuid| {
        let lease = async {
            f.db.pool
                .get()
                .await
                .unwrap()
                .execute(
                    "INSERT INTO thread_leases(thread_id,agent_id,instance_id) VALUES($1,$2,$3)",
                    &[&thread, &f.agent, &instance],
                )
                .await
                .unwrap()
        };
        lease.await;
    };
    let held = async |thread: Uuid| {
        let held = async {
            f.db.pool
                .get()
                .await
                .unwrap()
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM thread_leases WHERE thread_id=$1)",
                    &[&thread],
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        };
        held.await
    };
    let thread = Uuid::new_v4();
    lease(thread, f.instance).await;
    // A lease of a replica that never registered: recovery's first pass drops it.
    let probe = Uuid::new_v4();
    lease(probe, Uuid::new_v4()).await;
    let heartbeat = async |ready: bool| {
        f.host
            .heartbeat(run::HeartbeatRequest {
                instance_id: f.instance.to_string(),
                ready,
                ..Default::default()
            })
            .await
            .unwrap();
    };
    heartbeat(true).await;

    let (stop, stopped) = tokio::sync::watch::channel(false);
    let worker = f.deployments.clone().recovery_worker(stopped);
    let worker = tokio::spawn(worker);
    // The first pass runs once recovery listens; its next is a third of the liveness window on.
    let deadline = Instant::now() + Duration::from_secs(20);
    while held(probe).await {
        assert!(Instant::now() < deadline, "recovery's first pass runs");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let first_pass = Instant::now();
    assert!(held(thread).await, "a ready replica keeps its thread");

    heartbeat(false).await;
    while held(thread).await {
        assert!(
            first_pass.elapsed() < tilde::deployment::LIVENESS / 4,
            "the not-ready heartbeat wakes recovery before its interval"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let _ = stop.send(true);
    worker.await.unwrap();
    f.close().await;
}
