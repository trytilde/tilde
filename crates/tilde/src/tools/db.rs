//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbResult, GenericClient};
use tilde_queries::queries::tools as q;
use uuid::Uuid;

pub use q::agent_functions::Record as AgentFunctionRow;
pub use q::agent_tool_list::Record as AgentToolRow;
pub use q::source_list::Record as SourceRow;
pub use q::tool_connection::Record as ToolConnectionRow;

/// For an agent or a sandbox blueprint; `sandbox` (the agent) makes it the agent's sandbox.
pub async fn source_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    agent: Option<Uuid>,
    connection: Option<Uuid>,
    tool_host: Option<Uuid>,
    slug: &str,
    blueprint: Option<Uuid>,
    sandbox: Option<Uuid>,
) -> DbResult<u64> {
    Ok(q::source_insert::run()
        .bind(
            db,
            &id,
            &agent,
            &connection,
            &tool_host,
            &slug,
            &blueprint,
            &sandbox,
        )
        .await?)
}
pub async fn source_list_all(
    db: &impl GenericClient,
    id: Option<Uuid>,
    agent: Option<Uuid>,
    connection: Option<Uuid>,
    tool_host: Option<Uuid>,
    blueprint: Option<Uuid>,
) -> DbResult<Vec<SourceRow>> {
    Ok(q::source_list::run()
        .bind(db, &id, &agent, &connection, &tool_host, &blueprint)
        .all()
        .await?)
}
pub async fn source_of_agent_opt(
    db: &impl GenericClient,
    agent: Option<Uuid>,
    connection: Option<Uuid>,
    tool_host: Option<Uuid>,
    blueprint: Option<Uuid>,
) -> DbResult<Option<Uuid>> {
    Ok(q::source_of_agent::run()
        .bind(db, &agent, &blueprint, &connection, &tool_host)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn tool_mode_get_opt(db: &impl GenericClient, agent: Uuid) -> DbResult<Option<String>> {
    Ok(q::tool_mode_get::run()
        .bind(db, &agent)
        .opt()
        .await?
        .map(|r| r.tool_mode))
}
pub async fn tool_mode_set_execute(
    db: &impl GenericClient,
    agent: Uuid,
    mode: &str,
) -> DbResult<u64> {
    Ok(q::tool_mode_set::run().bind(db, &mode, &agent).await?)
}
pub async fn source_delete_execute(db: &impl GenericClient, id: Uuid) -> DbResult<u64> {
    Ok(q::source_delete::run().bind(db, &id).await?)
}
pub async fn agent_tool_set_execute(
    db: &impl GenericClient,
    source: Uuid,
    tool_name: &str,
    name: &str,
    settings: &super::ToolSettings,
) -> DbResult<u64> {
    Ok(q::agent_tool_set::run()
        .bind(
            db,
            &source,
            &tool_name,
            &name,
            &settings.is_async,
            &settings.summary,
            &settings.description,
            &crate::chat::tools::audit::display_text(settings.display),
        )
        .await?)
}
pub async fn agent_tool_remove_execute(
    db: &impl GenericClient,
    source: Uuid,
    tool_name: &str,
) -> DbResult<u64> {
    Ok(q::agent_tool_remove::run()
        .bind(db, &source, &tool_name)
        .await?)
}
pub async fn agent_tool_list_all(
    db: &impl GenericClient,
    sources: &[Uuid],
) -> DbResult<Vec<AgentToolRow>> {
    Ok(q::agent_tool_list::run().bind(db, &sources).all().await?)
}
pub async fn tool_connection_opt(
    db: &impl GenericClient,
    connection: Uuid,
) -> DbResult<Option<ToolConnectionRow>> {
    Ok(q::tool_connection::run()
        .bind(db, &connection)
        .opt()
        .await?)
}
/// An agent's or a sandbox blueprint's.
pub async fn agent_functions_all(
    db: &impl GenericClient,
    agent: Option<Uuid>,
    blueprint: Option<Uuid>,
) -> DbResult<Vec<AgentFunctionRow>> {
    Ok(q::agent_functions::run()
        .bind(db, &agent, &blueprint)
        .all()
        .await?)
}
pub use q::agent_sandbox_functions::Record as AgentSandboxFunctionRow;
pub async fn agent_sandbox_functions_all(
    db: &impl GenericClient,
    agent: Uuid,
) -> DbResult<Vec<AgentSandboxFunctionRow>> {
    Ok(q::agent_sandbox_functions::run()
        .bind(db, &agent)
        .all()
        .await?)
}

pub use q::agent_host_functions::Record as AgentHostFunctionRow;
pub use q::host_call_claim::Record as HostCallRow;
pub use q::host_call_take::Record as HostCallResultRow;
pub use q::host_list::Record as HostRow;
pub use q::host_tools::Record as HostToolRow;

/// An agent's or a sandbox blueprint's.
pub async fn agent_host_functions_all(
    db: &impl GenericClient,
    agent: Option<Uuid>,
    liveness_secs: f64,
    blueprint: Option<Uuid>,
) -> DbResult<Vec<AgentHostFunctionRow>> {
    Ok(q::agent_host_functions::run()
        .bind(db, &agent, &blueprint, &liveness_secs)
        .all()
        .await?)
}
pub async fn host_insert_execute(
    db: &impl GenericClient,
    id: Uuid,
    name: &str,
    execution_type: &str,
    function_arn: Option<&str>,
    token_hash: Option<&[u8]>,
) -> DbResult<u64> {
    Ok(q::host_insert::run()
        .bind(db, &id, &name, &execution_type, &function_arn, &token_hash)
        .await?)
}
pub async fn host_list_all(
    db: &impl GenericClient,
    id: Option<Uuid>,
    liveness_secs: f64,
    search: Option<&str>,
    with_provider: Option<bool>,
) -> DbResult<Vec<HostRow>> {
    Ok(q::host_list::run()
        .bind(db, &liveness_secs, &id, &search, &with_provider)
        .all()
        .await?)
}
pub async fn host_tools_all(db: &impl GenericClient, hosts: &[Uuid]) -> DbResult<Vec<HostToolRow>> {
    Ok(q::host_tools::run().bind(db, &hosts).all().await?)
}
pub async fn host_tools_clear_execute(db: &impl GenericClient, host: Uuid) -> DbResult<u64> {
    Ok(q::host_tools_clear::run().bind(db, &host).await?)
}
pub async fn host_tool_insert_execute(
    db: &impl GenericClient,
    host: Uuid,
    name: &str,
    description: &str,
    summary: &str,
    input_schema_json: &str,
    output_schema_json: &str,
    read_only: bool,
    destructive: bool,
    idempotent: bool,
    open_world: bool,
) -> DbResult<u64> {
    Ok(q::host_tool_insert::run()
        .bind(
            db,
            &host,
            &name,
            &description,
            &summary,
            &input_schema_json,
            &output_schema_json,
            &read_only,
            &destructive,
            &idempotent,
            &open_world,
        )
        .await?)
}
pub async fn host_authenticate_opt(db: &impl GenericClient, hash: &[u8]) -> DbResult<Option<Uuid>> {
    Ok(q::host_authenticate::run()
        .bind(db, &hash)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn host_token_execute(db: &impl GenericClient, host: Uuid, hash: &[u8]) -> DbResult<u64> {
    Ok(q::host_token::run().bind(db, &hash, &host).await?)
}
pub async fn host_connected_execute(db: &impl GenericClient, host: Uuid) -> DbResult<u64> {
    Ok(q::host_connected::run().bind(db, &host).await?)
}
pub async fn host_disconnected_execute(db: &impl GenericClient, host: Uuid) -> DbResult<u64> {
    Ok(q::host_disconnected::run().bind(db, &host).await?)
}
pub use q::host_health_history::Record as HostHealthHourRow;
pub async fn host_health_sample_execute(
    db: &impl GenericClient,
    at: chrono::DateTime<chrono::Utc>,
    liveness_secs: f64,
) -> DbResult<u64> {
    Ok(q::host_health_sample::run()
        .bind(db, &at, &liveness_secs)
        .await?)
}
pub async fn host_health_cleanup_execute(
    db: &impl GenericClient,
    now: chrono::DateTime<chrono::Utc>,
) -> DbResult<u64> {
    Ok(q::host_health_cleanup::run().bind(db, &now).await?)
}
pub async fn host_health_history_all(
    db: &impl GenericClient,
    hosts: &[Uuid],
    now: chrono::DateTime<chrono::Utc>,
) -> DbResult<Vec<HostHealthHourRow>> {
    Ok(q::host_health_history::run()
        .bind(db, &now, &hosts)
        .all()
        .await?)
}
pub use q::mcp_health_history::Record as McpHealthHourRow;
pub use q::mcp_servers::Record as McpServerRow;
pub async fn mcp_servers_all(db: &impl GenericClient) -> DbResult<Vec<McpServerRow>> {
    Ok(q::mcp_servers::run().bind(db).all().await?)
}
pub async fn mcp_health_lock_one(db: &impl GenericClient) -> DbResult<bool> {
    Ok(q::mcp_health_lock::run().bind(db).one().await?.acquired)
}
pub async fn mcp_health_sample_execute(
    db: &impl GenericClient,
    provider: &str,
    at: chrono::DateTime<chrono::Utc>,
    healthy: bool,
) -> DbResult<u64> {
    Ok(q::mcp_health_sample::run()
        .bind(db, &provider, &at, &healthy)
        .await?)
}
pub async fn mcp_health_history_all(
    db: &impl GenericClient,
    providers: &[String],
    now: chrono::DateTime<chrono::Utc>,
) -> DbResult<Vec<McpHealthHourRow>> {
    Ok(q::mcp_health_history::run()
        .bind(db, &now, &providers)
        .all()
        .await?)
}
pub async fn mcp_health_cleanup_execute(
    db: &impl GenericClient,
    now: chrono::DateTime<chrono::Utc>,
) -> DbResult<u64> {
    Ok(q::mcp_health_cleanup::run().bind(db, &now).await?)
}
pub use q::host_of_provider::Record as ProviderHostRow;
pub async fn host_of_provider_opt(
    db: &impl GenericClient,
    provider: &str,
    liveness_secs: f64,
) -> DbResult<Option<ProviderHostRow>> {
    Ok(q::host_of_provider::run()
        .bind(db, &liveness_secs, &provider)
        .opt()
        .await?)
}
/// Removes the host's provider definition details and its instances; returns the instances.
pub async fn host_provider_remove_execute(db: &impl GenericClient, host: Uuid) -> DbResult<u64> {
    Ok(q::host_provider_remove::run().bind(db, &host).await?)
}
pub async fn host_delete_execute(db: &impl GenericClient, host: Uuid) -> DbResult<u64> {
    Ok(q::host_delete::run().bind(db, &host).await?)
}
/// One queued item for a connected host: a tool call, or the verify of an instance's credentials.
pub struct HostCallInsert<'a> {
    pub id: Uuid,
    pub host: Uuid,
    pub kind: &'a str,
    pub name: &'a str,
    pub input_json: &'a str,
    pub agent: Option<Uuid>,
    pub thread: Option<Uuid>,
    pub connection: Option<Uuid>,
    /// Sealed with the row's ID; deleted with the row.
    pub credentials: Option<&'a [u8]>,
    /// The sandbox whose process made the call.
    pub sandbox: Option<Uuid>,
}
pub async fn host_call_insert_execute(
    db: &impl GenericClient,
    call: HostCallInsert<'_>,
) -> DbResult<u64> {
    Ok(q::host_call_insert::run()
        .bind(
            db,
            &call.id,
            &call.host,
            &call.kind,
            &call.name,
            &call.input_json,
            &call.agent,
            &call.thread,
            &call.connection,
            &call.credentials,
            &call.sandbox,
        )
        .await?)
}
pub async fn host_call_claim_all(
    db: &impl GenericClient,
    host: Uuid,
) -> DbResult<Vec<HostCallRow>> {
    Ok(q::host_call_claim::run().bind(db, &host).all().await?)
}
pub async fn host_call_finish_execute(
    db: &impl GenericClient,
    call: Uuid,
    host: Uuid,
    status: &str,
    output_json: &str,
    error: &str,
    account_label: &str,
) -> DbResult<u64> {
    Ok(q::host_call_finish::run()
        .bind(
            db,
            &status,
            &account_label,
            &output_json,
            &error,
            &call,
            &host,
        )
        .await?)
}
pub async fn host_call_take_opt(
    db: &impl GenericClient,
    call: Uuid,
) -> DbResult<Option<HostCallResultRow>> {
    Ok(q::host_call_take::run().bind(db, &call).opt().await?)
}
pub async fn host_call_abandon_execute(db: &impl GenericClient, call: Uuid) -> DbResult<u64> {
    Ok(q::host_call_abandon::run().bind(db, &call).await?)
}
pub async fn host_calls_expire_execute(db: &impl GenericClient) -> DbResult<u64> {
    Ok(q::host_calls_expire::run().bind(db).await?)
}

pub use q::tool_result::Record as ToolResultRow;
pub async fn tool_result_opt(
    db: &impl GenericClient,
    call: Uuid,
    thread: Uuid,
    agent: Uuid,
) -> DbResult<Option<ToolResultRow>> {
    Ok(q::tool_result::run()
        .bind(db, &call, &thread, &agent)
        .opt()
        .await?)
}
pub async fn call_invocation_opt(
    db: &impl GenericClient,
    invocation: Uuid,
) -> DbResult<Option<String>> {
    Ok(q::call_invocation::run()
        .bind(db, &invocation)
        .opt()
        .await?
        .map(|r| r.status))
}

pub use q::agents_visible::Record as VisibleAgentRow;
pub use q::run_messages::Record as RunMessageRow;
pub async fn agents_visible_all(db: &impl GenericClient) -> DbResult<Vec<VisibleAgentRow>> {
    Ok(q::agents_visible::run().bind(db).all().await?)
}
pub async fn run_messages_all(
    db: &impl GenericClient,
    run: Uuid,
    thread: Uuid,
) -> DbResult<Vec<RunMessageRow>> {
    Ok(q::run_messages::run().bind(db, &run, &thread).all().await?)
}

pub use q::invocation_user::Record as InvocationUserRow;
pub use q::personal_connections::Record as PersonalConnectionRow;
pub use q::tool_provider_types::Record as ToolProviderTypeRow;
pub async fn invocation_user_opt(
    db: &impl GenericClient,
    run: Uuid,
    thread: Uuid,
) -> DbResult<Option<InvocationUserRow>> {
    Ok(q::invocation_user::run()
        .bind(db, &thread, &run)
        .opt()
        .await?)
}
pub async fn personal_connections_all(
    db: &impl GenericClient,
    user: Uuid,
    root: Option<Uuid>,
) -> DbResult<Vec<PersonalConnectionRow>> {
    Ok(q::personal_connections::run()
        .bind(db, &user, &root)
        .all()
        .await?)
}
pub async fn personal_owner_set_execute(
    db: &impl GenericClient,
    connection: Uuid,
    user: Uuid,
) -> DbResult<u64> {
    Ok(q::personal_owner_set::run()
        .bind(db, &user, &connection)
        .await?)
}
pub async fn tool_provider_types_all(
    db: &impl GenericClient,
) -> DbResult<Vec<ToolProviderTypeRow>> {
    Ok(q::tool_provider_types::run().bind(db).all().await?)
}

pub use q::bundled_tools::Record as BundledToolRow;
pub use q::bundled_tools_latest::Record as BundledToolsLatestRow;
pub async fn bundled_tools_clear_execute(
    db: &impl GenericClient,
    invocation: Uuid,
) -> DbResult<u64> {
    Ok(q::bundled_tools_clear::run().bind(db, &invocation).await?)
}
pub async fn bundled_tool_insert_execute(
    db: &impl GenericClient,
    invocation: Uuid,
    tool: &crate::proto::tilde::types::v1::ToolDefinition,
    display: &str,
) -> DbResult<u64> {
    let hints = tool.annotations.as_option().cloned().unwrap_or_default();
    Ok(q::bundled_tool_insert::run()
        .bind(
            db,
            &invocation,
            &tool.name,
            &tool.description,
            &tool.summary,
            &tool.input_schema_json,
            &tool.output_schema_json,
            &hints.read_only,
            &hints.destructive,
            &hints.idempotent,
            &hints.open_world,
            &display,
        )
        .await?)
}
pub async fn bundled_tools_all(
    db: &impl GenericClient,
    invocation: Uuid,
) -> DbResult<Vec<BundledToolRow>> {
    Ok(q::bundled_tools::run().bind(db, &invocation).all().await?)
}
pub async fn bundled_tools_latest_opt(
    db: &impl GenericClient,
    agent: Uuid,
) -> DbResult<Option<BundledToolsLatestRow>> {
    Ok(q::bundled_tools_latest::run()
        .bind(db, &agent)
        .opt()
        .await?)
}

pub use q::deployment_tools::Record as DeploymentToolRow;
pub async fn deployment_tool_insert_execute(
    db: &impl GenericClient,
    deployment: Uuid,
    tool: &crate::proto::tilde::types::v1::ToolDefinition,
    display: &str,
    origin: &str,
) -> DbResult<u64> {
    let hints = tool.annotations.as_option().cloned().unwrap_or_default();
    Ok(q::deployment_tool_insert::run()
        .bind(
            db,
            &deployment,
            &tool.name,
            &tool.description,
            &tool.summary,
            &tool.input_schema_json,
            &tool.output_schema_json,
            &hints.read_only,
            &hints.destructive,
            &hints.idempotent,
            &hints.open_world,
            &display,
            &origin,
        )
        .await?)
}
pub async fn deployment_tools_all(
    db: &impl GenericClient,
    deployment: Uuid,
) -> DbResult<Vec<DeploymentToolRow>> {
    Ok(q::deployment_tools::run()
        .bind(db, &deployment)
        .all()
        .await?)
}

pub use q::agent_mcp_functions::Record as AgentMcpFunctionRow;
pub use q::connection_tools::Record as ConnectionToolRow;
pub async fn connection_tools_all(
    db: &impl GenericClient,
    connection: Uuid,
) -> DbResult<Vec<ConnectionToolRow>> {
    Ok(q::connection_tools::run()
        .bind(db, &connection)
        .all()
        .await?)
}
/// What the installation connections of a provider discovered from its
/// MCP server, one row per tool name.
pub async fn provider_mcp_tools_all(
    db: &impl GenericClient,
    provider: &str,
) -> DbResult<Vec<ConnectionToolRow>> {
    Ok(q::provider_mcp_tools::run()
        .bind(db, &provider)
        .all()
        .await?
        .into_iter()
        .map(|t| ConnectionToolRow {
            name: t.name,
            description: t.description,
            input_schema_json: t.input_schema_json,
            output_schema_json: t.output_schema_json,
            read_only: t.read_only,
            destructive: t.destructive,
            idempotent: t.idempotent,
            open_world: t.open_world,
        })
        .collect())
}
pub async fn connection_tools_clear_execute(
    db: &impl GenericClient,
    connection: Uuid,
) -> DbResult<u64> {
    Ok(q::connection_tools_clear::run()
        .bind(db, &connection)
        .await?)
}
pub async fn connection_tool_insert_execute(
    db: &impl GenericClient,
    connection: Uuid,
    name: &str,
    description: &str,
    input_schema_json: &str,
    output_schema_json: &str,
    read_only: bool,
    destructive: bool,
    idempotent: bool,
    open_world: bool,
) -> DbResult<u64> {
    Ok(q::connection_tool_insert::run()
        .bind(
            db,
            &connection,
            &name,
            &description,
            &input_schema_json,
            &output_schema_json,
            &read_only,
            &destructive,
            &idempotent,
            &open_world,
        )
        .await?)
}
pub async fn discovery_get_opt(
    db: &impl GenericClient,
    connection: Uuid,
) -> DbResult<Option<Vec<u8>>> {
    Ok(q::discovery_get::run()
        .bind(db, &connection)
        .opt()
        .await?
        .map(|r| r.toolset_hash))
}
pub async fn discovery_set_execute(
    db: &impl GenericClient,
    connection: Uuid,
    hash: &[u8],
) -> DbResult<u64> {
    Ok(q::discovery_set::run().bind(db, &connection, &hash).await?)
}
/// An agent's or a sandbox blueprint's.
pub async fn agent_mcp_functions_all(
    db: &impl GenericClient,
    agent: Option<Uuid>,
    blueprint: Option<Uuid>,
) -> DbResult<Vec<AgentMcpFunctionRow>> {
    Ok(q::agent_mcp_functions::run()
        .bind(db, &agent, &blueprint)
        .all()
        .await?)
}
