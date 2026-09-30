//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};

pub use tilde_queries::queries::connections::agent_exists::Record as AgentExistsRow;

pub async fn agent_exists_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<AgentExistsRow>> {
    Ok(tilde_queries::queries::connections::agent_exists::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub async fn assignment_delete_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::assignment_delete::run()
            .bind(db, &p1, &p2, &p3)
            .await?,
    )
}

pub async fn assignment_insert_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
    p4: Option<&str>,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::assignment_insert::run()
            .bind(db, &p1, &p2, &p3, &p4)
            .await?,
    )
}

pub async fn connection_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: &str,
    p4: &str,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::connection_create::run()
            .bind(db, &p1, &p2, &p3, &p4)
            .await?,
    )
}

pub async fn connection_disconnect_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::connection_disconnect::run()
            .bind(db, &p1)
            .await?,
    )
}

pub async fn connection_fail_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::connection_fail::run()
        .bind(db, &p1)
        .await?)
}

fn connection_get_row(
    r: tilde_queries::queries::connections::connection_get::Record,
) -> DbResult<crate::connections::model::Connection> {
    Ok(crate::connections::model::Connection {
        id: r.id,
        name: r.name,
        provider_id: r.provider_id,
        type_id: r.type_id,
        status: r.status,
        account_label: r.account_label,
        token_expires_at: r.token_expires_at,
        credential_version: r.credential_version,
        created_at: r.created_at,
        updated_at: r.updated_at,
        channel_capable: r.channel_capable,
        inference_capable: r.inference_capable,
        tool_capable: r.tool_capable,
        associated_agents: tokio_postgres::types::Json(
            serde_json::from_value(r.associated_agents)
                .map_err(crate::database::DbError::decode)?,
        ),
    })
}

pub async fn connection_get_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<crate::connections::model::Connection> {
    connection_get_row(
        tilde_queries::queries::connections::connection_get::run()
            .bind(db, &p1)
            .opt()
            .await?
            .ok_or(DbError::NotFound)?,
    )
}

pub async fn connection_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<crate::connections::model::Connection>> {
    tilde_queries::queries::connections::connection_get::run()
        .bind(db, &p1)
        .opt()
        .await?
        .map(connection_get_row)
        .transpose()
}

fn connection_list_row(
    r: tilde_queries::queries::connections::connection_list::Record,
) -> DbResult<crate::connections::model::Connection> {
    Ok(crate::connections::model::Connection {
        id: r.id,
        name: r.name,
        provider_id: r.provider_id,
        type_id: r.type_id,
        status: r.status,
        account_label: r.account_label,
        token_expires_at: r.token_expires_at,
        credential_version: r.credential_version,
        created_at: r.created_at,
        updated_at: r.updated_at,
        channel_capable: r.channel_capable,
        inference_capable: r.inference_capable,
        tool_capable: r.tool_capable,
        associated_agents: tokio_postgres::types::Json(
            serde_json::from_value(r.associated_agents)
                .map_err(crate::database::DbError::decode)?,
        ),
    })
}

pub async fn connection_list_all(
    db: &impl GenericClient,
    p1: Option<chrono::DateTime<chrono::Utc>>,
    p2: Option<uuid::Uuid>,
    p3: i64,
    filter: &crate::connections::service::ConnectionFilter<'_>,
) -> DbResult<Vec<crate::connections::model::Connection>> {
    tilde_queries::queries::connections::connection_list::run()
        .bind(
            db,
            &p1,
            &p2,
            &filter.agent_id,
            &filter
                .capability
                .map(crate::connections::model::Capability::as_str),
            &filter.search,
            &filter.provider_id,
            &filter.status,
            &filter
                .source
                .map(crate::connections::catalog::ProviderSource::as_str),
            &p3,
        )
        .all()
        .await?
        .into_iter()
        .map(connection_list_row)
        .collect()
}

pub async fn connection_lock_execute(db: &impl GenericClient, p1: &str) -> DbResult<u64> {
    tilde_queries::queries::connections::connection_lock::run()
        .bind(db, &p1)
        .one()
        .await?;
    Ok(1)
}

pub async fn connection_ready_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: Option<&str>,
    p3: Option<chrono::DateTime<chrono::Utc>>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::connection_ready::run()
        .bind(db, &p2, &p3, &p1)
        .await?)
}

pub async fn connection_reauthorize_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: i64,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::connection_reauthorize::run()
            .bind(db, &p1, &p2)
            .await?,
    )
}

pub use tilde_queries::queries::connections::connection_refresh_claim::Record as ConnectionRefreshClaimRow;

pub async fn connection_refresh_claim_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: i64,
) -> DbResult<Option<ConnectionRefreshClaimRow>> {
    Ok(
        tilde_queries::queries::connections::connection_refresh_claim::run()
            .bind(db, &p1, &p2)
            .opt()
            .await?,
    )
}

pub use tilde_queries::queries::connections::connection_refresh_finish::Record as ConnectionRefreshFinishRow;

pub async fn connection_refresh_finish_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: i64,
    p3: Option<chrono::DateTime<chrono::Utc>>,
) -> DbResult<Option<ConnectionRefreshFinishRow>> {
    Ok(
        tilde_queries::queries::connections::connection_refresh_finish::run()
            .bind(db, &p3, &p1, &p2)
            .opt()
            .await?,
    )
}

pub async fn connection_refresh_release_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: i64,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::connection_refresh_release::run()
            .bind(db, &p1, &p2)
            .await?,
    )
}

pub async fn connection_setup_pending_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::connection_setup_pending::run()
            .bind(db, &p1)
            .await?,
    )
}

pub use tilde_queries::queries::connections::draft_get::Record as DraftGetRow;

pub async fn draft_get_all(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<Vec<DraftGetRow>> {
    Ok(tilde_queries::queries::connections::draft_get::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub async fn draft_put_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::draft_put::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub async fn parameters_insert_execute(
    db: &impl GenericClient,
    p1: &str,
    p2: &str,
    p3: &str,
    p4: &str,
    p5: &str,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::parameters_insert::run()
            .bind(db, &p1, &p2, &p3, &p4, &p5)
            .await?,
    )
}

pub use tilde_queries::queries::connections::parameters_list::Record as ParametersListRow;

pub async fn parameters_list_all(
    db: &impl GenericClient,
    p1: &str,
    p2: &str,
) -> DbResult<Vec<ParametersListRow>> {
    Ok(tilde_queries::queries::connections::parameters_list::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub use tilde_queries::queries::connections::provider_backend::Record as ProviderBackendRow;

pub async fn provider_backend_one(
    db: &impl GenericClient,
    p1: &str,
) -> DbResult<ProviderBackendRow> {
    tilde_queries::queries::connections::provider_backend::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn provider_clear_details_execute(db: &impl GenericClient, p1: &str) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::provider_clear_details::run()
            .bind(db, &p1)
            .await?,
    )
}

pub use tilde_queries::queries::connections::provider_get::Record as ProviderGetRow;

pub async fn provider_get_opt(
    db: &impl GenericClient,
    p1: &str,
) -> DbResult<Option<ProviderGetRow>> {
    Ok(tilde_queries::queries::connections::provider_get::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub async fn provider_insert_execute(
    db: &impl GenericClient,
    p1: &str,
    p2: &str,
    p3: &str,
    p4: Option<&str>,
    p5: Option<&str>,
    p6: Option<&[u8]>,
    p7: Option<uuid::Uuid>,
    p8: &[String],
    p9: Option<&str>,
    p10: Option<&str>,
    p11: Option<&str>,
    p12: Option<uuid::Uuid>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::provider_insert::run()
        .bind(
            db, &p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8, &p9, &p10, &p11, &p12,
        )
        .await?)
}

pub use tilde_queries::queries::connections::provider_list::Record as ProviderListRow;

pub async fn provider_list_all(
    db: &impl GenericClient,
    after: &str,
    filter: &crate::connections::catalog::ProviderFilter<'_>,
    limit: i64,
) -> DbResult<Vec<ProviderListRow>> {
    Ok(tilde_queries::queries::connections::provider_list::run()
        .bind(
            db,
            &after,
            &filter.search,
            &filter.category,
            &filter
                .capability
                .map(crate::connections::model::Capability::as_str),
            &filter
                .source
                .map(crate::connections::catalog::ProviderSource::as_str),
            &limit,
        )
        .all()
        .await?)
}

pub async fn provider_categories_all(
    db: &impl GenericClient,
    filter: &crate::connections::catalog::ProviderFilter<'_>,
) -> DbResult<Vec<String>> {
    Ok(
        tilde_queries::queries::connections::provider_categories::run()
            .bind(
                db,
                &filter
                    .capability
                    .map(crate::connections::model::Capability::as_str),
                &filter
                    .source
                    .map(crate::connections::catalog::ProviderSource::as_str),
            )
            .all()
            .await?
            .into_iter()
            .map(|row| row.category)
            .collect(),
    )
}

pub async fn provider_lock_execute(db: &impl GenericClient, p1: &str) -> DbResult<u64> {
    tilde_queries::queries::connections::provider_lock::run()
        .bind(db, &p1)
        .one()
        .await?;
    Ok(1)
}

pub async fn provider_remove_types_execute(
    db: &impl GenericClient,
    p1: &str,
    p2: &[String],
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::provider_remove_types::run()
            .bind(db, &p1, &p2)
            .await?,
    )
}

pub async fn provider_snapshot_execute(db: &impl GenericClient) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::provider_snapshot::run()
            .bind(db)
            .await?,
    )
}

pub async fn result_fields_insert_execute(
    db: &impl GenericClient,
    p1: &str,
    p2: &str,
    p3: &str,
    p4: &str,
    p5: bool,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::result_fields_insert::run()
            .bind(db, &p1, &p2, &p3, &p4, &p5)
            .await?,
    )
}

pub use tilde_queries::queries::connections::result_fields_list::Record as ResultFieldsListRow;

pub async fn result_fields_list_all(
    db: &impl GenericClient,
    p1: &str,
    p2: &str,
) -> DbResult<Vec<ResultFieldsListRow>> {
    Ok(
        tilde_queries::queries::connections::result_fields_list::run()
            .bind(db, &p1, &p2)
            .all()
            .await?,
    )
}

pub use tilde_queries::queries::connections::setup_callback_rotate::Record as SetupCallbackRotateRow;

pub async fn setup_callback_rotate_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &[u8],
    p4: &[u8],
) -> DbResult<Option<SetupCallbackRotateRow>> {
    Ok(
        tilde_queries::queries::connections::setup_callback_rotate::run()
            .bind(db, &p3, &p4, &p1, &p2)
            .opt()
            .await?,
    )
}

pub async fn setup_cancel_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_cancel::run()
        .bind(db, &p1)
        .await?)
}

pub async fn setup_cancel_one_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_cancel_one::run()
        .bind(db, &p1)
        .await?)
}

pub use tilde_queries::queries::connections::setup_claim::Record as SetupClaimRow;

pub async fn setup_claim_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
) -> DbResult<Option<SetupClaimRow>> {
    Ok(tilde_queries::queries::connections::setup_claim::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub async fn setup_copy_values_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::setup_copy_values::run()
            .bind(db, &p1, &p2)
            .await?,
    )
}

pub async fn setup_discard_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_discard::run()
        .bind(db, &p1)
        .await?)
}

pub async fn setup_expire_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_expire::run()
        .bind(db, &p1)
        .await?)
}

pub async fn setup_fail_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_fail::run()
        .bind(db, &p3, &p1, &p2)
        .await?)
}

fn setup_get_row(
    r: tilde_queries::queries::connections::setup_get::Record,
) -> DbResult<crate::connections::model::Setup> {
    Ok(crate::connections::model::Setup {
        provider_redirect_url: r.provider_redirect_url,
        id: r.id,
        connection_id: r.connection_id,
        step: r.step,
        action_id: r.action_id,
        connection_setup_token: r.connection_setup_token,
        connection_setup_token_hash: r.connection_setup_token_hash,
        callback_token: r.callback_token,
        callback_hash: r.callback_hash,
        error_code: r.error_code,
        expires_at: r.expires_at,
        claimed_at: r.claimed_at,
    })
}

pub(crate) async fn setup_get_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<crate::connections::model::Setup> {
    setup_get_row(
        tilde_queries::queries::connections::setup_get::run()
            .bind(db, &p1)
            .opt()
            .await?
            .ok_or(DbError::NotFound)?,
    )
}

pub(crate) async fn setup_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<crate::connections::model::Setup>> {
    tilde_queries::queries::connections::setup_get::run()
        .bind(db, &p1)
        .opt()
        .await?
        .map(setup_get_row)
        .transpose()
}

pub async fn setup_insert_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: &[u8],
    p5: &[u8],
    p6: &[u8],
    p7: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_insert::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7)
        .await?)
}

fn setup_latest_row(
    r: tilde_queries::queries::connections::setup_latest::Record,
) -> DbResult<crate::connections::model::Setup> {
    Ok(crate::connections::model::Setup {
        provider_redirect_url: r.provider_redirect_url,
        id: r.id,
        connection_id: r.connection_id,
        step: r.step,
        action_id: r.action_id,
        connection_setup_token: r.connection_setup_token,
        connection_setup_token_hash: r.connection_setup_token_hash,
        callback_token: r.callback_token,
        callback_hash: r.callback_hash,
        error_code: r.error_code,
        expires_at: r.expires_at,
        claimed_at: r.claimed_at,
    })
}

pub(crate) async fn setup_latest_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<crate::connections::model::Setup>> {
    tilde_queries::queries::connections::setup_latest::run()
        .bind(db, &p1)
        .opt()
        .await?
        .map(setup_latest_row)
        .transpose()
}

fn setup_lock_row(
    r: tilde_queries::queries::connections::setup_lock::Record,
) -> DbResult<crate::connections::model::Setup> {
    Ok(crate::connections::model::Setup {
        provider_redirect_url: r.provider_redirect_url,
        id: r.id,
        connection_id: r.connection_id,
        step: r.step,
        action_id: r.action_id,
        connection_setup_token: r.connection_setup_token,
        connection_setup_token_hash: r.connection_setup_token_hash,
        callback_token: r.callback_token,
        callback_hash: r.callback_hash,
        error_code: r.error_code,
        expires_at: r.expires_at,
        claimed_at: r.claimed_at,
    })
}

pub(crate) async fn setup_lock_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<crate::connections::model::Setup> {
    setup_lock_row(
        tilde_queries::queries::connections::setup_lock::run()
            .bind(db, &p1)
            .opt()
            .await?
            .ok_or(DbError::NotFound)?,
    )
}

pub async fn setup_name_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_name::run()
        .bind(db, &p2, &p1, &p4, &p3)
        .await?)
}

pub async fn setup_purge_execute(db: &impl GenericClient) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_purge::run()
        .bind(db)
        .await?)
}

pub use tilde_queries::queries::connections::setup_recover::Record as SetupRecoverRow;

pub async fn setup_recover_all(db: &impl GenericClient) -> DbResult<Vec<SetupRecoverRow>> {
    Ok(tilde_queries::queries::connections::setup_recover::run()
        .bind(db)
        .all()
        .await?)
}

pub async fn setup_redirect_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: Option<&str>,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_redirect::run()
        .bind(db, &p3, &p1, &p2)
        .await?)
}

pub async fn setup_resume_app_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_resume_app::run()
        .bind(db, &p2, &p1)
        .await?)
}

pub use tilde_queries::queries::connections::setup_transition::Record as SetupTransitionRow;

pub async fn setup_transition_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: uuid::Uuid,
) -> DbResult<Option<SetupTransitionRow>> {
    Ok(tilde_queries::queries::connections::setup_transition::run()
        .bind(db, &p3, &p4, &p1, &p2)
        .opt()
        .await?)
}

pub async fn setup_values_delete_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::setup_values_delete::run()
            .bind(db, &p1)
            .await?,
    )
}

pub use tilde_queries::queries::connections::setup_values_get::Record as SetupValuesGetRow;

pub async fn setup_values_get_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<SetupValuesGetRow>> {
    Ok(tilde_queries::queries::connections::setup_values_get::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub async fn setup_values_put_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::setup_values_put::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub async fn types_insert_execute(
    db: &impl GenericClient,
    p1: &str,
    p2: &str,
    p3: &str,
    p4: &str,
    p5: bool,
    p6: Option<&str>,
    p7: Option<&str>,
    p8: &str,
    p9: bool,
    p10: &[String],
    p11: &str,
    p12: &str,
    p13: &str,
    p14: &str,
    p15: &str,
    p16: Option<&str>,
    p17: Option<&serde_json::Value>,
    p18: bool,
    p19: bool,
    p20: Option<&str>,
    p21: Option<&str>,
    p22: Option<&str>,
    p23: &str,
    p24: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::types_insert::run()
        .bind(
            db, &p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8, &p9, &p10, &p11, &p12, &p13, &p14, &p15,
            &p16, &p17, &p18, &p19, &p20, &p21, &p22, &p23, &p24,
        )
        .await?)
}

pub use tilde_queries::queries::connections::slug_taken::Record as SlugTakenRow;

pub async fn slug_taken_opt(
    db: &impl GenericClient,
    p1: &str,
    p2: &str,
    p3: uuid::Uuid,
) -> DbResult<Option<SlugTakenRow>> {
    Ok(tilde_queries::queries::connections::slug_taken::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?)
}

pub use tilde_queries::queries::connections::types_list::Record as TypesListRow;

pub async fn types_list_all(db: &impl GenericClient, p1: &str) -> DbResult<Vec<TypesListRow>> {
    Ok(tilde_queries::queries::connections::types_list::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub async fn values_delete_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::values_delete::run()
        .bind(db, &p1)
        .await?)
}

pub use tilde_queries::queries::connections::values_get::Record as ValuesGetRow;

pub async fn values_get_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<ValuesGetRow>> {
    Ok(tilde_queries::queries::connections::values_get::run()
        .bind(db, &p1)
        .all()
        .await?)
}

pub async fn values_put_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::values_put::run()
        .bind(db, &p1, &p2, &p3)
        .await?)
}

pub use tilde_queries::queries::connections::tilde_connection::Record as TildeConnectionRow;

pub async fn tilde_connection_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<TildeConnectionRow>> {
    Ok(tilde_queries::queries::connections::tilde_connection::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::connections::tilde_missing::Record as TildeMissingRow;

pub async fn tilde_missing_all(db: &impl GenericClient) -> DbResult<Vec<TildeMissingRow>> {
    Ok(tilde_queries::queries::connections::tilde_missing::run()
        .bind(db)
        .all()
        .await?)
}

pub async fn tilde_identities_delete_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::tilde_identities_delete::run()
            .bind(db, &p1)
            .await?,
    )
}

pub async fn tilde_delete_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::connections::tilde_delete::run()
        .bind(db, &p1)
        .await?)
}

pub async fn connection_bump_version_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::connections::connection_bump_version::run()
            .bind(db, &p1)
            .await?,
    )
}
