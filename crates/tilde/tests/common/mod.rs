use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::str::FromStr;
use uuid::Uuid;

pub struct Database {
    pub pool: PgPool,
    admin: PgPool,
    schema: String,
}
impl Database {
    pub async fn new() -> Self {
        let db = Self::unmigrated().await;
        tilde::database::migrate(&db.pool).await.unwrap();
        db
    }
    /// Start from an empty isolated schema when verifying historical migrations.
    pub async fn unmigrated() -> Self {
        let url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL is required; run task test for isolated Postgres");
        let admin = PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        let schema = format!("test_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let options = PgConnectOptions::from_str(&url)
            .unwrap()
            .options([("search_path", schema.as_str())]);
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .unwrap();
        Self {
            pool,
            admin,
            schema,
        }
    }
    pub async fn close(self) {
        self.pool.close().await;
        sqlx::query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}

pub fn seed(byte: u8) -> tilde::encryption::KeyProtection {
    use base64::Engine;
    tilde::encryption::KeyProtection::seed(secrecy::SecretString::from(
        base64::engine::general_purpose::STANDARD.encode([byte; 32]),
    ))
    .unwrap()
}

pub fn binding(id: Uuid, name: &str) -> tilde::encryption::SecretBinding<'_> {
    tilde::encryption::SecretBinding {
        resource_kind: "agent",
        resource_id: id,
        name,
    }
}
