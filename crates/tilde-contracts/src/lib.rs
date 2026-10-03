//! The Protobuf messages and ConnectRPC service definitions generated from `proto/`, as their
//! own crate so that both the engine and the `tilde` CLI build against one set of contracts
//! instead of hand-writing the wire shapes.
//!
//! Nothing here is written by hand: `task generate` regenerates both trees from `buf.gen.yaml`.
//! The message module is called `proto` because the service generator refers to it as
//! `crate::proto` (`buffa_module` in `buf.gen.yaml`).
#[rustfmt::skip]
#[path = "generated/messages/mod.rs"]
pub mod proto;
#[rustfmt::skip]
#[path = "generated/services/mod.rs"]
pub mod services;
