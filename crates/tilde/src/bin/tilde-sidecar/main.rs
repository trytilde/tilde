use clap::Parser;
#[derive(Parser)]
#[command(
    name = "tilde-sidecar",
    version,
    about = "Tilde sidecar: in-memory conversation cache and executor beside an agent process"
)]
struct Args {
    #[arg(long)]
    check_config: bool,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Several rustls crypto providers are compiled into the workspace, so there is no implicit
    // default; dependencies that call `ClientConfig::builder()` need this one.
    let _ = rustls::crypto::ring::default_provider().install_default();
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tilde=info".into()),
        )
        .init();
    let args = Args::parse();
    let options = tilde::deployment::sidecar::Options::from_env()?;
    if args.check_config {
        return Ok(());
    }
    tilde::deployment::sidecar::run(options).await?;
    Ok(())
}
