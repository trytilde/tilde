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
        let config = self.configuration()?;
        let selected = config
            .connections
            .iter()
            .find(|c| {
                c.id == connection.to_string()
                    && c.status == "ready"
                    && c.capability == types::Capability::Channel
            })
            .ok_or_else(|| {
                ConnectError::permission_denied("Connection is not ready and assigned")
            })?;
        let mut values = crate::connections::model::Values::new();
        for field in &selected.credentials {
            values.insert(field.name.clone(), SecretString::from(field.value.clone()));
        }
        let access = Access {
            connection_id: connection,
            values,
            http: crate::connections::oauth::Http::new()?,
            endpoints: Default::default(),
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

/// Memory tools, answered by the gateway: memory lives in its Postgres, so listing and every
/// call are relayed (`tilde.runtime.v1.MemoryService`) and nothing is held here. Relay checks the
/// token, the thread lease and the capability ceiling as for the agent's registry calls.
pub struct GatewayMemory(pub Arc<Runtime>);
const MEMORY_SERVICE: &str = "tilde.runtime.v1.MemoryService";
impl GatewayMemory {
    async fn relay<T: serde::de::DeserializeOwned>(
        &self,
        token: &SecretString,
        method: &str,
        body: Value,
    ) -> ToolResult<T> {
        let result = self
            .0
            .gateway
            .relay(
                self.0.instance_id,
                &format!("{MEMORY_SERVICE}/{method}"),
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
                value["message"].as_str().unwrap_or("Memory is unavailable"),
            ));
        }
        serde_json::from_value(value)
            .map_err(|_| ConnectError::internal("Invalid memory response from the gateway"))
    }
}
impl Provider for GatewayMemory {
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
            // An unreachable gateway hides memory for this listing rather than every tool.
            match self
                .relay::<crate::proto::tilde::runtime::v1::ListMemoryToolsResponse>(
                    &token,
                    "ListMemoryTools",
                    json!({}),
                )
                .await
            {
                Ok(listed) => Ok(listed.tools),
                Err(error) => {
                    tracing::warn!(%error, "Memory tools unavailable from the gateway");
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
                .relay::<crate::proto::tilde::runtime::v1::InvokeMemoryToolResponse>(
                    context.capability(),
                    "InvokeMemoryTool",
                    json!({"name": name, "inputJson": input.to_string(), "callId": context.call_id}),
                )
                .await?;
            serde_json::from_str(&output.output_json)
                .map_err(|_| ConnectError::internal("Invalid memory tool result"))
        })
    }
}
