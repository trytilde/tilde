//! Connections own credential lifecycle; Chat owns channel delivery and agent/thread bindings.
//! Providers have a current catalog definition; connection types select their credential source.
//! Capabilities belong to that provider/type pair; there are no per-connection enable switches.
//! All private values are encrypted in Postgres. Core lifecycle and leases are typed columns.
//! Provider-owned React pages run in sandboxed iframes. The host holds the expiring connection
//! setup token; verified OAuth callbacks cannot be replaced with client assertions of success.
pub mod assets;
pub mod catalog;
pub mod categories;
pub mod model;
pub mod oauth;
pub mod rpc;
pub mod service;

/// Shared provider callback path. It bypasses management login and validates its own OAuth state.
pub const CALLBACK_PATH: &str = "/connections/callback";

mod remote;
pub mod setup;

pub mod schema;
