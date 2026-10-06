//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbResult, GenericClient};
use tilde_queries::queries::sandboxes as q;
use uuid::Uuid;

pub use q::agent_sandbox_get::Record as AgentSandboxRow;
pub use q::blueprint_list::Record as BlueprintRow;
pub use q::call_claim::Record as CallRow;
pub use q::call_take::Record as CallResultRow;
pub use q::env_all::Record as EnvRow;
pub use q::launch_connection::Record as LaunchConnectionRow;
pub use q::renew_due::Record as RenewRow;
pub use q::sandbox_authenticate::Record as SessionRow;
pub use q::sandbox_enroll::Record as EnrolledRow;
pub use q::sandbox_get::Record as SandboxRow;
pub use q::sandbox_list::Record as SandboxListRow;
pub use q::sweep_due::Record as DueRow;
pub use q::trace_scope::Record as TraceScopeRow;

pub async fn blueprint_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    name: &str,
    connection: Uuid,
    template: &str,
    reuse: &str,
    timings: super::Timings,
) -> DbResult<u64> {
    Ok(q::blueprint_insert::run()
        .bind(
            db,
            &id,
            &name,
            &connection,
            &template,
            &reuse,
            &(timings.sleep_after as i32),
            &(timings.terminate_after as i32),
            &(timings.connect_timeout as i32),
        )
        .await?)
}
pub async fn blueprint_list_all(
    db: &impl GenericClient,
    id: Option<Uuid>,
    search: Option<&str>,
) -> DbResult<Vec<BlueprintRow>> {
    Ok(q::blueprint_list::run()
        .bind(db, &id, &search)
        .all()
        .await?)
}
pub async fn blueprint_update_execute(
    db: &impl GenericClient,
    id: Uuid,
    name: Option<&str>,
    connection: Option<Uuid>,
    template: Option<&str>,
    reuse: Option<&str>,
    timings: Option<super::Timings>,
) -> DbResult<u64> {
    let seconds = |pick: fn(&super::Timings) -> u32| timings.as_ref().map(|t| pick(t) as i32);
    Ok(q::blueprint_update::run()
        .bind(
            db,
            &name,
            &connection,
            &template,
            &reuse,
            &seconds(|t| t.sleep_after),
            &seconds(|t| t.terminate_after),
            &seconds(|t| t.connect_timeout),
            &id,
        )
        .await?)
}
pub async fn blueprint_delete_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::blueprint_delete::run().bind(db, &id).await?)
}
pub async fn launch_connection_opt(
    db: &impl GenericClient,
    connection: Uuid,
) -> DbResult<Option<LaunchConnectionRow>> {
    Ok(q::launch_connection::run()
        .bind(db, &connection)
        .opt()
        .await?)
}
pub async fn env_set_execute(
    db: &impl GenericClient,
    blueprint: Uuid,
    name: &str,
    sealed: &[u8],
) -> DbResult<u64> {
    Ok(q::env_set::run()
        .bind(db, &blueprint, &name, &sealed)
        .await?)
}
pub async fn env_delete_execute(
    db: &impl GenericClient,
    blueprint: Uuid,
    name: &str,
) -> DbResult<u64> {
    Ok(q::env_delete::run().bind(db, &blueprint, &name).await?)
}
pub async fn env_all(db: &impl GenericClient, blueprint: Uuid) -> DbResult<Vec<EnvRow>> {
    Ok(q::env_all::run().bind(db, &blueprint).all().await?)
}
pub async fn agent_sandbox_get_opt(
    db: &impl GenericClient,
    agent: Uuid,
) -> DbResult<Option<AgentSandboxRow>> {
    Ok(q::agent_sandbox_get::run().bind(db, &agent).opt().await?)
}
pub async fn agent_sandbox_set_execute(
    db: &impl GenericClient,
    agent: Uuid,
    blueprint: Uuid,
) -> DbResult<u64> {
    Ok(q::agent_sandbox_set::run()
        .bind(db, &agent, &blueprint)
        .await?)
}
pub async fn agent_sandbox_delete_execute(db: &impl GenericClient, agent: Uuid) -> DbResult<u64> {
    Ok(q::agent_sandbox_delete::run().bind(db, &agent).await?)
}
pub async fn sandbox_find_opt(db: &impl GenericClient, key: &super::Key) -> DbResult<Option<Uuid>> {
    Ok(q::sandbox_find::run()
        .bind(
            db,
            &key.blueprint,
            &key.reuse,
            &key.agent,
            &key.thread,
            &key.identity,
        )
        .opt()
        .await?
        .map(|r| r.id))
}
/// Created leased for `lease_secs` by the caller, returning the database's clock; `None` when
/// the key is taken.
pub async fn sandbox_insert_opt(
    db: &impl GenericClient,
    id: Uuid,
    key: &super::Key,
    lease_secs: f64,
) -> DbResult<Option<chrono::DateTime<chrono::Utc>>> {
    Ok(q::sandbox_insert::run()
        .bind(
            db,
            &id,
            &key.blueprint,
            &key.reuse,
            &key.connection,
            &key.agent,
            &key.thread,
            &key.identity,
            &lease_secs,
        )
        .opt()
        .await?
        .map(|r| r.now))
}
pub async fn sandbox_get_opt(
    db: &impl GenericClient,
    id: Uuid,
    liveness_secs: f64,
    since: chrono::DateTime<chrono::Utc>,
) -> DbResult<Option<SandboxRow>> {
    Ok(q::sandbox_get::run()
        .bind(db, &liveness_secs, &since, &id)
        .opt()
        .await?)
}
pub async fn sandbox_list_all(
    db: &impl GenericClient,
    blueprint: Option<Uuid>,
    agent: Option<Uuid>,
    liveness_secs: f64,
) -> DbResult<Vec<SandboxListRow>> {
    Ok(q::sandbox_list::run()
        .bind(db, &liveness_secs, &blueprint, &agent)
        .all()
        .await?)
}
/// The database's clock when the lease was taken; `None` while another process holds it.
pub async fn sandbox_lease_opt(
    db: &impl GenericClient,
    id: Uuid,
    lease_secs: f64,
) -> DbResult<Option<chrono::DateTime<chrono::Utc>>> {
    Ok(q::sandbox_lease::run()
        .bind(db, &lease_secs, &id)
        .opt()
        .await?
        .map(|r| r.now))
}
/// The provider ends or pauses it after `lifetime_secs` unless renewed.
pub async fn sandbox_launched_execute(
    db: &impl GenericClient,
    id: Uuid,
    provider_sandbox_id: &str,
    lifetime_secs: f64,
) -> DbResult<u64> {
    Ok(q::sandbox_launched::run()
        .bind(db, &provider_sandbox_id, &lifetime_secs, &id)
        .await?)
}
pub async fn sandbox_renewed_execute(
    db: &impl GenericClient,
    id: Uuid,
    lifetime_secs: f64,
) -> DbResult<u64> {
    Ok(q::sandbox_renewed::run()
        .bind(db, &lifetime_secs, &id)
        .await?)
}
/// Every unleased sandbox due for renewal, or whether `leased` (held by the caller) still is.
pub async fn renew_due_all(
    db: &impl GenericClient,
    leased: Option<Uuid>,
) -> DbResult<Vec<RenewRow>> {
    Ok(q::renew_due::run().bind(db, &leased).all().await?)
}
pub async fn sandbox_enrollment_execute(
    db: &impl GenericClient,
    id: Uuid,
    hash: &[u8],
) -> DbResult<u64> {
    Ok(q::sandbox_enrollment::run().bind(db, &hash, &id).await?)
}
pub async fn sandbox_settle_execute(
    db: &impl GenericClient,
    id: Uuid,
    status: &str,
    error: &str,
) -> DbResult<u64> {
    Ok(q::sandbox_settle::run()
        .bind(db, &status, &error, &id)
        .await?)
}
pub async fn sandbox_slept_execute(
    db: &impl GenericClient,
    id: Uuid,
    provider_sandbox_id: Option<&str>,
    snapshot: Option<&str>,
) -> DbResult<u64> {
    Ok(q::sandbox_slept::run()
        .bind(db, &provider_sandbox_id, &snapshot, &id)
        .await?)
}
/// Used now by `invocation`.
pub async fn sandbox_touch_execute(
    db: &impl GenericClient,
    id: Uuid,
    invocation: Uuid,
) -> DbResult<u64> {
    Ok(q::sandbox_touch::run().bind(db, &invocation, &id).await?)
}
pub async fn sandbox_delete_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::sandbox_delete::run().bind(db, &id).await?)
}
pub async fn sandbox_enroll_opt(
    db: &impl GenericClient,
    enrollment: &[u8],
    session: &[u8],
) -> DbResult<Option<EnrolledRow>> {
    Ok(q::sandbox_enroll::run()
        .bind(db, &session, &enrollment)
        .opt()
        .await?)
}
pub async fn sandbox_authenticate_opt(
    db: &impl GenericClient,
    session: &[u8],
) -> DbResult<Option<SessionRow>> {
    Ok(q::sandbox_authenticate::run()
        .bind(db, &session)
        .opt()
        .await?)
}
pub async fn sandbox_connected_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::sandbox_connected::run().bind(db, &id).await?)
}
pub async fn sweep_lock_one(db: &impl GenericClient) -> DbResult<bool> {
    Ok(q::sweep_lock::run().bind(db).one().await?.acquired)
}
/// Every unleased sandbox due, or whether `leased` (held by the caller) still is.
pub async fn sweep_due_all(db: &impl GenericClient, leased: Option<Uuid>) -> DbResult<Vec<DueRow>> {
    Ok(q::sweep_due::run().bind(db, &leased).all().await?)
}
pub async fn sandbox_release_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::sandbox_release::run().bind(db, &id).await?)
}
/// `trace` is the agent tool call's trace context, which tools called while it runs nest under.
pub async fn call_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    sandbox: Uuid,
    operation: &str,
    input_json: &str,
    invocation: Uuid,
    trace: &(String, String),
) -> DbResult<u64> {
    Ok(q::call_insert::run()
        .bind(
            db,
            &id,
            &sandbox,
            &operation,
            &input_json,
            &invocation,
            &trace.0,
            &trace.1,
        )
        .await?)
}
pub async fn call_claim_all(db: &impl GenericClient, sandbox: Uuid) -> DbResult<Vec<CallRow>> {
    Ok(q::call_claim::run().bind(db, &sandbox).all().await?)
}
pub async fn call_finish_execute(
    db: &impl GenericClient,
    id: Uuid,
    sandbox: Uuid,
    status: &str,
    output_json: &str,
    error: &str,
) -> DbResult<u64> {
    Ok(q::call_finish::run()
        .bind(db, &status, &output_json, &error, &id, &sandbox)
        .await?)
}
pub async fn call_take_opt(db: &impl GenericClient, id: Uuid) -> DbResult<Option<CallResultRow>> {
    Ok(q::call_take::run().bind(db, &id).opt().await?)
}
pub async fn call_abandon_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::call_abandon::run().bind(db, &id).await?)
}
pub async fn calls_expire_execute(db: &impl GenericClient) -> DbResult<u64> {
    Ok(q::calls_expire::run().bind(db).await?)
}
/// The invocation and trace context a call from inside the sandbox belongs to.
pub async fn trace_scope_opt(
    db: &impl GenericClient,
    sandbox: Uuid,
    operation: Option<Uuid>,
) -> DbResult<Option<TraceScopeRow>> {
    Ok(q::trace_scope::run()
        .bind(db, &operation, &sandbox)
        .opt()
        .await?)
}
