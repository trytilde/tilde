use axum::{Router, routing::get};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::{Parser, Subcommand};
use envconfig::Envconfig;
use rand::RngCore;
use secrecy::ExposeSecret;
use std::{net::SocketAddr, sync::Arc};
use tilde::{
    agent::Agents,
    config::{Backend, Config},
    database,
    encryption::Encryption,
    network,
};
use zeroize::Zeroizing;

#[cfg(feature = "embedded-web")]
mod web;

#[derive(Parser)]
#[command(
    version,
    about = "A single-workspace Tilde engine with ConnectRPC and encrypted Postgres secrets"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(long)]
    management_listen: Option<SocketAddr>,
    #[arg(long)]
    agent_runtime_listen: Option<SocketAddr>,
    #[arg(long)]
    event_ingress_listen: Option<SocketAddr>,
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
    if let Some(listen) = args.management_listen {
        config.management_listen = listen;
    }
    if let Some(listen) = args.agent_runtime_listen {
        config.agent_runtime_listen = listen;
    }
    if let Some(listen) = args.event_ingress_listen {
        config.event_ingress_listen = listen;
    }
    network::Boundary::new(
        config.event_ingress_listen,
        config.allow_network || args.allow_network,
        vec![],
    )?;
    network::Boundary::new(
        config.agent_runtime_listen,
        config.allow_network || args.allow_network,
        vec![],
    )?;
    if args.allow_network {
        config.allow_network = true;
    }
    if !args.web_origin.is_empty() {
        config.web_origins.0 = args.web_origin;
    }
    if let Some(backend) = args.encryption_backend {
        config.encryption_backend = backend;
    }
    if config.management_listener_enabled() {
        network::Boundary::new(
            config.management_listen,
            config.allow_network,
            config.web_origins.0.clone(),
        )?;
    }
    let avatars = config.avatar_store()?;
    let protection = config.key_protection()?;
    let trace_destination = tilde::telemetry::Destination::new(
        config.tracing_export_endpoint.take(),
        config.tracing_export_headers.take(),
    )?;
    if matches!(args.command, Some(Command::CheckConfig)) {
        return Ok(());
    }
    let management_listener_enabled = config.management_listener_enabled();
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(config.log_filter)
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
    let mut agents = Agents::new(pool.clone(), encryption.clone());
    if let Some(avatars) = avatars {
        agents = agents.with_avatar_store(avatars);
    }
    let listener = if management_listener_enabled {
        Some(tokio::net::TcpListener::bind(config.management_listen).await?)
    } else {
        None
    };
    let address = listener
        .as_ref()
        .map(|listener| listener.local_addr())
        .transpose()?
        .unwrap_or(config.management_listen);
    let ingress_listener = tokio::net::TcpListener::bind(config.event_ingress_listen).await?;
    let ingress_address = ingress_listener.local_addr()?;
    let ingress_url = config
        .event_ingress_public_url
        .unwrap_or_else(|| format!("http://{ingress_address}"));
    let ingress_boundary = network::Boundary::new(
        ingress_address,
        config.allow_network,
        vec![ingress_url.clone()],
    )?;
    let agent_runtime_listener = tokio::net::TcpListener::bind(config.agent_runtime_listen).await?;
    let agent_address = agent_runtime_listener.local_addr()?;
    let agent_callback_url = config
        .agent_runtime_public_url
        .unwrap_or_else(|| format!("http://{agent_address}"));
    let agent_origin = url::Url::parse(&agent_callback_url)?
        .origin()
        .ascii_serialization();
    let agent_boundary =
        network::Boundary::new(agent_address, config.allow_network, vec![agent_origin])?;
    let callback_url = config
        .management_public_url
        .unwrap_or_else(|| format!("http://{address}"));
    let callback = url::Url::parse(&callback_url)?;
    if !matches!(callback.scheme(), "http" | "https")
        || callback.host_str().is_none()
        || !callback.username().is_empty()
        || callback.password().is_some()
        || callback.query().is_some()
        || callback.fragment().is_some()
    {
        return Err(
            "ENGINE_MANAGEMENT_PUBLIC_URL must be an HTTP(S) origin without credentials, query or fragment"
                .into(),
        );
    }
    config
        .web_origins
        .0
        .push(callback.origin().ascii_serialization());
    let connection_setup_url = config
        .connection_setup_public_url
        .clone()
        .unwrap_or_else(|| callback_url.clone());
    config.web_origins.0.push(
        url::Url::parse(&connection_setup_url)?
            .origin()
            .ascii_serialization(),
    );
    let boundary = listener
        .as_ref()
        .map(|_| network::Boundary::new(address, config.allow_network, config.web_origins.0))
        .transpose()?;
    let connections = tilde::connections::service::Connections::new(
        pool.clone(),
        encryption.clone(),
        connection_setup_url,
        ingress_url,
    )?;
    connections.seed().await?;
    connections.recover().await?;
    let health = tilde::agent::health::AgentHealth::new(pool.clone(), encryption.clone());
    health.cleanup().await?;
    let chat = tilde::chat::Chat::new(pool.clone(), encryption.clone(), agent_callback_url)
        .with_connections(connections.clone());
    let telemetry =
        tilde::telemetry::Runtime::start(pool.clone(), chat.tokens.clone(), trace_destination);
    opentelemetry::global::set_tracer_provider(telemetry.provider.clone());
    let agent_router = tilde::iam::listeners::agent_runtime_router(
        agents.clone(),
        chat.clone(),
        &telemetry.tracing,
    )
    .layer(axum::middleware::from_fn(
        tilde::telemetry::context::request,
    ))
    .layer(axum::middleware::from_fn_with_state(
        agent_boundary,
        network::guard,
    ));
    let ingress_router =
        tilde::iam::listeners::event_ingress_router(chat.clone(), connections.clone())
            .layer(axum::middleware::from_fn(
                tilde::telemetry::context::request,
            ))
            .layer(axum::middleware::from_fn_with_state(
                ingress_boundary,
                network::guard,
            ));
    let (chat_shutdown, chat_shutdown_rx) = tokio::sync::watch::channel(false);
    let mut connection_worker = tokio::spawn(connections.clone().worker(chat_shutdown_rx.clone()));
    let mut health_worker = tokio::spawn(health.run(chat_shutdown_rx.clone()));
    let chat_worker = tokio::spawn(chat.clone().worker(chat_shutdown_rx));
    let readiness_pool = pool.clone();
    let router = if config.management_enabled {
        let oidc = tilde::iam::oidc::Oidc::new(
            pool.clone(),
            encryption,
            config.oidc_issuer.expect("validated OIDC issuer"),
            config.oidc_client_id.expect("validated OIDC client ID"),
            config.oidc_client_secret.expect("validated OIDC secret").0,
            callback_url,
            config.oidc_allow_http,
        )?;
        tilde::iam::listeners::management_router(agents, chat.clone(), connections.clone(), oidc)
    } else {
        Router::new()
    };
    let router = if config.management_enabled {
        router.merge(tilde::connections::assets::router(
            config.connection_ui_dev_url.clone(),
            Some(connections),
        )?)
    } else {
        router
    };
    let router = router.route("/healthz", get(|| async { "ok" })).route(
        "/readyz",
        get(move || {
            let pool = readiness_pool.clone();
            async move {
                if sqlx::query_file!("../../queries/system/ready.sql")
                    .fetch_one(&pool)
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
    #[cfg(feature = "embedded-web")]
    let router = if config.web_enabled {
        router.fallback(web::serve)
    } else {
        router
    };
    let router = if let Some(boundary) = boundary {
        router.layer(axum::middleware::from_fn_with_state(
            boundary,
            network::guard,
        ))
    } else {
        router
    };
    let router = router.layer(axum::middleware::from_fn(
        tilde::telemetry::context::request,
    ));
    tracing::info!(event_ingress_address=%ingress_address, management_address=%address, agent_runtime_address=%agent_address, management_enabled=config.management_enabled, web_enabled=config.web_enabled && cfg!(feature="embedded-web"), management_listener=listener.is_some(), "tilde listening");
    let (server_shutdown, server_shutdown_rx) = tokio::sync::watch::channel(false);
    let user_shutdown = server_shutdown_rx.clone();
    let server = async move {
        match listener {
            Some(listener) => {
                axum::serve(listener, router)
                    .with_graceful_shutdown(wait_shutdown(user_shutdown))
                    .await
            }
            None => {
                wait_shutdown(user_shutdown).await;
                Ok(())
            }
        }
    };
    let ingress_server = axum::serve(ingress_listener, ingress_router)
        .with_graceful_shutdown(wait_shutdown(server_shutdown_rx.clone()));
    let agent_server = axum::serve(agent_runtime_listener, agent_router)
        .with_graceful_shutdown(wait_shutdown(server_shutdown_rx));
    let mut servers =
        tokio::spawn(
            async move { tokio::try_join!(server, agent_server, ingress_server).map(|_| ()) },
        );
    let result = tokio::select! {
        _ = shutdown() => Ok(()),
        result = &mut servers => result.unwrap_or_else(|e|Err(std::io::Error::other(e))),
        _ = &mut connection_worker => Err(std::io::Error::other("Connection setup worker stopped unexpectedly")),
        _ = &mut health_worker => Err(std::io::Error::other("Agent health worker stopped unexpectedly")),
    };
    let _ = server_shutdown.send(true);
    let _ = chat_shutdown.send(true);
    if !servers.is_finished()
        && tokio::time::timeout(std::time::Duration::from_secs(5), &mut servers)
            .await
            .is_err()
    {
        servers.abort();
    }
    if !health_worker.is_finished() {
        let _ = health_worker.await;
    }
    let _ = chat_worker.await;
    if !connection_worker.is_finished() {
        let _ = connection_worker.await;
    }
    telemetry.shutdown().await;
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
