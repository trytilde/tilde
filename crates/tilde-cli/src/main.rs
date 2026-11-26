//! The `tilde` command. Distributed as a standalone binary and as the `@trytilde/cli` and
//! `trytilde-cli` packages, which install this same binary under the name `tilde`.
mod api;
mod apply;
mod build;
mod chat;
mod deploy;
mod dev;
mod discover;
mod doctor;
mod hosted;
mod iac;
mod local;
mod ontology;
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
    about = "Develop, deploy and inspect Tilde agents, connect local agents, and serve Tilde sandbox VMs",
    propagate_version = true
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build an agent's deployable artifacts as `tilde deploy` would (a Hosted agent's bundle,
    /// its declarations, its spec) into a folder, contacting no server.
    Build(build::Build),
    /// Run this project's agent against a gateway and chat with it locally.
    Dev(dev::Dev),
    /// Apply the `@trytilde/iac` specs the code exports (the project's ontologies first, then its
    /// agents, each deploying one releasing its own code from `agents/<key>/` or its `code`), or
    /// register a release carrying the prompts, skills and tools the entry declares.
    Deploy(deploy::Deploy),
    /// Check this project, its SDK and the gateway.
    Doctor(doctor::Doctor),
    /// Tools for `@trytilde/iac` specs.
    Iac(iac::Iac),
    /// Connect a harness on this machine (Claude Code, Codex, Claude Desktop) to its local agent.
    Local(local::Local),
    /// Inside a sandbox VM: serve the agent's operations, and call the sandbox's tools.
    #[cfg(unix)]
    Sandbox(sandbox::Sandbox),
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let outcome = match args.command {
        Command::Build(args) => build::run(args).await,
        Command::Dev(args) => dev::run(args).await,
        Command::Deploy(args) => deploy::run(args).await,
        Command::Doctor(args) => doctor::run(args).await,
        Command::Iac(args) => iac::run(args).await,
        Command::Local(args) => local::run(args).await,
        #[cfg(unix)]
        Command::Sandbox(args) => sandbox::run(args).await,
    };
    if let Err(error) = outcome {
        eprintln!("tilde: {error}");
        std::process::exit(1);
    }
}
