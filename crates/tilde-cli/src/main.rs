//! The `tilde` command. Distributed as a standalone binary and as the `@trytilde/cli` and
//! `trytilde-cli` packages, which install this same binary under the name `tilde`.
mod api;
mod chat;
mod deploy;
mod dev;
mod discover;
mod doctor;
mod project;
mod registry;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "tilde",
    bin_name = "tilde",
    version,
    about = "Develop, deploy and inspect Tilde agents",
    propagate_version = true
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run this project's agent against a gateway and chat with it locally.
    Dev(dev::Dev),
    /// Register a release carrying the prompts, skills and tools the code declares.
    Deploy(deploy::Deploy),
    /// Check this project, its SDK and the gateway.
    Doctor(doctor::Doctor),
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let outcome = match args.command {
        Command::Dev(args) => dev::run(args).await,
        Command::Deploy(args) => deploy::run(args).await,
        Command::Doctor(args) => doctor::run(args).await,
    };
    if let Err(error) = outcome {
        eprintln!("tilde: {error}");
        std::process::exit(1);
    }
}
