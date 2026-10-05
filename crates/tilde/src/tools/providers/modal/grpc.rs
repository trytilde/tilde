//! Modal's gRPC API over plain HTTP/2 requests, with the few messages the sandbox tools need
//! declared by hand (field numbers from Modal's `api.proto` and `task_command_router.proto`).
//! Sandbox management goes to `modal.client.ModalClient`, authenticated with the workspace
//! token and, for sandbox calls, a short-lived auth token; commands go to the sandbox's task
//! command router with the JWT Modal issues for it. Responses are read whole (bounded), then
//! split into frames; `grpc-status` is honored from headers or trailers.
use crate::chat::{providers::Access, tools::ToolResult};
use crate::error::Error;
use connectrpc::ConnectError;
use http_body::Body as _;
use std::time::Duration;
use zeroize::Zeroizing;

const API: &str = "https://api.modal.com";
const MAX_RESPONSE: usize = 8 * 1024 * 1024;
pub(super) const UNARY_TIMEOUT: Duration = Duration::from_secs(90);

pub(super) struct Api<'a> {
    http: reqwest::Client,
    base: &'a str,
    token_id: &'a str,
    token_secret: &'a str,
    auth_token: Option<Zeroizing<String>>,
}
impl<'a> Api<'a> {
    pub(super) fn new(access: &'a Access) -> ToolResult<Self> {
        Ok(Self {
            // Modal speaks gRPC only, so HTTP/2 is assumed rather than negotiated.
            http: reqwest::Client::builder()
                .http2_prior_knowledge()
                .http2_adaptive_window(true)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| ConnectError::internal("Transport failed"))?,
            base: access
                .endpoints
                .0
                .get("modal_api")
                .map_or(API, String::as_str),
            token_id: access.secret("api_key_id")?,
            token_secret: access.secret("api_key_secret")?,
            auth_token: None,
        })
    }
    /// A `ModalClient` call; `sandbox` calls also carry the auth token.
    pub(super) async fn call<Req: prost::Message, Res: prost::Message + Default>(
        &mut self,
        method: &str,
        request: Req,
        sandbox: bool,
        timeout: Duration,
    ) -> ToolResult<Vec<Res>> {
        let auth_token = if sandbox {
            Some(self.auth_token().await?)
        } else {
            None
        };
        exchange(
            self.request(method, auth_token.as_deref().map(String::as_str))?,
            request,
            timeout,
        )
        .await
    }
    fn request(
        &self,
        method: &str,
        auth_token: Option<&str>,
    ) -> ToolResult<reqwest::RequestBuilder> {
        let mut builder = self
            .http
            .post(format!(
                "{}/modal.client.ModalClient/{method}",
                self.base.trim_end_matches('/')
            ))
            .header("x-modal-client-version", "1.1.4")
            .header("x-modal-client-type", "1")
            .header("x-modal-python-version", "3.13.3")
            .header("x-modal-node", "tilde")
            .header("x-modal-platform", "Linux")
            .header("x-modal-token-id", sensitive(self.token_id)?)
            .header("x-modal-token-secret", sensitive(self.token_secret)?);
        if let Some(token) = auth_token {
            builder = builder.header("x-modal-auth-token", sensitive(token)?);
        }
        Ok(builder)
    }
    pub(super) async fn unary<Req: prost::Message, Res: prost::Message + Default>(
        &mut self,
        method: &str,
        request: Req,
        sandbox: bool,
    ) -> ToolResult<Res> {
        self.call(method, request, sandbox, UNARY_TIMEOUT)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ConnectError::unknown("Modal returned an empty response"))
    }
    async fn auth_token(&mut self) -> ToolResult<Zeroizing<String>> {
        if let Some(token) = &self.auth_token {
            return Ok(token.clone());
        }
        let response: AuthTokenGetResponse = exchange(
            self.request("AuthTokenGet", None)?,
            AuthTokenGetRequest {},
            UNARY_TIMEOUT,
        )
        .await?
        .into_iter()
        .next()
        .unwrap_or_default();
        if response.token.is_empty() {
            return Err(ConnectError::unknown("Modal returned no auth token"));
        }
        let token = Zeroizing::new(response.token);
        self.auth_token = Some(token.clone());
        Ok(token)
    }
    /// A task command router call for one sandbox.
    pub(super) async fn router<Req: prost::Message, Res: prost::Message + Default>(
        &self,
        access: &RouterAccess,
        method: &str,
        request: Req,
        timeout: Duration,
    ) -> ToolResult<Vec<Res>> {
        let builder = self
            .http
            .post(format!(
                "{}/modal.task_command_router.TaskCommandRouter/{method}",
                access.url.trim_end_matches('/')
            ))
            .header(
                "authorization",
                sensitive(&format!("Bearer {}", access.jwt.as_str()))?,
            );
        exchange(builder, request, timeout).await
    }
}
fn sensitive(value: &str) -> ToolResult<reqwest::header::HeaderValue> {
    let mut header = reqwest::header::HeaderValue::from_str(value)
        .map_err(|_| ConnectError::invalid_argument("Invalid Modal credential"))?;
    header.set_sensitive(true);
    Ok(header)
}

async fn exchange<Req: prost::Message, Res: prost::Message + Default>(
    builder: reqwest::RequestBuilder,
    request: Req,
    timeout: Duration,
) -> ToolResult<Vec<Res>> {
    let payload = request.encode_to_vec();
    let mut body = Vec::with_capacity(payload.len() + 5);
    body.push(0);
    body.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    body.extend_from_slice(&payload);
    let response = builder
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("user-agent", "modal-client/1.1.4 (Linux; CPython/3.13.3)")
        .timeout(timeout)
        .body(body)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                ConnectError::deadline_exceeded("Modal did not answer in time")
            } else {
                ConnectError::unavailable("Modal could not be reached")
            }
        })?;
    if !response.status().is_success() {
        return Err(ConnectError::unavailable(format!(
            "Modal returned HTTP {}",
            response.status().as_u16()
        )));
    }
    let mut status = grpc_status(response.headers());
    let mut body = reqwest::Body::from(response);
    let mut bytes = Vec::new();
    while let Some(frame) =
        std::future::poll_fn(|cx| std::pin::Pin::new(&mut body).poll_frame(cx)).await
    {
        let frame =
            frame.map_err(|_| ConnectError::unavailable("The Modal response was interrupted"))?;
        match frame.into_data() {
            Ok(data) => {
                if bytes.len() + data.len() > MAX_RESPONSE {
                    return Err(ConnectError::resource_exhausted(
                        "The Modal response is larger than 8 MiB",
                    ));
                }
                bytes.extend_from_slice(&data);
            }
            Err(frame) => {
                if let Ok(trailers) = frame.into_trailers() {
                    status = status.or_else(|| grpc_status(&trailers));
                }
            }
        }
    }
    if let Some((code, message)) = status.filter(|(code, _)| *code != 0) {
        return Err(match code {
            5 => ConnectError::not_found(format!("Modal: {message}")),
            7 | 16 => Error::ConnectionAuthorizationRequired.into(),
            14 => ConnectError::unavailable(format!("Modal: {message}")),
            _ => ConnectError::unknown(format!("Modal error {code}: {message}")),
        });
    }
    let mut messages = Vec::new();
    let mut rest = bytes.as_slice();
    while !rest.is_empty() {
        let (header, tail) = rest
            .split_at_checked(5)
            .ok_or_else(|| ConnectError::unknown("Truncated Modal response"))?;
        if header[0] != 0 {
            return Err(ConnectError::unknown("Modal sent a compressed message"));
        }
        let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
        let (message, tail) = tail
            .split_at_checked(len)
            .ok_or_else(|| ConnectError::unknown("Truncated Modal response"))?;
        messages.push(
            Res::decode(message)
                .map_err(|_| ConnectError::unknown("Modal sent an invalid message"))?,
        );
        rest = tail;
    }
    Ok(messages)
}
fn grpc_status(headers: &reqwest::header::HeaderMap) -> Option<(u32, String)> {
    let code = headers.get("grpc-status")?.to_str().ok()?.parse().ok()?;
    let message = headers
        .get("grpc-message")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .chars()
        .take(1024)
        .collect();
    Some((code, message))
}

#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct AuthTokenGetRequest {}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct AuthTokenGetResponse {
    #[prost(string, tag = "1")]
    pub token: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct AppGetOrCreateRequest {
    #[prost(string, tag = "1")]
    pub app_name: String,
    #[prost(string, tag = "2")]
    pub environment_name: String,
    #[prost(int32, tag = "3")]
    pub object_creation_type: i32,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct AppGetOrCreateResponse {
    #[prost(string, tag = "1")]
    pub app_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct ImageRegistryConfig {
    #[prost(int32, tag = "1")]
    pub registry_auth_type: i32,
    #[prost(string, tag = "2")]
    pub secret_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct Image {
    #[prost(string, repeated, tag = "6")]
    pub dockerfile_commands: Vec<String>,
    #[prost(string, tag = "11")]
    pub version: String,
    #[prost(message, optional, tag = "17")]
    pub image_registry_config: Option<ImageRegistryConfig>,
}
/// `status`: 0 unspecified (still running), 1 success, anything else failed.
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct GenericResult {
    #[prost(int32, tag = "1")]
    pub status: i32,
    #[prost(string, tag = "2")]
    pub exception: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct ImageGetOrCreateRequest {
    #[prost(message, optional, tag = "2")]
    pub image: Option<Image>,
    #[prost(string, tag = "4")]
    pub app_id: String,
    #[prost(bool, tag = "7")]
    pub force_build: bool,
    #[prost(int32, tag = "8")]
    pub namespace: i32,
    #[prost(string, tag = "9")]
    pub builder_version: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct ImageGetOrCreateResponse {
    #[prost(string, tag = "1")]
    pub image_id: String,
    #[prost(message, optional, tag = "2")]
    pub result: Option<GenericResult>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct ImageJoinStreamingRequest {
    #[prost(string, tag = "1")]
    pub image_id: String,
    #[prost(float, tag = "2")]
    pub timeout: f32,
    #[prost(string, tag = "3")]
    pub last_entry_id: String,
    #[prost(bool, tag = "4")]
    pub include_logs_for_finished: bool,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct ImageJoinStreamingResponse {
    #[prost(message, optional, tag = "1")]
    pub result: Option<GenericResult>,
    #[prost(string, tag = "3")]
    pub entry_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct Sandbox {
    /// Empty runs the image's own command.
    #[prost(string, repeated, tag = "1")]
    pub entrypoint_args: Vec<String>,
    #[prost(string, tag = "3")]
    pub image_id: String,
    #[prost(uint32, tag = "7")]
    pub timeout_secs: u32,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct SandboxCreateV2Request {
    #[prost(string, tag = "1")]
    pub app_id: String,
    #[prost(message, optional, tag = "2")]
    pub definition: Option<Sandbox>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct SandboxCreateV2Response {
    #[prost(string, tag = "1")]
    pub sandbox_id: String,
    #[prost(string, tag = "3")]
    pub task_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct SandboxGetTaskIdRequest {
    #[prost(string, tag = "1")]
    pub sandbox_id: String,
    #[prost(float, optional, tag = "2")]
    pub timeout: Option<f32>,
    #[prost(bool, tag = "3")]
    pub wait_until_ready: bool,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct SandboxGetTaskIdResponse {
    #[prost(string, optional, tag = "1")]
    pub task_id: Option<String>,
    #[prost(message, optional, tag = "2")]
    pub task_result: Option<GenericResult>,
}
/// A V2 sandbox's filesystem snapshot, taken through its task command router.
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct TaskSnapshotFilesystemRequest {
    #[prost(string, tag = "1")]
    pub task_id: String,
    /// Idempotency key.
    #[prost(string, tag = "2")]
    pub snapshot_id: String,
    #[prost(int64, optional, tag = "3")]
    pub ttl_seconds: Option<i64>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct TaskSnapshotFilesystemResponse {
    #[prost(string, tag = "1")]
    pub image_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct SandboxTerminateRequest {
    #[prost(string, tag = "1")]
    pub sandbox_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct SandboxTerminateResponse {}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct SandboxGetCommandRouterAccessRequest {
    #[prost(string, tag = "1")]
    pub sandbox_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct SandboxGetCommandRouterAccessResponse {
    #[prost(string, tag = "1")]
    pub jwt: String,
    #[prost(string, tag = "2")]
    pub url: String,
}
pub(super) struct RouterAccess {
    pub url: String,
    pub jwt: Zeroizing<String>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct TaskExecStartRequest {
    #[prost(string, tag = "1")]
    pub task_id: String,
    #[prost(string, tag = "2")]
    pub exec_id: String,
    #[prost(string, repeated, tag = "3")]
    pub command_args: Vec<String>,
    /// 1 pipes the stream so it can be read back.
    #[prost(int32, tag = "4")]
    pub stdout_config: i32,
    #[prost(int32, tag = "5")]
    pub stderr_config: i32,
    #[prost(uint32, optional, tag = "6")]
    pub timeout_secs: Option<u32>,
    #[prost(string, optional, tag = "7")]
    pub workdir: Option<String>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct TaskExecStartResponse {}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct TaskExecStdioReadRequest {
    #[prost(string, tag = "1")]
    pub task_id: String,
    #[prost(string, tag = "2")]
    pub exec_id: String,
    #[prost(uint64, tag = "3")]
    pub offset: u64,
    /// 0 stdout, 1 stderr.
    #[prost(int32, tag = "4")]
    pub file_descriptor: i32,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct TaskExecStdioReadResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub data: Vec<u8>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct TaskExecWaitRequest {
    #[prost(string, tag = "1")]
    pub task_id: String,
    #[prost(string, tag = "2")]
    pub exec_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct TaskExecWaitResponse {
    #[prost(int32, optional, tag = "1")]
    pub code: Option<i32>,
    #[prost(int32, optional, tag = "2")]
    pub signal: Option<i32>,
}
