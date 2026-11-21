//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
use crate::database::{DbResult, GenericClient};
use tilde_queries::queries::secrets as q;
use uuid::Uuid;

pub use q::secret_get::Record as SecretRow;
pub use q::secret_list::Record as ListRow;

pub async fn secret_list_all(db: &impl GenericClient) -> DbResult<Vec<ListRow>> {
    Ok(q::secret_list::run().bind(db).all().await?)
}
/// The secret `name` a spec of `team` references: the team's own before the organization's.
pub async fn secret_by_name_opt(db: &impl GenericClient, name: &str) -> DbResult<Option<Uuid>> {
    Ok(q::secret_by_name::run()
        .bind(db, &name.to_owned())
        .opt()
        .await?
        .map(|row| row.id))
}
pub async fn secret_get_opt(db: &impl GenericClient, id: Uuid) -> DbResult<Option<SecretRow>> {
    Ok(q::secret_get::run().bind(db, &id).opt().await?)
}
pub async fn secret_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    name: &str,
    description: &str,
    sealed: &[u8],
) -> DbResult<u64> {
    Ok(q::secret_insert::run()
        .bind(db, &id, &name, &description, &sealed)
        .await?)
}
pub async fn secret_update_execute(
    db: &impl GenericClient,
    id: Uuid,
    sealed: Option<&[u8]>,
    description: &str,
) -> DbResult<u64> {
    Ok(q::secret_update::run()
        .bind(db, &sealed, &description, &id)
        .await?)
}
pub async fn secret_delete_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::secret_delete::run().bind(db, &id).await?)
}
