//! Stable application errors. Provider/SQL details never become public RPC messages.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Connection chat capability already belongs to another agent")]
    ConnectionAssignmentConflict,
    #[error("Operation is not permitted")]
    Denied,
    #[error("{0}")]
    Invalid(String),
    #[error("Provider authorization was rejected; reconnect the connection")]
    ConnectionAuthorizationRequired,
    #[error("agent not found")]
    NotFound,
    #[error("agent ID already exists with different creation parameters")]
    Conflict,
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("encryption operation failed")]
    Encryption,
    #[error("configured encryption backend does not match the database")]
    BackendMismatch,
    #[error("AWS KMS operation failed")]
    Kms,
}

impl From<Error> for connectrpc::ConnectError {
    fn from(error: Error) -> Self {
        match error {
            Error::ConnectionAssignmentConflict => {
                Self::already_exists("Connection chat capability already belongs to another agent")
            }
            Error::Denied => Self::permission_denied("Operation is not permitted"),
            Error::ConnectionAuthorizationRequired => Self::failed_precondition(
                "Provider authorization was rejected; reconnect the connection",
            ),
            Error::Invalid(message) => Self::invalid_argument(message),
            Error::NotFound => Self::not_found("Agent not found"),
            Error::Conflict => {
                Self::already_exists("Agent ID already exists with different parameters")
            }
            other => {
                tracing::error!(error = %other, "engine operation failed");
                Self::internal("Engine operation failed")
            }
        }
    }
}
