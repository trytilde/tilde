//! Cornucopia-backed native provider persistence. Application callers own transactions.
use crate::database::{DbResult, GenericClient};
pub use q::{Agent, Queue, Run, Session};
use tilde_queries::queries::chat::tilde::provider as q;
use uuid::Uuid;
/// The asserting application vouches for the identity; no management actor is recorded.
pub async fn attest_identity(db: &impl GenericClient, user: Uuid) -> DbResult<()> {
    q::attest_identity().bind(db, &user).await?;
    Ok(())
}
pub async fn sessions(
    db: &impl GenericClient,
    agent: Uuid,
    user: Option<Uuid>,
    query: &str,
    after: Option<Uuid>,
    limit: i64,
) -> DbResult<Vec<Session>> {
    Ok(q::sessions()
        .bind(db, &user, &agent, &after, &query, &limit)
        .all()
        .await?)
}
pub async fn read_state(
    db: &impl GenericClient,
    thread: Uuid,
    user: Uuid,
    sequence: i64,
    unread: bool,
) -> DbResult<()> {
    q::read_state()
        .bind(db, &thread, &user, &sequence, &unread)
        .await?;
    Ok(())
}
pub async fn unread(db: &impl GenericClient, thread: Uuid, user: Uuid) -> DbResult<bool> {
    Ok(q::unread().bind(db, &thread, &user).one().await?.unread)
}
pub async fn rename(db: &impl GenericClient, thread: Uuid, title: &str) -> DbResult<()> {
    q::rename().bind(db, &title, &thread).await?;
    Ok(())
}
pub async fn agent(db: &impl GenericClient, agent: Uuid) -> DbResult<Option<Agent>> {
    Ok(q::agent().bind(db, &agent).opt().await?)
}
pub async fn latest_run(
    db: &impl GenericClient,
    thread: Uuid,
    agent: Uuid,
) -> DbResult<Option<Run>> {
    Ok(q::latest_run().bind(db, &thread, &agent).opt().await?)
}
pub async fn queue(db: &impl GenericClient, thread: Uuid, agent: Uuid) -> DbResult<Vec<Queue>> {
    Ok(q::queue().bind(db, &thread, &agent).all().await?)
}
pub async fn remove_input(db: &impl GenericClient, invocation: Uuid, id: Uuid) -> DbResult<()> {
    q::remove_input().bind(db, &invocation, &id).await?;
    Ok(())
}
pub async fn order_input(
    db: &impl GenericClient,
    invocation: Uuid,
    id: Uuid,
    position: i64,
) -> DbResult<()> {
    q::order_input()
        .bind(db, &position, &invocation, &id)
        .await?;
    Ok(())
}
pub async fn sidecar_queue(
    db: &impl GenericClient,
    thread: Uuid,
    agent: Uuid,
) -> DbResult<Vec<Queue>> {
    Ok(q::sidecar_queue().bind(db, &thread, &agent).all().await?)
}
pub async fn replace_queue(
    db: &impl GenericClient,
    thread: Uuid,
    agent: Uuid,
    items: &[crate::proto::tilde::types::v1::QueuedInput],
) -> DbResult<()> {
    q::clear_sidecar_queue().bind(db, &thread, &agent).await?;
    for (position, item) in items.iter().enumerate() {
        let id = Uuid::parse_str(&item.id)
            .map_err(|_| crate::database::DbError::decode("Invalid queue ID"))?;
        let invocation = Uuid::parse_str(&item.invocation_id)
            .map_err(|_| crate::database::DbError::decode("Invalid invocation ID"))?;
        q::put_sidecar_queue()
            .bind(
                db,
                &thread,
                &agent,
                &id,
                &invocation,
                &item.text,
                &item.history_through_message_id,
                &(position as i64),
            )
            .await?;
    }
    Ok(())
}
pub async fn sequence(db: &impl GenericClient, thread: Uuid) -> DbResult<i64> {
    Ok(q::sequence()
        .bind(db, &thread)
        .one()
        .await?
        .activity_sequence)
}

pub async fn search_messages(
    db: &impl GenericClient,
    thread: Uuid,
    query: &str,
    after: Option<Uuid>,
    limit: i64,
) -> DbResult<Vec<Uuid>> {
    Ok(q::search_messages()
        .bind(db, &thread, &query, &after, &limit)
        .all()
        .await?
        .into_iter()
        .map(|r| r.id)
        .collect())
}
pub async fn cancel_message(db: &impl GenericClient, thread: Uuid, id: Uuid) -> DbResult<()> {
    q::cancel_message().bind(db, &thread, &id).await?;
    Ok(())
}
