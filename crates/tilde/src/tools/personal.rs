//! Personal tool federation: the person an agent is conversing with connects their own accounts,
//! and an agent holding `tools.personal` for that provider may use them on that person's behalf,
//! in that person's conversations only. Nothing is added to the agent's tools; the catalog resolves
//! the invocation's user each time, so a connection follows its owner across agents and is
//! invisible everywhere else. Catalog names are `user.{provider}.{account}.{tool}`.
use super::{Tools, builtin::definition, db, providers};
use crate::chat::{
    Scope,
    providers::Access,
    tools::{Context, InputStream, Provider, ToolResult},
};
use crate::error::Error;
use crate::iam::capabilities::Capability;
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::{StreamExt, future::BoxFuture};
use serde_json::{Value, json};
use uuid::Uuid;

pub struct Personal(pub Tools);

struct Target {
    connection: Uuid,
    provider_id: String,
    type_id: String,
    tool_name: String,
    /// Set for an instance of a tool host's provider: the host runs the tool.
    host: Option<(Uuid, Option<String>)>,
    /// The type's tools are served by an MCP server.
    mcp: bool,
}
/// Account names allow characters a catalog name does not; dots would blur the segments.
fn segment(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect()
}
impl Personal {
    async fn user(&self, scope: &Scope) -> ToolResult<Option<db::InvocationUserRow>> {
        if !scope.capabilities.grants(Capability::ToolsPersonal) {
            return Ok(None);
        }
        Ok(db::invocation_user_opt(
            &self.0.connections.pool.get().await.map_err(Error::from)?,
            scope.run_id,
            scope.thread_id,
        )
        .await
        .map_err(Error::from)?)
    }
    /// Tool-capable providers this agent may use personally and that have an installed provider.
    async fn connectable(&self, scope: &Scope) -> ToolResult<Vec<db::ToolProviderTypeRow>> {
        let mut seen = std::collections::BTreeSet::new();
        Ok(
            db::tool_provider_types_all(&self.0.connections.pool.get().await.map_err(Error::from)?)
                .await
                .map_err(Error::from)?
                .into_iter()
                .filter(|t| {
                    scope
                        .capabilities
                        .permits(Capability::ToolsPersonal, &t.provider_id)
                        && (t.tool_host_id.is_some()
                            || t.mcp
                            || providers::provider(&t.provider_id, &t.type_id).is_some())
                        && seen.insert(t.provider_id.clone())
                })
                .collect(),
        )
    }
    async fn catalog(
        &self,
        scope: &Scope,
        user: &db::InvocationUserRow,
    ) -> ToolResult<Vec<(types::ToolDefinition, Target)>> {
        let rows = db::personal_connections_all(
            &self.0.connections.pool.get().await.map_err(Error::from)?,
            user.id,
            user.root_identity_id,
        )
        .await
        .map_err(Error::from)?;
        let mut names = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        for row in rows {
            if !scope
                .capabilities
                .permits(Capability::ToolsPersonal, &row.provider_id)
            {
                continue;
            }
            // A tool host's instances offer the host's tools while it can take a call.
            let (tools, host) = match row.tool_host_id {
                Some(host) => {
                    let host = self.0.hosts.get(host).await?;
                    if !host.available {
                        continue;
                    }
                    (host.tools, Some((host.id, host.function_arn)))
                }
                None if row.mcp => (
                    self.0
                        .mcp_tools(row.id, &row.provider_id, &row.type_id)
                        .await?,
                    None,
                ),
                None => match providers::provider(&row.provider_id, &row.type_id) {
                    Some(provider) => (provider.tools(), None),
                    None => continue,
                },
            };
            let mut prefix = format!("user.{}.{}", segment(&row.provider_id), segment(&row.name));
            // Two accounts may sanitize alike; the connection ID tells them apart.
            if !names.insert(prefix.clone()) {
                prefix = format!("{prefix}-{}", &row.id.simple().to_string()[..8]);
                names.insert(prefix.clone());
            }
            for mut tool in tools {
                let tool_name = std::mem::take(&mut tool.name);
                tool.name = format!("{prefix}.{tool_name}");
                tool.description = format!(
                    "On the user's own {} account {:?}: {}",
                    row.provider_id, row.name, tool.description
                );
                tool.provider_id = row.provider_id.clone();
                out.push((
                    tool,
                    Target {
                        connection: row.id,
                        provider_id: row.provider_id.clone(),
                        type_id: row.type_id.clone(),
                        tool_name,
                        host: host.clone(),
                        mcp: row.mcp,
                    },
                ));
            }
        }
        Ok(out)
    }
    async fn connect(
        &self,
        scope: &Scope,
        user: &db::InvocationUserRow,
        provider: &str,
    ) -> ToolResult<Value> {
        let typ = self
            .connectable(scope)
            .await?
            .into_iter()
            .find(|t| t.provider_id == provider)
            .ok_or_else(|| {
                ConnectError::permission_denied("This agent may not use that provider personally")
            })?;
        let connection = Uuid::new_v4();
        // The setup page collects the real account name; until then the provider names it.
        let started = self
            .0
            .connections
            .start(connection, provider, provider, &typ.type_id, &[])
            .await?;
        if db::personal_owner_set_execute(
            &self.0.connections.pool.get().await.map_err(Error::from)?,
            connection,
            user.id,
        )
        .await
        .map_err(Error::from)?
            == 0
        {
            return Err(ConnectError::internal(
                "Connection ownership could not be recorded",
            ));
        }
        Ok(json!({
            "setup_url": started.brokering_url,
            "expires_in_minutes": 10,
            "next_step": "Send this link to the user. Only they should open it; their credentials never pass through this conversation. The tools appear once they finish.",
        }))
    }
}
impl Provider for Personal {
    fn tools<'a>(
        &'a self,
        scope: &'a Scope,
    ) -> BoxFuture<'a, ToolResult<Vec<types::ToolDefinition>>> {
        Box::pin(async move {
            let Some(user) = self.user(scope).await? else {
                return Ok(vec![]);
            };
            let mut out: Vec<_> = self
                .catalog(scope, &user)
                .await?
                .into_iter()
                .map(|(d, _)| d)
                .collect();
            let providers: Vec<String> = self
                .connectable(scope)
                .await?
                .into_iter()
                .map(|t| t.provider_id)
                .collect();
            if !providers.is_empty() {
                out.push(definition(
                    "user.connect_tool",
                    "Prepared a connection link",
                    "Create a private setup link so the user can connect their own account for a provider. Returns a URL to send them.",
                    json!({"provider":{"enum":providers}}),
                    &["provider"],
                    false,
                ));
            }
            Ok(out)
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
            let scope = context.scope();
            let user = self.user(scope).await?.ok_or_else(|| {
                ConnectError::permission_denied("No verified user in this conversation")
            })?;
            if name == "user.connect_tool" {
                let provider = input["provider"]
                    .as_str()
                    .ok_or_else(|| ConnectError::invalid_argument("A provider is required"))?;
                return self.connect(scope, &user, provider).await;
            }
            // Ownership, readiness and the grant are rechecked from the catalog the list used.
            let (definition, target) = self
                .catalog(scope, &user)
                .await?
                .into_iter()
                .find(|(d, _)| d.name == name)
                .ok_or_else(|| ConnectError::not_found("Tool is not available for this user"))?;
            let schema: Value = serde_json::from_str(&definition.input_schema_json)
                .map_err(|_| ConnectError::internal("Invalid provider schema"))?;
            if !jsonschema::validator_for(&schema)
                .map_err(|_| ConnectError::internal("Invalid provider schema"))?
                .is_valid(&input)
            {
                return Err(ConnectError::invalid_argument(
                    "Input does not match this tool's schema",
                ));
            }
            if target.mcp {
                let server = self
                    .0
                    .mcp_target(target.connection, &target.provider_id, &target.type_id)
                    .await?;
                let values = self.0.connections.resolve(target.connection).await?;
                return super::mcp::call(
                    &self.0.connections.endpoints,
                    &server.server,
                    &values,
                    &target.tool_name,
                    input,
                )
                .await;
            }
            if let Some((host, function_arn)) = target.host {
                return self
                    .0
                    .hosts
                    .call(
                        &super::hosts::Target {
                            host,
                            function_arn,
                            connection: Some((target.connection, target.type_id)),
                        },
                        crate::proto::tilde::tool_host::v1::ToolCallRequest {
                            call_id: context.call_id.to_string(),
                            name: target.tool_name,
                            input_json: input.to_string(),
                            agent_id: scope.agent_id.to_string(),
                            thread_id: scope.thread_id.to_string(),
                            ..Default::default()
                        },
                    )
                    .await;
            }
            let provider = providers::provider(&target.provider_id, &target.type_id)
                .ok_or_else(|| ConnectError::not_found("No installed tool provider"))?;
            let access = Access {
                connection_id: target.connection,
                values: self.0.connections.resolve(target.connection).await?,
                http: self.0.connections.http.clone(),
                endpoints: self.0.connections.endpoints.clone(),
            };
            provider
                .invoke(&access, context.call_id, &target.tool_name, input)
                .await
        })
    }
}
