#[expect(dead_code, reason = "shared test fixtures")]
mod common;
use secrecy::SecretString;
use std::sync::Arc;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{Chat, CreateThread, StartRun},
    connections::service::Connections,
    deployment::{Deployments, RegisterDeployment},
    encryption::Encryption,
    proto::tilde::types::v1 as types,
};
use uuid::Uuid;

struct Fx {
    db: common::Database,
    agents: Agents,
    chat: Chat,
    deployments: Deployments,
    agent: Uuid,
}
impl Fx {
    async fn new() -> Self {
        let db = common::Database::new().await;
        let crypto = Arc::new(
            Encryption::initialize(&db.pool, common::seed(91))
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
        let chat = Chat::new(db.pool.clone(), crypto, "http://127.0.0.1:1".into())
            .with_connections(connections)
            .with_deployments(deployments.clone());
        let agent = Uuid::new_v4();
        agents
            .create(CreateAgent {
                id: agent,
                name: "Deployed".into(),
                endpoint_url: "http://127.0.0.1:3001".into(),
                webhook_signing_key: SecretString::from("deployment-fixture-key-0123456789abcdef"),
                capabilities: Default::default(),
            })
            .await
            .unwrap();
        Self {
            db,
            agents,
            chat,
            deployments,
            agent,
        }
    }
    fn direct(&self, endpoint: &str, external: &str) -> RegisterDeployment {
        RegisterDeployment {
            source: types::DeploymentSource::Ci,
            target: types::DeploymentTarget::Direct,
            endpoint_url: Some(endpoint.into()),
            target_reference: None,
            repository: Some("acme/agent".into()),
            commit_sha: Some("0123abcd".into()),
            external_id: Some(external.into()),
            label: None,
        }
    }
    async fn serving(&self) -> Option<String> {
        let d = self.deployments.get(self.agent).await.unwrap();
        (!d.serving_deployment_id.is_empty()).then_some(d.serving_deployment_id)
    }
    async fn mirrored_endpoint(&self) -> Option<String> {
        self.agents.get(self.agent).await.unwrap().endpoint_url
    }
}

#[tokio::test]
async fn creating_an_agent_with_an_endpoint_registers_its_initial_deployment() {
    let fx = Fx::new().await;
    let list = fx.deployments.deployments(fx.agent).await.unwrap();
    assert_eq!(list.len(), 1);
    let initial = &list[0];
    assert_eq!(
        initial.source.as_known(),
        Some(types::DeploymentSource::Manual)
    );
    assert_eq!(
        initial.target.as_known(),
        Some(types::DeploymentTarget::Direct)
    );
    assert_eq!(initial.endpoint_url, "http://127.0.0.1:3001/");
    assert!(initial.serving && !initial.token_issued);
    assert_eq!(fx.serving().await, Some(initial.id.clone()));
    fx.db.close().await;
}

#[tokio::test]
async fn latest_routing_serves_new_deployments_and_manual_routing_waits_for_promotion() {
    let fx = Fx::new().await;
    // CI registers a deployment: it serves at once and the agent endpoint mirrors it.
    let (second, token, created) = fx
        .deployments
        .register_deployment(fx.agent, fx.direct("http://127.0.0.1:3002", "run-2"))
        .await
        .unwrap();
    assert!(created && token.is_some() && second.serving);
    assert_eq!(fx.serving().await, Some(second.id.clone()));
    assert_eq!(
        fx.mirrored_endpoint().await.as_deref(),
        Some("http://127.0.0.1:3002/")
    );
    // The retry of the same CI job is idempotent and never returns a token again.
    let (again, none, created) = fx
        .deployments
        .register_deployment(fx.agent, fx.direct("http://127.0.0.1:3002", "run-2"))
        .await
        .unwrap();
    assert!(!created && none.is_none() && again.id == second.id);
    // Manual routing: the next deployment registers but does not serve until promoted.
    fx.deployments
        .set(
            fx.agent,
            types::DeploymentMode::Gateway,
            types::SidecarFailureMode::Reassign,
            types::DeploymentRouting::Manual,
        )
        .await
        .unwrap();
    let (third, _, _) = fx
        .deployments
        .register_deployment(fx.agent, fx.direct("http://127.0.0.1:3003", "run-3"))
        .await
        .unwrap();
    assert!(!third.serving);
    assert_eq!(fx.serving().await, Some(second.id.clone()));
    fx.deployments
        .promote(fx.agent, Uuid::parse_str(&third.id).unwrap())
        .await
        .unwrap();
    assert_eq!(fx.serving().await, Some(third.id.clone()));
    assert_eq!(
        fx.mirrored_endpoint().await.as_deref(),
        Some("http://127.0.0.1:3003/")
    );
    // Mirroring never mints a deployment of its own.
    assert_eq!(fx.deployments.deployments(fx.agent).await.unwrap().len(), 3);
    // Retiring the serving deployment hands serving to the newest other one and
    // invalidates its token; an already retired one cannot be promoted.
    let retired = fx
        .deployments
        .retire(fx.agent, Uuid::parse_str(&third.id).unwrap())
        .await
        .unwrap();
    assert_eq!(
        retired.status.as_known(),
        Some(types::DeploymentStatus::Retired)
    );
    assert!(!retired.token_issued);
    assert_eq!(fx.serving().await, Some(second.id.clone()));
    assert!(
        fx.deployments
            .promote(fx.agent, Uuid::parse_str(&third.id).unwrap())
            .await
            .is_err()
    );
    let token = token.unwrap();
    let auth = fx
        .deployments
        .authenticate(secrecy::ExposeSecret::expose_secret(&token))
        .await
        .unwrap();
    assert_eq!(auth.deployment.to_string(), second.id);
    fx.db.close().await;
}

#[tokio::test]
async fn threads_pin_to_the_serving_deployment_on_first_invocation_and_stay_pinned() {
    let fx = Fx::new().await;
    let initial = fx.serving().await.unwrap();
    let thread = fx
        .chat
        .create_thread(CreateThread {
            title: "Pinned".into(),
            primary_agent_id: fx.agent.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(fx.agent.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let run = fx
        .chat
        .start_run(StartRun {
            thread_id: thread.id.clone(),
            agent_id: fx.agent.to_string(),
            objective: "First".into(),
            idempotency_key: "first".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let pinned: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT p.deployment_id,i.deployment_id FROM chat_participants p JOIN chat_invocations i ON i.thread_id=p.thread_id AND i.agent_id=p.agent_id WHERE p.thread_id=$1 AND p.agent_id=$2 AND i.id=$3",
    )
    .bind(Uuid::parse_str(&thread.id).unwrap())
    .bind(fx.agent)
    .bind(Uuid::parse_str(&run.invocation_id).unwrap())
    .fetch_one(&fx.db.pool)
    .await
    .unwrap();
    assert_eq!(pinned.0.map(|v| v.to_string()), Some(initial.clone()));
    assert_eq!(pinned.1, pinned.0);
    // A newer deployment serves new threads; this thread keeps its pin.
    let (second, _, _) = fx
        .deployments
        .register_deployment(fx.agent, fx.direct("http://127.0.0.1:3002", "run-2"))
        .await
        .unwrap();
    assert_eq!(fx.serving().await, Some(second.id.clone()));
    let other = fx
        .chat
        .create_thread(CreateThread {
            title: "New".into(),
            primary_agent_id: fx.agent.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(fx.agent.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    fx.chat
        .start_run(StartRun {
            thread_id: other.id.clone(),
            agent_id: fx.agent.to_string(),
            objective: "Second".into(),
            idempotency_key: "second".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let pins: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT thread_id,deployment_id FROM chat_participants WHERE agent_id=$1 ORDER BY thread_id",
    )
    .bind(fx.agent)
    .fetch_all(&fx.db.pool)
    .await
    .unwrap();
    let by_thread = |id: &str| {
        pins.iter()
            .find(|(t, _)| t.to_string() == id)
            .and_then(|(_, d)| *d)
            .map(|d| d.to_string())
    };
    assert_eq!(by_thread(&thread.id), Some(initial));
    assert_eq!(by_thread(&other.id), Some(second.id));
    fx.db.close().await;
}
