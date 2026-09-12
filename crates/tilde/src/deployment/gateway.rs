//! The sidecar's client for the gateway. Every call carries the deployment token;
//! the sidecar always dials out, so it needs no reachable address of its own.
use crate::chat::{ChatError, Result};
use crate::proto::tilde::{agent_event_ingress::v1 as wire, types::v1 as types};
use crate::services::tilde::agent_event_ingress::v1::SidecarServiceClient;
use connectrpc::client::{CallOptions, ClientConfig, ClientTransport, HttpClient};
use secrecy::{ExposeSecret, SecretString};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;
pub type WatchStream = connectrpc::client::ServerStream<
    <HttpClient as ClientTransport>::ResponseBody,
    wire::__buffa::view::WatchResponseView<'static>,
>;
#[derive(Clone)]
pub struct Client {
    inner: SidecarServiceClient<HttpClient>,
}
impl Client {
    pub fn new(endpoint: &str, token: SecretString) -> Result<Self> {
        let endpoint = endpoint.trim_end_matches('/');
        let url = url::Url::parse(endpoint)
            .map_err(|_| ChatError::Invalid("Invalid gateway URL".into()))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(ChatError::Invalid("Invalid gateway URL".into()));
        }
        let transport = if url.scheme() == "https" {
            let roots = connectrpc::rustls::RootCertStore::from_iter(
                webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
            );
            let tls = connectrpc::rustls::ClientConfig::builder_with_provider(Arc::new(
                connectrpc::rustls::crypto::aws_lc_rs::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|_| ChatError::Transport)?
            .with_root_certificates(roots)
            .with_no_client_auth();
            HttpClient::with_tls(Arc::new(tls))
        } else {
            HttpClient::plaintext_http2_only()
        };
        let mut headers = http::HeaderMap::new();
        let mut value = http::HeaderValue::from_str(&format!("Bearer {}", token.expose_secret()))
            .map_err(|_| ChatError::Transport)?;
        value.set_sensitive(true);
        headers.insert(http::header::AUTHORIZATION, value);
        let config = ClientConfig::new(endpoint.parse().map_err(|_| ChatError::Transport)?)
            .with_default_headers(headers);
        Ok(Self {
            inner: SidecarServiceClient::new(transport, config),
        })
    }
    fn options() -> CallOptions {
        CallOptions::default().with_timeout(Duration::from_secs(15))
    }
    pub async fn watch(&self, request: wire::WatchRequest) -> Result<WatchStream> {
        self.inner
            .watch(request)
            .await
            .map_err(|_| ChatError::Transport)
    }
    pub async fn publish(
        &self,
        instance: Uuid,
        frames: Vec<wire::Upstream>,
    ) -> Result<wire::PublishResponse> {
        self.inner
            .publish_with_options(
                wire::PublishRequest {
                    instance_id: instance.to_string(),
                    frames,
                    ..Default::default()
                },
                Self::options(),
            )
            .await
            .map(|r| r.into_owned())
            .map_err(|_| ChatError::Transport)
    }
    pub async fn hydrate(&self, request: wire::HydrateRequest) -> Result<wire::HydrateResponse> {
        self.inner
            .hydrate_with_options(request, Self::options())
            .await
            .map(|r| r.into_owned())
            .map_err(|_| ChatError::Transport)
    }
    pub async fn forward(
        &self,
        thread: Option<Uuid>,
        call: wire::IngressCall,
    ) -> Result<wire::CallResult> {
        self.inner
            .forward_with_options(
                wire::ForwardRequest {
                    thread_id: thread.map(|t| t.to_string()).unwrap_or_default(),
                    work: Some(wire::forward_request::Work::Call(Box::new(call))),
                    ..Default::default()
                },
                CallOptions::default().with_timeout(Duration::from_secs(30)),
            )
            .await
            .map_err(|_| ChatError::Transport)?
            .into_owned()
            .result
            .into_option()
            .ok_or(ChatError::Transport)
    }
    pub async fn forward_provider_event(
        &self,
        thread: Uuid,
        connection: Uuid,
        event: types::ProviderEvent,
    ) -> Result<()> {
        let directive = wire::ProviderEventDirective {
            connection_id: connection.to_string(),
            event: event.into(),
            ..Default::default()
        };
        self.inner
            .forward_with_options(
                wire::ForwardRequest {
                    thread_id: thread.to_string(),
                    work: Some(wire::forward_request::Work::ProviderEvent(Box::new(
                        directive,
                    ))),
                    ..Default::default()
                },
                Self::options(),
            )
            .await
            .map(|_| ())
            .map_err(|_| ChatError::Transport)
    }
    pub async fn relay(
        &self,
        instance: Uuid,
        path: &str,
        content_type: &str,
        body: Vec<u8>,
        caller_token: &str,
    ) -> Result<wire::CallResult> {
        let request = wire::RelayRequest {
            instance_id: instance.to_string(),
            path: path.into(),
            content_type: content_type.into(),
            body,
            caller_token: caller_token.into(),
            ..Default::default()
        };
        self.inner
            .relay_with_options(request, Self::options())
            .await
            .map_err(|error| match error.code {
                connectrpc::ErrorCode::PermissionDenied
                | connectrpc::ErrorCode::Unauthenticated => ChatError::Denied,
                connectrpc::ErrorCode::NotFound => ChatError::NotFound,
                connectrpc::ErrorCode::InvalidArgument => {
                    ChatError::Invalid(error.message.unwrap_or_default())
                }
                _ => ChatError::Invalid(format!(
                    "Gateway relay failed: {:?} {}",
                    error.code,
                    error.message.unwrap_or_default()
                )),
            })?
            .into_owned()
            .result
            .into_option()
            .ok_or(ChatError::Transport)
    }
    pub async fn upload_attachment(
        &self,
        attachment: types::Attachment,
        content: Vec<u8>,
    ) -> Result<types::Attachment> {
        self.inner
            .upload_attachment_with_options(
                wire::UploadAttachmentRequest {
                    attachment: attachment.into(),
                    content,
                    ..Default::default()
                },
                CallOptions::default().with_timeout(Duration::from_secs(120)),
            )
            .await
            .map_err(|_| ChatError::Transport)?
            .into_owned()
            .attachment
            .into_option()
            .ok_or(ChatError::Transport)
    }
    pub async fn download_attachment(&self, thread: Uuid, key: Uuid) -> Result<Vec<u8>> {
        Ok(self
            .inner
            .download_attachment_with_options(
                wire::DownloadAttachmentRequest {
                    thread_id: thread.to_string(),
                    attachment_id: key.to_string(),
                    ..Default::default()
                },
                CallOptions::default().with_timeout(Duration::from_secs(120)),
            )
            .await
            .map_err(|_| ChatError::Transport)?
            .into_owned()
            .content)
    }
    pub async fn resolve_agent(&self, agent: Uuid) -> Result<wire::ResolveParticipantResponse> {
        let request = wire::ResolveParticipantRequest {
            key: Some(wire::resolve_participant_request::Key::AgentId(
                agent.to_string(),
            )),
            ..Default::default()
        };
        self.resolve(request).await
    }
    pub async fn resolve_user(&self, user: Uuid) -> Result<wire::ResolveParticipantResponse> {
        let request = wire::ResolveParticipantRequest {
            key: Some(wire::resolve_participant_request::Key::UserId(
                user.to_string(),
            )),
            ..Default::default()
        };
        self.resolve(request).await
    }
    async fn resolve(
        &self,
        request: wire::ResolveParticipantRequest,
    ) -> Result<wire::ResolveParticipantResponse> {
        match self
            .inner
            .resolve_participant_with_options(request, Self::options())
            .await
        {
            Ok(response) => Ok(response.into_owned()),
            Err(error) if error.code == connectrpc::ErrorCode::NotFound => Err(ChatError::NotFound),
            Err(_) => Err(ChatError::Transport),
        }
    }
}
