//! A single-workspace Tilde engine. Contracts and central migrations are shared by every transport.
#[rustfmt::skip]
#[path = "generated/messages/mod.rs"]
pub mod proto;
#[rustfmt::skip]
#[path = "generated/services/mod.rs"]
pub mod services;

pub mod agent;
pub mod config;
pub mod connections;
pub mod database;
pub mod deployment;
pub mod encryption;
pub mod error;
pub mod logs;
pub mod network;

pub mod chat;

pub mod iam;

// Avoid shadowing the external `tracing` macros throughout the domain.
#[path = "tracing/mod.rs"]
pub mod telemetry;

mod rpc;
