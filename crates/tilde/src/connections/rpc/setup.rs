use super::*;
use crate::connections::{model as m, service::Connections};
use crate::proto::tilde::setup::v1 as setup_pb;
use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use secrecy::SecretString;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;
struct Rpc(Connections);
use crate::services::tilde::setup::v1::ConnectionSetupService;
pub fn router(connections: Connections) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(connections.clone()))),
        2 * 1024 * 1024,
    )
    .merge(
        axum::Router::new()
            .route(
                crate::connections::CALLBACK_PATH,
                axum::routing::get(callback).post(callback_form),
            )
            .with_state(connections),
    )
}
#[derive(serde::Deserialize)]
struct Callback {
    state: String,
    #[serde(flatten)]
    parameters: BTreeMap<String, String>,
    error: Option<String>,
}
impl Drop for Callback {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.state.zeroize();
        self.error.zeroize();
        for (mut key, mut value) in std::mem::take(&mut self.parameters) {
            key.zeroize();
            value.zeroize();
        }
    }
}
async fn callback(
    axum::extract::State(service): axum::extract::State<Connections>,
    axum::extract::Query(query): axum::extract::Query<Callback>,
) -> axum::response::Response {
    handle_callback(service, query).await
}
async fn callback_form(
    axum::extract::State(service): axum::extract::State<Connections>,
    axum::Form(query): axum::Form<Callback>,
) -> axum::response::Response {
    handle_callback(service, query).await
}
async fn handle_callback(service: Connections, query: Callback) -> axum::response::Response {
    use axum::response::IntoResponse;
    let Some((setup, state)) = query.state.split_once('.') else {
        return (http::StatusCode::BAD_REQUEST, "Invalid callback state").into_response();
    };
    let Ok(setup) = Uuid::parse_str(setup) else {
        return (http::StatusCode::BAD_REQUEST, "Invalid callback state").into_response();
    };
    let parameters = query
        .parameters
        .iter()
        .map(|(key, value)| (key.clone(), SecretString::from(value.as_str())))
        .collect();
    let call = service.callback(setup, state, &parameters, query.error.is_some());
    let mut response = match call.await {
        Ok(url) => axum::response::Redirect::to(&url).into_response(),
        Err(_) => (
            http::StatusCode::BAD_REQUEST,
            "Invalid, expired, or already-consumed callback",
        )
            .into_response(),
    };
    response.headers_mut().insert(
        http::header::CACHE_CONTROL,
        http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        "referrer-policy",
        http::HeaderValue::from_static("no-referrer"),
    );
    response
}
impl Rpc {
    /// Run one setup call; the setup names its organization (ADR-0005).
    async fn in_setup<T>(
        &self,
        setup: Uuid,
        call: impl std::future::Future<Output = Result<T, Error>>,
    ) -> Result<T, Error> {
        let _ = setup; // Open-source Tilde has a single organization.
        call.await
    }
}
impl ConnectionSetupService for Rpc {
    async fn set_connection_name<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, setup_pb::SetConnectionNameRequest>,
    ) -> ServiceResult<impl Encodable<setup_pb::SetConnectionNameResponse> + Send + use<'a>> {
        let setup = id(request.setup_id)?;
        let view = self
            .in_setup(
                setup,
                self.0.set_connection_name(
                    setup,
                    request.connection_setup_token,
                    id(request.action_id)?,
                    request.name,
                ),
            )
            .await?;
        Response::ok(setup_pb::SetConnectionNameResponse {
            state: broker_wire(view).into(),
            ..Default::default()
        })
    }

    async fn save_draft<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, setup_pb::SaveDraftRequest>,
    ) -> ServiceResult<impl Encodable<setup_pb::SaveDraftResponse> + Send + use<'a>> {
        let body = request.to_owned_message();
        let values = input_values(body.draft)?;
        let setup = id(&body.setup_id)?;
        let view = self
            .in_setup(
                setup,
                self.0.save_draft(
                    setup,
                    &body.connection_setup_token,
                    id(&body.action_id)?,
                    values,
                ),
            )
            .await?;
        Response::ok(setup_pb::SaveDraftResponse {
            state: broker_wire(view).into(),
            ..Default::default()
        })
    }
    async fn start_o_auth<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, setup_pb::StartOAuthRequest>,
    ) -> ServiceResult<impl Encodable<setup_pb::StartOAuthResponse> + Send + use<'a>> {
        let body = request.to_owned_message();
        let values = input_values(body.fields)?;
        let setup = id(&body.setup_id)?;
        let view = self
            .in_setup(
                setup,
                self.0.start_oauth(
                    setup,
                    &body.connection_setup_token,
                    id(&body.action_id)?,
                    values,
                ),
            )
            .await?;
        Response::ok(setup_pb::StartOAuthResponse {
            state: broker_wire(view).into(),
            ..Default::default()
        })
    }
    async fn save_credentials<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, setup_pb::SaveCredentialsRequest>,
    ) -> ServiceResult<impl Encodable<setup_pb::SaveCredentialsResponse> + Send + use<'a>> {
        let body = request.to_owned_message();
        let values = input_values(body.fields)?;
        let setup = id(&body.setup_id)?;
        let view = self
            .in_setup(
                setup,
                self.0.save_credentials(
                    setup,
                    &body.connection_setup_token,
                    id(&body.action_id)?,
                    values,
                ),
            )
            .await?;
        Response::ok(setup_pb::SaveCredentialsResponse {
            state: broker_wire(view).into(),
            ..Default::default()
        })
    }
    async fn get_setup<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, setup_pb::GetSetupRequest>,
    ) -> ServiceResult<impl Encodable<setup_pb::GetSetupResponse> + Send + use<'a>> {
        let setup = id(request.setup_id)?;
        let view = self
            .in_setup(setup, self.0.view(setup, request.connection_setup_token))
            .await?;
        Response::ok(setup_pb::GetSetupResponse {
            state: broker_wire(view).into(),
            ..Default::default()
        })
    }
    async fn execute_provider_action<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, setup_pb::ExecuteProviderActionRequest>,
    ) -> ServiceResult<impl Encodable<setup_pb::ExecuteProviderActionResponse> + Send + use<'a>>
    {
        let body = request.to_owned_message();
        let mut values = m::Values::new();
        for field in body.fields {
            if values
                .insert(field.key, SecretString::from(field.value))
                .is_some()
            {
                return Err(m::invalid("Duplicate input field").into());
            }
        }
        let setup = id(&body.setup_id)?;
        let view = self
            .in_setup(
                setup,
                self.0.provider_action(
                    setup,
                    &body.connection_setup_token,
                    id(&body.action_id)?,
                    &body.action,
                    values,
                ),
            )
            .await?;
        Response::ok(setup_pb::ExecuteProviderActionResponse {
            state: broker_wire(view).into(),
            ..Default::default()
        })
    }
    async fn cancel_setup<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, setup_pb::CancelSetupRequest>,
    ) -> ServiceResult<impl Encodable<setup_pb::CancelSetupResponse> + Send + use<'a>> {
        let setup = id(request.setup_id)?;
        let view = self
            .in_setup(setup, self.0.cancel(setup, request.connection_setup_token))
            .await?;
        Response::ok(setup_pb::CancelSetupResponse {
            state: broker_wire(view).into(),
            ..Default::default()
        })
    }
}
