//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};

pub use tilde_queries::queries::deployment::agent_runs::Record as AgentRunsRow;

pub async fn agent_runs_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<AgentRunsRow>> {
    Ok(tilde_queries::queries::deployment::agent_runs::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub use tilde_queries::queries::deployment::agent_secrets::Record as AgentSecretsRow;

pub async fn agent_secrets_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<AgentSecretsRow> {
    tilde_queries::queries::deployment::agent_secrets::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::deployment::assigned_connections::Record as AssignedConnectionsRow;

pub async fn assigned_connections_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<AssignedConnectionsRow>> {
    Ok(
        tilde_queries::queries::deployment::assigned_connections::run()
            .bind(db, &p1)
            .all()
            .await?,
    )
}

pub async fn attachment_object_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: i64,
    p4: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::attachment_object::run()
        .bind(db, &p2, &p3, &p4, &p1)
        .await?)
}

pub use tilde_queries::queries::deployment::authenticate::Record as AuthenticateRow;

pub async fn authenticate_opt(
    db: &impl GenericClient,
    p1: &[u8],
) -> DbResult<Option<AuthenticateRow>> {
    Ok(tilde_queries::queries::deployment::authenticate::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::connected_instance::Record as ConnectedInstanceRow;

pub async fn connected_instance_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: f64,
) -> DbResult<Option<ConnectedInstanceRow>> {
    Ok(
        tilde_queries::queries::deployment::connected_instance::run()
            .bind(db, &p1, &p2, &p3)
            .opt()
            .await?,
    )
}

pub use tilde_queries::queries::deployment::converted_messages::Record as ConvertedMessagesRow;

pub async fn converted_messages_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<ConvertedMessagesRow>> {
    Ok(
        tilde_queries::queries::deployment::converted_messages::run()
            .bind(db, &p1, &p2)
            .all()
            .await?,
    )
}

pub use tilde_queries::queries::deployment::dead_leases::Record as DeadLeasesRow;

pub async fn dead_leases_all(db: &impl GenericClient, p1: f64) -> DbResult<Vec<DeadLeasesRow>> {
    Ok(tilde_queries::queries::deployment::dead_leases::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::deployment::deployment_create::Record as DeploymentCreateRow;

pub async fn deployment_create_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: &str,
    p5: Option<&str>,
    p6: Option<&str>,
    p7: Option<&str>,
    p8: Option<&str>,
    p9: &str,
    p10: &[u8],
    p11: Option<&str>,
    p12: Option<&str>,
    p13: Option<&str>,
) -> DbResult<DeploymentCreateRow> {
    tilde_queries::queries::deployment::deployment_create::run()
        .bind(
            db, &p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8, &p9, &p10, &p11, &p12, &p13,
        )
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::deployment::deployment_external::Record as DeploymentExternalRow;

pub async fn deployment_external_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<Option<DeploymentExternalRow>> {
    Ok(
        tilde_queries::queries::deployment::deployment_external::run()
            .bind(db, &p1, &p2)
            .opt()
            .await?,
    )
}

pub use tilde_queries::queries::deployment::deployment_get::Record as DeploymentGetRow;

pub async fn deployment_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<DeploymentGetRow>> {
    Ok(tilde_queries::queries::deployment::deployment_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::deployment_latest::Record as DeploymentLatestRow;

pub async fn deployment_latest_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[String],
) -> DbResult<Option<DeploymentLatestRow>> {
    Ok(tilde_queries::queries::deployment::deployment_latest::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::deployment_latest_except::Record as DeploymentLatestExceptRow;

pub async fn deployment_latest_except_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[String],
    p3: uuid::Uuid,
) -> DbResult<Option<DeploymentLatestExceptRow>> {
    Ok(
        tilde_queries::queries::deployment::deployment_latest_except::run()
            .bind(db, &p1, &p2, &p3)
            .opt()
            .await?,
    )
}

pub use tilde_queries::queries::deployment::deployment_pin::Record as DeploymentPinRow;

pub async fn deployment_pin_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<DeploymentPinRow>> {
    Ok(tilde_queries::queries::deployment::deployment_pin::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::deployment_promote::Record as DeploymentPromoteRow;

pub async fn deployment_promote_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<DeploymentPromoteRow>> {
    Ok(
        tilde_queries::queries::deployment::deployment_promote::run()
            .bind(db, &p1, &p2)
            .opt()
            .await?,
    )
}

pub use tilde_queries::queries::deployment::deployment_retire::Record as DeploymentRetireRow;

pub async fn deployment_retire_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<DeploymentRetireRow>> {
    Ok(tilde_queries::queries::deployment::deployment_retire::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::deployments_list::Record as DeploymentsListRow;

pub async fn deployments_list_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<DeploymentsListRow>> {
    Ok(tilde_queries::queries::deployment::deployments_list::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::deployment::directive_ack::Record as DirectiveAckRow;

pub async fn directive_ack_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<DirectiveAckRow>> {
    Ok(tilde_queries::queries::deployment::directive_ack::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::directive_get::Record as DirectiveGetRow;

pub async fn directive_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<DirectiveGetRow>> {
    Ok(tilde_queries::queries::deployment::directive_get::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub async fn directive_insert_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: Option<uuid::Uuid>,
    p5: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::directive_insert::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5)
        .await?)
}

pub use tilde_queries::queries::deployment::directive_result::Record as DirectiveResultRow;

pub async fn directive_result_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &[u8],
) -> DbResult<Option<DirectiveResultRow>> {
    Ok(tilde_queries::queries::deployment::directive_result::run()
        .bind(db, &p4, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub async fn directives_move_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::directives_move::run()
        .bind(db, &p3, &p1, &p2)
        .await?)
}

pub use tilde_queries::queries::deployment::directives_pending::Record as DirectivesPendingRow;

pub async fn directives_pending_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<DirectivesPendingRow>> {
    Ok(
        tilde_queries::queries::deployment::directives_pending::run()
            .bind(db, &p1, &p2)
            .all()
            .await?,
    )
}

pub use tilde_queries::queries::deployment::event_receipt::Record as EventReceiptRow;

pub async fn event_receipt_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: Option<uuid::Uuid>,
    p4: uuid::Uuid,
    p5: i64,
    p6: chrono::DateTime<chrono::Utc>,
) -> DbResult<Option<EventReceiptRow>> {
    Ok(tilde_queries::queries::deployment::event_receipt::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::external_thread::Record as ExternalThreadRow;

pub async fn external_thread_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
) -> DbResult<Option<ExternalThreadRow>> {
    Ok(tilde_queries::queries::deployment::external_thread::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::fail_old_invocations::Record as FailOldInvocationsRow;

pub async fn fail_old_invocations_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<FailOldInvocationsRow>> {
    Ok(
        tilde_queries::queries::deployment::fail_old_invocations::run()
            .bind(db, &p1, &p2)
            .all()
            .await?,
    )
}

pub use tilde_queries::queries::deployment::get::Record as GetRow;

pub async fn get_opt(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<Option<GetRow>> {
    Ok(tilde_queries::queries::deployment::get::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub async fn heartbeat_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: bool,
    p4: bool,
    p5: bool,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::heartbeat::run()
        .bind(db, &p3, &p4, &p5, &p1, &p2)
        .await?)
}

pub use tilde_queries::queries::deployment::identity_grants::Record as IdentityGrantsRow;

pub async fn identity_grants_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<IdentityGrantsRow>> {
    Ok(tilde_queries::queries::deployment::identity_grants::run()
        .bind(db, &p2, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::deployment::instance_live::Record as InstanceLiveRow;

pub async fn instance_live_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: f64,
) -> DbResult<InstanceLiveRow> {
    tilde_queries::queries::deployment::instance_live::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::deployment::issue_token::Record as IssueTokenRow;

pub async fn issue_token_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[u8],
    p3: uuid::Uuid,
) -> DbResult<Option<IssueTokenRow>> {
    Ok(tilde_queries::queries::deployment::issue_token::run()
        .bind(db, &p2, &p1, &p3)
        .opt()
        .await?)
}

pub async fn lease_delete_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::lease_delete::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub use tilde_queries::queries::deployment::lease_holder::Record as LeaseHolderRow;

pub async fn lease_holder_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: f64,
) -> DbResult<Option<LeaseHolderRow>> {
    Ok(tilde_queries::queries::deployment::lease_holder::run()
        .bind(db, &p3, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::lease_insert::Record as LeaseInsertRow;

pub async fn lease_insert_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
) -> DbResult<Option<LeaseInsertRow>> {
    Ok(tilde_queries::queries::deployment::lease_insert::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::lease_lock::Record as LeaseLockRow;

pub async fn lease_lock_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<LeaseLockRow>> {
    Ok(tilde_queries::queries::deployment::lease_lock::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub async fn lease_move_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::lease_move::run()
        .bind(db, &p3, &p1, &p2)
        .await?)
}

pub use tilde_queries::queries::deployment::lease_pin::Record as LeasePinRow;

pub async fn lease_pin_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
) -> DbResult<Option<LeasePinRow>> {
    Ok(tilde_queries::queries::deployment::lease_pin::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub async fn lease_release_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::lease_release::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub use tilde_queries::queries::deployment::leases_held::Record as LeasesHeldRow;

pub async fn leases_held_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<LeasesHeldRow>> {
    Ok(tilde_queries::queries::deployment::leases_held::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub use tilde_queries::queries::deployment::leases_since::Record as LeasesSinceRow;

pub async fn leases_since_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: chrono::DateTime<chrono::Utc>,
) -> DbResult<Vec<LeasesSinceRow>> {
    Ok(tilde_queries::queries::deployment::leases_since::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub use tilde_queries::queries::deployment::live_node::Record as LiveNodeRow;

pub async fn live_node_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: Option<uuid::Uuid>,
    p3: Option<uuid::Uuid>,
    p4: f64,
) -> DbResult<Option<LiveNodeRow>> {
    Ok(tilde_queries::queries::deployment::live_node::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::lock::Record as LockRow;

pub async fn lock_opt(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<Option<LockRow>> {
    Ok(tilde_queries::queries::deployment::lock::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::node_url::Record as NodeUrlRow;

pub use tilde_queries::queries::deployment::nodes::Record as NodesRow;

pub async fn nodes_all(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<Vec<NodesRow>> {
    Ok(tilde_queries::queries::deployment::nodes::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub async fn project_attachment_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: &str,
    p5: i64,
    p6: &str,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::deployment::project::attachment::run()
            .bind(db, &p1, &p2, &p3, &p4, &p5, &p6)
            .await?,
    )
}

pub async fn project_delivery_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: &str,
    p5: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::project::delivery::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5)
        .await?)
}

pub async fn project_dispatch_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::project::dispatch::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub async fn project_goal_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::project::goal::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5)
        .await?)
}

pub use tilde_queries::queries::deployment::project::has_thread::Record as ProjectHasThreadRow;

pub async fn project_has_thread_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<ProjectHasThreadRow> {
    tilde_queries::queries::deployment::project::has_thread::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn project_invocation_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
    p5: &str,
    p6: chrono::DateTime<chrono::Utc>,
    p7: Option<chrono::DateTime<chrono::Utc>>,
    p8: Option<uuid::Uuid>,
    p9: Option<uuid::Uuid>,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::deployment::project::invocation::run()
            .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8, &p9)
            .await?,
    )
}

pub async fn project_message_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: &str,
    p6: Option<uuid::Uuid>,
    p7: chrono::DateTime<chrono::Utc>,
    p8: &str,
    p9: Option<&str>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::project::message::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8, &p9)
        .await?)
}

pub async fn project_participant_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: Option<uuid::Uuid>,
    p4: Option<uuid::Uuid>,
    p5: bool,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::deployment::project::participant::run()
            .bind(db, &p1, &p2, &p3, &p4, &p5)
            .await?,
    )
}

pub async fn project_run_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: &str,
    p6: Option<uuid::Uuid>,
    p7: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::project::run::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7)
        .await?)
}

pub async fn project_task_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: &str,
    p6: Option<uuid::Uuid>,
    p7: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::project::task::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7)
        .await?)
}

pub async fn project_thread_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::project::thread::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub async fn project_tool_execute(
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
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::project::tool::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8, &p9, &p10, &p11)
        .await?)
}

pub async fn project_user_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::project::user::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub use tilde_queries::queries::deployment::prune::Record as PruneRow;

pub async fn prune_one(db: &impl GenericClient) -> DbResult<PruneRow> {
    tilde_queries::queries::deployment::prune::run()
        .bind(db)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn register_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &str,
    p5: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::register::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5)
        .await?)
}

pub use tilde_queries::queries::deployment::relay_invocation::Record as RelayInvocationRow;

pub async fn relay_invocation_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
) -> DbResult<Option<RelayInvocationRow>> {
    Ok(tilde_queries::queries::deployment::relay_invocation::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .opt()
        .await?)
}

pub use tilde_queries::queries::deployment::relay_pending::Record as RelayPendingRow;

pub async fn relay_pending_all(db: &impl GenericClient, p1: f64) -> DbResult<Vec<RelayPendingRow>> {
    Ok(tilde_queries::queries::deployment::relay_pending::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub use tilde_queries::queries::deployment::run_origins::Record as RunOriginsRow;

pub async fn run_origins_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<RunOriginsRow>> {
    Ok(tilde_queries::queries::deployment::run_origins::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub async fn secrets_init_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::secrets_init::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub async fn settings_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::settings::run()
        .bind(db, &p2, &p3, &p1)
        .await?)
}

pub use tilde_queries::queries::deployment::stop_runs::Record as StopRunsRow;

pub async fn stop_runs_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<StopRunsRow>> {
    Ok(tilde_queries::queries::deployment::stop_runs::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub async fn store_secrets_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::store_secrets::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub async fn telemetry_health_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: chrono::DateTime<chrono::Utc>,
    p5: bool,
    p6: i32,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::telemetry::health::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6)
        .await?)
}

pub use tilde_queries::queries::deployment::thread_agent::Record as ThreadAgentRow;

pub async fn thread_agent_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<ThreadAgentRow> {
    tilde_queries::queries::deployment::thread_agent::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::deployment::thread_participant::Record as ThreadParticipantRow;

pub async fn thread_participant_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<ThreadParticipantRow>> {
    Ok(
        tilde_queries::queries::deployment::thread_participant::run()
            .bind(db, &p1, &p2)
            .opt()
            .await?,
    )
}

pub use tilde_queries::queries::deployment::thread_sidecar_agents::Record as ThreadSidecarAgentsRow;

pub async fn thread_sidecar_agents_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<ThreadSidecarAgentsRow>> {
    Ok(
        tilde_queries::queries::deployment::thread_sidecar_agents::run()
            .bind(db, &p1)
            .all()
            .await?,
    )
}

/// Choose a deployment for a new conversation; existing pins are retained by the caller.
pub async fn choose_opt(
    db: &impl GenericClient,
    agent: uuid::Uuid,
) -> DbResult<Option<tilde_queries::queries::deployment::choose::Record>> {
    Ok(tilde_queries::queries::deployment::choose::run()
        .bind(db, &agent, &super::liveness_secs())
        .opt()
        .await?)
}
pub async fn weights_seed_execute(
    db: &impl GenericClient,
    agent: uuid::Uuid,
    deployment: Option<uuid::Uuid>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::weights_seed::run()
        .bind(db, &deployment, &agent)
        .await?)
}
pub async fn weight_set_execute(
    db: &impl GenericClient,
    agent: uuid::Uuid,
    deployment: uuid::Uuid,
    weight: i32,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::weight_set::run()
        .bind(db, &weight, &agent, &deployment)
        .await?)
}

pub async fn project_lease_pin_execute(
    db: &impl GenericClient,
    thread: uuid::Uuid,
    agent: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::deployment::project::lease_pin::run()
            .bind(db, &thread, &agent)
            .await?,
    )
}

pub async fn available_all(
    db: &impl GenericClient,
    agent: uuid::Uuid,
) -> DbResult<Vec<tilde_queries::queries::deployment::available::Record>> {
    Ok(tilde_queries::queries::deployment::available::run()
        .bind(db, &agent, &super::liveness_secs())
        .all()
        .await?)
}
pub async fn watch_open_execute(
    db: &impl GenericClient,
    agent: uuid::Uuid,
    instance: uuid::Uuid,
    connection: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::watch_open::run()
        .bind(db, &connection, &agent, &instance)
        .await?)
}
pub async fn watch_close_execute(
    db: &impl GenericClient,
    agent: uuid::Uuid,
    instance: uuid::Uuid,
    connection: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::deployment::watch_close::run()
        .bind(db, &agent, &instance, &connection)
        .await?)
}

pub async fn project_message_target(
    db: &impl GenericClient,
    message: uuid::Uuid,
    thread: uuid::Uuid,
    participant: uuid::Uuid,
) -> DbResult<()> {
    tilde_queries::queries::deployment::project::message_links::targets()
        .bind(db, &message, &thread, &participant)
        .await?;
    Ok(())
}
pub async fn project_message_attachment(
    db: &impl GenericClient,
    thread: uuid::Uuid,
    message: uuid::Uuid,
    attachment: uuid::Uuid,
) -> DbResult<()> {
    tilde_queries::queries::deployment::project::message_links::attachment()
        .bind(db, &thread, &message, &attachment)
        .await?;
    Ok(())
}
