mod common;
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
                description: String::new(),
                concurrency_policy: Default::default(),
                id: agent,
                name: "Deployed".into(),
                capabilities: Default::default(),
            })
            .await
            .unwrap();
        let fx = Self {
            db,
            chat,
            deployments,
            agent,
        };
        // An agent is only a registry entry: nothing serves until a deployment is
        // registered and its host dials in.
        assert!(fx.deployments.deployments(agent).await.unwrap().is_empty());
        let (initial, _, _) = fx
            .deployments
            .register_deployment(agent, fx.gateway("initial"))
            .await
            .unwrap();
        assert!(!initial.serving && fx.serving().await.is_none());
        fx.connect(Uuid::parse_str(&initial.id).unwrap()).await;
        fx
    }
    // Model the authenticated Watch/heartbeat boundary for routing database tests.
    async fn connect(&self, deployment: Uuid) -> (Uuid, Uuid) {
        let instance = Uuid::new_v4();
        let connection = Uuid::new_v4();
        self.deployments
            .register(
                self.agent,
                deployment,
                &tilde::proto::tilde::agent_event_ingress::v1::WatchRequest {
                    instance_id: instance.to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        tilde::deployment::db::watch_open_execute(
            &self.db.pool.get().await.unwrap(),
            self.agent,
            instance,
            connection,
        )
        .await
        .unwrap();
        self.deployments
            .heartbeat(
                self.agent,
                instance,
                &tilde::proto::tilde::agent_event_ingress::v1::Heartbeat {
                    ready: true,
                    agent_ready: true,
                    agent_connected: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        (instance, connection)
    }
    fn gateway(&self, external: &str) -> RegisterDeployment {
        RegisterDeployment {
            source: types::DeploymentSource::Ci,
            target: types::DeploymentTarget::Gateway,
            target_reference: None,
            repository: Some("acme/agent".into()),
            commit_sha: Some("0123abcd".into()),
            external_id: Some(external.into()),
            label: None,
            commit_message: Some("Ship the agent".into()),
            branch: Some("main".into()),
            commit_author: Some("ada".into()),
            declarations: Default::default(),
        }
    }
    async fn serving(&self) -> Option<String> {
        let d = self.deployments.get(self.agent).await.unwrap();
        (!d.serving_deployment_id.is_empty()).then_some(d.serving_deployment_id)
    }
}

#[tokio::test]
async fn latest_routing_serves_new_deployments_and_weighted_routing_waits_for_promotion() {
    let fx = Fx::new().await;
    // CI registers a deployment: it serves once connected.
    let (second, token, created) = fx
        .deployments
        .register_deployment(fx.agent, fx.gateway("run-2"))
        .await
        .unwrap();
    fx.connect(Uuid::parse_str(&second.id).unwrap()).await;
    let second = fx
        .deployments
        .deployment(fx.agent, Uuid::parse_str(&second.id).unwrap())
        .await
        .unwrap();
    assert!(created && token.is_some() && second.serving);
    assert_eq!(fx.serving().await, Some(second.id.clone()));
    // The retry of the same CI job is idempotent and never returns a token again.
    let (again, none, created) = fx
        .deployments
        .register_deployment(fx.agent, fx.gateway("run-2"))
        .await
        .unwrap();
    assert!(!created && none.is_none() && again.id == second.id);
    // Weighted routing: the next deployment registers but does not serve until promoted.
    fx.deployments
        .set(
            fx.agent,
            types::SidecarFailureMode::Reassign,
            types::DeploymentRouting::Weighted,
        )
        .await
        .unwrap();
    let (third, _, _) = fx
        .deployments
        .register_deployment(fx.agent, fx.gateway("run-3"))
        .await
        .unwrap();
    fx.connect(Uuid::parse_str(&third.id).unwrap()).await;
    let third = fx
        .deployments
        .deployment(fx.agent, Uuid::parse_str(&third.id).unwrap())
        .await
        .unwrap();
    assert!(!third.serving);
    assert_eq!(fx.serving().await, Some(second.id.clone()));
    fx.deployments
        .promote(fx.agent, Uuid::parse_str(&third.id).unwrap())
        .await
        .unwrap();
    assert_eq!(fx.serving().await, Some(third.id.clone()));
    // Promotion never mints a deployment of its own.
    assert_eq!(fx.deployments.deployments(fx.agent).await.unwrap().len(), 3);
    // Under weighted routing the serving deployment stays until another is promoted;
    // retiring then invalidates its token, and a retired one cannot be promoted.
    assert!(
        fx.deployments
            .retire(fx.agent, Uuid::parse_str(&third.id).unwrap())
            .await
            .is_err()
    );
    fx.deployments
        .promote(fx.agent, Uuid::parse_str(&second.id).unwrap())
        .await
        .unwrap();
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
    let pinned: (Option<Uuid>, Option<Uuid>) = fx.db.pool.get().await.unwrap().query_one("SELECT p.deployment_id,i.deployment_id FROM chat_participants p JOIN chat_invocations i ON i.thread_id=p.thread_id AND i.agent_id=p.agent_id WHERE p.thread_id=$1 AND p.agent_id=$2 AND i.id=$3", &[&(Uuid::parse_str(&thread.id).unwrap()), &fx.agent, &(Uuid::parse_str(&run.invocation_id).unwrap())]).await.map(|row| (row.get::<_, Option<Uuid>>(0),row.get::<_, Option<Uuid>>(1)))
    .unwrap();
    assert_eq!(pinned.0.map(|v| v.to_string()), Some(initial.clone()));
    assert_eq!(pinned.1, pinned.0);
    // A newer deployment serves new threads; this thread keeps its pin.
    let (second, _, _) = fx
        .deployments
        .register_deployment(fx.agent, fx.gateway("run-2"))
        .await
        .unwrap();
    fx.connect(Uuid::parse_str(&second.id).unwrap()).await;
    let second = fx
        .deployments
        .deployment(fx.agent, Uuid::parse_str(&second.id).unwrap())
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
    let pins: Vec<(Uuid, Option<Uuid>)> = fx.db.pool.get().await.unwrap().query("SELECT thread_id,deployment_id FROM chat_participants WHERE agent_id=$1 ORDER BY thread_id", &[&fx.agent]).await.map(|rows| rows.into_iter().map(|row| (row.get::<_, Uuid>(0),row.get::<_, Option<Uuid>>(1))).collect::<Vec<_>>())
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

#[tokio::test]
async fn lambda_deployments_serve_gateway_agents_and_weighted_routing_guards_retirement() {
    let fx = Fx::new().await;
    let initial = Uuid::parse_str(&fx.serving().await.unwrap()).unwrap();
    // Under latest routing a Lambda deployment serves a gateway-mode agent at once.
    let (lambda, token, created) = fx
        .deployments
        .register_deployment(
            fx.agent,
            RegisterDeployment {
                source: types::DeploymentSource::Ci,
                target: types::DeploymentTarget::Lambda,
                target_reference: Some(
                    "arn:aws:lambda:eu-central-1:123456789012:function:support".into(),
                ),
                repository: None,
                commit_sha: None,
                external_id: Some("lambda-1".into()),
                label: None,
                commit_message: None,
                branch: None,
                commit_author: None,
                declarations: Default::default(),
            },
        )
        .await
        .unwrap();
    assert!(created && token.is_some() && lambda.serving);
    assert_eq!(fx.serving().await, Some(lambda.id.clone()));
    // Under weighted routing it can be promoted explicitly, and the serving deployment
    // cannot be retired until another one is promoted.
    fx.deployments
        .set(
            fx.agent,
            types::SidecarFailureMode::Reassign,
            types::DeploymentRouting::Weighted,
        )
        .await
        .unwrap();
    fx.deployments.promote(fx.agent, initial).await.unwrap();
    assert_eq!(fx.serving().await, Some(initial.to_string()));
    let lambda_id = Uuid::parse_str(&lambda.id).unwrap();
    fx.deployments.promote(fx.agent, lambda_id).await.unwrap();
    assert_eq!(fx.serving().await, Some(lambda.id.clone()));
    assert!(fx.deployments.retire(fx.agent, lambda_id).await.is_err());
    fx.deployments.promote(fx.agent, initial).await.unwrap();
    fx.deployments.retire(fx.agent, lambda_id).await.unwrap();
    assert_eq!(fx.serving().await, Some(initial.to_string()));
    fx.db.close().await;
}

#[tokio::test]
async fn weighted_routing_validates_atomically_and_preserves_conversation_pins() {
    let fx = Fx::new().await;
    let first = Uuid::parse_str(&fx.serving().await.unwrap()).unwrap();
    let (second, _, _) = fx
        .deployments
        .register_deployment(fx.agent, fx.gateway("weighted-next"))
        .await
        .unwrap();
    fx.connect(Uuid::parse_str(&second.id).unwrap()).await;
    let second = fx
        .deployments
        .deployment(fx.agent, Uuid::parse_str(&second.id).unwrap())
        .await
        .unwrap();
    let second = Uuid::parse_str(&second.id).unwrap();
    fx.deployments
        .set(
            fx.agent,
            types::SidecarFailureMode::Reassign,
            types::DeploymentRouting::Weighted,
        )
        .await
        .unwrap();
    for invalid in [
        vec![(first, 30), (second, 40)],
        vec![(first, 101), (second, 0)],
        vec![(first, 50), (first, 50)],
        vec![(Uuid::new_v4(), 100)],
        vec![(first, 100)],
    ] {
        assert!(fx.deployments.set_weights(fx.agent, invalid).await.is_err());
        assert_eq!(
            fx.deployments
                .deployment(fx.agent, second)
                .await
                .unwrap()
                .traffic_weight,
            100
        );
    }
    fx.deployments
        .set_weights(fx.agent, vec![(first, 100), (second, 0)])
        .await
        .unwrap();
    let thread = fx
        .chat
        .create_thread(CreateThread {
            title: "Weighted pin".into(),
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
            thread_id: thread.id.clone(),
            agent_id: fx.agent.to_string(),
            objective: "Pin first".into(),
            idempotency_key: "weighted-first".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let thread_id = Uuid::parse_str(&thread.id).unwrap();
    assert_eq!(
        fx.deployments.pinned(fx.agent, thread_id).await.unwrap(),
        Some(first)
    );
    fx.deployments
        .set_weights(fx.agent, vec![(first, 0), (second, 100)])
        .await
        .unwrap();
    for _ in 0..20 {
        assert_eq!(
            tilde::deployment::db::choose_opt(&fx.db.pool.get().await.unwrap(), fx.agent)
                .await
                .unwrap()
                .unwrap()
                .id,
            second
        );
    }
    assert_eq!(
        fx.deployments.pinned(fx.agent, thread_id).await.unwrap(),
        Some(first)
    );
    assert!(fx.deployments.retire(fx.agent, second).await.is_err());
    fx.deployments
        .set_weights(fx.agent, vec![(first, 50), (second, 50)])
        .await
        .unwrap();
    let mut selected = std::collections::BTreeSet::new();
    for _ in 0..100 {
        selected.insert(
            tilde::deployment::db::choose_opt(&fx.db.pool.get().await.unwrap(), fx.agent)
                .await
                .unwrap()
                .unwrap()
                .id,
        );
    }
    assert_eq!(selected, std::collections::BTreeSet::from([first, second]));
    // New registrations start at zero and cannot silently steal assigned traffic.
    let (third, _, _) = fx
        .deployments
        .register_deployment(fx.agent, fx.gateway("weighted-third"))
        .await
        .unwrap();
    fx.connect(Uuid::parse_str(&third.id).unwrap()).await;
    let third = fx
        .deployments
        .deployment(fx.agent, Uuid::parse_str(&third.id).unwrap())
        .await
        .unwrap();
    assert_eq!(third.traffic_weight, 0);
    assert!(
        fx.deployments
            .set_weights(fx.agent, vec![(first, 50), (second, 50)])
            .await
            .is_err()
    );
    let third_id = Uuid::parse_str(&third.id).unwrap();
    fx.deployments
        .set_weights(fx.agent, vec![(first, 0), (second, 100), (third_id, 0)])
        .await
        .unwrap();
    fx.deployments.retire(fx.agent, first).await.unwrap();
    fx.deployments
        .set(
            fx.agent,
            types::SidecarFailureMode::Reassign,
            types::DeploymentRouting::Latest,
        )
        .await
        .unwrap();
    assert_eq!(fx.serving().await, Some(third.id));
    assert!(
        fx.deployments
            .set_weights(fx.agent, vec![(second, 100), (third_id, 0)])
            .await
            .is_err()
    );
    fx.db.close().await;
}

#[tokio::test]
async fn mixed_execution_types_register_without_urls_and_share_routing() {
    let fx = Fx::new().await;
    let gateway = Uuid::parse_str(&fx.serving().await.unwrap()).unwrap();
    let mut spec = fx.gateway("dial-in-sidecar");
    spec.target = types::DeploymentTarget::Sidecar;
    let (sidecar, _, _) = fx
        .deployments
        .register_deployment(fx.agent, spec)
        .await
        .unwrap();
    fx.connect(Uuid::parse_str(&sidecar.id).unwrap()).await;
    let sidecar = fx
        .deployments
        .deployment(fx.agent, Uuid::parse_str(&sidecar.id).unwrap())
        .await
        .unwrap();
    assert!(sidecar.serving);
    let sidecar = Uuid::parse_str(&sidecar.id).unwrap();
    let mut spec = fx.gateway("lambda");
    spec.target = types::DeploymentTarget::Lambda;
    spec.target_reference = Some("arn:aws:lambda:eu-central-1:123456789012:function:next".into());
    let (lambda, _, _) = fx
        .deployments
        .register_deployment(fx.agent, spec)
        .await
        .unwrap();
    assert!(
        lambda.serving,
        "Latest crosses execution types without an agent mode switch"
    );
    let lambda = Uuid::parse_str(&lambda.id).unwrap();
    let spec = fx.gateway("dial-in-gateway");
    let (connected, token, _) = fx
        .deployments
        .register_deployment(fx.agent, spec)
        .await
        .unwrap();
    fx.connect(Uuid::parse_str(&connected.id).unwrap()).await;
    let connected = fx
        .deployments
        .deployment(fx.agent, Uuid::parse_str(&connected.id).unwrap())
        .await
        .unwrap();
    assert!(connected.serving && token.is_some());
    let connected = Uuid::parse_str(&connected.id).unwrap();
    fx.deployments
        .set(
            fx.agent,
            types::SidecarFailureMode::Reassign,
            types::DeploymentRouting::Weighted,
        )
        .await
        .unwrap();
    fx.deployments
        .set_weights(
            fx.agent,
            vec![(gateway, 0), (sidecar, 30), (lambda, 30), (connected, 40)],
        )
        .await
        .unwrap();
    let mut selected = std::collections::BTreeSet::new();
    for _ in 0..100 {
        selected.insert(
            tilde::deployment::db::choose_opt(&fx.db.pool.get().await.unwrap(), fx.agent)
                .await
                .unwrap()
                .unwrap()
                .id,
        );
    }
    assert_eq!(
        selected,
        std::collections::BTreeSet::from([sidecar, lambda, connected])
    );
    let mut missing = fx.gateway("missing-type");
    missing.target = types::DeploymentTarget::Unspecified;
    assert!(
        fx.deployments
            .register_deployment(fx.agent, missing)
            .await
            .is_err()
    );
    let mut invalid = fx.gateway("missing-function");
    invalid.target = types::DeploymentTarget::Lambda;
    assert!(
        fx.deployments
            .register_deployment(fx.agent, invalid)
            .await
            .is_err()
    );
    fx.db.close().await;
}

#[tokio::test]
async fn migration_preserves_old_conversations_before_latest_crosses_execution_types() {
    let db = common::Database::unmigrated().await;
    tilde::database::migrate_through(&db.pool, 20260916020000)
        .await
        .unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .batch_execute(include_str!(
            "../../../queries/_tests/deployment_type_upgrade.sql"
        ))
        .await
        .unwrap();
    tilde::database::migrate(&db.pool).await.unwrap();
    let id = |n: u8| Uuid::parse_str(&format!("00000000-0000-4000-8000-{n:012}")).unwrap();
    let client = db.pool.get().await.unwrap();
    let pin = tilde::deployment::db::deployment_pin_opt(&client, id(74), id(71))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pin.deployment_id, Some(id(72)));
    let settings = tilde::deployment::db::get_opt(&client, id(71))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(settings.serving_deployment_id, Some(id(73)));
    let next = tilde::deployment::db::deployment_get_opt(&client, id(73), id(71))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.target, "lambda");
    assert_eq!(next.traffic_weight, 100);
    assert_eq!(
        next.target_reference.as_deref(),
        Some("arn:aws:lambda:eu-central-1:123456789012:function:next")
    );
    assert_eq!(
        tilde::deployment::db::deployment_get_opt(&client, id(72), id(71))
            .await
            .unwrap()
            .unwrap()
            .traffic_weight,
        0
    );
    drop(client);
    db.close().await;
}

#[tokio::test]
async fn routing_rejects_disconnected_and_partial_chains_and_ignores_old_stream_closes() {
    use tilde::proto::tilde::agent_event_ingress::v1 as wire;
    let fx = Fx::new().await;
    let first = Uuid::parse_str(&fx.serving().await.unwrap()).unwrap();
    let (second, _, _) = fx
        .deployments
        .register_deployment(fx.agent, fx.gateway("not-connected"))
        .await
        .unwrap();
    let second_id = Uuid::parse_str(&second.id).unwrap();
    assert!(!second.routable && !second.serving);
    assert_eq!(
        fx.serving().await,
        Some(first.to_string()),
        "Latest keeps a live deployment while the newer one connects"
    );
    assert!(fx.deployments.promote(fx.agent, second_id).await.is_err());
    fx.deployments
        .set(
            fx.agent,
            types::SidecarFailureMode::Reassign,
            types::DeploymentRouting::Weighted,
        )
        .await
        .unwrap();
    assert!(
        fx.deployments
            .set_weights(fx.agent, vec![(first, 50), (second_id, 50)])
            .await
            .is_err()
    );
    let (instance, old_connection) = fx.connect(second_id).await;
    fx.deployments
        .set_weights(fx.agent, vec![(first, 50), (second_id, 50)])
        .await
        .unwrap();
    // A closed stream cannot be revived by heartbeat requests.
    tilde::deployment::db::watch_close_execute(
        &fx.db.pool.get().await.unwrap(),
        fx.agent,
        instance,
        old_connection,
    )
    .await
    .unwrap();
    let ready = wire::Heartbeat {
        ready: true,
        agent_ready: true,
        agent_connected: true,
        ..Default::default()
    };
    fx.deployments
        .heartbeat(fx.agent, instance, &ready)
        .await
        .unwrap();
    assert!(
        !fx.deployments
            .deployment(fx.agent, second_id)
            .await
            .unwrap()
            .routable
    );
    for _ in 0..20 {
        assert_eq!(
            tilde::deployment::db::choose_opt(&fx.db.pool.get().await.unwrap(), fx.agent)
                .await
                .unwrap()
                .unwrap()
                .id,
            first
        );
    }
    let connection = Uuid::new_v4();
    tilde::deployment::db::watch_open_execute(
        &fx.db.pool.get().await.unwrap(),
        fx.agent,
        instance,
        connection,
    )
    .await
    .unwrap();
    fx.deployments
        .heartbeat(fx.agent, instance, &ready)
        .await
        .unwrap();
    tilde::deployment::db::watch_close_execute(
        &fx.db.pool.get().await.unwrap(),
        fx.agent,
        instance,
        old_connection,
    )
    .await
    .unwrap();
    assert!(
        fx.deployments
            .deployment(fx.agent, second_id)
            .await
            .unwrap()
            .routable
    );
    // Sidecar -> gateway alone is insufficient: the local agent must also be connected.
    let mut spec = fx.gateway("partial-sidecar");
    spec.target = types::DeploymentTarget::Sidecar;
    let (sidecar, _, _) = fx
        .deployments
        .register_deployment(fx.agent, spec)
        .await
        .unwrap();
    let sidecar_id = Uuid::parse_str(&sidecar.id).unwrap();
    let (instance, _) = fx.connect(sidecar_id).await;
    fx.deployments
        .heartbeat(
            fx.agent,
            instance,
            &wire::Heartbeat {
                agent_connected: false,
                ..ready.clone()
            },
        )
        .await
        .unwrap();
    assert!(
        !fx.deployments
            .deployment(fx.agent, sidecar_id)
            .await
            .unwrap()
            .routable
    );
    assert!(
        fx.deployments
            .set_weights(
                fx.agent,
                vec![(first, 0), (second_id, 0), (sidecar_id, 100)]
            )
            .await
            .is_err()
    );
    fx.deployments
        .heartbeat(fx.agent, instance, &ready)
        .await
        .unwrap();
    fx.deployments
        .set_weights(
            fx.agent,
            vec![(first, 0), (second_id, 0), (sidecar_id, 100)],
        )
        .await
        .unwrap();
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            include_str!("../../../queries/_tests/expire_deployment_connection.sql"),
            &[&instance],
        )
        .await
        .unwrap();
    assert!(
        !fx.deployments
            .deployment(fx.agent, sidecar_id)
            .await
            .unwrap()
            .routable
    );
    assert!(
        tilde::deployment::db::choose_opt(&fx.db.pool.get().await.unwrap(), fx.agent)
            .await
            .unwrap()
            .is_none()
    );
    fx.db.close().await;
}

#[tokio::test]
async fn registry_health_samples_the_routing_policy_and_persists_disconnects() {
    use tilde::agent::health::{AgentHealth, metrics};
    use types::AgentHealthStatus;
    let fx = Fx::new().await;
    let health = AgentHealth::new(fx.db.pool.clone());
    let initial = Uuid::parse_str(&fx.serving().await.unwrap()).unwrap();
    health.poll_once().await.unwrap();
    assert_eq!(
        metrics(&fx.db.pool, &[fx.agent], chrono::Utc::now())
            .await
            .unwrap()[&fx.agent]
            .health,
        AgentHealthStatus::Healthy
    );
    let (next, _, _) = fx
        .deployments
        .register_deployment(fx.agent, fx.gateway("health-next"))
        .await
        .unwrap();
    let next = Uuid::parse_str(&next.id).unwrap();
    // Latest falls back to the connected deployment while a new registration is offline.
    health.poll_once().await.unwrap();
    assert_eq!(
        metrics(&fx.db.pool, &[fx.agent], chrono::Utc::now())
            .await
            .unwrap()[&fx.agent]
            .health,
        AgentHealthStatus::Healthy
    );
    let (instance, connection) = fx.connect(next).await;
    fx.deployments
        .set(
            fx.agent,
            types::SidecarFailureMode::Reassign,
            types::DeploymentRouting::Weighted,
        )
        .await
        .unwrap();
    fx.deployments
        .set_weights(fx.agent, vec![(initial, 50), (next, 50)])
        .await
        .unwrap();
    health.poll_once().await.unwrap();
    assert_eq!(
        metrics(&fx.db.pool, &[fx.agent], chrono::Utc::now())
            .await
            .unwrap()[&fx.agent]
            .health,
        AgentHealthStatus::Healthy
    );
    tilde::deployment::db::watch_close_execute(
        &fx.db.pool.get().await.unwrap(),
        fx.agent,
        instance,
        connection,
    )
    .await
    .unwrap();
    health.poll_once().await.unwrap();
    let view = metrics(&fx.db.pool, &[fx.agent], chrono::Utc::now())
        .await
        .unwrap();
    assert_eq!(
        view[&fx.agent].health,
        AgentHealthStatus::Degraded,
        "remaining positive-weight deployment can serve all traffic"
    );
    assert_eq!(
        view[&fx.agent]
            .health_history
            .iter()
            .map(|h| h.failed_checks)
            .sum::<u32>(),
        0
    );
    assert_eq!(
        view[&fx.agent]
            .health_history
            .iter()
            .map(|h| h.degraded_checks)
            .sum::<u32>(),
        1
    );
    for _ in 0..20 {
        assert_eq!(
            tilde::deployment::db::choose_opt(&fx.db.pool.get().await.unwrap(), fx.agent)
                .await
                .unwrap()
                .unwrap()
                .id,
            initial,
            "all traffic falls back to the remaining healthy deployment"
        );
    }
    fx.deployments
        .set_weights(fx.agent, vec![(initial, 100), (next, 0)])
        .await
        .unwrap();
    health.poll_once().await.unwrap();
    let view = metrics(&fx.db.pool, &[fx.agent], chrono::Utc::now())
        .await
        .unwrap();
    assert_eq!(
        view[&fx.agent].health,
        AgentHealthStatus::Healthy,
        "zero-weight releases do not affect serving health"
    );
    assert_eq!(
        view[&fx.agent]
            .health_history
            .iter()
            .map(|h| h.failed_checks)
            .sum::<u32>(),
        0,
        "partial capacity loss did not record an outage"
    );
    // Heartbeats are insufficient without a fresh connection chain.
    let instances = fx.deployments.instances(fx.agent).await.unwrap();
    for instance in instances {
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .execute(
                include_str!("../../../queries/_tests/expire_deployment_connection.sql"),
                &[&Uuid::parse_str(&instance.instance_id).unwrap()],
            )
            .await
            .unwrap();
    }
    health.poll_once().await.unwrap();
    assert_eq!(
        metrics(&fx.db.pool, &[fx.agent], chrono::Utc::now())
            .await
            .unwrap()[&fx.agent]
            .health,
        AgentHealthStatus::Unhealthy
    );
    let view = metrics(&fx.db.pool, &[fx.agent], chrono::Utc::now())
        .await
        .unwrap();
    assert_eq!(
        view[&fx.agent]
            .health_history
            .iter()
            .map(|h| h.failed_checks)
            .sum::<u32>(),
        1
    );
    fx.connect(initial).await;
    health.poll_once().await.unwrap();
    let recovered = metrics(&fx.db.pool, &[fx.agent], chrono::Utc::now())
        .await
        .unwrap();
    assert_eq!(recovered[&fx.agent].health, AgentHealthStatus::Healthy);
    assert_eq!(
        recovered[&fx.agent]
            .health_history
            .iter()
            .map(|h| h.failed_checks)
            .sum::<u32>(),
        1,
        "recovery preserves the recorded outage"
    );
    fx.db.close().await;
}
