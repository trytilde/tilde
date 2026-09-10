//! One connection pool and one ordered migration history for the entire monolith.
use crate::error::Error;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

pub mod notifications;

/// Open the runtime pool and apply the central migrations before serving requests.
pub async fn connect(url: &str) -> Result<PgPool, Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(url)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

/// Used by startup and tests; migration checksums protect applied history.
pub async fn migrate(pool: &PgPool) -> Result<(), Error> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    Ok(())
}
