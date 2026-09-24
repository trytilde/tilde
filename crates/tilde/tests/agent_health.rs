mod common;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tilde::{
    agent::health::{AgentHealth, metrics},
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::service::Connections,
    deployment::{Deployments, RegisterDeployment},
    encryption::Encryption,
    proto::tilde::{run::v1 as run, types::v1 as types, types::v1::AgentHealthStatus},
    services::tilde::run::v1::RunServiceClient,
};
use uuid::Uuid;

async fn create(agents: &Agents) -> Uuid {
    let id = Uuid::new_v4();
    agents
        .create(CreateAgent {
            concurrency_policy: Default::default(),
            capabilities: Default::default(),
            id,
            name: "Health fixture".into(),
        })
        .await
        .unwrap();
    id
}

/// The sweep's advisory lock is database-wide, so a sweep from a parallel test (another
/// schema) makes `poll_once` skip; retry until this agent has a new sample.
async fn sample(health: &AgentHealth, pool: &tilde::database::Pool, id: Uuid) {
    let count = async || -> i64 {
        pool.get()
            .await
            .unwrap()
            .query_one(
                "SELECT COUNT(*) FROM agent_health WHERE agent_id=$1",
                &[&id],
            )
            .await
            .unwrap()
            .get(0)
    };
    let before = count().await;
    while count().await == before {
        health.poll_once().await.unwrap();
    }
}

#[tokio::test]
async fn missing_deployments_are_sampled_as_unhealthy_and_missing_observations_stay_unknown() {
    let db = common::Database::new().await;
    let encryption = std::sync::Arc::new(
        tilde::encryption::Encryption::initialize(&db.pool, common::seed(5))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), encryption);
    let health = AgentHealth::new(db.pool.clone());
    let id = create(&agents).await;
    assert_eq!(
        metrics(&db.pool, &[id], Utc::now()).await.unwrap()[&id].health,
        AgentHealthStatus::Unknown
    );
    sample(&health, &db.pool, id).await;
    let view = metrics(&db.pool, &[id], Utc::now()).await.unwrap();
    assert_eq!(view[&id].health, AgentHealthStatus::Unhealthy);
    assert_eq!(view[&id].health_history.len(), 12);
    assert_eq!(
        view[&id]
            .health_history
            .iter()
            .map(|h| h.failed_checks)
            .sum::<u32>(),
        1
    );
    let stale = metrics(&db.pool, &[id], Utc::now() + Duration::seconds(100))
        .await
        .unwrap();
    assert_eq!(stale[&id].health, AgentHealthStatus::Unknown);
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            include_str!("../../../queries/_tests/health_retention.sql"),
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(health.cleanup().await.unwrap(), 1);
    let (stop, rx) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(health.clone().run(rx));
    stop.send(true).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
    sample(&health, &db.pool, id).await;
    let before = metrics(&db.pool, &[id], Utc::now()).await.unwrap()[&id]
        .health_history
        .iter()
        .map(|h| h.total_checks)
        .sum::<u32>();
    agents.pause(id).await.unwrap();
    agents.delete(id).await.unwrap();
    health.poll_once().await.unwrap();
    let after =
        tilde::agent::db::health_history_all(&db.pool.get().await.unwrap(), &[id], Utc::now())
            .await
            .unwrap()
            .iter()
            .map(|h| h.total_checks)
            .sum::<i64>();
    assert_eq!(
        i64::from(before),
        after,
        "deleted agents receive no more samples"
    );
    db.close().await;
}

/// Health is never probed: it is sampled from whether a host is dialled in. A Gateway
/// deployment is healthy only while its host holds an open Watch and heartbeats ready.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_dialled_in_host_is_sampled_healthy_until_its_stream_closes() {
    use connectrpc::client::{ClientConfig, HttpClient};
    let db = common::Database::new().await;
    let crypto = Arc::new(
        Encryption::initialize(&db.pool, common::seed(7))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), crypto.clone());
    let connections = Connections::new(
        db.pool.clone(),
        crypto.clone(),
        "http://localhost:1".into(),
        "http://localhost:2".into(),
    )
    .unwrap();
    let deployments = Deployments::new(
        db.pool.clone(),
        crypto.clone(),
        agents.clone(),
        connections.clone(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let chat = Chat::new(db.pool.clone(), crypto, base.clone())
        .with_connections(connections)
        .with_deployments(deployments.clone());
    let router = tilde::iam::listeners::agent_rpc_router(agents.clone(), chat);
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let health = AgentHealth::new(db.pool.clone());
    let id = create(&agents).await;
    let status = async || {
        sample(&health, &db.pool, id).await;
        metrics(&db.pool, &[id], Utc::now()).await.unwrap()[&id].health
    };
    let (_, token, _) = deployments
        .register_deployment(
            id,
            RegisterDeployment {
                source: types::DeploymentSource::Manual,
                target: types::DeploymentTarget::Gateway,
                target_reference: None,
                repository: None,
                commit_sha: None,
                external_id: Some("health".into()),
                label: None,
                commit_message: None,
                branch: None,
                commit_author: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        status().await,
        AgentHealthStatus::Unhealthy,
        "a registered deployment nobody dialled in from cannot serve"
    );
    let mut headers = http::HeaderMap::new();
    headers.insert(
        http::header::AUTHORIZATION,
        format!(
            "Bearer {}",
            secrecy::ExposeSecret::expose_secret(&token.unwrap())
        )
        .parse()
        .unwrap(),
    );
    let host = RunServiceClient::new(
        HttpClient::plaintext_http2_only(),
        ClientConfig::new(base.parse().unwrap()).with_default_headers(headers),
    );
    let instance = Uuid::new_v4().to_string();
    let mut watch = host
        .watch(run::WatchRequest {
            instance_id: instance.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    watch.message().await.unwrap().unwrap();
    let heartbeat = |ready| run::HeartbeatRequest {
        instance_id: instance.clone(),
        ready,
        ..Default::default()
    };
    host.heartbeat(heartbeat(false)).await.unwrap();
    assert_eq!(
        status().await,
        AgentHealthStatus::Unhealthy,
        "connected but not ready"
    );
    host.heartbeat(heartbeat(true)).await.unwrap();
    assert_eq!(status().await, AgentHealthStatus::Healthy);
    drop(watch);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while status().await != AgentHealthStatus::Unhealthy {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("closing the stream ends health without waiting for a probe");
    // A heartbeat alone cannot revive a deployment whose stream is gone.
    host.heartbeat(heartbeat(true)).await.unwrap();
    assert_eq!(status().await, AgentHealthStatus::Unhealthy);
    server.abort();
    db.close().await;
}

#[tokio::test]
async fn registry_metrics_count_sessions_and_measure_first_visible_reply() {
    let db = common::Database::new().await;
    let crypto = std::sync::Arc::new(
        tilde::encryption::Encryption::initialize(&db.pool, common::seed(6))
            .await
            .unwrap(),
    );
    let agents = Agents::new(db.pool.clone(), crypto);
    let agent = create(&agents).await;
    let other = create(&agents).await;
    let thread = Uuid::new_v4();
    let empty_thread = Uuid::new_v4();
    let participant = Uuid::new_v4();
    db.pool.get().await.unwrap().execute("INSERT INTO chat_threads(id,title,primary_agent_id) VALUES($1,'Messages',$3),($2,'Empty',$3)", &[&thread, &empty_thread, &agent]).await.unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO chat_participants(id,thread_id,agent_id) VALUES($1,$2,$3),($4,$5,$3)",
            &[
                &participant,
                &thread,
                &agent,
                &(Uuid::new_v4()),
                &empty_thread,
            ],
        )
        .await
        .unwrap();
    let run = Uuid::new_v4();
    let invocation = Uuid::new_v4();
    let message = Uuid::new_v4();
    db.pool.get().await.unwrap().execute("INSERT INTO chat_runs(id,thread_id,agent_id,objective,idempotency_key) VALUES($1,$2,$3,'Test','test')", &[&run, &thread, &agent]).await.unwrap();
    let started = Utc::now() - Duration::seconds(2);
    db.pool.get().await.unwrap().execute("INSERT INTO chat_invocations(id,run_id,thread_id,agent_id,status,started_at) VALUES($1,$2,$3,$4,'running',$5)", &[&invocation, &run, &thread, &agent, &started]).await.unwrap();
    db.pool.get().await.unwrap().execute("INSERT INTO chat_messages(id,thread_id,participant_id,text,status,invocation_id) VALUES($1,$2,$3,'','streaming',$4)", &[&message, &thread, &participant, &invocation]).await.unwrap();
    let before = metrics(&db.pool, &[agent, other], Utc::now())
        .await
        .unwrap();
    assert_eq!(before[&agent].thread_count, 2);
    assert_eq!(before[&agent].average_turns_per_thread, Some(0.0));
    assert_eq!(before[&agent].average_response_ms, None);
    assert_eq!(before[&other].thread_count, 0);
    assert_eq!(before[&other].average_turns_per_thread, None);
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_messages SET text='First chunk' WHERE id=$1",
            &[&message],
        )
        .await
        .unwrap();
    let first: chrono::DateTime<Utc> = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT first_content_at FROM chat_messages WHERE id=$1",
            &[&message],
        )
        .await
        .unwrap()
        .get(0);
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_messages SET text='Completed response',status='complete' WHERE id=$1",
            &[&message],
        )
        .await
        .unwrap();
    let after = metrics(&db.pool, &[agent], Utc::now()).await.unwrap();
    assert_eq!(after[&agent].average_turns_per_thread, Some(0.5));
    let expected = (first - started).num_microseconds().unwrap() as f64 / 1000.0;
    assert!((after[&agent].average_response_ms.unwrap() - expected).abs() < 0.01);
    db.close().await;
}
