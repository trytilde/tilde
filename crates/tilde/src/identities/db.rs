//! Typed persistence operations; identity services own transactions and association locks.
use crate::chat::audit::timestamp;
use crate::database::{DbResult, GenericClient};
use crate::proto::tilde::management::v1 as wire;
use tilde_queries::queries::identities as q;
use uuid::Uuid;
pub async fn get(db: &impl GenericClient, id: Uuid) -> DbResult<Option<wire::Identity>> {
    Ok(q::get::run()
        .bind(db, &id)
        .opt()
        .await?
        .map(|r| wire::Identity {
            id: r.id.to_string(),
            name: r.name,
            provider_id: r.provider_id,
            connection_id: r.connection_id.map(|id| id.to_string()),
            identity_type: crate::chat::access::identity::kind_value(&r.identity_type).into(),
            value: r.value,
            root_identity_id: r.root_identity_id.map(|id| id.to_string()),
            verified_at: r.verified_at.map(timestamp).into(),
            attested_at: r.attested_at.map(timestamp).into(),
            ..Default::default()
        }))
}
pub async fn list(
    db: &impl GenericClient,
    after: Option<Uuid>,
    limit: i64,
    root: Option<Uuid>,
    connection: Option<Uuid>,
) -> DbResult<Vec<wire::Identity>> {
    Ok(q::list::run()
        .bind(db, &after, &root, &connection, &limit)
        .all()
        .await?
        .into_iter()
        .map(|r| wire::Identity {
            id: r.id.to_string(),
            name: r.name,
            provider_id: r.provider_id,
            connection_id: r.connection_id.map(|id| id.to_string()),
            identity_type: crate::chat::access::identity::kind_value(&r.identity_type).into(),
            value: r.value,
            root_identity_id: r.root_identity_id.map(|id| id.to_string()),
            verified_at: r.verified_at.map(timestamp).into(),
            attested_at: r.attested_at.map(timestamp).into(),
            ..Default::default()
        })
        .collect())
}
pub async fn lock(db: &impl GenericClient, id: Uuid) -> DbResult<Option<q::lock::Record>> {
    Ok(q::lock::run().bind(db, &id).opt().await?)
}
pub async fn link(db: &impl GenericClient, id: Uuid, root: Option<Uuid>) -> DbResult<()> {
    q::link::run().bind(db, &root, &id).await?;
    Ok(())
}
pub async fn native_create(db: &impl GenericClient, id: Uuid, value: &str) -> DbResult<()> {
    q::native_create::run().bind(db, &id, &value).await?;
    Ok(())
}
pub async fn attest(db: &impl GenericClient, id: Uuid) -> DbResult<()> {
    q::attest::run().bind(db, &id).await?;
    Ok(())
}
pub async fn root_create(db: &impl GenericClient, id: Uuid) -> DbResult<()> {
    q::root_create::run().bind(db, &id).await?;
    Ok(())
}
pub async fn root_get(db: &impl GenericClient, id: Uuid) -> DbResult<Option<wire::RootIdentity>> {
    Ok(q::root_get::run()
        .bind(db, &id)
        .opt()
        .await?
        .map(|r| wire::RootIdentity {
            id: r.id.to_string(),
            created_at: timestamp(r.created_at).into(),
            ..Default::default()
        }))
}
pub async fn root_list(
    db: &impl GenericClient,
    after: Option<Uuid>,
    limit: i64,
) -> DbResult<Vec<wire::RootIdentity>> {
    Ok(q::root_list::run()
        .bind(db, &after, &limit)
        .all()
        .await?
        .into_iter()
        .map(|r| wire::RootIdentity {
            id: r.id.to_string(),
            created_at: timestamp(r.created_at).into(),
            ..Default::default()
        })
        .collect())
}

pub async fn provider_connection(
    db: &impl GenericClient,
    agent: Uuid,
    provider: &str,
    name: &str,
) -> DbResult<Option<Uuid>> {
    Ok(q::provider_connection::run()
        .bind(db, &agent, &provider, &name)
        .opt()
        .await?
        .map(|r| r.id))
}
