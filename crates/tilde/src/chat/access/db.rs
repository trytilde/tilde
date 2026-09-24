//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};

pub use tilde_queries::queries::channel_access::agent_identities_missing::Record as AgentIdentitiesMissingRow;

pub async fn agent_identities_missing_all(
    db: &impl GenericClient,
) -> DbResult<Vec<AgentIdentitiesMissingRow>> {
    Ok(
        tilde_queries::queries::channel_access::agent_identities_missing::run()
            .bind(db)
            .all()
            .await?,
    )
}

pub async fn agent_identity_delete_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::channel_access::agent_identity_delete::run()
            .bind(db, &p1)
            .await?,
    )
}

pub async fn agent_identity_put_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: &str,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::channel_access::agent_identity_put::run()
            .bind(db, &p2, &p3, &p1)
            .await?,
    )
}

pub use tilde_queries::queries::channel_access::allowed::Record as AllowedRow;

pub async fn allowed_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<AllowedRow> {
    tilde_queries::queries::channel_access::allowed::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn audit_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
    p5: &str,
    p6: bool,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::channel_access::audit::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6)
        .await?)
}

pub async fn cleanup_execute(db: &impl GenericClient) -> DbResult<u64> {
    Ok(tilde_queries::queries::channel_access::cleanup::run()
        .bind(db)
        .await?)
}

pub async fn grant_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: bool,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::channel_access::grant::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .await?)
}

pub use tilde_queries::queries::channel_access::identities::Record as IdentitiesRow;

pub async fn identities_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: Option<uuid::Uuid>,
    p3: Option<uuid::Uuid>,
    p4: i64,
) -> DbResult<Vec<IdentitiesRow>> {
    Ok(tilde_queries::queries::channel_access::identities::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .all()
        .await?)
}

pub use tilde_queries::queries::channel_access::identity_get::Record as IdentityGetRow;

pub async fn identity_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<IdentityGetRow>> {
    Ok(tilde_queries::queries::channel_access::identity_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub use tilde_queries::queries::channel_access::identity_upsert::Record as IdentityUpsertRow;

pub async fn identity_upsert_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
    p4: &str,
) -> DbResult<IdentityUpsertRow> {
    tilde_queries::queries::channel_access::identity_upsert::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn identity_verify_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::channel_access::identity_verify::run()
            .bind(db, &p1)
            .await?,
    )
}

pub use tilde_queries::queries::channel_access::invalid_invocations::Record as InvalidInvocationsRow;

pub async fn invalid_invocations_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<InvalidInvocationsRow>> {
    Ok(
        tilde_queries::queries::channel_access::invalid_invocations::run()
            .bind(db, &p1, &p2)
            .all()
            .await?,
    )
}

pub use tilde_queries::queries::channel_access::message_allowed::Record as MessageAllowedRow;

pub async fn message_allowed_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: Option<uuid::Uuid>,
) -> DbResult<MessageAllowedRow> {
    tilde_queries::queries::channel_access::message_allowed::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn message_source_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::channel_access::message_source::run()
            .bind(db, &p2, &p1)
            .await?,
    )
}

pub async fn policy_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::channel_access::policy::run()
        .bind(db, &p3, &p1, &p2)
        .await?)
}

fn route_row(
    r: tilde_queries::queries::channel_access::route::Record,
) -> DbResult<crate::chat::access::Route> {
    Ok(crate::chat::access::Route {
        connection_id: r.connection_id,
        agent_id: r.agent_id,
        access_mode: r.access_mode,
        name: r.name,
        provider_id: r.provider_id,
        type_id: r.type_id,
        status: r.status,
        provider_name: r.provider_name,
        icon_url: r.icon_url,
        agent_name: r.agent_name,
        agent_identity_type: r.agent_identity_type,
        agent_identity_value: r.agent_identity_value,
    })
}

pub(super) async fn route_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<crate::chat::access::Route>> {
    tilde_queries::queries::channel_access::route::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?
        .map(route_row)
        .transpose()
}

fn routes_row(
    r: tilde_queries::queries::channel_access::routes::Record,
) -> DbResult<crate::chat::access::Route> {
    Ok(crate::chat::access::Route {
        connection_id: r.connection_id,
        agent_id: r.agent_id,
        access_mode: r.access_mode,
        name: r.name,
        provider_id: r.provider_id,
        type_id: r.type_id,
        status: r.status,
        provider_name: r.provider_name,
        icon_url: r.icon_url,
        agent_name: r.agent_name,
        agent_identity_type: r.agent_identity_type,
        agent_identity_value: r.agent_identity_value,
    })
}

pub(super) async fn routes_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: Option<uuid::Uuid>,
    p3: i64,
) -> DbResult<Vec<crate::chat::access::Route>> {
    tilde_queries::queries::channel_access::routes::run()
        .bind(db, &p1, &p2, &p3)
        .all()
        .await?
        .into_iter()
        .map(routes_row)
        .collect()
}

pub use tilde_queries::queries::channel_access::run_allowed::Record as RunAllowedRow;

pub async fn run_allowed_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: Option<uuid::Uuid>,
) -> DbResult<RunAllowedRow> {
    tilde_queries::queries::channel_access::run_allowed::run()
        .bind(db, &p1, &p2, &p3)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn run_source_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: Option<uuid::Uuid>,
    p3: bool,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::channel_access::run_source::run()
        .bind(db, &p2, &p3, &p1)
        .await?)
}

pub use tilde_queries::queries::channel_access::verification_claim::Record as VerificationClaimRow;

pub async fn verification_claim_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[u8],
) -> DbResult<Option<VerificationClaimRow>> {
    Ok(
        tilde_queries::queries::channel_access::verification_claim::run()
            .bind(db, &p1, &p2)
            .opt()
            .await?,
    )
}

pub async fn verification_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
    p5: &[u8],
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::channel_access::verification_create::run()
            .bind(db, &p1, &p2, &p3, &p4, &p5)
            .await?,
    )
}

pub use tilde_queries::queries::channel_access::verification_get::Record as VerificationGetRow;

pub async fn verification_get_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<VerificationGetRow> {
    tilde_queries::queries::channel_access::verification_get::run()
        .bind(db, &p1)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn verification_get_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<VerificationGetRow>> {
    Ok(
        tilde_queries::queries::channel_access::verification_get::run()
            .bind(db, &p1)
            .opt()
            .await?,
    )
}

pub use tilde_queries::queries::channel_access::verification_latest::Record as VerificationLatestRow;

pub async fn verification_latest_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Option<VerificationLatestRow>> {
    Ok(
        tilde_queries::queries::channel_access::verification_latest::run()
            .bind(db, &p1, &p2)
            .opt()
            .await?,
    )
}

pub async fn verification_result_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::channel_access::verification_result::run()
            .bind(db, &p2, &p1)
            .await?,
    )
}

pub async fn verification_supersede_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(
        tilde_queries::queries::channel_access::verification_supersede::run()
            .bind(db, &p1, &p2)
            .await?,
    )
}
