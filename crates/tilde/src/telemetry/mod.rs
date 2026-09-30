//! Everything the engine does with agent telemetry: OTLP traces, logs and metrics accepted
//! from agents and sidecars, spooled durably through object storage and Postgres pointers,
//! stored in ClickHouse and read back by the management API.
//!
//! The `tracing` submodule shadows the `tracing` crate inside this module; use `::tracing::`
//! for the log macros here.
pub mod clickhouse;
pub mod logs;
pub mod metrics;
pub mod spool;
pub mod tracing;
