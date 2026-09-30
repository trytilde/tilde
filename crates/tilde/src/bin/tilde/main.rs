use axum::{Router, routing::get, serve::ListenerExt};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::{Parser, Subcommand};
use envconfig::Envconfig;
use rand::RngCore;
use secrecy::ExposeSecret;
use std::{net::SocketAddr, sync::Arc};
use tilde::{
    agent::Agents,
    config::{Backend, Config, Serve},
    database,
    encryption::Encryption,
    network,
};
use zeroize::Zeroizing;

#[cfg(feature = "embedded-web")]
mod web;

#[cfg(debug_assertions)]
mod dev_agent;
#[cfg(debug_assertions)]
mod dev_skills;

#[derive(Parser)]
#[command(
    version,
    about = "A single-workspace Tilde engine with ConnectRPC and encrypted Postgres secrets"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    /// Address for the single listener serving every enabled route group.
    #[arg(long)]
    listen: Option<SocketAddr>,
    /// Comma-separated route groups: management, ingress, runtime, sidecar.
    #[arg(long)]
    serve: Option<Serve>,
    #[arg(long)]
    allow_network: bool,
    #[arg(long, value_delimiter = ',')]
    web_origin: Vec<String>,
    #[arg(long)]
    encryption_backend: Option<Backend>,
}
#[derive(Subcommand)]
enum Command {
    /// Print a fresh base64 seed for ENGINE_ENCRYPTION_KEY.
    GenerateKey,
    /// Validate typed configuration without connecting to Postgres or AWS.
    CheckConfig,
    /// Register and host the local SDK example (development builds only).
    #[cfg(debug_assertions)]
    DevAgent,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if matches!(args.command, Some(Command::GenerateKey)) {
        let mut key = Zeroizing::new([0u8; 32]);
        rand::rngs::OsRng.fill_bytes(&mut *key);
        let encoded = Zeroizing::new(STANDARD.encode(key.as_slice()));
        println!("{}", encoded.as_str());
        return Ok(());
    }
    let mut config = Config::init_from_env()?;
    if let Some(listen) = args.listen {
        config.listen = listen;
    }
    if let Some(serve) = args.serve {
        config.serve = serve;
    }
    if args.allow_network {
        config.allow_network = true;
    }
    if !args.web_origin.is_empty() {
        config.web_origins.0 = args.web_origin;
    }
    if let Some(backend) = args.encryption_backend {
        config.encryption_backend = backend;
    }
    let protection = config.key_protection()?;
    let buckets = config.buckets()?;
    let avatars = buckets.avatars;
    let trace_forwarding = tilde::telemetry::tracing::Destination::new(
        config
            .traces_otlp_endpoint
            .clone()
            .filter(|v| !v.trim().is_empty()),
        config.traces_otlp_headers.clone(),
    )?;
    let _ = tilde::telemetry::clickhouse::Store::from_config(&config)?;
    if matches!(args.command, Some(Command::CheckConfig)) {
        return Ok(());
    }
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(config.log_filter.clone())
        .init();
    let pool = database::connect(config.database_url.0.expose_secret()).await?;
    let encryption = Arc::new(
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            Encryption::initialize(&pool, protection),
        )
        .await
        .map_err(|_| "Encryption key initialization timed out")??,
    );
    #[cfg(debug_assertions)]
    if matches!(args.command, Some(Command::DevAgent)) {
        return dev_agent::run(pool, encryption, buckets.skills.clone()).await;
    }
    // Telemetry intake and delivery have their own connections, so a full or slow telemetry
    // queue can never take the ones management requests and agent heartbeats need.
    let telemetry_pool = database::pool(config.database_url.0.expose_secret(), 6)?;
    let logs =
        tilde::telemetry::logs::Runtime::start(telemetry_pool.clone(), &config, buckets.logs)?;
    let metrics = tilde::telemetry::metrics::Runtime::start(
        telemetry_pool.clone(),
        &config,
        buckets.metrics,
    )?;
    let trace_store = tilde::telemetry::tracing::store::Store(
        tilde::telemetry::clickhouse::Store::from_config(&config)?,
    );
    let mut agents = Agents::new(pool.clone(), encryption.clone());
    if let Some(avatars) = avatars {
        agents = agents.with_avatar_store(avatars);
    }
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    let address = listener.local_addr()?;
    let serve = config.serve;
    let web_served = config.web_served();
    let public_url = config
        .public_url
        .clone()
        .unwrap_or_else(|| format!("http://{address}"));
    let public = url::Url::parse(&public_url)?;
    if !matches!(public.scheme(), "http" | "https")
        || public.host_str().is_none()
        || !public.username().is_empty()
        || public.password().is_some()
        || public.query().is_some()
        || public.fragment().is_some()
    {
        return Err(
            "ENGINE_PUBLIC_URL must be an HTTP(S) origin without credentials, query or fragment"
                .into(),
        );
    }
    let ingress_url = config
        .ingress_public_url
        .clone()
        .unwrap_or_else(|| public_url.clone());
    let runtime_url = config
        .runtime_public_url
        .clone()
        .unwrap_or_else(|| public_url.clone());
    let connection_setup_url = config
        .connection_setup_public_url
        .clone()
        .unwrap_or_else(|| public_url.clone());
    let mut origins = config.web_origins.0.clone();
    for value in [
        &public_url,
        &ingress_url,
        &runtime_url,
        &connection_setup_url,
    ] {
        origins.push(url::Url::parse(value)?.origin().ascii_serialization());
    }
    let boundary = network::Boundary::new(address, config.allow_network, origins)?;
    let connections = tilde::connections::service::Connections::new(
        pool.clone(),
        encryption.clone(),
        connection_setup_url,
        ingress_url,
    )?;
    connections.seed().await?;
    connections.backfill_tilde().await?;
    connections.recover().await?;
    connections.restore_agent_identities().await?;
    let trace_delivery = tilde::telemetry::tracing::Delivery::start(
        telemetry_pool.clone(),
        tilde::telemetry::tracing::Sinks {
            objects: buckets.traces,
            media: buckets.media.clone(),
            store: trace_store.clone(),
            external: trace_forwarding,
        },
    )?;
    let sweep_cancel = tokio_util::sync::CancellationToken::new();
    let sweeper = tokio::spawn(tilde::telemetry::spool::sweep_worker(
        telemetry_pool.clone(),
        sweep_cancel.clone(),
    ));
    let deployments = tilde::deployment::Deployments::new(
        pool.clone(),
        encryption.clone(),
        agents.clone(),
        connections.clone(),
    )
    .with_logs(logs.delivery.clone())
    .with_metrics(metrics.delivery.clone())
    .with_telemetry(trace_delivery.queue.clone());
    let health = tilde::agent::health::AgentHealth::new(pool.clone());
    health.cleanup().await?;
    let inference = tilde::inference::gateway::Loader::new(pool.clone(), connections.clone());
    inference.load().await?;
    tilde::inference::prices::reconcile(&pool).await?;
    let skills = tilde::skills::Skills::new(pool.clone())
        .with_store(buckets.skills.clone())
        .with_github(tilde::skills::github::GitHub::new(
            &config.github_api_url,
            &config.github_raw_url,
            config
                .github_token
                .as_ref()
                .map(|t| secrecy::SecretString::from(t.0.expose_secret().to_owned())),
        )?);
    skills.reconcile().await?;
    let deployments = deployments.with_skills(skills.clone());
    let chat = tilde::chat::Chat::new(pool.clone(), encryption.clone(), runtime_url)
        .with_connections(connections.clone())
        .with_deployments(deployments.clone())
        .with_inference(tilde::inference::Gateway {
            upstreams: inference.upstreams.clone(),
            audit: tilde::inference::audit::Audit::start(tilde::inference::audit::Sink::Postgres(
                pool.clone(),
            )),
        })
        .with_objects(agents.object_store().cloned())
        .with_skills(Some(skills.clone()));
    let trace_reader =
        tilde::telemetry::tracing::viewer::Reader::new(pool.clone(), trace_store, buckets.media);
    let telemetry = tilde::telemetry::tracing::Runtime::start(
        telemetry_pool.clone(),
        chat.tokens.clone(),
        trace_delivery,
    );
    opentelemetry::global::set_tracer_provider(telemetry.provider.clone());
    let (chat_shutdown, chat_shutdown_rx) = tokio::sync::watch::channel(false);
    let mut relay_worker = tokio::spawn(deployments.clone().relay_worker(chat_shutdown_rx.clone()));
    let mut recovery_worker = tokio::spawn(
        deployments
            .clone()
            .recovery_worker(chat_shutdown_rx.clone()),
    );
    let mut connection_worker = tokio::spawn(connections.clone().worker(chat_shutdown_rx.clone()));
    let _inference_worker = tokio::spawn(inference.worker(chat_shutdown_rx.clone()));
    let _skills_worker = tokio::spawn(skills.worker(chat_shutdown_rx.clone()));
    let _budget_worker = tokio::spawn(tilde::inference::budgets::worker(
        pool.clone(),
        chat_shutdown_rx.clone(),
    ));
    let _price_worker = tokio::spawn(tilde::inference::prices::worker(
        pool.clone(),
        config.inference_prices_url.clone(),
        chat_shutdown_rx.clone(),
    ));
    let mut health_worker = tokio::spawn(health.run(chat_shutdown_rx.clone()));
    let _mcp_health_worker = chat
        .tools
        .clone()
        .map(|tools| tokio::spawn(tools.run_mcp_health(chat_shutdown_rx.clone())));
    let chat_worker = tokio::spawn(chat.clone().worker(chat_shutdown_rx));
    let readiness_pool = pool.clone();
    let mut router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/readyz",
            get(move || {
                let pool = readiness_pool.clone();
                async move {
                    if async { crate::database::db::ready_one(&pool.get().await?).await }
                        .await
                        .is_ok()
                    {
                        (http::StatusCode::OK, "ready")
                    } else {
                        (http::StatusCode::SERVICE_UNAVAILABLE, "unavailable")
                    }
                }
            }),
        );
    if serve.runtime {
        router = router.merge(
            tilde::iam::listeners::agent_runtime_router(
                agents.clone(),
                chat.clone(),
                &telemetry.tracing,
            )
            .merge(logs.router(&telemetry.tracing))
            .merge(metrics.router(&telemetry.tracing)),
        );
    }
    if serve.ingress {
        router = router.merge(
            tilde::iam::listeners::public_event_ingress_router(chat.clone(), connections.clone())
                .merge(tilde::deployment::public::router(
                    deployments.clone(),
                    chat.clone(),
                )),
        );
    }
    if serve.sidecar {
        router = router.merge(tilde::deployment::rpc::sidecar_router(deployments.clone()));
    }
    if serve.management {
        // Unauthenticated: operators put their own proxy in front of the management group.
        router = router
            .merge(
                tilde::iam::listeners::management_router(agents, chat.clone(), connections.clone())
                    .merge(tilde::telemetry::tracing::viewer::router(trace_reader))
                    .merge(tilde::telemetry::logs::viewer::router(logs.reader.clone())),
            )
            .merge(tilde::connections::assets::router(
                config.connection_ui_dev_url.clone(),
                Some(connections),
            )?);
    }
    #[cfg(feature = "embedded-web")]
    let router = if config.web_enabled {
        router.fallback(web::serve)
    } else {
        router
    };
    let router = router
        .layer(axum::middleware::from_fn_with_state(
            boundary,
            network::guard,
        ))
        .layer(axum::middleware::from_fn(
            tilde::telemetry::tracing::context::request,
        ));
    tracing::info!(address=%address, serve=%serve, web_enabled=web_served, "tilde listening");
    let (server_shutdown, server_shutdown_rx) = tokio::sync::watch::channel(false);
    // Small HTTP/2 writes must not wait for a delayed TCP acknowledgement.
    let listener = listener.tap_io(|stream| {
        if let Err(error) = stream.set_nodelay(true) {
            tracing::warn!(%error, "failed to set TCP_NODELAY");
        }
    });
    let mut server = tokio::spawn(
        axum::serve(listener, router)
            .with_graceful_shutdown(wait_shutdown(server_shutdown_rx))
            .into_future(),
    );
    let result = tokio::select! {
        _ = shutdown() => Ok(()),
        result = &mut server => result.unwrap_or_else(|e|Err(std::io::Error::other(e))),
        _ = &mut connection_worker => Err(std::io::Error::other("Connection setup worker stopped unexpectedly")),
        _ = &mut relay_worker => Err(std::io::Error::other("Sidecar relay worker stopped unexpectedly")),
        _ = &mut recovery_worker => Err(std::io::Error::other("Sidecar recovery worker stopped unexpectedly")),
        _ = &mut health_worker => Err(std::io::Error::other("Agent health worker stopped unexpectedly")),
    };
    let _ = server_shutdown.send(true);
    let _ = chat_shutdown.send(true);
    if !server.is_finished()
        && tokio::time::timeout(std::time::Duration::from_secs(5), &mut server)
            .await
            .is_err()
    {
        server.abort();
    }
    for worker in [
        health_worker,
        connection_worker,
        relay_worker,
        recovery_worker,
    ] {
        if !worker.is_finished() {
            let _ = worker.await;
        }
    }
    let _ = chat_worker.await;
    logs.shutdown().await;
    metrics.shutdown().await;
    telemetry.shutdown().await;
    sweep_cancel.cancel();
    // A sweep stops at the next page of expired pointers; the rest waits for the next replica.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), sweeper).await;
    if let Err(error) = tilde::chat::audit::flush(&pool).await {
        tracing::warn!(%error, "Audit queue did not drain during shutdown");
    }
    telemetry_pool.close().await;
    pool.close().await;
    result?;
    Ok(())
}

async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn wait_shutdown(mut receiver: tokio::sync::watch::Receiver<bool>) {
    let _ = receiver.wait_for(|stop| *stop).await;
}
