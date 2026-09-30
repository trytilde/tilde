//! Customer-hosted tool backends. A connected host dials `ToolHostService.Watch` with its token,
//! publishes its tools and receives calls as frames, answering through `Respond`; a Lambda host
//! is invoked synchronously. Calls to a connected host cross processes through the transient
//! `tool_host_calls` queue: the process serving InvokeTool inserts a row and waits, whichever
//! process holds the host's stream claims and delivers it, and Respond completes it. Commit-time
//! notifications wake both sides; the rows remain the source of truth.
//!
//! A host that needs credentials also publishes a provider definition. Its connections are the
//! host's instances: Tilde runs their setup, asks the host to verify the result, stores them and
//! sends the instance's credentials with each call. Credentials in the queue are sealed to their
//! row and never stored in plaintext.
use super::db;
use crate::chat::tools::ToolResult;
use crate::connections::{
    catalog,
    model::{Capability, Driver, OAuthClient, Values, invalid},
    service::Connections,
};
use crate::database::notifications::Notifications;
use crate::encryption::{SealedSecret, SecretBinding};
use crate::error::Error;
use crate::proto::tilde::{tool_host::v1 as wire, types::v1 as types};
use crate::services::tilde::tool_host::v1::ToolHostService;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

/// A connected host is available while its stream refreshed `connected_at` this recently.
pub(crate) const LIVENESS_SECS: f64 = 30.0;
const CALL_TIMEOUT: Duration = Duration::from_secs(120);
/// Setup waits on a verify, so it is short: a host needing longer should accept and check later.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(30);
const NOTIFY_CHANNEL: &str = "tilde_tool_host_calls";

pub struct ToolHost {
    pub id: Uuid,
    pub name: String,
    pub lambda: bool,
    pub function_arn: Option<String>,
    pub available: bool,
    pub tools: Vec<types::ToolDefinition>,
    /// The provider it published; its tools are then used through that provider's connections.
    pub provider_id: Option<String>,
    /// Names of that provider's credential methods.
    pub auth_methods: Vec<String>,
    /// Twelve hourly availability buckets, oldest first.
    pub health_history: Vec<types::AgentHealthHour>,
}
/// What a catalog entry needs to reach its host.
#[derive(Clone)]
pub(crate) struct Target {
    pub host: Uuid,
    pub function_arn: Option<String>,
    /// The instance the call is made on and the method it was set up with; its credentials go
    /// with the call.
    pub connection: Option<(Uuid, String)>,
}

#[derive(Clone)]
pub struct ToolHosts {
    connections: Connections,
    calls: Arc<Notifications>,
}
fn digest(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}
fn credential_fields(values: &Values) -> Vec<types::InputField> {
    values
        .iter()
        .map(|(key, value)| types::InputField {
            key: key.clone(),
            value: value.expose_secret().to_owned(),
            ..Default::default()
        })
        .collect()
}
fn forget(fields: &mut [types::InputField]) {
    for field in fields {
        field.value.zeroize();
    }
}
/// Credentials leave the request once it is encoded for Lambda.
fn scrub(request: &mut wire::LambdaRequest) {
    use wire::lambda_request::Request;
    match &mut request.request {
        Some(Request::Call(call)) => forget(&mut call.credentials),
        Some(Request::Verify(verify)) => forget(&mut verify.credentials),
        _ => {}
    }
}
/// A Watch frame. Credentials opened from the queue are plaintext only inside it: the framework
/// encodes it and then drops it, which clears them. The encoded bytes belong to the transport.
struct Frame(wire::WatchResponse);
impl connectrpc::Encodable<wire::WatchResponse> for Frame {
    fn encode(&self, codec: connectrpc::CodecFormat) -> Result<buffa::bytes::Bytes, ConnectError> {
        connectrpc::Encodable::<wire::WatchResponse>::encode(&self.0, codec)
    }
}
impl Drop for Frame {
    fn drop(&mut self) {
        use wire::watch_response::Frame as F;
        match &mut self.0.frame {
            Some(F::Call(call)) => forget(&mut call.credentials),
            Some(F::Verify(verify)) => forget(&mut verify.credentials),
            _ => {}
        }
    }
}
/// Queued credentials are sealed to the queue row that carries them.
fn binding(item: Uuid) -> SecretBinding<'static> {
    SecretBinding {
        resource_kind: "tool_host_call",
        resource_id: item,
        name: "credentials",
    }
}
/// Hosts describe their own tools, so the catalog's schema rules are enforced on the way in.
fn validate(tools: &[types::ToolDefinition]) -> Result<(), Error> {
    let mut names = std::collections::BTreeSet::new();
    if tools.len() > 256 {
        return Err(invalid("A tool host publishes at most 256 tools"));
    }
    for tool in tools {
        let object = |schema: &str| {
            schema.len() <= 64 * 1024
                && serde_json::from_str::<Value>(schema).is_ok_and(|v| v.is_object())
        };
        if tool.name.is_empty()
            || tool.name.len() > 128
            || tool.description.is_empty()
            || tool.description.chars().count() > super::DESCRIPTION_CHARS
            || tool.summary.chars().count() > super::SUMMARY_CHARS
            || !object(&tool.input_schema_json)
            || !(tool.output_schema_json.is_empty() || object(&tool.output_schema_json))
            || !tool.chunk_schema_json.is_empty()
            || !names.insert(tool.name.as_str())
        {
            return Err(invalid(
                "Tool host tools need a unique name, a description and JSON object schemas, without streamed input",
            ));
        }
    }
    Ok(())
}
impl ToolHosts {
    pub fn new(connections: Connections) -> Self {
        Self {
            connections,
            calls: Arc::default(),
        }
    }
    fn seal(&self, item: Uuid, values: &Values) -> Result<Vec<u8>, Error> {
        let plain: BTreeMap<&str, &str> = values
            .iter()
            .map(|(key, value)| (key.as_str(), value.expose_secret()))
            .collect();
        let json =
            SecretString::from(serde_json::to_string(&plain).map_err(|_| Error::Encryption)?);
        Ok(self
            .connections
            .crypto
            .seal(binding(item), &json)?
            .into_bytes())
    }
    /// The plaintext moves into the fields without copies; the `Frame` carrying them clears it.
    fn open(&self, item: Uuid, sealed: &[u8]) -> Result<Vec<types::InputField>, Error> {
        let json = self
            .connections
            .crypto
            .open(binding(item), SealedSecret::from_bytes(sealed)?)?;
        let values: BTreeMap<String, String> =
            serde_json::from_str(json.expose_secret()).map_err(|_| Error::Encryption)?;
        Ok(values
            .into_iter()
            .map(|(key, value)| types::InputField {
                key,
                value,
                ..Default::default()
            })
            .collect())
    }
    async fn read(
        &self,
        id: Option<Uuid>,
        search: Option<&str>,
        with_provider: Option<bool>,
    ) -> Result<Vec<ToolHost>, Error> {
        let client = self.connections.pool.get().await?;
        let rows = db::host_list_all(&client, id, LIVENESS_SECS, search, with_provider).await?;
        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        let mut tools = db::host_tools_all(&client, &ids).await?;
        let mut hours = db::host_health_history_all(&client, &ids, chrono::Utc::now()).await?;
        Ok(rows
            .into_iter()
            .map(|r| ToolHost {
                tools: tools
                    .extract_if(.., |t| t.tool_host_id == r.id)
                    .map(|t| types::ToolDefinition {
                        name: t.name,
                        provider_id: "tool_host".into(),
                        description: t.description,
                        summary: t.summary,
                        input_schema_json: t.input_schema_json,
                        output_schema_json: t.output_schema_json,
                        annotations: types::ToolAnnotations {
                            read_only: t.read_only,
                            destructive: t.destructive,
                            idempotent: t.idempotent,
                            open_world: t.open_world,
                            ..Default::default()
                        }
                        .into(),
                        ..Default::default()
                    })
                    .collect(),
                id: r.id,
                name: r.name,
                lambda: r.execution_type == "lambda",
                function_arn: r.function_arn,
                available: r.available,
                provider_id: r.provider_id,
                auth_methods: r.auth_methods,
                health_history: hours
                    .extract_if(.., |h| h.id == r.id)
                    .map(|h| types::AgentHealthHour {
                        hour_start: buffa_types::google::protobuf::Timestamp {
                            seconds: h.hour_start.timestamp(),
                            ..Default::default()
                        }
                        .into(),
                        total_checks: h.total_checks as u32,
                        failed_checks: h.failed_checks as u32,
                        ..Default::default()
                    })
                    .collect(),
            })
            .collect())
    }
    /// `search` matches the name; `with_provider` keeps hosts that publish a provider (true) or
    /// that do not (false).
    pub async fn list(
        &self,
        search: Option<&str>,
        with_provider: Option<bool>,
    ) -> Result<Vec<ToolHost>, Error> {
        self.read(None, search, with_provider).await
    }
    pub async fn get(&self, id: Uuid) -> Result<ToolHost, Error> {
        self.read(Some(id), None, None)
            .await?
            .pop()
            .ok_or(Error::NotFound)
    }
    /// A function ARN makes a Lambda host; otherwise the host is connected and its token is
    /// returned once. A Lambda host's tools are read before it is returned.
    pub async fn register(
        &self,
        name: &str,
        function_arn: Option<&str>,
    ) -> Result<(ToolHost, Option<SecretString>), Error> {
        if function_arn.is_some_and(|arn| !arn.starts_with("arn:aws:lambda:")) {
            return Err(invalid("Lambda tool hosts need a function ARN"));
        }
        let id = Uuid::new_v4();
        let token = function_arn
            .is_none()
            .then(crate::deployment::secrets::random_secret);
        let hash = token.as_ref().map(|t| digest(t.expose_secret()));
        if db::host_insert_execute(
            &self.connections.pool.get().await?,
            id,
            name,
            if function_arn.is_some() {
                "lambda"
            } else {
                "connected"
            },
            function_arn,
            hash.as_deref(),
        )
        .await?
            == 0
        {
            return Err(Error::Conflict);
        }
        let host = match function_arn {
            Some(_) => self.refresh(id).await?,
            None => self.get(id).await?,
        };
        Ok((host, token))
    }
    pub async fn rotate_token(&self, id: Uuid) -> Result<SecretString, Error> {
        let token = crate::deployment::secrets::random_secret();
        match db::host_token_execute(
            &self.connections.pool.get().await?,
            id,
            &digest(token.expose_secret()),
        )
        .await?
        {
            0 => Err(Error::NotFound),
            _ => Ok(token),
        }
    }
    /// Its provider and every instance go with it.
    pub async fn delete(&self, id: Uuid) -> Result<(), Error> {
        let mut client = self.connections.pool.get().await?;
        let tx = client.transaction().await?;
        db::host_provider_remove_execute(&tx, id).await?;
        if db::host_delete_execute(&tx, id).await? == 0 {
            return Err(Error::NotFound);
        }
        tx.commit().await?;
        Ok(())
    }
    /// Ask a Lambda host for its tools and replace the stored definitions.
    pub async fn refresh(&self, id: Uuid) -> Result<ToolHost, Error> {
        let host = self.get(id).await?;
        let arn = host
            .function_arn
            .as_deref()
            .ok_or_else(|| invalid("Connected tool hosts publish their tools when they connect"))?;
        let response = self
            .lambda(
                arn,
                wire::LambdaRequest {
                    request: Some(wire::LambdaListTools::default().into()),
                    ..Default::default()
                },
                CALL_TIMEOUT,
            )
            .await?;
        self.publish(id, response.tools, response.provider.into_option())
            .await?;
        self.get(id).await
    }
    /// Replaces the host's tools and, when it publishes one, its provider definition. A host that
    /// published a provider keeps doing so: its instances depend on it.
    async fn publish(
        &self,
        host: Uuid,
        tools: Vec<types::ToolDefinition>,
        provider: Option<types::Provider>,
    ) -> Result<(), Error> {
        validate(&tools)?;
        match provider {
            Some(provider) => self.publish_provider(host, provider).await?,
            None if self.get(host).await?.provider_id.is_some() => {
                return Err(invalid(
                    "This tool host published a provider its instances depend on; keep publishing it",
                ));
            }
            None => {}
        }
        let mut client = self.connections.pool.get().await?;
        let tx = client.transaction().await?;
        db::host_tools_clear_execute(&tx, host).await?;
        for tool in &tools {
            let hints = tool.annotations.as_option().cloned().unwrap_or_default();
            db::host_tool_insert_execute(
                &tx,
                host,
                &tool.name,
                &tool.description,
                &tool.summary,
                &tool.input_schema_json,
                &tool.output_schema_json,
                hints.read_only,
                hints.destructive,
                hints.idempotent,
                hints.open_world,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    /// The host's own vocabulary for its instances' credentials. Setup always runs on Tilde's
    /// standard page, so a custom credential source has nothing to render.
    async fn publish_provider(
        &self,
        host: Uuid,
        mut provider: types::Provider,
    ) -> Result<(), Error> {
        provider.kind = Some(types::provider::Kind::Configured(Default::default()));
        let mut provider = crate::connections::rpc::provider_model(provider)?;
        if provider.categories.is_empty() {
            provider.categories = vec!["developer_tools".into()];
        }
        for typ in &mut provider.connection_types {
            if typ.driver() == Driver::Custom {
                return Err(invalid(
                    "Tool host providers collect static credentials or use OAuth",
                ));
            }
            // The host defines these endpoints: it signs in with the person's own client, over
            // HTTPS (its token endpoint is also pinned to public addresses when called).
            if let Some(oauth) = typ.oauth() {
                let https = |raw: &str| url::Url::parse(raw).is_ok_and(|u| u.scheme() == "https");
                if oauth.client != OAuthClient::Form
                    || !https(&oauth.token_url)
                    || oauth
                        .authorization_url
                        .as_deref()
                        .is_some_and(|u| !https(u))
                {
                    return Err(invalid(
                        "Tool host providers sign in with the person's own OAuth client over HTTPS",
                    ));
                }
            }
            typ.capabilities = vec![Capability::Tool];
        }
        catalog::register(
            &self.connections.pool,
            provider,
            false,
            None,
            None,
            Some(host),
        )
        .await?;
        Ok(())
    }
    async fn lambda(
        &self,
        arn: &str,
        mut request: wire::LambdaRequest,
        timeout: Duration,
    ) -> Result<wire::LambdaResponse, Error> {
        let payload = Zeroizing::new(
            serde_json::to_vec(&request)
                .map_err(|_| Error::Invalid("Invalid tool host request".into()))?,
        );
        scrub(&mut request);
        let body = crate::deployment::wake::lambda_response(
            arn,
            &payload,
            self.connections
                .endpoints
                .0
                .get("aws_lambda_api")
                .map(String::as_str),
            timeout.as_secs(),
        )
        .await?;
        serde_json::from_slice(&body).map_err(|_| invalid("Tool host returned an invalid response"))
    }
    /// The host a provider belongs to, if any.
    pub(crate) async fn of_provider(
        &self,
        provider: &str,
    ) -> Result<Option<db::ProviderHostRow>, Error> {
        Ok(
            db::host_of_provider_opt(&self.connections.pool.get().await?, provider, LIVENESS_SECS)
                .await?,
        )
    }
    /// Ask a provider's host to accept an instance's credentials before the instance is ready.
    /// `None` when the host names no account.
    pub(crate) async fn verify(
        &self,
        host: db::ProviderHostRow,
        connection: Uuid,
        connection_type: &str,
        values: &Values,
    ) -> Result<Option<String>, Error> {
        if !host.available {
            return Err(Error::CredentialsRejected(
                "The tool host is offline. Start it, then try again.".into(),
            ));
        }
        let id = Uuid::new_v4();
        let (label, error) = match &host.function_arn {
            Some(arn) => {
                let request = wire::VerifyRequest {
                    call_id: id.to_string(),
                    connection_id: connection.to_string(),
                    connection_type: connection_type.to_owned(),
                    credentials: credential_fields(values),
                    ..Default::default()
                };
                let response = self
                    .lambda(
                        arn,
                        wire::LambdaRequest {
                            request: Some(wire::lambda_request::Request::Verify(Box::new(request))),
                            ..Default::default()
                        },
                        VERIFY_TIMEOUT,
                    )
                    .await?;
                (response.account_label.unwrap_or_default(), response.error)
            }
            None => {
                let sealed = self.seal(id, values)?;
                self.enqueue(
                    db::HostCallInsert {
                        id,
                        host: host.id,
                        kind: "verify",
                        name: "",
                        input_json: "",
                        agent: None,
                        thread: None,
                        connection: Some(connection),
                        credentials: Some(&sealed),
                    },
                    VERIFY_TIMEOUT,
                )
                .await?
                .ok_or_else(|| {
                    Error::CredentialsRejected(
                        "The tool host did not answer in time. Try again.".into(),
                    )
                })?
            }
        };
        if !error.is_empty() {
            return Err(Error::CredentialsRejected(
                error.chars().take(512).collect(),
            ));
        }
        let label: String = label.trim().chars().take(200).collect();
        Ok((!label.is_empty()).then_some(label))
    }
    /// Run one call on the host and wait for its outcome. A host error is the tool's failure;
    /// its message belongs to the customer and is safe to return to their agent.
    pub(crate) async fn call(
        &self,
        target: &Target,
        mut request: wire::ToolCallRequest,
    ) -> ToolResult<Value> {
        // Resolved per call, so OAuth instances are refreshed here and never by the host.
        let values = match &target.connection {
            Some((connection, _)) => Some(self.connections.resolve(*connection).await?),
            None => None,
        };
        let (output, error) = match &target.function_arn {
            Some(arn) => {
                if let (Some(values), Some((connection, method))) = (&values, &target.connection) {
                    request.connection_id = Some(connection.to_string());
                    request.connection_type = Some(method.clone());
                    request.credentials = credential_fields(values);
                }
                let response = self
                    .lambda(
                        arn,
                        wire::LambdaRequest {
                            request: Some(wire::lambda_request::Request::Call(Box::new(request))),
                            ..Default::default()
                        },
                        CALL_TIMEOUT,
                    )
                    .await?;
                (response.output_json, response.error)
            }
            None => {
                // The invocation's catalog is cached, so a host that has since gone away is
                // refused here rather than left to time out.
                let live = db::host_list_all(
                    &self.connections.pool.get().await.map_err(Error::from)?,
                    Some(target.host),
                    LIVENESS_SECS,
                    None,
                    None,
                )
                .await
                .map_err(Error::from)?
                .first()
                .is_some_and(|host| host.available);
                if !live {
                    return Err(ConnectError::unavailable("The tool host is offline"));
                }
                let id = crate::chat::id(&request.call_id)?;
                let sealed = values.as_ref().map(|v| self.seal(id, v)).transpose()?;
                self.enqueue(
                    db::HostCallInsert {
                        id,
                        host: target.host,
                        kind: "call",
                        name: &request.name,
                        input_json: &request.input_json,
                        agent: Some(crate::chat::id(&request.agent_id)?),
                        thread: Some(crate::chat::id(&request.thread_id)?),
                        connection: target.connection.as_ref().map(|(id, _)| *id),
                        credentials: sealed.as_deref(),
                    },
                    CALL_TIMEOUT,
                )
                .await?
                .ok_or_else(|| {
                    ConnectError::deadline_exceeded("Tool host did not respond in time")
                })?
            }
        };
        if !error.is_empty() {
            return Err(ConnectError::unknown(
                error.chars().take(2048).collect::<String>(),
            ));
        }
        serde_json::from_str(&output)
            .map_err(|_| ConnectError::unknown("Tool host returned invalid output JSON"))
    }
    /// Queue work for a connected host and wait for its answer: `None` once `timeout` passes.
    async fn enqueue(
        &self,
        item: db::HostCallInsert<'_>,
        timeout: Duration,
    ) -> Result<Option<(String, String)>, Error> {
        let pool = &self.connections.pool;
        let id = item.id;
        // Subscribe before the insert so the completion cannot be missed.
        let mut changes = self.calls.subscribe(pool, NOTIFY_CHANNEL).await?;
        changes.borrow_and_update();
        db::host_call_insert_execute(&pool.get().await?, item).await?;
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(done) = db::host_call_take_opt(&pool.get().await?, id).await? {
                return Ok(Some((done.output_json, done.error)));
            }
            // The slow tick covers a notification lost to a listener reconnect.
            let tick = tokio::time::sleep(Duration::from_secs(5));
            tokio::select! {
                _ = changes.changed() => { changes.borrow_and_update(); }
                _ = tick => {}
                _ = tokio::time::sleep_until(deadline) => {
                    // The host may still run it; without the row its late Respond finds nothing.
                    db::host_call_abandon_execute(&pool.get().await?, id).await?;
                    return Ok(None);
                }
            }
        }
    }
    /// Calls abandoned by a crashed waiter are removed by age.
    pub async fn expire(&self) -> Result<(), Error> {
        db::host_calls_expire_execute(&self.connections.pool.get().await?).await?;
        Ok(())
    }
}

/// Served on the agent runtime listener; tool host tokens authenticate it, not invocation tokens.
pub fn router(hosts: ToolHosts) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(hosts))),
        4 * 1024 * 1024,
    )
}
struct Rpc(ToolHosts);
impl Rpc {
    async fn host(&self, ctx: &RequestContext) -> Result<Uuid, ConnectError> {
        let token = ctx
            .headers()
            .get(http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or_else(|| ConnectError::unauthenticated("Tool host token required"))?;
        db::host_authenticate_opt(
            &self.0.connections.pool.get().await.map_err(Error::from)?,
            &digest(token),
        )
        .await
        .map_err(Error::from)?
        .ok_or_else(|| ConnectError::unauthenticated("Invalid tool host token"))
    }
}
impl ToolHostService for Rpc {
    async fn watch(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::WatchRequest>,
    ) -> ServiceResult<
        connectrpc::ServiceStream<impl connectrpc::Encodable<wire::WatchResponse> + Send + use<>>,
    > {
        let host = self.host(&ctx).await?;
        let request = request.to_owned_message();
        self.0
            .publish(host, request.tools, request.provider.into_option())
            .await?;
        let service = self.0.clone();
        let mut changes = service
            .calls
            .subscribe(&service.connections.pool, NOTIFY_CHANNEL)
            .await
            .map_err(Error::from)?;
        changes.borrow_and_update();
        let token_hash = ctx
            .headers()
            .get(http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(digest)
            .unwrap_or_default();
        // Available before the host hears it is registered, so it can be called at once.
        db::host_connected_execute(
            &service.connections.pool.get().await.map_err(Error::from)?,
            host,
        )
        .await
        .map_err(Error::from)?;
        Response::stream_ok(async_stream::try_stream! {
            yield Frame(wire::WatchResponse { frame: Some(wire::HostRegistered { tool_host_id: host.to_string(), ..Default::default() }.into()), ..Default::default() });
            loop {
                let client = service.connections.pool.get().await.map_err(Error::from)?;
                // A rotated token or deleted host ends the stream at its next turn.
                if db::host_authenticate_opt(&client, &token_hash).await.map_err(Error::from)? != Some(host) { break; }
                db::host_connected_execute(&client, host).await.map_err(Error::from)?;
                let calls = db::host_call_claim_all(&client, host).await.map_err(Error::from)?;
                drop(client);
                for call in calls {
                    let credentials = match &call.credentials {
                        Some(sealed) => service.open(call.id, sealed)?,
                        None => vec![],
                    };
                    let connection_id = call.connection_id.map(|c| c.to_string());
                    let frame = if call.kind == "verify" {
                        wire::watch_response::Frame::Verify(Box::new(wire::VerifyRequest {
                            call_id: call.id.to_string(), connection_id: connection_id.unwrap_or_default(),
                            connection_type: call.connection_type.clone().unwrap_or_default(), credentials, ..Default::default()
                        }))
                    } else {
                        wire::watch_response::Frame::Call(Box::new(wire::ToolCallRequest {
                            call_id: call.id.to_string(), name: call.name, input_json: call.input_json,
                            agent_id: call.agent_id.map(|a| a.to_string()).unwrap_or_default(),
                            thread_id: call.thread_id.map(|t| t.to_string()).unwrap_or_default(),
                            connection_id, connection_type: call.connection_type, credentials, ..Default::default()
                        }))
                    };
                    yield Frame(wire::WatchResponse { frame: Some(frame), ..Default::default() });
                }
                tokio::select! {
                    changed = changes.changed() => { if changed.is_err() { break; } changes.borrow_and_update(); }
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {
                        yield Frame(wire::WatchResponse { frame: Some(wire::HostPing::default().into()), ..Default::default() });
                    }
                }
            }
        })
    }
    async fn respond<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::RespondRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::RespondResponse> + Send + use<'a>> {
        let host = self.host(&ctx).await?;
        let request = request.to_owned_message();
        let (status, output, error) = match request.outcome {
            // Too large to record: the call fails with that, rather than waiting out its timeout.
            Some(wire::respond_request::Outcome::OutputJson(output))
                if output.len() > super::MAX_OUTPUT =>
            {
                ("failed", String::new(), super::OUTPUT_TOO_LARGE.into())
            }
            Some(wire::respond_request::Outcome::OutputJson(output)) => {
                if serde_json::from_str::<Value>(&output).is_err() {
                    return Err(ConnectError::invalid_argument("Output must be JSON"));
                }
                ("completed", output, String::new())
            }
            Some(wire::respond_request::Outcome::Error(error)) if !error.is_empty() => {
                ("failed", String::new(), error.chars().take(2048).collect())
            }
            _ => {
                return Err(ConnectError::invalid_argument(
                    "An output or an error is required",
                ));
            }
        };
        // Only the host the call was delivered to can complete it, and only once.
        let label: String = request
            .account_label
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect();
        if db::host_call_finish_execute(
            &self.0.connections.pool.get().await.map_err(Error::from)?,
            crate::chat::id(&request.call_id)?,
            host,
            status,
            &output,
            &error,
            &label,
        )
        .await
        .map_err(Error::from)?
            == 0
        {
            return Err(ConnectError::not_found("No delivered call with this ID"));
        }
        Response::ok(wire::RespondResponse::default())
    }
}
