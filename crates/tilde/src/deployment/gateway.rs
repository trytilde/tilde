//! Authenticated agent-event-ingress client. Original caller credentials travel
//! separately from the deployment credential on conversation proxy requests.
use crate::chat::{ChatError, Result};
use crate::proto::tilde::agent_event_ingress::v1 as wire;
use secrecy::{ExposeSecret, SecretString};
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;
use uuid::Uuid;
#[derive(Clone)]
pub struct Client {
    pub(crate) endpoint: url::Url,
    pub(crate) token: SecretString,
    pub(crate) http: reqwest::Client,
}
impl Client {
    pub fn new(endpoint: &str, token: SecretString) -> Result<Self> {
        let endpoint = url::Url::parse(&format!("{}/", endpoint.trim_end_matches('/')))
            .map_err(|_| ChatError::Invalid("Invalid gateway agent-event-ingress URL".into()))?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err(ChatError::Invalid("Invalid gateway URL".into()));
        }
        Ok(Self {
            endpoint,
            token,
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| ChatError::Transport)?,
        })
    }
    pub async fn rpc<I: Serialize, O: DeserializeOwned>(
        &self,
        method: &str,
        request: &I,
    ) -> Result<O> {
        let url = self
            .endpoint
            .join(&format!(
                "tilde.agent_event_ingress.v1.SidecarService/{method}"
            ))
            .map_err(|_| ChatError::Transport)?;
        let response = self
            .http
            .post(url)
            .timeout(Duration::from_secs(15))
            .bearer_auth(self.token.expose_secret())
            .header("connect-protocol-version", "1")
            .json(request)
            .send()
            .await
            .map_err(|_| ChatError::Transport)?;
        if !response.status().is_success() {
            return Err(ChatError::Transport);
        }
        response.json().await.map_err(|_| ChatError::Transport)
    }
    pub async fn register(
        &self,
        r: &wire::RegisterSidecarRequest,
    ) -> Result<wire::RegisterSidecarResponse> {
        self.rpc("RegisterSidecar", r).await
    }
    pub async fn configuration(&self) -> Result<wire::GetConfigurationResponse> {
        self.rpc(
            "GetConfiguration",
            &wire::GetConfigurationRequest::default(),
        )
        .await
    }
    pub async fn heartbeat(&self, r: &wire::HeartbeatRequest) -> Result<wire::HeartbeatResponse> {
        self.rpc("Heartbeat", r).await
    }
    pub async fn proxy(
        &self,
        agent: uuid::Uuid,
        request: axum::extract::Request,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        let (mut parts, body) = request.into_parts();
        let path = parts
            .uri
            .path_and_query()
            .map(|p| p.as_str())
            .unwrap_or("/");
        let Ok(url) = self.endpoint.join(&format!("agents/{agent}/proxy{}", path)) else {
            return http::StatusCode::BAD_GATEWAY.into_response();
        };
        parts.headers.remove(http::header::HOST);
        parts.headers.remove(http::header::CONTENT_LENGTH);
        let Ok(token) = http::HeaderValue::from_str(self.token.expose_secret()) else {
            return http::StatusCode::BAD_GATEWAY.into_response();
        };
        parts.headers.insert("x-tilde-sidecar-token", token);
        let body = reqwest::Body::wrap_stream(body.into_data_stream());
        match self
            .http
            .request(parts.method, url)
            .headers(parts.headers)
            .body(body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();
                let mut out =
                    axum::body::Body::from_stream(response.bytes_stream()).into_response();
                *out.status_mut() = status;
                for (name, value) in headers.iter() {
                    if name != http::header::TRANSFER_ENCODING
                        && name != http::header::CONTENT_LENGTH
                    {
                        out.headers_mut().insert(name, value.clone());
                    }
                }
                out
            }
            Err(_) => (
                http::StatusCode::SERVICE_UNAVAILABLE,
                "Gateway is unavailable",
            )
                .into_response(),
        }
    }
}
pub type ResponseStream<T> = std::pin::Pin<Box<dyn futures::Stream<Item = Result<T>> + Send>>;
impl Client {
    pub async fn stream<I: Serialize, O: DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        request: &I,
        caller_token: Option<&str>,
    ) -> Result<ResponseStream<O>> {
        let bytes = serde_json::to_vec(request).map_err(|_| ChatError::Transport)?;
        let mut frame = vec![0];
        frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        frame.extend(bytes);
        let url = self.endpoint.join(path).map_err(|_| ChatError::Transport)?;
        let mut request = self
            .http
            .post(url)
            .bearer_auth(caller_token.unwrap_or(self.token.expose_secret()))
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(frame);
        if caller_token.is_some() {
            request = request.header("x-tilde-sidecar-token", self.token.expose_secret());
        }
        let mut response = request.send().await.map_err(|_| ChatError::Transport)?;
        if !response.status().is_success() {
            return Err(ChatError::Transport);
        }
        Ok(Box::pin(async_stream::try_stream! {
            let mut pending=vec![];
            while let Some(chunk)=response.chunk().await.map_err(|_|ChatError::Transport)?{
                pending.extend_from_slice(&chunk);if pending.len()>32*1024*1024{Err(ChatError::Transport)?;}
                while pending.len()>=5 {
                    let length=u32::from_be_bytes(pending[1..5].try_into().map_err(|_|ChatError::Transport)?) as usize;
                    if pending.len()<length+5{break;}
                    let flags=pending[0];let frame:Vec<u8>=pending.drain(..length+5).collect();
                    if flags==2{let end:serde_json::Value=serde_json::from_slice(&frame[5..]).map_err(|_|ChatError::Transport)?;if end.get("error").is_some(){Err(ChatError::Transport)?;}return;}
                    if flags!=0{Err(ChatError::Transport)?;}
                    yield serde_json::from_slice::<O>(&frame[5..]).map_err(|_|ChatError::Transport)?;
                }
            }
        }))
    }
    pub async fn watch_commands(
        &self,
        instance: uuid::Uuid,
    ) -> Result<ResponseStream<crate::proto::tilde::types::v1::AgentCommand>> {
        let mut stream = self
            .stream::<_, wire::WatchCommandsResponse>(
                "tilde.agent_event_ingress.v1.SidecarService/WatchCommands",
                &wire::WatchCommandsRequest {
                    instance_id: instance.to_string(),
                    ..Default::default()
                },
                None,
            )
            .await?;
        Ok(Box::pin(
            async_stream::try_stream! {use futures::StreamExt;while let Some(value)=stream.next().await{yield value?.command.into_option().ok_or(ChatError::Transport)?;}},
        ))
    }
}

impl Client {
    pub async fn attachment(
        &self,
        agent: uuid::Uuid,
        thread: uuid::Uuid,
        key: uuid::Uuid,
    ) -> Result<Vec<u8>> {
        let url = self
            .endpoint
            .join(&format!("agents/{agent}/attachments/{thread}/{key}"))
            .map_err(|_| ChatError::Transport)?;
        let mut response = self
            .http
            .get(url)
            .timeout(Duration::from_secs(30))
            .bearer_auth(self.token.expose_secret())
            .send()
            .await
            .map_err(|_| ChatError::Transport)?;
        if !response.status().is_success() {
            return Err(ChatError::Transport);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| ChatError::Transport)? {
            if bytes.len() + chunk.len() > 128 * 1024 * 1024 {
                return Err(ChatError::Transport);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentReference {
    pub id: Uuid,
    pub name: String,
}
impl Client {
    pub(crate) async fn agent_reference(
        &self,
        agent: Uuid,
        target: Uuid,
    ) -> Result<AgentReference> {
        let url = self
            .endpoint
            .join(&format!("agents/{agent}/registry/{target}"))
            .map_err(|_| ChatError::Transport)?;
        let response = self
            .http
            .get(url)
            .timeout(Duration::from_secs(10))
            .bearer_auth(self.token.expose_secret())
            .send()
            .await
            .map_err(|_| ChatError::Transport)?;
        if response.status() == http::StatusCode::NOT_FOUND {
            return Err(ChatError::NotFound);
        }
        if !response.status().is_success() {
            return Err(ChatError::Transport);
        }
        response.json().await.map_err(|_| ChatError::Transport)
    }
}
