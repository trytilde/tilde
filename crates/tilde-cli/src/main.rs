//! The `tilde` command. Distributed as a standalone binary and as the `@trytilde/cli` and
//! `trytilde-cli` packages, which install this same binary under the name `tilde`.
mod api;
mod apply;
mod chat;
mod deploy;
mod dev;
mod discover;
mod doctor;
mod iac;
mod project;
mod registry;
#[cfg(unix)]
mod sandbox;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "tilde",
    bin_name = "tilde",
    version,
    about = "Develop, deploy and inspect Tilde agents, and serve Tilde sandbox VMs",
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
    /// Apply the `@trytilde/iac` specs the code exports, or register a release carrying the
    /// prompts, skills and tools it declares.
    Deploy(deploy::Deploy),
    /// Check this project, its SDK and the gateway.
    Doctor(doctor::Doctor),
    /// Tools for `@trytilde/iac` specs.
    Iac(iac::Iac),
    /// Inside a sandbox VM: serve the agent's operations, and call the sandbox's tools.
    #[cfg(unix)]
    Sandbox(sandbox::Sandbox),
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let outcome = match args.command {
        Command::Dev(args) => dev::run(args).await,
        Command::Deploy(args) => deploy::run(args).await,
        Command::Doctor(args) => doctor::run(args).await,
        Command::Iac(args) => iac::run(args).await,
        #[cfg(unix)]
        Command::Sandbox(args) => sandbox::run(args).await,
    };
    if let Err(error) = outcome {
        eprintln!("tilde: {error}");
        std::process::exit(1);
    }
}
