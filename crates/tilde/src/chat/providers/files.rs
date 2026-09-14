//! Remote attachment bytes are fetched only when requested, then encrypted and cached in Postgres.
//! Public HTTPS destinations use pinned DNS and reject private addresses. Provider authorization
//! is attached only to the exact provider file origins declared below, never to arbitrary links.
use super::*;
use std::net::{IpAddr, SocketAddr};
impl Channels {
    pub async fn attachment(
        &self,
        connection: Uuid,
        reference: Option<&str>,
        url: Option<&str>,
    ) -> ToolResult<Vec<u8>> {
        let row = self.connections.get(connection).await?;
        let a = Access {
            connection_id: connection,
            values: self.connections.resolve(connection).await?,
            http: self.connections.http.clone(),
            endpoints: self.connections.endpoints.clone(),
        };
        download(&row.provider_id, &a, reference, url).await
    }
}
pub(crate) async fn download(
    provider: &str,
    a: &Access,
    reference: Option<&str>,
    url: Option<&str>,
) -> ToolResult<Vec<u8>> {
    let mut authorization = None;
    let url = if let Some(reference) = reference {
        match provider {
            "whatsapp" => {
                let response = a
                    .json(
                        a.http
                            .client
                            .get(a.url(
                                "meta_graph",
                                "https://graph.facebook.com/v23.0",
                                &[reference],
                            )?)
                            .bearer_auth(a.secret("access_token")?),
                    )
                    .await?;
                authorization = Some(a.secret("access_token")?);
                required(&response, "url")?.to_owned()
            }
            "agentmail" => {
                let (message, attachment): (String, String) = serde_json::from_str(reference)
                    .map_err(|_| {
                        ConnectError::internal("Invalid stored AgentMail attachment reference")
                    })?;
                let response = a
                    .json(
                        a.http
                            .client
                            .get(a.url(
                                "agentmail_api",
                                "https://api.agentmail.to/v0",
                                &[
                                    "inboxes",
                                    a.secret("inbox_id")?,
                                    "messages",
                                    &message,
                                    "attachments",
                                    &attachment,
                                ],
                            )?)
                            .bearer_auth(a.secret("api_key")?),
                    )
                    .await?;
                required(&response, "download_url")?.to_owned()
            }
            _ => {
                return Err(ConnectError::failed_precondition(
                    "Provider attachment is unavailable",
                ));
            }
        }
    } else {
        url.ok_or_else(|| ConnectError::not_found("Attachment source missing"))?
            .to_owned()
    };
    let url = url::Url::parse(&url)
        .map_err(|_| ConnectError::invalid_argument("Invalid attachment URL"))?;
    let host = url
        .host_str()
        .ok_or_else(|| ConnectError::invalid_argument("Invalid attachment URL"))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(ConnectError::permission_denied(
            "Attachment needs public HTTPS",
        ));
    }
    if provider == "slack" {
        if host != "files.slack.com" {
            return Err(ConnectError::permission_denied(
                "Invalid Slack attachment origin",
            ));
        }
        authorization = Some(a.secret("access_token")?);
    }
    if authorization.is_some()
        && provider == "whatsapp"
        && !(host == "lookaside.fbsbx.com"
            || host.ends_with(".facebook.com")
            || host.ends_with(".whatsapp.net"))
    {
        return Err(ConnectError::permission_denied(
            "Invalid Meta attachment origin",
        ));
    }
    let addresses = tokio::net::lookup_host((host, url.port_or_known_default().unwrap_or(443)))
        .await
        .map_err(|_| ConnectError::unavailable("Attachment host lookup failed"))?
        .collect::<Vec<SocketAddr>>();
    if addresses.is_empty() || addresses.iter().any(|a| !public(a.ip())) {
        return Err(ConnectError::permission_denied(
            "Attachment cannot access a private address",
        ));
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .resolve_to_addrs(host, &addresses)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| ConnectError::internal("Attachment transport failed"))?;
    let mut request = client.get(url);
    if let Some(token) = authorization {
        request = request.bearer_auth(token);
    }
    let mut response = request
        .send()
        .await
        .map_err(|_| ConnectError::unavailable("Attachment download failed"))?;
    if !response.status().is_success() {
        return Err(ConnectError::unavailable("Attachment download rejected"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ConnectError::unavailable("Attachment download interrupted"))?
    {
        if bytes.len() + chunk.len() > crate::chat::audit::MAX_ATTACHMENT_BYTES {
            return Err(ConnectError::resource_exhausted(
                "Attachment exceeds 128 MiB",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_multicast()
                || v.is_broadcast()
                || v.is_documentation()
                || o[0] == 0
                || o[0] >= 240
                || (o[0] == 100 && (64..=127).contains(&o[1]))
                || (o[0] == 198 && (18..=19).contains(&o[1])))
        }
        IpAddr::V6(v) => {
            if let Some(v) = v.to_ipv4_mapped() {
                return public(IpAddr::V4(v));
            }
            let s = v.segments();
            !(v.is_loopback()
                || v.is_unspecified()
                || v.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
                || (s[0] == 0x2001 && s[1] == 0xdb8))
        }
    }
}
