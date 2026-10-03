//! Built-in Tilde chat provider. The public protocol is ConnectRPC, sharing the
//! chat application service with external channels and invocation-scoped runtime APIs.
//! Its credentials and access policy live on the agent's `tilde/application` connection.
mod adapter;
pub(crate) mod credentials;
pub(crate) mod db;
mod management;
pub mod rpc;
pub(crate) mod sessions;
pub use adapter::Tilde;
pub(crate) use management::router as management_router;
pub(crate) use sessions::QueueChange;
