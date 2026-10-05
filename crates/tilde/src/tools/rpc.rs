//! Management surface for agents' tools and tool hosts.
use super::{Filter, Owner, Source, Target, ToolSettings, Tools, hosts::ToolHost};
use crate::chat::tools::audit;
use crate::error::Error;
use crate::proto::tilde::management::v1 as management;
use crate::services::tilde::management::v1::{ToolHostRegistryService, ToolService};
use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::ExposeSecret;
use std::sync::Arc;
use uuid::Uuid;

struct Rpc(Tools);
struct HostRpc(super::hosts::ToolHosts);
pub fn router(tools: Tools) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new()
            .add_service(Arc::new(HostRpc(tools.hosts.clone())))
            .add_service(Arc::new(Rpc(tools))),
        256 * 1024,
    )
}
fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Invalid UUID".into()))
}
/// Exactly one of a connection and a tool host.
fn target(connection: Option<&str>, tool_host: Option<&str>) -> Result<Target, Error> {
    match (connection, tool_host) {
        (Some(c), None) => Ok(Target::Connection(id(c)?)),
        (None, Some(h)) => Ok(Target::ToolHost(id(h)?)),
        _ => Err(Error::Invalid(
            "Name exactly one connection or tool host".into(),
        )),
    }
}
fn provider_tool(t: crate::proto::tilde::types::v1::ToolDefinition) -> management::ProviderTool {
    management::ProviderTool {
        name: t.name,
        description: t.description,
        summary: t.summary,
        input_schema_json: t.input_schema_json,
        output_schema_json: t.output_schema_json,
        ..Default::default()
    }
}
fn mode(dynamic: bool) -> management::ToolMode {
    if dynamic {
        management::ToolMode::Dynamic
    } else {
        management::ToolMode::Direct
    }
}
fn wire(s: Source) -> management::ToolSource {
    let (connection_id, tool_host_id) = s.target.columns();
    let (agent_id, sandbox_blueprint_id) = s.owner.columns();
    management::ToolSource {
        id: s.id.to_string(),
        agent_id: agent_id.map(|a| a.to_string()).unwrap_or_default(),
        sandbox_blueprint_id: sandbox_blueprint_id.map(|b| b.to_string()),
        connection_id: connection_id.map(|c| c.to_string()),
        tool_host_id: tool_host_id.map(|h| h.to_string()),
        sandbox: s.target == Target::Sandbox,
        slug: s.slug,
        tools: s
            .tools
            .into_iter()
            .map(|t| management::AgentTool {
                tool_name: t.tool_name,
                name: t.name,
                is_async: t.is_async,
                display: audit::display_value(&t.display).into(),
                summary: t.summary,
                description: t.description,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}
fn host_wire(h: ToolHost) -> management::ToolHost {
    management::ToolHost {
        id: h.id.to_string(),
        name: h.name,
        r#type: if h.lambda {
            management::ToolHostType::Lambda
        } else {
            management::ToolHostType::Connected
        }
        .into(),
        function_arn: h.function_arn,
        available: h.available,
        tools: h.tools.into_iter().map(provider_tool).collect(),
        provider_id: h.provider_id,
        auth_methods: h.auth_methods,
        health_history: h.health_history,
        ..Default::default()
    }
}
impl ToolService for Rpc {
    async fn list_provider_tools<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListProviderToolsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListProviderToolsResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let search = crate::rpc::search(request.search.as_deref())?.map(str::to_lowercase);
        let tools = match (request.provider_id, request.tool_host_id) {
            _ if request.sandbox => crate::sandboxes::tools::definitions(),
            (Some(provider), None) => self.0.catalog_tools(&provider).await?,
            (None, Some(host)) => {
                let host = id(&host)?;
                self.0.hosts.get(host).await?.tools
            }
            (None, None) => {
                let connection = id(&request.connection_id)?;
                self.0.provider_tools(connection).await?
            }
            _ => return Err(Error::Invalid("Name one provider or tool host".into()).into()),
        };
        // Tools come from code, a tool host, a stored snapshot or a live MCP server, so the search
        // is applied here rather than in SQL.
        Response::ok(management::ListProviderToolsResponse {
            tools: tools
                .into_iter()
                .filter(|t| {
                    search.as_deref().is_none_or(|s| {
                        t.name.to_lowercase().contains(s)
                            || t.description.to_lowercase().contains(s)
                    })
                })
                .map(provider_tool)
                .collect(),
            ..Default::default()
        })
    }
    async fn refresh_connection_tools<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RefreshConnectionToolsRequest>,
    ) -> ServiceResult<impl Encodable<management::RefreshConnectionToolsResponse> + Send + use<'a>>
    {
        let connection = id(&request.to_owned_message().connection_id)?;
        let (tools, changed) = self.0.refresh_connection_tools(connection).await?;
        Response::ok(management::RefreshConnectionToolsResponse {
            changed,
            tools: tools.into_iter().map(provider_tool).collect(),
            ..Default::default()
        })
    }
    async fn list_tool_sources<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListToolSourcesRequest>,
    ) -> ServiceResult<impl Encodable<management::ListToolSourcesResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let filter = match (
            r.agent_id,
            r.sandbox_blueprint_id,
            r.connection_id,
            r.tool_host_id,
        ) {
            (Some(agent), None, None, None) => Filter {
                agent: Some(id(&agent)?),
                ..Default::default()
            },
            (None, Some(blueprint), None, None) => Filter {
                blueprint: Some(id(&blueprint)?),
                ..Default::default()
            },
            (None, None, connection, tool_host) => {
                let target = target(connection.as_deref(), tool_host.as_deref())?;
                let (connection, tool_host) = target.columns();
                Filter {
                    connection,
                    tool_host,
                    ..Default::default()
                }
            }
            _ => return Err(Error::Invalid("Name exactly one filter".into()).into()),
        };
        Response::ok(management::ListToolSourcesResponse {
            sources: self
                .0
                .sources(filter)
                .await?
                .into_iter()
                .map(wire)
                .collect(),
            ..Default::default()
        })
    }
    async fn add_tool_source<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::AddToolSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::AddToolSourceResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let owner = match r.sandbox_blueprint_id {
            Some(blueprint) if r.agent_id.is_empty() => Owner::Blueprint(id(&blueprint)?),
            None => Owner::Agent(id(&r.agent_id)?),
            Some(_) => {
                return Err(Error::Invalid("Name an agent or a sandbox blueprint".into()).into());
            }
        };
        let target = target(r.connection_id.as_deref(), r.tool_host_id.as_deref())?;
        let source = self.0.add_source(owner, target, &r.tool_names).await?;
        Response::ok(management::AddToolSourceResponse {
            source: wire(source).into(),
            ..Default::default()
        })
    }
    async fn get_tool_mode<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetToolModeRequest>,
    ) -> ServiceResult<impl Encodable<management::GetToolModeResponse> + Send + use<'a>> {
        let agent = id(&request.to_owned_message().agent_id)?;
        Response::ok(management::GetToolModeResponse {
            mode: mode(self.0.mode(agent).await?).into(),
            ..Default::default()
        })
    }
    async fn list_bundled_tools<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListBundledToolsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListBundledToolsResponse> + Send + use<'a>> {
        let agent = id(&request.to_owned_message().agent_id)?;
        let Some((latest, tools)) = self.0.bundled(agent).await? else {
            return Response::ok(management::ListBundledToolsResponse::default());
        };
        Response::ok(management::ListBundledToolsResponse {
            tools,
            invocation_id: Some(latest.id.to_string()),
            registered_at: crate::chat::audit::timestamp(latest.started_at).into(),
            deployment_id: latest.deployment_id.map(|d| d.to_string()),
            ..Default::default()
        })
    }
    /// One mode covers all the agent's tools; it applies from the agent's next invocation.
    async fn set_tool_mode<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetToolModeRequest>,
    ) -> ServiceResult<impl Encodable<management::SetToolModeResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let agent = id(&r.agent_id)?;
        let dynamic = r.mode == management::ToolMode::Dynamic;
        self.0.set_mode(agent, dynamic).await?;
        Response::ok(management::SetToolModeResponse {
            mode: mode(dynamic).into(),
            ..Default::default()
        })
    }
    async fn remove_tool_source<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RemoveToolSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::RemoveToolSourceResponse> + Send + use<'a>> {
        let source = self.0.source(id(&request.to_owned_message().id)?).await?;
        if source.target == Target::Sandbox {
            return Err(Error::Invalid(
                "The sandbox's tools go with the agent's sandbox; remove the sandbox instead"
                    .into(),
            )
            .into());
        }
        self.0.remove_source(source.id).await?;
        Response::ok(management::RemoveToolSourceResponse::default())
    }
    async fn set_agent_tool<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetAgentToolRequest>,
    ) -> ServiceResult<impl Encodable<management::SetAgentToolResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let source = self.0.source(id(&r.source_id)?).await?;
        let settings = ToolSettings {
            is_async: r.is_async,
            summary: r.summary,
            description: r.description,
            display: r.display,
        };
        let source = self.0.set_tool(source.id, &r.tool_name, &settings).await?;
        Response::ok(management::SetAgentToolResponse {
            source: wire(source).into(),
            ..Default::default()
        })
    }
    async fn remove_agent_tool<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RemoveAgentToolRequest>,
    ) -> ServiceResult<impl Encodable<management::RemoveAgentToolResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let source = self.0.source(id(&r.source_id)?).await?;
        let source = self.0.remove_tool(source.id, &r.tool_name).await?;
        Response::ok(management::RemoveAgentToolResponse {
            source: wire(source).into(),
            ..Default::default()
        })
    }
    async fn list_mcp_server_health<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListMcpServerHealthRequest>,
    ) -> ServiceResult<impl Encodable<management::ListMcpServerHealthResponse> + Send + use<'a>>
    {
        let providers = request.to_owned_message().provider_ids;
        let health = self.0.mcp_health(&providers).await?;
        Response::ok(management::ListMcpServerHealthResponse {
            servers: health
                .into_iter()
                .map(
                    |(provider_id, health_history)| management::McpServerHealth {
                        provider_id,
                        health_history,
                        ..Default::default()
                    },
                )
                .collect(),
            ..Default::default()
        })
    }
}
impl ToolHostRegistryService for HostRpc {
    async fn list_tool_hosts<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListToolHostsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListToolHostsResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let hosts = self
            .0
            .list(
                crate::rpc::search(request.search.as_deref())?,
                request.with_provider,
            )
            .await?;
        Response::ok(management::ListToolHostsResponse {
            tool_hosts: hosts.into_iter().map(host_wire).collect(),
            ..Default::default()
        })
    }
    async fn register_tool_host<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RegisterToolHostRequest>,
    ) -> ServiceResult<impl Encodable<management::RegisterToolHostResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let arn =
            if r.r#type == management::ToolHostType::Lambda {
                Some(r.function_arn.ok_or_else(|| {
                    Error::Invalid("Lambda tool hosts need a function ARN".into())
                })?)
            } else if r.r#type == management::ToolHostType::Connected && r.function_arn.is_none() {
                None
            } else {
                return Err(Error::Invalid("Tool host type required".into()).into());
            };
        let (host, token) = self.0.register(&r.name, arn.as_deref()).await?;
        Response::ok(management::RegisterToolHostResponse {
            tool_host: host_wire(host).into(),
            token: token.map(|t| t.expose_secret().to_owned()),
            ..Default::default()
        })
    }
    async fn rotate_tool_host_token<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RotateToolHostTokenRequest>,
    ) -> ServiceResult<impl Encodable<management::RotateToolHostTokenResponse> + Send + use<'a>>
    {
        let host = id(&request.to_owned_message().id)?;
        let token = self.0.rotate_token(host).await?;
        Response::ok(management::RotateToolHostTokenResponse {
            token: token.expose_secret().to_owned(),
            ..Default::default()
        })
    }
    async fn refresh_tool_host<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RefreshToolHostRequest>,
    ) -> ServiceResult<impl Encodable<management::RefreshToolHostResponse> + Send + use<'a>> {
        let host = id(&request.to_owned_message().id)?;
        let host = self.0.refresh(host).await?;
        Response::ok(management::RefreshToolHostResponse {
            tool_host: host_wire(host).into(),
            ..Default::default()
        })
    }
    async fn delete_tool_host<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteToolHostRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteToolHostResponse> + Send + use<'a>> {
        let host = id(&request.to_owned_message().id)?;
        self.0.delete(host).await?;
        Response::ok(management::DeleteToolHostResponse::default())
    }
}
