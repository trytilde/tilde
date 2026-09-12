///Shorthand for `OwnedView<GetDeploymentRequestView<'static>>`.
pub type OwnedGetDeploymentRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetDeploymentRequestView<'static>,
>;
///Shorthand for `OwnedView<GetDeploymentResponseView<'static>>`.
pub type OwnedGetDeploymentResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetDeploymentResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<SetDeploymentRequestView<'static>>`.
pub type OwnedSetDeploymentRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::SetDeploymentRequestView<'static>,
>;
///Shorthand for `OwnedView<SetDeploymentResponseView<'static>>`.
pub type OwnedSetDeploymentResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::SetDeploymentResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<IssueDeploymentTokenRequestView<'static>>`.
pub type OwnedIssueDeploymentTokenRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::IssueDeploymentTokenRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<IssueDeploymentTokenResponseView<'static>>`.
pub type OwnedIssueDeploymentTokenResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::IssueDeploymentTokenResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<CreateSidecarAgentRequestView<'static>>`.
pub type OwnedCreateSidecarAgentRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateSidecarAgentRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<CreateSidecarAgentResponseView<'static>>`.
pub type OwnedCreateSidecarAgentResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateSidecarAgentResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<IssueIngressTokenRequestView<'static>>`.
pub type OwnedIssueIngressTokenRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::IssueIngressTokenRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<IssueIngressTokenResponseView<'static>>`.
pub type OwnedIssueIngressTokenResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::IssueIngressTokenResponseView<
        'static,
    >,
>;
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::GetDeploymentResponse>
for crate::proto::tilde::management::v1::__buffa::view::GetDeploymentResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::GetDeploymentResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetDeploymentResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::SetDeploymentResponse>
for crate::proto::tilde::management::v1::__buffa::view::SetDeploymentResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::SetDeploymentResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::SetDeploymentResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<
    crate::proto::tilde::management::v1::IssueDeploymentTokenResponse,
>
for crate::proto::tilde::management::v1::__buffa::view::IssueDeploymentTokenResponseView<
    '_,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::proto::tilde::management::v1::IssueDeploymentTokenResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::IssueDeploymentTokenResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<
    crate::proto::tilde::management::v1::CreateSidecarAgentResponse,
>
for crate::proto::tilde::management::v1::__buffa::view::CreateSidecarAgentResponseView<
    '_,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::proto::tilde::management::v1::CreateSidecarAgentResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateSidecarAgentResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<
    crate::proto::tilde::management::v1::IssueIngressTokenResponse,
>
for crate::proto::tilde::management::v1::__buffa::view::IssueIngressTokenResponseView<
    '_,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::proto::tilde::management::v1::IssueIngressTokenResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::IssueIngressTokenResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
/// Full service name for this service.
pub const DEPLOYMENT_SERVICE_SERVICE_NAME: &str = "tilde.management.v1.DeploymentService";
/// Static [`Spec`](::connectrpc::Spec) for the `GetDeployment` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const DEPLOYMENT_SERVICE_GET_DEPLOYMENT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.DeploymentService/GetDeployment",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `SetDeployment` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const DEPLOYMENT_SERVICE_SET_DEPLOYMENT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.DeploymentService/SetDeployment",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `IssueDeploymentToken` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const DEPLOYMENT_SERVICE_ISSUE_DEPLOYMENT_TOKEN_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.DeploymentService/IssueDeploymentToken",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `CreateSidecarAgent` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const DEPLOYMENT_SERVICE_CREATE_SIDECAR_AGENT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.DeploymentService/CreateSidecarAgent",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `IssueIngressToken` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const DEPLOYMENT_SERVICE_ISSUE_INGRESS_TOKEN_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.DeploymentService/IssueIngressToken",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Server trait for DeploymentService.
///
/// # Implementing handlers
///
/// Implement methods with plain `async fn`; the returned future satisfies
/// the `Send` bound automatically.
///
/// **Unary and server-streaming requests** arrive as
/// [`ServiceRequest<'_, Req>`](::connectrpc::ServiceRequest): a zero-copy
/// view of the request plus its body, valid for the duration of the call.
/// Fields are read directly (`request.name` is a `&str` into the decoded
/// buffer) and the borrow may be held across `.await` points. Anything
/// that must outlive the call — `tokio::spawn`, channels, server state,
/// or data captured by a returned response stream — takes owned data:
/// call `request.to_owned_message()` (or copy the specific fields)
/// first.
///
/// **Client-streaming and bidi requests** arrive as
/// [`InboundStream<Req>`](::connectrpc::InboundStream) — a
/// `ServiceStream` of [`StreamMessage`](::connectrpc::StreamMessage)s.
/// Each item owns its decoded buffer and is `Send + 'static`, so items
/// can be buffered or moved into spawned tasks; read fields zero-copy
/// through the generated accessor methods (`item.name()`) or `.view()`,
/// convert with `.to_owned_message()`, or yield an item back unchanged —
/// `StreamMessage<M>` implements `Encodable<M>`.
///
/// Request types resolved through `extern_path` (e.g. well-known types
/// from another crate) use the same wrappers; the crate that owns the
/// type must be generated with buffa ≥ 0.9.0 and views enabled so the
/// backing `HasMessageView` impl exists.
///
/// The `impl Encodable<Out>` return bound accepts the owned `Out`, the
/// generated `OutView<'_>` / `OwnedOutView`,
/// [`MaybeBorrowed`](::connectrpc::MaybeBorrowed), or
/// [`PreEncoded`](::connectrpc::PreEncoded) for handlers that encode a
/// non-`'static` view internally and pass the bytes across the handler
/// boundary. View bodies are not emitted for output types mapped via
/// `extern_path` (the impl would be an orphan); return owned for
/// WKT/extern outputs.
///
/// Server-streaming and bidi-streaming methods return
/// `ServiceStream<impl Encodable<Out> + Send + use<Self>>`. The
/// `use<Self>` precise-capturing clause excludes `&self`'s lifetime and
/// the request's lifetime (unary methods use `use<'a, Self>` and may
/// borrow from `&self`), so stream items must be `'static` and cannot
/// borrow from the request. To stream view-encoded data, encode each
/// item inside the stream body and yield
/// [`PreEncoded`](::connectrpc::PreEncoded) — see its `# Streaming
/// example` doc.
#[allow(clippy::type_complexity)]
pub trait DeploymentService: Send + Sync + 'static {
    /// Handle the GetDeployment RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_deployment<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::GetDeploymentRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::GetDeploymentResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the SetDeployment RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn set_deployment<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::SetDeploymentRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::SetDeploymentResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the IssueDeploymentToken RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn issue_deployment_token<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::IssueDeploymentTokenRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::IssueDeploymentTokenResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the CreateSidecarAgent RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn create_sidecar_agent<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::CreateSidecarAgentRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::CreateSidecarAgentResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the IssueIngressToken RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn issue_ingress_token<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::IssueIngressTokenRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::IssueIngressTokenResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
}
/// Extension trait for registering a service implementation with a Router.
///
/// This trait is automatically implemented for all types that implement the service trait.
/// Prefer [`Router::add_service`](::connectrpc::Router::add_service) for
/// top-down registration; `register` remains available for compatibility
/// and cases where the service-first call shape is more convenient.
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
///
/// let service = Arc::new(MyServiceImpl);
/// let router = service.register(Router::new());
/// ```
pub trait DeploymentServiceExt: DeploymentService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: DeploymentService> DeploymentServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                DEPLOYMENT_SERVICE_SERVICE_NAME,
                "GetDeployment",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::GetDeploymentRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::GetDeploymentRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_deployment(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::GetDeploymentResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(DEPLOYMENT_SERVICE_GET_DEPLOYMENT_SPEC)
            .route_view(
                DEPLOYMENT_SERVICE_SERVICE_NAME,
                "SetDeployment",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::SetDeploymentRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::SetDeploymentRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.set_deployment(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::SetDeploymentResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(DEPLOYMENT_SERVICE_SET_DEPLOYMENT_SPEC)
            .route_view(
                DEPLOYMENT_SERVICE_SERVICE_NAME,
                "IssueDeploymentToken",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::IssueDeploymentTokenRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::IssueDeploymentTokenRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.issue_deployment_token(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::IssueDeploymentTokenResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(DEPLOYMENT_SERVICE_ISSUE_DEPLOYMENT_TOKEN_SPEC)
            .route_view(
                DEPLOYMENT_SERVICE_SERVICE_NAME,
                "CreateSidecarAgent",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::CreateSidecarAgentRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::CreateSidecarAgentRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.create_sidecar_agent(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::CreateSidecarAgentResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(DEPLOYMENT_SERVICE_CREATE_SIDECAR_AGENT_SPEC)
            .route_view(
                DEPLOYMENT_SERVICE_SERVICE_NAME,
                "IssueIngressToken",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::IssueIngressTokenRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::IssueIngressTokenRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.issue_ingress_token(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::IssueIngressTokenResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(DEPLOYMENT_SERVICE_ISSUE_INGRESS_TOKEN_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct DeploymentServiceRegisterMarker;
impl<S: DeploymentService> ::connectrpc::ServiceRegister<DeploymentServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as DeploymentServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `DeploymentService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = DeploymentServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct DeploymentServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: DeploymentService> DeploymentServiceServer<T> {
    /// Wrap a service implementation in a monomorphic dispatcher.
    pub fn new(service: T) -> Self {
        Self {
            inner: ::std::sync::Arc::new(service),
        }
    }
    /// Wrap an already-`Arc`'d service implementation.
    pub fn from_arc(inner: ::std::sync::Arc<T>) -> Self {
        Self { inner }
    }
}
impl<T> Clone for DeploymentServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: DeploymentService> ::connectrpc::Dispatcher for DeploymentServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("tilde.management.v1.DeploymentService/")?;
        match method {
            "GetDeployment" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(DEPLOYMENT_SERVICE_GET_DEPLOYMENT_SPEC),
                )
            }
            "SetDeployment" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(DEPLOYMENT_SERVICE_SET_DEPLOYMENT_SPEC),
                )
            }
            "IssueDeploymentToken" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(DEPLOYMENT_SERVICE_ISSUE_DEPLOYMENT_TOKEN_SPEC),
                )
            }
            "CreateSidecarAgent" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(DEPLOYMENT_SERVICE_CREATE_SIDECAR_AGENT_SPEC),
                )
            }
            "IssueIngressToken" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(DEPLOYMENT_SERVICE_ISSUE_INGRESS_TOKEN_SPEC),
                )
            }
            _ => None,
        }
    }
    fn call_unary(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::Payload,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::UnaryResult {
        let Some(method) = path.strip_prefix("tilde.management.v1.DeploymentService/")
        else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "GetDeployment" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::GetDeploymentRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::GetDeploymentRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::GetDeploymentRequest,
                    >::from_parts(&req, &body);
                    svc.get_deployment(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::GetDeploymentResponse,
                        >(format)
                })
            }
            "SetDeployment" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::SetDeploymentRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::SetDeploymentRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::SetDeploymentRequest,
                    >::from_parts(&req, &body);
                    svc.set_deployment(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::SetDeploymentResponse,
                        >(format)
                })
            }
            "IssueDeploymentToken" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::IssueDeploymentTokenRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::IssueDeploymentTokenRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::IssueDeploymentTokenRequest,
                    >::from_parts(&req, &body);
                    svc.issue_deployment_token(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::IssueDeploymentTokenResponse,
                        >(format)
                })
            }
            "CreateSidecarAgent" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::CreateSidecarAgentRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::CreateSidecarAgentRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::CreateSidecarAgentRequest,
                    >::from_parts(&req, &body);
                    svc.create_sidecar_agent(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::CreateSidecarAgentResponse,
                        >(format)
                })
            }
            "IssueIngressToken" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::IssueIngressTokenRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::IssueIngressTokenRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::IssueIngressTokenRequest,
                    >::from_parts(&req, &body);
                    svc.issue_ingress_token(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::IssueIngressTokenResponse,
                        >(format)
                })
            }
            _ => ::connectrpc::dispatcher::codegen::unimplemented_unary(path),
        }
    }
    fn call_server_streaming(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        request: ::buffa::bytes::Bytes,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::StreamingResult {
        let Some(method) = path.strip_prefix("tilde.management.v1.DeploymentService/")
        else {
            return ::connectrpc::dispatcher::codegen::unimplemented_streaming(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            _ => ::connectrpc::dispatcher::codegen::unimplemented_streaming(path),
        }
    }
    fn call_client_streaming(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        requests: ::connectrpc::dispatcher::codegen::RequestStream,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::UnaryResult {
        let Some(method) = path.strip_prefix("tilde.management.v1.DeploymentService/")
        else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &requests, &format);
        match method {
            _ => ::connectrpc::dispatcher::codegen::unimplemented_unary(path),
        }
    }
    fn call_bidi_streaming(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        requests: ::connectrpc::dispatcher::codegen::RequestStream,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::StreamingResult {
        let Some(method) = path.strip_prefix("tilde.management.v1.DeploymentService/")
        else {
            return ::connectrpc::dispatcher::codegen::unimplemented_streaming(path);
        };
        let _ = (&ctx, &requests, &format);
        match method {
            _ => ::connectrpc::dispatcher::codegen::unimplemented_streaming(path),
        }
    }
}
/// Client for this service.
///
/// Generic over `T: ClientTransport`. For **gRPC** (HTTP/2), use
/// `Http2Connection` — it has honest `poll_ready` and composes with
/// `tower::balance` for multi-connection load balancing. For **Connect
/// over HTTP/1.1** (or unknown protocol), use `HttpClient`.
///
/// # Example (gRPC / HTTP/2)
///
/// ```rust,ignore
/// use connectrpc::client::{Http2Connection, ClientConfig};
/// use connectrpc::Protocol;
///
/// let uri: http::Uri = "http://localhost:8080".parse()?;
/// let conn = Http2Connection::connect_plaintext(uri.clone()).await?.shared(1024);
/// let config = ClientConfig::new(uri).with_protocol(Protocol::Grpc);
///
/// let client = DeploymentServiceClient::new(conn, config);
/// let response = client.get_deployment(request).await?;
/// ```
///
/// # Example (Connect / HTTP/1.1 or ALPN)
///
/// ```rust,ignore
/// use connectrpc::client::{HttpClient, ClientConfig};
///
/// let http = HttpClient::plaintext();  // cleartext http:// only
/// let config = ClientConfig::new("http://localhost:8080".parse()?);
///
/// let client = DeploymentServiceClient::new(http, config);
/// let response = client.get_deployment(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.get_deployment(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.get_deployment(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct DeploymentServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> DeploymentServiceClient<T>
where
    T: ::connectrpc::client::ClientTransport,
    <T::ResponseBody as ::connectrpc::http_body::Body>::Error: ::std::fmt::Display,
{
    /// Create a new client with the given transport and configuration.
    pub fn new(transport: T, config: ::connectrpc::client::ClientConfig) -> Self {
        Self { transport, config }
    }
    /// Get the client configuration.
    pub fn config(&self) -> &::connectrpc::client::ClientConfig {
        &self.config
    }
    /// Get a mutable reference to the client configuration.
    pub fn config_mut(&mut self) -> &mut ::connectrpc::client::ClientConfig {
        &mut self.config
    }
    /// Call the GetDeployment RPC. Sends a request to /tilde.management.v1.DeploymentService/GetDeployment.
    pub async fn get_deployment(
        &self,
        request: crate::proto::tilde::management::v1::GetDeploymentRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetDeploymentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_deployment_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetDeployment RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_deployment_with_options(
        &self,
        request: crate::proto::tilde::management::v1::GetDeploymentRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetDeploymentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                DEPLOYMENT_SERVICE_GET_DEPLOYMENT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the SetDeployment RPC. Sends a request to /tilde.management.v1.DeploymentService/SetDeployment.
    pub async fn set_deployment(
        &self,
        request: crate::proto::tilde::management::v1::SetDeploymentRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::SetDeploymentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.set_deployment_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the SetDeployment RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn set_deployment_with_options(
        &self,
        request: crate::proto::tilde::management::v1::SetDeploymentRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::SetDeploymentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                DEPLOYMENT_SERVICE_SET_DEPLOYMENT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the IssueDeploymentToken RPC. Sends a request to /tilde.management.v1.DeploymentService/IssueDeploymentToken.
    pub async fn issue_deployment_token(
        &self,
        request: crate::proto::tilde::management::v1::IssueDeploymentTokenRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::IssueDeploymentTokenResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.issue_deployment_token_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the IssueDeploymentToken RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn issue_deployment_token_with_options(
        &self,
        request: crate::proto::tilde::management::v1::IssueDeploymentTokenRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::IssueDeploymentTokenResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                DEPLOYMENT_SERVICE_ISSUE_DEPLOYMENT_TOKEN_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the CreateSidecarAgent RPC. Sends a request to /tilde.management.v1.DeploymentService/CreateSidecarAgent.
    pub async fn create_sidecar_agent(
        &self,
        request: crate::proto::tilde::management::v1::CreateSidecarAgentRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::CreateSidecarAgentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.create_sidecar_agent_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CreateSidecarAgent RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn create_sidecar_agent_with_options(
        &self,
        request: crate::proto::tilde::management::v1::CreateSidecarAgentRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::CreateSidecarAgentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                DEPLOYMENT_SERVICE_CREATE_SIDECAR_AGENT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the IssueIngressToken RPC. Sends a request to /tilde.management.v1.DeploymentService/IssueIngressToken.
    pub async fn issue_ingress_token(
        &self,
        request: crate::proto::tilde::management::v1::IssueIngressTokenRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::IssueIngressTokenResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.issue_ingress_token_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the IssueIngressToken RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn issue_ingress_token_with_options(
        &self,
        request: crate::proto::tilde::management::v1::IssueIngressTokenRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::IssueIngressTokenResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                DEPLOYMENT_SERVICE_ISSUE_INGRESS_TOKEN_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
