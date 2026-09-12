//! Local SQLite state accessed exclusively through Corrosion's API. SQL and
//! domain payloads remain separate from the gateway's Postgres implementation.
pub mod client;
pub mod process;
pub use client::Client;
