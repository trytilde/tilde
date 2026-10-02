//! The management calls the CLI makes: resolving an agent, uploading skill files and
//! registering a deployment. Requests and responses are the generated contract types.
use crate::{
    api::{Client, failed, options},
    discover::{self, Declarations},
};
use anyhow::{Context, Result, bail};
#[cfg(test)]
use sha2::Digest as _;
use tilde_contracts::proto::tilde::management::v1 as wire;
use tilde_contracts::proto::tilde::types::v1 as types;

pub async fn get_agent(client: &Client, id: &str) -> Result<types::Agent> {
    client
        .agents
        .get_agent_with_options(
            wire::GetAgentRequest {
                id: id.into(),
                ..Default::default()
            },
            options(),
        )
        .await
        .map_err(|error| failed("GetAgent", error))?
        .into_owned()
        .agent
        .into_option()
        .context("GetAgent returned no agent")
}

/// Find an agent by exact name. The CLI deliberately never creates one: an agent created
/// without capabilities, inference or channel access cannot serve an invocation, so it would
/// look registered and then fail at run time. Registering stays a deliberate act in the UI.
pub async fn find_agent(client: &Client, name: &str) -> Result<Option<types::Agent>> {
    let response = client
        .agents
        .list_agents_with_options(
            wire::ListAgentsRequest {
                search: name.into(),
                page_size: 100,
                ..Default::default()
            },
            options(),
        )
        .await
        .map_err(|error| failed("ListAgents", error))?
        .into_owned();
    Ok(response.agents.into_iter().find(|agent| agent.name == name))
}

pub struct Deployment {
    pub id: String,
    /// Empty when the deployment already existed: the engine issues a token only on creation.
    pub token: String,
    pub created: bool,
}

/// Upload the skill files the engine does not already hold, then register the deployment.
pub async fn register(
    client: &Client,
    agent_id: &str,
    declarations: &Declarations,
    mut request: wire::RegisterDeploymentRequest,
) -> Result<Deployment> {
    let mut declared = declarations.value.clone();
    let uploads = discover::pending_uploads(&declared);
    if !uploads.is_empty() {
        let missing = client
            .deployments
            .missing_deployment_files_with_options(
                wire::MissingDeploymentFilesRequest {
                    agent_id: agent_id.into(),
                    sha256: uploads.iter().map(|(digest, _)| digest.clone()).collect(),
                    ..Default::default()
                },
                options(),
            )
            .await
            .map_err(|error| failed("MissingDeploymentFiles", error))?
            .into_owned();
        for digest in &missing.sha256 {
            let Some((_, bytes)) = uploads.iter().find(|(candidate, _)| candidate == digest) else {
                continue;
            };
            let stored = client
                .deployments
                .upload_deployment_file_with_options(
                    wire::UploadDeploymentFileRequest {
                        agent_id: agent_id.into(),
                        data: bytes.clone(),
                        ..Default::default()
                    },
                    options(),
                )
                .await
                .map_err(|error| failed("UploadDeploymentFile", error))?
                .into_owned();
            if &stored.sha256 != digest {
                bail!("Tilde stored an upload under another digest");
            }
        }
        discover::seal_uploads(&mut declared);
    }

    request.agent_id = agent_id.into();
    // The reader prints protobuf JSON, which is exactly how the contract type deserialises.
    request.declarations = serde_json::from_value::<wire::DeploymentDeclarations>(declared)
        .context("The declaration reader produced declarations Tilde cannot accept")?
        .into();
    let registered = client
        .deployments
        .register_deployment_with_options(request, options())
        .await
        .map_err(|error| failed("RegisterDeployment", error))?
        .into_owned();
    Ok(Deployment {
        id: registered
            .deployment
            .into_option()
            .map(|deployment| deployment.id)
            .unwrap_or_default(),
        token: registered.token,
        created: registered.created,
    })
}

/// Mint a usable token for a deployment that already exists. Registration returns a token only
/// once, so `tilde dev` reconnecting to its own deployment asks for a fresh one.
pub async fn issue_token(client: &Client, agent_id: &str, deployment_id: &str) -> Result<String> {
    Ok(client
        .deployments
        .issue_deployment_token_with_options(
            wire::IssueDeploymentTokenRequest {
                agent_id: agent_id.into(),
                deployment_id: deployment_id.into(),
                ..Default::default()
            },
            options(),
        )
        .await
        .map_err(|error| failed("IssueDeploymentToken", error))?
        .into_owned()
        .token)
}

/// Every agent owns one built-in Tilde chat connection, created with it. Its application key is
/// what `tilde dev`'s local chat page authenticates with.
pub async fn chat_key(client: &Client, agent_id: &str) -> Result<String> {
    Ok(client
        .chat
        .get_credentials_with_options(
            wire::GetCredentialsRequest {
                agent_id: agent_id.into(),
                ..Default::default()
            },
            options(),
        )
        .await
        .map_err(|error| failed("GetCredentials", error))?
        .into_owned()
        .api_key)
}

/// The connection id of the agent's built-in Tilde chat channel.
async fn tilde_channel(client: &Client, agent_id: &str) -> Result<String> {
    client
        .access
        .list_channel_access_with_options(
            wire::ListChannelAccessRequest {
                agent_id: agent_id.into(),
                page_size: 100,
                ..Default::default()
            },
            options(),
        )
        .await
        .map_err(|error| failed("ListChannelAccess", error))?
        .into_owned()
        .routes
        .into_iter()
        .find(|route| route.provider_id == "tilde")
        .map(|route| route.connection_id)
        .context("This agent has no built-in Tilde chat channel")
}

async fn channel_identities(
    client: &Client,
    agent_id: &str,
    connection: &str,
) -> Result<Vec<types::ChannelIdentity>> {
    Ok(client
        .access
        .list_channel_identities_with_options(
            wire::ListChannelIdentitiesRequest {
                agent_id: agent_id.into(),
                connection_id: Some(connection.into()),
                page_size: 100,
                ..Default::default()
            },
            options(),
        )
        .await
        .map_err(|error| failed("ListChannelIdentities", error))?
        .into_owned()
        .identities)
}

/// Let one local identity into the agent's Tilde chat channel, which is private by default.
///
/// The channel records an identity even when it refuses it, so the probe that gets denied is
/// what makes the identity addressable; this then allows exactly that one. The channel's own
/// mode is left alone, so nothing else becomes reachable.
pub async fn admit_identity(
    client: &Client,
    agent_id: &str,
    api_key: &str,
    identity: &str,
) -> Result<bool> {
    let connection = tilde_channel(client, agent_id).await?;
    let mut identities = channel_identities(client, agent_id, &connection).await?;
    if !identities.iter().any(|known| known.value == identity) {
        crate::chat::probe_identity(client.gateway(), agent_id, api_key, identity).await;
        identities = channel_identities(client, agent_id, &connection).await?;
    }
    let known = identities
        .into_iter()
        .find(|known| known.value == identity)
        .with_context(|| format!("Tilde chat did not record the identity {identity}"))?;
    if known.allowed {
        return Ok(false);
    }
    client
        .access
        .set_identity_access_with_options(
            wire::SetIdentityAccessRequest {
                agent_id: agent_id.into(),
                connection_id: connection,
                identity_id: known.id,
                allowed: true,
                ..Default::default()
            },
            options(),
        )
        .await
        .map_err(|error| failed("SetIdentityAccess", error))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::Declarations;
    use axum::{
        Router,
        body::Bytes,
        http::header,
        response::{IntoResponse, Response},
        routing::post,
    };
    use buffa::Message;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    /// Reply to a Connect unary call with a protobuf-encoded message, which is what the
    /// generated client asks for by default.
    fn proto<M: Message>(message: M) -> Response {
        (
            [(header::CONTENT_TYPE, "application/proto")],
            message.encode_to_vec(),
        )
            .into_response()
    }

    /// A skill file that cannot travel as text has to become an upload before registration:
    /// the CLI asks which digests are missing, uploads only those, and then registers the
    /// declarations with the digest in place of the bytes. This drives that whole exchange
    /// against a stand-in gateway over the real wire format.
    #[tokio::test]
    async fn uploads_binary_skill_files_then_registers_them_by_digest() {
        let calls: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let registered: Arc<Mutex<Option<wire::RegisterDeploymentRequest>>> =
            Arc::new(Mutex::new(None));
        let asked: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let uploaded: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));

        let app = Router::new()
            .route(
                "/tilde.management.v1.DeploymentService/MissingDeploymentFiles",
                post({
                    let (calls, asked) = (calls.clone(), asked.clone());
                    move |body: Bytes| async move {
                        calls.lock().unwrap().push("missing");
                        let request =
                            wire::MissingDeploymentFilesRequest::decode_from_slice(&body).unwrap();
                        asked.lock().unwrap().extend(request.sha256.iter().cloned());
                        // Claim to hold nothing, so every digest is uploaded.
                        proto(wire::MissingDeploymentFilesResponse {
                            sha256: request.sha256,
                            ..Default::default()
                        })
                    }
                }),
            )
            .route(
                "/tilde.management.v1.DeploymentService/UploadDeploymentFile",
                post({
                    let (calls, uploaded) = (calls.clone(), uploaded.clone());
                    move |body: Bytes| async move {
                        calls.lock().unwrap().push("upload");
                        let request =
                            wire::UploadDeploymentFileRequest::decode_from_slice(&body).unwrap();
                        let data = request.data.to_vec();
                        uploaded.lock().unwrap().push(data.clone());
                        proto(wire::UploadDeploymentFileResponse {
                            sha256: hex::encode(sha2::Sha256::digest(&data)),
                            ..Default::default()
                        })
                    }
                }),
            )
            .route(
                "/tilde.management.v1.DeploymentService/RegisterDeployment",
                post({
                    let (calls, registered) = (calls.clone(), registered.clone());
                    move |body: Bytes| async move {
                        calls.lock().unwrap().push("register");
                        *registered.lock().unwrap() = Some(
                            wire::RegisterDeploymentRequest::decode_from_slice(&body).unwrap(),
                        );
                        proto(wire::RegisterDeploymentResponse {
                            deployment: types::AgentDeployment {
                                id: "11111111-2222-4333-8444-555555555555".into(),
                                ..Default::default()
                            }
                            .into(),
                            token: "deployment-token".into(),
                            created: true,
                            ..Default::default()
                        })
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        // "PNG" as bytes: not UTF-8, so the reader emits it inline as base64 `data`.
        let png: &[u8] = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0xff, 0x00];
        let encoded = {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            STANDARD.encode(png)
        };
        let digest = hex::encode(sha2::Sha256::digest(png));
        let declarations = Declarations {
            digest: "test".into(),
            value: json!({
                "prompts": [],
                "tools": [],
                "skills": [{
                    "name": "greet",
                    "origin": "dist/index.js#skills",
                    "files": [
                        { "path": "SKILL.md", "content": "hi" },
                        { "path": "logo.png", "data": encoded },
                    ],
                }],
            }),
        };

        let client = Client::new(&format!("http://{address}"), Some("api-key".into())).unwrap();
        let deployment = register(
            &client,
            "agent-1",
            &declarations,
            wire::RegisterDeploymentRequest {
                source: types::DeploymentSource::Manual.into(),
                label: Some("v1".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(deployment.token, "deployment-token");
        assert!(deployment.created);
        assert_eq!(deployment.id, "11111111-2222-4333-8444-555555555555");
        assert_eq!(
            *calls.lock().unwrap(),
            ["missing", "upload", "register"],
            "the digest is checked before it is uploaded, and both before registering"
        );
        assert_eq!(*asked.lock().unwrap(), std::slice::from_ref(&digest));
        assert_eq!(*uploaded.lock().unwrap(), [png.to_vec()]);

        let request = registered.lock().unwrap().clone().expect("registered");
        assert_eq!(request.agent_id, "agent-1");
        assert_eq!(request.label.as_deref(), Some("v1"));
        let files = &request
            .declarations
            .into_option()
            .expect("declarations travel with the registration")
            .skills[0]
            .files;
        // Text stays inline; the binary file now travels as the digest the gateway holds.
        assert_eq!(
            files[0].body.as_ref().map(|body| match body {
                wire::declared_skill_file::Body::Content(text) => format!("content:{text}"),
                wire::declared_skill_file::Body::Sha256(sha) => format!("sha256:{sha}"),
                wire::declared_skill_file::Body::Data(_) => "data".into(),
            }),
            Some("content:hi".into())
        );
        assert_eq!(
            files[1].body.as_ref().map(|body| match body {
                wire::declared_skill_file::Body::Content(text) => format!("content:{text}"),
                wire::declared_skill_file::Body::Sha256(sha) => format!("sha256:{sha}"),
                wire::declared_skill_file::Body::Data(_) => "data".into(),
            }),
            Some(format!("sha256:{digest}")),
            "the bytes are replaced by the digest, not sent twice"
        );
    }
}
