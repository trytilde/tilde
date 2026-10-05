//! A single-workspace Tilde engine. Contracts and central migrations are shared by every transport.
// Contracts live in their own crate so the CLI builds against the same generated types; the
// engine's own `crate::proto::…` and `crate::services::…` paths are unchanged by that move.
pub use tilde_contracts::{proto, services};

pub mod agent;
pub mod config;
pub mod connections;
pub mod database;
pub mod deployment;
pub mod encryption;
pub mod error;
pub mod network;
pub mod pricing;

pub mod chat;

pub mod iam;
pub mod inference;
pub mod prompts;
pub mod routines;
pub mod signals;
pub mod skills;
pub mod tools;

pub mod telemetry;

mod rpc;

pub mod identities;
