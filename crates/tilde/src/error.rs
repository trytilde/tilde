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
    #[error("agent not found")]
    NotFound,
    #[error("agent ID already exists with different creation parameters")]
    Conflict,
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
            Error::Invalid(message) => Self::invalid_argument(message),
            Error::NotFound => Self::not_found("Agent not found"),
            Error::Conflict => {
                Self::already_exists("Agent ID already exists with different parameters")
            }
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
