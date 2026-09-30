//! Stable application errors. Provider/SQL details never become public RPC messages.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Configure S3 storage to upload agent avatars")]
    AvatarStorageDisabled,
    #[error("Avatar object storage operation failed")]
    AvatarStorage,
    #[error("Pause the agent before deleting it")]
    AgentNotPaused,
    #[error("Chat lifecycle operation failed")]
    ChatLifecycle(#[from] crate::chat::ChatError),
    #[error("Connection chat capability already belongs to another agent")]
    ConnectionAssignmentConflict,
    #[error("Operation is not permitted")]
    Denied,
    #[error("{0}")]
    Invalid(String),
    #[error("Provider authorization was rejected; reconnect the connection")]
    ConnectionAuthorizationRequired,
    /// A tool host refused an instance's credentials at the end of setup. The message is the
    /// host's own, written for the person entering them.
    #[error("{0}")]
    CredentialsRejected(String),
    #[error("agent not found")]
    NotFound,
    #[error("agent ID already exists with different creation parameters")]
    Conflict,
    /// A concurrent write changed what this one was based on; the caller reloads and retries.
    #[error("{0}")]
    Stale(String),
    #[error("database operation failed")]
    Database(#[from] crate::database::DbError),
    #[error("database migration failed")]
    Migration(String),
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
            Error::ChatLifecycle(error) => error.into(),
            Error::AvatarStorageDisabled => {
                Self::failed_precondition("Configure S3 storage to upload agent avatars")
            }
            Error::AgentNotPaused => {
                Self::failed_precondition("Pause the agent before deleting it")
            }
            Error::ConnectionAssignmentConflict => {
                Self::already_exists("Connection chat capability already belongs to another agent")
            }
            Error::Denied => Self::permission_denied("Operation is not permitted"),
            Error::ConnectionAuthorizationRequired => Self::failed_precondition(
                "Provider authorization was rejected; reconnect the connection",
            ),
            Error::Invalid(message) | Error::CredentialsRejected(message) => {
                Self::invalid_argument(message)
            }
            Error::NotFound => Self::not_found("Agent not found"),
            Error::Conflict => {
                Self::already_exists("Agent ID already exists with different parameters")
            }
            Error::Stale(message) => Self::aborted(message),
            other => {
                tracing::error!(error = ?other, "engine operation failed");
                Self::internal("Engine operation failed")
            }
        }
    }
}

impl From<tokio_postgres::Error> for Error {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Database(error.into())
    }
}
impl From<deadpool_postgres::PoolError> for Error {
    fn from(error: deadpool_postgres::PoolError) -> Self {
        Self::Database(error.into())
    }
}
