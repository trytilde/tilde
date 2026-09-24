//! Shared fixtures. Each test binary compiles this module and uses a subset of it.
#![allow(dead_code)]
use std::str::FromStr;
use tilde::database::{self, Pool};
use uuid::Uuid;

pub struct Database {
    pub pool: Pool,
    admin: Pool,
    schema: String,
}
impl Database {
    pub async fn new() -> Self {
        let db = Self::unmigrated().await;
        database::migrate(&db.pool).await.unwrap();
        db
    }
    pub async fn unmigrated() -> Self {
        let url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL is required; run task test for isolated Postgres");
        let admin = database::pool(&url, 2).unwrap();
        let schema = format!("test_{}", Uuid::new_v4().simple());
        admin
            .get()
            .await
            .unwrap()
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let mut config = tokio_postgres::Config::from_str(&url).unwrap();
        config.options(format!("-c search_path={schema}"));
        let pool = database::pool_from_config(config, 4).unwrap();
        Self {
            pool,
            admin,
            schema,
        }
    }
    pub async fn close(self) {
        if !self.pool.is_closed() {
            tilde::chat::audit::flush(&self.pool).await.unwrap();
        }
        self.pool.close().await;
        self.admin
            .get()
            .await
            .unwrap()
            .batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .await
            .unwrap();
        self.admin.close().await;
    }
}

pub fn seed(byte: u8) -> tilde::encryption::KeyProtection {
    use base64::Engine;
    tilde::encryption::KeyProtection::seed(secrecy::SecretString::from(
        base64::engine::general_purpose::STANDARD.encode([byte; 32]),
    ))
    .unwrap()
}

pub fn binding(id: Uuid, name: &str) -> tilde::encryption::SecretBinding<'_> {
    tilde::encryption::SecretBinding {
        resource_kind: "agent",
        resource_id: id,
        name,
    }
}

/// Register a Gateway deployment and record a ready instance for it, as the Watch and
/// heartbeat boundary would. Routing sees a live deployment; no stream exists, so a
/// wake for it has nowhere to go and fails.
pub async fn dialled_in(
    pool: &Pool,
    crypto: std::sync::Arc<tilde::encryption::Encryption>,
    agent: Uuid,
) -> Uuid {
    use tilde::proto::tilde::{agent_event_ingress::v1 as wire, types::v1 as types};
    let connections = tilde::connections::service::Connections::new(
        pool.clone(),
        crypto.clone(),
        "http://localhost:1".into(),
        "http://localhost:2".into(),
    )
    .unwrap();
    let deployments = tilde::deployment::Deployments::new(
        pool.clone(),
        crypto.clone(),
        tilde::agent::Agents::new(pool.clone(), crypto),
        connections,
    );
    let (deployment, _, _) = deployments
        .register_deployment(
            agent,
            tilde::deployment::RegisterDeployment {
                source: types::DeploymentSource::Manual,
                target: types::DeploymentTarget::Gateway,
                target_reference: None,
                repository: None,
                commit_sha: None,
                external_id: None,
                label: None,
                commit_message: None,
                branch: None,
                commit_author: None,
            },
        )
        .await
        .unwrap();
    let deployment = Uuid::parse_str(&deployment.id).unwrap();
    let instance = Uuid::new_v4();
    deployments
        .register(
            agent,
            deployment,
            &wire::WatchRequest {
                instance_id: instance.to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    tilde::deployment::db::watch_open_execute(
        &pool.get().await.unwrap(),
        agent,
        instance,
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    deployments
        .heartbeat(
            agent,
            instance,
            &wire::Heartbeat {
                ready: true,
                agent_ready: true,
                agent_connected: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    deployment
}

/// Mount the actual agent-scoped Tilde provider ingress, including its credential guard.
pub async fn tilde_provider(
    pool: &Pool,
    encryption: std::sync::Arc<tilde::encryption::Encryption>,
    chat: tilde::chat::Chat,
    agent: Uuid,
) -> (String, secrecy::SecretString, tokio::task::JoinHandle<()>) {
    let connections = tilde::connections::service::Connections::new(
        pool.clone(),
        encryption.clone(),
        "http://127.0.0.1".into(),
        "http://127.0.0.1".into(),
    )
    .unwrap();
    let deployments = tilde::deployment::Deployments::new(
        pool.clone(),
        encryption.clone(),
        tilde::agent::Agents::new(pool.clone(), encryption),
        connections,
    );
    let token = deployments
        .issue_ingress_token(agent, None, Uuid::nil())
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/agents/{agent}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            tilde::deployment::public::router(deployments, chat),
        )
        .await
        .unwrap();
    });
    (base, token, server)
}

/// These repository/control tests drive claims themselves, without a host Watch.
/// Pin the registered Gateway deployment explicitly instead of pretending that
/// an unconnected deployment is eligible for automatic traffic selection.
pub async fn pin_gateway(pool: &Pool, agent: Uuid, thread: Uuid) {
    let updated=pool.get().await.unwrap().execute(
        "UPDATE chat_participants SET deployment_id=(SELECT id FROM agent_deployments WHERE agent_id=$1 AND target='gateway' AND status='registered' ORDER BY created_at,id LIMIT 1) WHERE agent_id=$1 AND thread_id=$2",
        &[&agent,&thread],
    ).await.unwrap();
    assert_eq!(updated, 1);
}

/// Mount a management service without the guard: every call runs with engine authority.
/// For tests of a domain behind the router, not of authorization.
pub fn unguarded(router: axum::Router, pool: &Pool) -> axum::Router {
    let authz = tilde::iam::authz::Authz::new(pool.clone());
    router.layer(axum::middleware::from_fn(
        move |mut request: axum::extract::Request, next: axum::middleware::Next| {
            let authz = authz.clone();
            async move {
                request
                    .extensions_mut()
                    .insert(tilde::iam::authz::Access::system());
                request.extensions_mut().insert(authz);
                next.run(request).await
            }
        },
    ))
}
