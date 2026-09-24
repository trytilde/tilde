//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbResult, GenericClient};

pub async fn create_key_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: &str,
    p4: Option<&[u8]>,
    p5: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::encryption::create_key::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5)
        .await?)
}

fn get_key_row(
    r: tilde_queries::queries::encryption::get_key::Record,
) -> DbResult<crate::encryption::KeyRow> {
    Ok(crate::encryption::KeyRow {
        id: r.id,
        backend: r.backend,
        wrapping_key_id: r.wrapping_key_id,
        wrap_nonce: r.wrap_nonce,
        wrapped_key: r.wrapped_key,
    })
}

pub(super) async fn get_key_opt(
    db: &impl GenericClient,
) -> DbResult<Option<crate::encryption::KeyRow>> {
    tilde_queries::queries::encryption::get_key::run()
        .bind(db)
        .opt()
        .await?
        .map(get_key_row)
        .transpose()
}

pub async fn lock_execute(db: &impl GenericClient, p1: i64) -> DbResult<u64> {
    tilde_queries::queries::encryption::lock::run()
        .bind(db, &p1)
        .one()
        .await?;
    Ok(1)
}
