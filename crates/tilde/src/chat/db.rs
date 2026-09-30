//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};

pub use tilde_queries::queries::chat::activity_list::Record as ActivityListRow;

pub async fn activity_list_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: i64,
    p3: i64,
) -> DbResult<Vec<ActivityListRow>> {
    Ok(tilde_queries::queries::chat::activity_list::run()
        .bind(db, &p2, &p3, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::activity_lock::Record as ActivityLockRow;

pub async fn activity_lock_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<ActivityLockRow>> {
    Ok(tilde_queries::queries::chat::activity_lock::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::activity_sequence::Record as ActivitySequenceRow;

pub async fn activity_sequence_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<ActivitySequenceRow> {
    tilde_queries::queries::chat::activity_sequence::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::agent_available::Record as AgentAvailableRow;

pub async fn agent_available_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<AgentAvailableRow>> {
    Ok(tilde_queries::queries::chat::agent_available::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::agent_channels::Record as AgentChannelsRow;

pub async fn agent_channels_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<AgentChannelsRow>> {
    Ok(tilde_queries::queries::chat::agent_channels::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::agent_invocations::Record as AgentInvocationsRow;

pub async fn agent_invocations_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: bool,
) -> DbResult<Vec<AgentInvocationsRow>> {
    Ok(tilde_queries::queries::chat::agent_invocations::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::agent_participants::Record as AgentParticipantsRow;

pub async fn agent_participants_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<AgentParticipantsRow>> {
    Ok(tilde_queries::queries::chat::agent_participants::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub async fn attachment_attach_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::attachment_attach::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub use tilde_queries::queries::chat::attachment_get::Record as AttachmentGetRow;

pub async fn attachment_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<AttachmentGetRow>> {
    Ok(tilde_queries::queries::chat::attachment_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::attachment_put::Record as AttachmentPutRow;

pub async fn attachment_put_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: &str,
    p5: i64,
    p6: &str,
    p7: &[u8],
) -> DbResult<Option<AttachmentPutRow>> {
    Ok(tilde_queries::queries::chat::attachment_put::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7)
        .opt()
        .await?)
}

pub async fn attachment_remote_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: &str,
    p5: i64,
    p6: uuid::Uuid,
    p7: Option<&str>,
    p8: Option<&[u8]>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::attachment_remote::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8)
        .await?)
}

pub async fn attachment_store_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[u8],
    p3: i64,
    p4: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::attachment_store::run()
        .bind(db, &p2, &p3, &p4, &p1)
        .await?)
}

pub use tilde_queries::queries::chat::audit_flush::Record as AuditFlushRow;

pub async fn audit_flush_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[i64],
    p3: &[String],
    p4: &[uuid::Uuid],
    p5: &[String],
    p6: &[Vec<u8>],
    p7: &[chrono::DateTime<chrono::Utc>],
    p8: &[bool],
) -> DbResult<Vec<AuditFlushRow>> {
    Ok(tilde_queries::queries::chat::audit_flush::run()
        .bind(db, &p2, &p3, &p4, &p5, &p6, &p7, &p8, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::audit_transactions::Record as AuditTransactionsRow;

pub async fn audit_transactions_all(
    db: &impl GenericClient,
    p1: &[i64],
) -> DbResult<Vec<AuditTransactionsRow>> {
    Ok(tilde_queries::queries::chat::audit_transactions::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::audit_writer_lock::Record as AuditWriterLockRow;

pub async fn audit_writer_lock_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<AuditWriterLockRow> {
    tilde_queries::queries::chat::audit_writer_lock::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::cache_hydrate::Record as CacheHydrateRow;

pub async fn cache_hydrate_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &[uuid::Uuid],
) -> DbResult<Vec<CacheHydrateRow>> {
    Ok(tilde_queries::queries::chat::cache_hydrate::run()
        .bind(db, &p1, &p2, &p3)
        .all()
        .await?)
}

pub async fn cache_invalidate_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::cache_invalidate::run()
        .bind(db, &p1)
        .await?)
}

pub use tilde_queries::queries::chat::cache_message_get::Record as CacheMessageGetRow;

pub async fn cache_message_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<CacheMessageGetRow>> {
    Ok(tilde_queries::queries::chat::cache_message_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub async fn cache_upsert_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &serde_json::Value,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::cache_upsert::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub use tilde_queries::queries::chat::cache_upsert_batch::Record as CacheUpsertBatchRow;

pub async fn cache_upsert_batch_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &[uuid::Uuid],
    p4: &[String],
) -> DbResult<CacheUpsertBatchRow> {
    tilde_queries::queries::chat::cache_upsert_batch::run()
        .bind(db, &p3, &p4, &p2, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::channel_binding::Record as ChannelBindingRow;

pub async fn channel_binding_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<ChannelBindingRow>> {
    Ok(tilde_queries::queries::chat::channel_binding::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::channel_owner::Record as ChannelOwnerRow;

pub async fn channel_owner_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<ChannelOwnerRow>> {
    Ok(tilde_queries::queries::chat::channel_owner::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub async fn channel_thread_bind_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::channel_thread_bind::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .await?)
}

pub async fn channel_thread_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::channel_thread_create::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub async fn channel_user_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::channel_user::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub use tilde_queries::queries::chat::controls::ack::Record as ControlsAckRow;

pub async fn controls_ack_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<ControlsAckRow>> {
    Ok(tilde_queries::queries::chat::controls::ack::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::controls::receipt::Record as ControlsReceiptRow;

pub async fn controls_receipt_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<ControlsReceiptRow> {
    tilde_queries::queries::chat::controls::receipt::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::controls::state::Record as ControlsStateRow;

pub async fn controls_state_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
) -> DbResult<Option<ControlsStateRow>> {
    Ok(tilde_queries::queries::chat::controls::state::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .opt()
        .await?)
}

pub async fn controls_suspend_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::controls::suspend::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub async fn delivery_insert_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::delivery_insert::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .await?)
}

pub use tilde_queries::queries::chat::delivery_lookup::Record as DeliveryLookupRow;

pub async fn delivery_lookup_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
) -> DbResult<Option<DeliveryLookupRow>> {
    Ok(tilde_queries::queries::chat::delivery_lookup::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::dependencies_ready::Record as DependenciesReadyRow;

pub async fn dependencies_ready_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<DependenciesReadyRow> {
    tilde_queries::queries::chat::dependencies_ready::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn dependency_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::dependency_create::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .await?)
}

pub use tilde_queries::queries::chat::deployment_pin::Record as DeploymentPinRow;

pub async fn deployment_pin_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: Option<uuid::Uuid>,
) -> DbResult<Option<DeploymentPinRow>> {
    Ok(tilde_queries::queries::chat::deployment_pin::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub async fn goal_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::goal_create::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .await?)
}

pub use tilde_queries::queries::chat::goal_get::Record as GoalGetRow;

pub async fn goal_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
) -> DbResult<Option<GoalGetRow>> {
    Ok(tilde_queries::queries::chat::goal_get::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::goal_update::Record as GoalUpdateRow;

pub async fn goal_update_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
) -> DbResult<Option<GoalUpdateRow>> {
    Ok(tilde_queries::queries::chat::goal_update::run()
        .bind(db, &p4, &p3, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::goals::Record as GoalsRow;

pub async fn goals_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<GoalsRow>> {
    Ok(tilde_queries::queries::chat::goals::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub async fn input_accept_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::input_accept::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub async fn input_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::input_create::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub use tilde_queries::queries::chat::input_get::Record as InputGetRow;

pub async fn input_get_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<InputGetRow> {
    tilde_queries::queries::chat::input_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn input_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<InputGetRow>> {
    Ok(tilde_queries::queries::chat::input_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub async fn input_move_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::input_move::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub use tilde_queries::queries::chat::inputs_pending::Record as InputsPendingRow;

pub async fn inputs_pending_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<InputsPendingRow>> {
    Ok(tilde_queries::queries::chat::inputs_pending::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::invocation_active::Record as InvocationActiveRow;

pub async fn invocation_active_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<InvocationActiveRow>> {
    Ok(tilde_queries::queries::chat::invocation_active::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub async fn invocation_batch_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::invocation_batch::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub use tilde_queries::queries::chat::invocation_claim::Record as InvocationClaimRow;

pub async fn invocation_claim_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<InvocationClaimRow> {
    tilde_queries::queries::chat::invocation_claim::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn invocation_claim_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<InvocationClaimRow>> {
    Ok(tilde_queries::queries::chat::invocation_claim::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub async fn invocation_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
    p5: &str,
    p6: &str,
    p7: Option<uuid::Uuid>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::invocation_create::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7)
        .await?)
}

pub async fn invocation_cutoff_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: Option<uuid::Uuid>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::invocation_cutoff::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub use tilde_queries::queries::chat::invocation_endpoint::Record as InvocationEndpointRow;

pub async fn invocation_endpoint_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<InvocationEndpointRow> {
    tilde_queries::queries::chat::invocation_endpoint::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn invocation_endpoint_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<InvocationEndpointRow>> {
    Ok(tilde_queries::queries::chat::invocation_endpoint::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::invocation_expire::Record as InvocationExpireRow;

pub async fn invocation_expire_all(db: &impl GenericClient) -> DbResult<Vec<InvocationExpireRow>> {
    Ok(tilde_queries::queries::chat::invocation_expire::run()
        .bind(db)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::invocation_finish::Record as InvocationFinishRow;

pub async fn invocation_finish_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<InvocationFinishRow> {
    tilde_queries::queries::chat::invocation_finish::run()
        .bind(db, &p2, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn invocation_finish_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<Option<InvocationFinishRow>> {
    Ok(tilde_queries::queries::chat::invocation_finish::run()
        .bind(db, &p2, &p1)
        .opt()
        .await?)
}

pub async fn invocation_heartbeat_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::invocation_heartbeat::run()
        .bind(db, &p1)
        .await?)
}

pub use tilde_queries::queries::chat::invocation_status::Record as InvocationStatusRow;

pub async fn invocation_status_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<InvocationStatusRow>> {
    Ok(tilde_queries::queries::chat::invocation_status::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::invocations_pending::Record as InvocationsPendingRow;

pub async fn invocations_pending_all(
    db: &impl GenericClient,
) -> DbResult<Vec<InvocationsPendingRow>> {
    Ok(tilde_queries::queries::chat::invocations_pending::run()
        .bind(db)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::message_actor::Record as MessageActorRow;

pub async fn message_actor_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<MessageActorRow> {
    tilde_queries::queries::chat::message_actor::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn message_chunk_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::message_chunk::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub use tilde_queries::queries::chat::message_cleanup::Record as MessageCleanupRow;

pub async fn message_cleanup_all(db: &impl GenericClient) -> DbResult<Vec<MessageCleanupRow>> {
    Ok(tilde_queries::queries::chat::message_cleanup::run()
        .bind(db)
        .all()
        .await?)
}

pub async fn message_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: &str,
    p6: Option<uuid::Uuid>,
    p7: Option<uuid::Uuid>,
    p8: &str,
    p9: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::message_create::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8, &p9)
        .await?)
}

pub async fn message_edit_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::message_edit::run()
        .bind(db, &p2, &p3, &p1)
        .await?)
}

pub async fn message_finish_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::message_finish::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub async fn message_format_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::message_format::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub struct MessageGetRow {
    pub connection_id: Option<uuid::Uuid>,
    pub external_message_id: Option<String>,
    pub destination: Option<String>,
    pub delivery_status: Option<String>,
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub participant_id: uuid::Uuid,
    pub text: String,
    pub status: String,
    pub in_reply_to_message_id: Option<uuid::Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub format: String,
    pub subject: Option<String>,
    pub invocation_id: Option<uuid::Uuid>,
    pub attachments: crate::chat::audit::AttachmentRecords,
    pub targets: Vec<uuid::Uuid>,
}

fn message_get_row(
    r: tilde_queries::queries::chat::message_get::Record,
) -> DbResult<MessageGetRow> {
    Ok(MessageGetRow {
        connection_id: r.connection_id,
        external_message_id: r.external_message_id,
        destination: r.destination,
        delivery_status: r.delivery_status,
        id: r.id,
        thread_id: r.thread_id,
        participant_id: r.participant_id,
        text: r.text,
        status: r.status,
        in_reply_to_message_id: r.in_reply_to_message_id,
        created_at: r.created_at,
        format: r.format,
        subject: r.subject,
        invocation_id: r.invocation_id,
        attachments: tokio_postgres::types::Json(
            serde_json::from_value(r.attachments).map_err(crate::database::DbError::decode)?,
        ),
        targets: r.targets,
    })
}

pub async fn message_get_one(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<MessageGetRow> {
    message_get_row(
        tilde_queries::queries::chat::message_get::run()
            .bind(db, &p1)
            .opt()
            .await?
            .ok_or(DbError::NotFound)?,
    )
}

pub async fn message_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<MessageGetRow>> {
    tilde_queries::queries::chat::message_get::run()
        .bind(db, &p1)
        .opt()
        .await?
        .map(message_get_row)
        .transpose()
}

pub async fn message_subject_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: Option<&str>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::message_subject::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub async fn message_target_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::message_target::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub struct MessagesRow {
    pub connection_id: Option<uuid::Uuid>,
    pub external_message_id: Option<String>,
    pub destination: Option<String>,
    pub delivery_status: Option<String>,
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub participant_id: uuid::Uuid,
    pub text: String,
    pub status: String,
    pub in_reply_to_message_id: Option<uuid::Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub format: String,
    pub subject: Option<String>,
    pub invocation_id: Option<uuid::Uuid>,
    pub attachments: crate::chat::audit::AttachmentRecords,
    pub targets: Vec<uuid::Uuid>,
}

fn messages_row(r: tilde_queries::queries::chat::messages::Record) -> DbResult<MessagesRow> {
    Ok(MessagesRow {
        connection_id: r.connection_id,
        external_message_id: r.external_message_id,
        destination: r.destination,
        delivery_status: r.delivery_status,
        id: r.id,
        thread_id: r.thread_id,
        participant_id: r.participant_id,
        text: r.text,
        status: r.status,
        in_reply_to_message_id: r.in_reply_to_message_id,
        created_at: r.created_at,
        format: r.format,
        subject: r.subject,
        invocation_id: r.invocation_id,
        attachments: tokio_postgres::types::Json(
            serde_json::from_value(r.attachments).map_err(crate::database::DbError::decode)?,
        ),
        targets: r.targets,
    })
}

pub async fn messages_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: i64,
) -> DbResult<Vec<MessagesRow>> {
    tilde_queries::queries::chat::messages::run()
        .bind(db, &p1, &p2)
        .all()
        .await?
        .into_iter()
        .map(messages_row)
        .collect()
}

pub struct MessagesPageRow {
    pub activity_sequence: i64,
    pub cursor_valid: bool,
    pub connection_id: Option<uuid::Uuid>,
    pub external_message_id: Option<String>,
    pub destination: Option<String>,
    pub delivery_status: Option<String>,
    pub id: Option<uuid::Uuid>,
    pub thread_id: uuid::Uuid,
    pub participant_id: Option<uuid::Uuid>,
    pub text: Option<String>,
    pub status: Option<String>,
    pub in_reply_to_message_id: Option<uuid::Uuid>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub format: Option<String>,
    pub subject: Option<String>,
    pub cached_representation: Option<serde_json::Value>,
    pub attachments: crate::chat::audit::AttachmentRecords,
    pub targets: Vec<uuid::Uuid>,
}

fn messages_page_row(
    r: tilde_queries::queries::chat::messages_page::Record,
) -> DbResult<MessagesPageRow> {
    Ok(MessagesPageRow {
        activity_sequence: r.activity_sequence,
        cursor_valid: r.cursor_valid,
        connection_id: r.connection_id,
        external_message_id: r.external_message_id,
        destination: r.destination,
        delivery_status: r.delivery_status,
        id: r.id,
        thread_id: r.thread_id,
        participant_id: r.participant_id,
        text: r.text,
        status: r.status,
        in_reply_to_message_id: r.in_reply_to_message_id,
        created_at: r.created_at,
        format: r.format,
        subject: r.subject,
        cached_representation: r.cached_representation,
        attachments: tokio_postgres::types::Json(
            serde_json::from_value(r.attachments).map_err(crate::database::DbError::decode)?,
        ),
        targets: r.targets,
    })
}

pub async fn messages_page_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: i64,
    p3: Option<uuid::Uuid>,
    p4: Option<uuid::Uuid>,
) -> DbResult<Vec<MessagesPageRow>> {
    tilde_queries::queries::chat::messages_page::run()
        .bind(db, &p3, &p4, &p2, &p1)
        .all()
        .await?
        .into_iter()
        .map(messages_page_row)
        .collect()
}

pub use tilde_queries::queries::chat::native_message_complete::Record as NativeMessageCompleteRow;

pub async fn native_message_complete_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: uuid::Uuid,
    p6: Option<uuid::Uuid>,
    p7: &[uuid::Uuid],
    p8: &[uuid::Uuid],
    p9: &str,
    p10: &str,
    p11: i64,
    p12: &str,
    p13: &str,
) -> DbResult<Option<NativeMessageCompleteRow>> {
    Ok(tilde_queries::queries::chat::native_message_complete::run()
        .bind(
            db, &p1, &p2, &p5, &p3, &p12, &p13, &p4, &p6, &p9, &p10, &p7, &p8, &p11,
        )
        .opt()
        .await?)
}

pub struct NativeMessagePrepareRow {
    pub at: chrono::DateTime<chrono::Utc>,
    pub attachments: crate::chat::audit::AttachmentRecords,
}

fn native_message_prepare_row(
    r: tilde_queries::queries::chat::native_message_prepare::Record,
) -> DbResult<NativeMessagePrepareRow> {
    Ok(NativeMessagePrepareRow {
        at: r.at,
        attachments: tokio_postgres::types::Json(
            serde_json::from_value(r.attachments).map_err(crate::database::DbError::decode)?,
        ),
    })
}

pub async fn native_message_prepare_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[uuid::Uuid],
) -> DbResult<Option<NativeMessagePrepareRow>> {
    tilde_queries::queries::chat::native_message_prepare::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .map(native_message_prepare_row)
        .transpose()
}

pub async fn participant_active_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: bool,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::participant_active::run()
        .bind(db, &p3, &p1, &p2)
        .await?)
}

pub async fn participant_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: Option<uuid::Uuid>,
    p4: Option<uuid::Uuid>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::participant_create::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .await?)
}

pub use tilde_queries::queries::chat::participant_find::Record as ParticipantFindRow;

pub async fn participant_find_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: Option<uuid::Uuid>,
    p3: Option<uuid::Uuid>,
) -> DbResult<Option<ParticipantFindRow>> {
    Ok(tilde_queries::queries::chat::participant_find::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::participant_get::Record as ParticipantGetRow;

pub async fn participant_get_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<ParticipantGetRow> {
    tilde_queries::queries::chat::participant_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn participant_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<ParticipantGetRow>> {
    Ok(tilde_queries::queries::chat::participant_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub async fn participant_join_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: Option<uuid::Uuid>,
    p4: Option<uuid::Uuid>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::participant_join::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .await?)
}

pub use tilde_queries::queries::chat::receipt_get::Record as ReceiptGetRow;

pub async fn receipt_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<Option<ReceiptGetRow>> {
    Ok(tilde_queries::queries::chat::receipt_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub async fn receipt_insert_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: Option<uuid::Uuid>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::receipt_insert::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub async fn route_claim_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::route_claim::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub use tilde_queries::queries::chat::route_pending::Record as RoutePendingRow;

pub async fn route_pending_all(db: &impl GenericClient) -> DbResult<Vec<RoutePendingRow>> {
    Ok(tilde_queries::queries::chat::route_pending::run()
        .bind(db, &crate::deployment::liveness_secs())
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::run_create::Record as RunCreateRow;

pub async fn run_create_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: Option<uuid::Uuid>,
    p6: &str,
) -> DbResult<RunCreateRow> {
    tilde_queries::queries::chat::run_create::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn run_create_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: Option<uuid::Uuid>,
    p6: &str,
) -> DbResult<Option<RunCreateRow>> {
    Ok(tilde_queries::queries::chat::run_create::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::run_get::Record as RunGetRow;

pub async fn run_get_opt(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<Option<RunGetRow>> {
    Ok(tilde_queries::queries::chat::run_get::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::run_replay::Record as RunReplayRow;

pub async fn run_replay_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
) -> DbResult<RunReplayRow> {
    tilde_queries::queries::chat::run_replay::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::run_resume::Record as RunResumeRow;

pub async fn run_resume_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<RunResumeRow>> {
    Ok(tilde_queries::queries::chat::run_resume::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::run_state::Record as RunStateRow;

pub async fn run_state_one(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<RunStateRow> {
    tilde_queries::queries::chat::run_state::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn run_status_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::run_status::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub use tilde_queries::queries::chat::steering_message::Record as SteeringMessageRow;

pub async fn steering_message_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<SteeringMessageRow>> {
    Ok(tilde_queries::queries::chat::steering_message::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::task_create_audited::Record as TaskCreateAuditedRow;

pub async fn task_create_audited_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: Option<uuid::Uuid>,
    p6: &[uuid::Uuid],
    p7: i64,
) -> DbResult<Option<TaskCreateAuditedRow>> {
    Ok(tilde_queries::queries::chat::task_create_audited::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::task_update::Record as TaskUpdateRow;

pub async fn task_update_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: &str,
) -> DbResult<Option<TaskUpdateRow>> {
    Ok(tilde_queries::queries::chat::task_update::run()
        .bind(db, &p4, &p5, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::tasks::Record as TasksRow;

pub async fn tasks_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<TasksRow>> {
    Ok(tilde_queries::queries::chat::tasks::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub async fn thread_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::thread_create::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub use tilde_queries::queries::chat::thread_detail::Record as ThreadDetailRow;

pub async fn thread_detail_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<ThreadDetailRow>> {
    Ok(tilde_queries::queries::chat::thread_detail::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::thread_get::Record as ThreadGetRow;

pub async fn thread_get_one(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<ThreadGetRow> {
    tilde_queries::queries::chat::thread_get::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn thread_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<ThreadGetRow>> {
    Ok(tilde_queries::queries::chat::thread_get::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::chat::thread_lock::Record as ThreadLockRow;

pub async fn thread_lock_one(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<ThreadLockRow> {
    tilde_queries::queries::chat::thread_lock::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::tool_get::Record as ToolGetRow;

pub async fn tool_get_one(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<ToolGetRow> {
    tilde_queries::queries::chat::tool_get::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::tool_interrupted::Record as ToolInterruptedRow;

pub async fn tool_interrupted_all(db: &impl GenericClient) -> DbResult<Vec<ToolInterruptedRow>> {
    Ok(tilde_queries::queries::chat::tool_interrupted::run()
        .bind(db)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::tool_write::Record as ToolWriteRow;

pub async fn tool_write_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
    p5: &str,
    p6: &str,
    p7: &str,
    p8: &str,
    p9: &str,
    p10: &str,
    p11: &str,
    p12: i64,
    p13: &str,
    p14: bool,
    p15: &str,
) -> DbResult<ToolWriteRow> {
    tilde_queries::queries::chat::tool_write::run()
        .bind(
            db, &p1, &p2, &p3, &p4, &p5, &p6, &p8, &p13, &p14, &p15, &p11, &p9, &p7, &p10, &p12,
        )
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn typing_clear_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::typing_clear::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub async fn typing_expire_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::chat::typing_expire::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub use tilde_queries::queries::chat::typing_expired::Record as TypingExpiredRow;

pub async fn typing_expired_all(db: &impl GenericClient) -> DbResult<Vec<TypingExpiredRow>> {
    Ok(tilde_queries::queries::chat::typing_expired::run()
        .bind(db)
        .all()
        .await?)
}

pub use tilde_queries::queries::chat::typing_set::Record as TypingSetRow;

pub async fn typing_set_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<TypingSetRow> {
    tilde_queries::queries::chat::typing_set::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::typing_write::Record as TypingWriteRow;

pub async fn typing_write_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: Option<chrono::DateTime<chrono::Utc>>,
    p4: i64,
) -> DbResult<TypingWriteRow> {
    tilde_queries::queries::chat::typing_write::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::user_create::Record as UserCreateRow;

pub async fn user_create_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<UserCreateRow> {
    tilde_queries::queries::chat::user_create::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::chat::user_get::Record as UserGetRow;

pub async fn user_get_opt(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<Option<UserGetRow>> {
    Ok(tilde_queries::queries::chat::user_get::run()
        .bind(db, &p1)
        .opt()
        .await?)
}
