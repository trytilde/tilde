use clap::Parser;
#[derive(Parser)]
#[command(
    name = "tilde-sidecar",
    version,
    about = "Tilde sidecar: in-memory conversation owner beside an agent process"
)]
struct Args {
    #[arg(long)]
    check_config: bool,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
