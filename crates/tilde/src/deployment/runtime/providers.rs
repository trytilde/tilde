//! Provider wire formats and tool implementations are shared with the gateway.
//! Credentials come from the replicated configuration held in memory.
use super::*;
use crate::chat::{
    providers::{self, Access},
    tools::{Context, InputStream, Provider, ToolResult},
};
use connectrpc::ConnectError;
use futures::{StreamExt, future::BoxFuture};
use secrecy::ExposeSecret;
use serde_json::{Value, json};
pub struct LocalChannels(pub Arc<Runtime>);
impl Runtime {
    pub async fn connection_access(
        &self,
        connection: Uuid,
    ) -> ToolResult<(String, String, Access)> {
        self.ready_access(&connection.to_string(), Some(types::Capability::Channel))
    }
    /// A ready replicated connection's provider, type and credentials, optionally only when it
    /// is replicated with `capability`. A tool's connection may be replicated as a channel or
    /// for inference instead of as a tool.
    fn ready_access(
        &self,
        connection: &str,
        capability: Option<types::Capability>,
    ) -> ToolResult<(String, String, Access)> {
        let config = self.configuration()?;
        let selected = config
            .connections
            .iter()
            .find(|c| {
                c.id == connection
                    && c.status == "ready"
                    && capability.is_none_or(|capability| c.capability == capability)
            })
            .ok_or_else(|| {
                ConnectError::permission_denied("Connection is not ready and assigned")
            })?;
        let mut values = crate::connections::model::Values::new();
        for field in &selected.credentials {
            values.insert(field.name.clone(), SecretString::from(field.value.clone()));
        }
        let access = Access {
            connection_id: id(connection)?,
            values,
            http: crate::connections::oauth::Http::new()?,
            endpoints: self.endpoints.clone(),
            oidc: None,
        };
        Ok((
            selected.provider_id.clone(),
            selected.type_id.clone(),
            access,
        ))
    }
}
pub(crate) fn clear_secrets(config: &mut control::GetConfigurationResponse) {
    for connection in &mut config.connections {
        for field in &mut connection.credentials {
            field.value.zeroize();
        }
    }
}
impl Provider for LocalChannels {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            if scope.agent_id != self.0.agent_id {
                return Err(ConnectError::permission_denied("Wrong agent"));
            }
            let config = self.0.configuration()?;
            let mut tools = vec![];
            for connection in &config.connections {
                if connection.status != "ready"
                    || connection.capability != types::Capability::Channel
                {
                    continue;
                }
                if let Some(provider) =
                    providers::adapter(&connection.provider_id, &connection.type_id)
                {
                    for mut tool in provider.tools() {
                        tool.name =
                            format!("channel_{}.{}", id(&connection.id)?.simple(), tool.name);
                        tool.provider_id = connection.provider_id.clone();
                        tool.description = format!(
                            "{} [{}]: {}",
                            connection.name, connection.account_label, tool.description
                        );
                        tools.push(tool);
                    }
                }
            }
            Ok(tools)
        })
    }
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        mut input: Value,
        mut chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let (connection, name) = name
                .strip_prefix("channel_")
                .and_then(|s| s.split_once('.'))
                .ok_or_else(|| ConnectError::not_found("Unknown channel tool"))?;
            while let Some(chunk) = chunks.next().await {
                let chunk = chunk?;
                if chunk.as_object().is_none_or(|o| o.len() != 1) {
                    return Err(ConnectError::invalid_argument("Invalid message chunk"));
                }
                let delta = chunk
                    .get("textDelta")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ConnectError::invalid_argument("Expected textDelta"))?;
                let text = input
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ConnectError::invalid_argument("Expected text"))?;
                input["text"] = json!(format!("{text}{delta}"));
            }
            context.authorize().await?;
            let (provider, typ, access) = self.0.connection_access(id(connection)?).await?;
            let provider = providers::adapter(&provider, &typ)
                .ok_or_else(|| ConnectError::not_found("No installed adapter"))?;
            let definition = provider
                .tools()
                .into_iter()
                .find(|d| d.name == name)
                .ok_or_else(|| ConnectError::not_found("Unknown provider tool"))?;
            let schema: Value = serde_json::from_str(&definition.input_schema_json)
                .map_err(|_| ConnectError::internal("Invalid tool schema"))?;
            if !jsonschema::validator_for(&schema)
                .map_err(|_| ConnectError::internal("Invalid tool schema"))?
                .is_valid(&input)
            {
                return Err(ConnectError::invalid_argument(
                    "Input does not match tool schema",
                ));
            }
            provider.invoke(&access, &context, name, input).await
        })
    }
}

/// Memory and wiki tools, answered by the gateway: both live in its Postgres, so listing and
/// every call are relayed (`tilde.runtime.v1.KnowledgeService`) and nothing is held here. Relay
/// checks the token, the thread lease and the capability ceiling as for the agent's registry
/// calls.
pub struct GatewayKnowledge(pub Arc<Runtime>);
const KNOWLEDGE_SERVICE: &str = "tilde.runtime.v1.KnowledgeService";
impl Runtime {
    /// One JSON call of a gateway tool service through the registry relay, under `token`.
    async fn relay_tools<T: serde::de::DeserializeOwned>(
        &self,
        service: &str,
        token: &SecretString,
        method: &str,
        body: Value,
    ) -> ToolResult<T> {
        let result = self
            .gateway
            .relay(
                self.instance_id,
                &format!("{service}/{method}"),
                "application/json",
                body.to_string().into_bytes(),
                token.expose_secret(),
            )
            .await?;
        let value: Value = serde_json::from_slice(&result.body).unwrap_or(Value::Null);
        if result.status != 200 {
            let code = value["code"]
                .as_str()
                .and_then(|c| c.parse().ok())
                .unwrap_or(connectrpc::ErrorCode::Unavailable);
            return Err(ConnectError::new(
                code,
                value["message"]
                    .as_str()
                    .unwrap_or("The gateway's tools are unavailable"),
            ));
        }
        serde_json::from_value(value)
            .map_err(|_| ConnectError::internal("Invalid response from the gateway"))
    }
}
impl Provider for GatewayKnowledge {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            if scope.agent_id != self.0.agent_id {
                return Err(ConnectError::permission_denied("Wrong agent"));
            }
            // The registry lists tools from the verified scope alone, so the listing goes under a
            // token minted for it; calls go under the agent's own.
            let token = self.0.relay_token(scope)?;
            // An unreachable gateway hides memory and wikis for this listing rather than every
            // tool.
            match self
                .0
                .relay_tools::<crate::proto::tilde::runtime::v1::ListKnowledgeToolsResponse>(
                    KNOWLEDGE_SERVICE,
                    &token,
                    "ListKnowledgeTools",
                    json!({}),
                )
                .await
            {
                Ok(listed) => Ok(listed.tools),
                Err(error) => {
                    tracing::warn!(%error, "Memory and wiki tools unavailable from the gateway");
                    Ok(vec![])
                }
            }
        })
    }
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        input: Value,
        mut chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            while let Some(chunk) = chunks.next().await {
                chunk?;
            }
            context.authorize().await?;
            let output = self
                .0
                .relay_tools::<crate::proto::tilde::runtime::v1::InvokeKnowledgeToolResponse>(
                    KNOWLEDGE_SERVICE,
                    context.capability(),
                    "InvokeKnowledgeTool",
                    json!({"name": name, "inputJson": input.to_string(), "callId": context.call_id}),
                )
                .await?;
            serde_json::from_str(&output.output_json)
                .map_err(|_| ConnectError::internal("Invalid tool result from the gateway"))
        })
    }
}

/// The agent's tools the sidecar runs itself (`control::SidecarTool`), from the replicated
/// configuration: offered exactly as the gateway's catalog offers them, and called with the
/// replicated credentials, which a rotation replaces with the next configuration frame.
pub struct ReplicatedTools(pub Arc<Runtime>);
impl ReplicatedTools {
    fn listed(&self, scope: &Scope, deferred: bool) -> ToolResult<Vec<types::ToolDefinition>> {
        if scope.agent_id != self.0.agent_id {
            return Err(ConnectError::permission_denied("Wrong agent"));
        }
        Ok(self
            .0
            .configuration()?
            .tools
            .iter()
            .filter(|tool| tool.deferred == deferred)
            .filter_map(|tool| tool.definition.as_option().cloned())
            .collect())
    }
}
impl Provider for ReplicatedTools {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move { self.listed(scope, false) })
    }
    fn deferred<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move { self.listed(scope, true) })
    }
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        input: Value,
        mut chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            // Managed tools take complete input; the registry already refused streamed frames.
            while let Some(chunk) = chunks.next().await {
                chunk?;
            }
            let config = self.0.configuration()?;
            let tool = config
                .tools
                .iter()
                .find(|tool| tool.definition.as_option().is_some_and(|d| d.name == name))
                .ok_or_else(|| ConnectError::not_found("Tool is not available to this agent"))?;
            crate::tools::validate(&tool.definition, &input)?;
            context.authorize().await?;
            let (provider, typ, access) = self.0.ready_access(&tool.connection_id, None)?;
            if let Some(server) = tool.mcp.as_option() {
                let server = crate::connections::model::McpServer {
                    url: server.url.clone(),
                    credential: crate::connections::model::McpCredential::from_storage(
                        &server.credential,
                        Some(server.credential_name.clone()).filter(|n| !n.is_empty()),
                        server.credential_prefix.clone(),
                    )
                    .map_err(|_| ConnectError::internal("Invalid MCP server"))?,
                };
                return crate::tools::mcp::call(
                    &access.endpoints,
                    &server,
                    &access.values,
                    &tool.tool_name,
                    input,
                )
                .await;
            }
            crate::tools::providers::provider(&provider, &typ)
                .ok_or_else(|| ConnectError::not_found("No installed tool provider"))?
                .invoke(&access, context.call_id, &tool.tool_name, input)
                .await
        })
    }
}

/// The agent's tools only the gateway can run (tool hosts, sandbox tools and other sources only the gateway
/// reaches), listed
/// and called through the relay (`tilde.runtime.v1.GatewayToolService`), which checks the token,
/// the thread lease and the capability ceiling. Nothing about them is held here.
pub struct GatewayTools {
    runtime: Arc<Runtime>,
    /// The deferred half of each invocation's last listing: the registry asks a provider for
    /// listed tools and then at once for deferred ones, and one relay answers both. The provider
    /// is shared by every invocation on this runtime.
    deferred: std::sync::Mutex<HashMap<Uuid, Vec<types::ToolDefinition>>>,
}
const GATEWAY_TOOL_SERVICE: &str = "tilde.runtime.v1.GatewayToolService";
impl GatewayTools {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            runtime,
            deferred: Default::default(),
        }
    }
    async fn list(
        &self,
        scope: &Scope,
    ) -> ToolResult<crate::proto::tilde::runtime::v1::ListGatewayToolsResponse> {
        if scope.agent_id != self.runtime.agent_id {
            return Err(ConnectError::permission_denied("Wrong agent"));
        }
        let token = self.runtime.relay_token(scope)?;
        // An unreachable gateway hides these tools for this listing rather than every tool.
        Ok(self
            .runtime
            .relay_tools(GATEWAY_TOOL_SERVICE, &token, "ListGatewayTools", json!({}))
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Gateway-run tools unavailable from the gateway");
                Default::default()
            }))
    }
}
impl Provider for GatewayTools {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            let listed = self.list(scope).await?;
            let mut deferred = self.deferred.lock().unwrap_or_else(|e| e.into_inner());
            if deferred.len() > 1024 {
                deferred.clear();
            }
            deferred.insert(scope.id, listed.deferred);
            Ok(listed.tools)
        })
    }
    fn deferred<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            let kept = self
                .deferred
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&scope.id);
            match kept {
                Some(deferred) => Ok(deferred),
                None => Ok(self.list(scope).await?.deferred),
            }
        })
    }
    fn invoke<'a>(
        &'a self,
        context: Context,
        name: &'a str,
        input: Value,
        mut chunks: InputStream,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            while let Some(chunk) = chunks.next().await {
                chunk?;
            }
            context.authorize().await?;
            let output = self
                .runtime
                .relay_tools::<crate::proto::tilde::runtime::v1::InvokeGatewayToolResponse>(
                    GATEWAY_TOOL_SERVICE,
                    context.capability(),
                    "InvokeGatewayTool",
                    json!({"name": name, "inputJson": input.to_string(), "callId": context.call_id}),
                )
                .await?;
            serde_json::from_str(&output.output_json)
                .map_err(|_| ConnectError::internal("Invalid tool result from the gateway"))
        })
    }
}
