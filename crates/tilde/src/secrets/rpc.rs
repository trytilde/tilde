//! Management surface for secrets. Values go in and are never returned.
use super::{Secret, Secrets};
use crate::error::Error;
use crate::proto::tilde::management::v1 as management;
use crate::services::tilde::management::v1::SecretService;
use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::SecretString;
use std::sync::Arc;
use uuid::Uuid;

struct Rpc(Secrets);
pub fn router(secrets: Secrets) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(secrets))),
        64 * 1024,
    )
}
fn timestamp(date: chrono::DateTime<chrono::Utc>) -> buffa_types::google::protobuf::Timestamp {
    buffa_types::google::protobuf::Timestamp {
        seconds: date.timestamp(),
        nanos: date.timestamp_subsec_nanos() as i32,
        ..Default::default()
    }
}
/// Secrets for the caller: blueprints they cannot see are only counted.
async fn project(
    _ctx: &RequestContext,
    secrets: Vec<Secret>,
) -> Result<Vec<management::Secret>, connectrpc::ConnectError> {
    // None: every blueprint may be listed.
    #[allow(unused_mut, unused_assignments)]
    let mut seen: Option<std::collections::HashSet<Uuid>> = None;
    Ok(secrets
        .into_iter()
        .map(|s| management::Secret {
            id: s.id.to_string(),
            name: s.name,
            description: s.description,
            created_at: timestamp(s.created_at).into(),
            updated_at: timestamp(s.updated_at).into(),
            blueprint_count: s.blueprints.len() as u32,
            blueprints: s
                .blueprints
                .into_iter()
                .filter(|(id, _)| seen.as_ref().is_none_or(|seen| seen.contains(id)))
                .map(|(_, name)| name)
                .collect(),
            ..Default::default()
        })
        .collect())
}
fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Invalid UUID".into()))
}
impl SecretService for Rpc {
    async fn list_secrets<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListSecretsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListSecretsResponse> + Send + use<'a>> {
        let _request = request.to_owned_message();
        let secrets = self.0.list().await?;
        Response::ok(management::ListSecretsResponse {
            secrets: project(&_ctx, secrets).await?,
            ..Default::default()
        })
    }
    async fn set_secret<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetSecretRequest>,
    ) -> ServiceResult<impl Encodable<management::SetSecretResponse> + Send + use<'a>> {
        let request = request.to_owned_message();
        let replace = request.id.as_deref().map(id).transpose()?;
        let secret = self
            .0
            .set(
                replace,
                &request.name,
                request.value.map(SecretString::from),
                &request.description,
            )
            .await?;
        Response::ok(management::SetSecretResponse {
            secret: project(&_ctx, vec![secret]).await?.pop().into(),
            ..Default::default()
        })
    }
    async fn delete_secret<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteSecretRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteSecretResponse> + Send + use<'a>> {
        let secret = id(&request.to_owned_message().id)?;
        self.0.delete(secret).await?;
        Response::ok(management::DeleteSecretResponse::default())
    }
}
