mod common;
use common::Database;
use std::time::Duration;
use tilde::database::{self, MIGRATIONS};

#[tokio::test]
async fn migration_preserves_legacy_history_serializes_starters_and_checks_checksums() {
    let db = Database::unmigrated().await;
    database::migrate_through(&db.pool, 202).await.unwrap();
    let original: Vec<u8> = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT checksum FROM _tilde_migrations WHERE version=1",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    db.pool
        .get()
        .await
        .unwrap()
        .batch_execute("ALTER TABLE _tilde_migrations RENAME TO _sqlx_migrations")
        .await
        .unwrap();
    let other = database::pool_from_config(db.pool.config().as_ref().clone(), 2).unwrap();
    let (first, second) = tokio::join!(database::migrate(&db.pool), database::migrate(&other));
    first.unwrap();
    second.unwrap();
    let row = db.pool.get().await.unwrap().query_one(
        "SELECT COUNT(*)::BIGINT, to_regclass('_sqlx_migrations') IS NULL FROM _tilde_migrations", &[]).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), MIGRATIONS.len() as i64);
    assert!(row.get::<_, bool>(1));
    assert_eq!(
        db.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT checksum FROM _tilde_migrations WHERE version=1",
                &[]
            )
            .await
            .unwrap()
            .get::<_, Vec<u8>>(0),
        original
    );
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE _tilde_migrations SET checksum=$1 WHERE version=1",
            &[&vec![0u8]],
        )
        .await
        .unwrap();
    assert!(matches!(database::migrate(&db.pool).await,
        Err(tilde::error::Error::Migration(message)) if message.contains("modified")));

    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE _tilde_migrations SET checksum=$1 WHERE version=1",
            &[&original],
        )
        .await
        .unwrap();
    database::migrate(&db.pool).await.unwrap();
    other.close().await;
    db.close().await;
}

#[tokio::test]
async fn canceled_migration_releases_its_connection_and_lock() {
    let db = Database::new().await;
    let mut blocker = db.pool.get().await.unwrap();
    let lock = blocker.transaction().await.unwrap();
    lock.batch_execute(include_str!("../../../queries/_migrations/lock.sql"))
        .await
        .unwrap();
    let mut config = db.pool.config().as_ref().clone();
    config.application_name("tilde-canceled-migration-test");
    let pool = database::pool_from_config(config, 1).unwrap();
    let attempt = tokio::spawn({
        let pool = pool.clone();
        async move { database::migrate(&pool).await }
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting: bool = db.pool.get().await.unwrap().query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name='tilde-canceled-migration-test' AND wait_event='advisory')", &[]
            ).await.unwrap().get(0);
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    attempt.abort();
    assert!(attempt.await.unwrap_err().is_cancelled());
    lock.rollback().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), database::migrate(&pool))
        .await
        .unwrap()
        .unwrap();
    pool.close().await;
    db.close().await;
}
