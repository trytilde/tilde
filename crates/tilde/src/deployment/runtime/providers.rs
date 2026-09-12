//! Provider wire formats and tool implementations are shared with the gateway.
use super::*;
use crate::chat::{
    providers::{self, Access},
    tools::{Context, InputStream, Provider, ToolResult},
};
use connectrpc::ConnectError;
use futures::{StreamExt, future::BoxFuture};
use zeroize::Zeroize;
pub struct LocalChannels(pub Arc<Runtime>);
impl Runtime {
    pub async fn connection_access(
        &self,
        connection: Uuid,
    ) -> ToolResult<(String, String, Access)> {
        let mut config = self.configuration().await?;
        let selected = config
            .connections
            .iter_mut()
            .find(|c| c.id == connection.to_string() && c.status == "ready")
            .ok_or_else(|| {
                ConnectError::permission_denied("Connection is not ready and assigned")
            })?;
        let mut values = crate::connections::model::Values::new();
        for field in &mut selected.credentials {
            values.insert(
                field.name.clone(),
                SecretString::from(std::mem::take(&mut field.value)),
            );
        }
        let provider = selected.provider_id.clone();
        let typ = selected.type_id.clone();
        clear_secrets(&mut config);
        let access = Access {
            connection_id: connection,
            values,
            http: crate::connections::oauth::Http::new()?,
            endpoints: Default::default(),
        };
        Ok((provider, typ, access))
    }
}
pub(crate) fn clear_secrets(config: &mut control::GetConfigurationResponse) {
    config.webhook_signing_key.zeroize();
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
            let mut config = self.0.configuration().await?;
            let mut tools = vec![];
            for connection in &config.connections {
                if connection.status != "ready" {
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
            clear_secrets(&mut config);
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
