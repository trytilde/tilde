///Shorthand for `OwnedView<GetTracingStatusRequestView<'static>>`.
pub type OwnedGetTracingStatusRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetTracingStatusRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetTracingStatusResponseView<'static>>`.
pub type OwnedGetTracingStatusResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetTracingStatusResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ListObservationsRequestView<'static>>`.
pub type OwnedListObservationsRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListObservationsRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ListObservationsResponseView<'static>>`.
pub type OwnedListObservationsResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListObservationsResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetTraceRequestView<'static>>`.
pub type OwnedGetTraceRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetTraceRequestView<'static>,
>;
///Shorthand for `OwnedView<GetTraceResponseView<'static>>`.
pub type OwnedGetTraceResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetTraceResponseView<'static>,
>;
///Shorthand for `OwnedView<GetSessionRequestView<'static>>`.
pub type OwnedGetSessionRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetSessionRequestView<'static>,
>;
///Shorthand for `OwnedView<GetSessionResponseView<'static>>`.
pub type OwnedGetSessionResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetSessionResponseView<'static>,
>;
impl ::connectrpc::Encodable<
    crate::proto::tilde::management::v1::GetTracingStatusResponse,
>
for crate::proto::tilde::management::v1::__buffa::view::GetTracingStatusResponseView<
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
    crate::proto::tilde::management::v1::GetTracingStatusResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetTracingStatusResponseView<
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
    crate::proto::tilde::management::v1::ListObservationsResponse,
>
for crate::proto::tilde::management::v1::__buffa::view::ListObservationsResponseView<
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
    crate::proto::tilde::management::v1::ListObservationsResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListObservationsResponseView<
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
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::GetTraceResponse>
for crate::proto::tilde::management::v1::__buffa::view::GetTraceResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::GetTraceResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetTraceResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::GetSessionResponse>
for crate::proto::tilde::management::v1::__buffa::view::GetSessionResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::GetSessionResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetSessionResponseView<'static>,
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
pub const TRACING_SERVICE_SERVICE_NAME: &str = "tilde.management.v1.TracingService";
/// Static [`Spec`](::connectrpc::Spec) for the `GetTracingStatus` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const TRACING_SERVICE_GET_TRACING_STATUS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.TracingService/GetTracingStatus",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ListObservations` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const TRACING_SERVICE_LIST_OBSERVATIONS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.TracingService/ListObservations",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `GetTrace` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const TRACING_SERVICE_GET_TRACE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.TracingService/GetTrace",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `GetSession` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const TRACING_SERVICE_GET_SESSION_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.TracingService/GetSession",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Read-only observability. Langfuse credentials never cross this API boundary.
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
pub trait TracingService: Send + Sync + 'static {
    /// Handle the GetTracingStatus RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_tracing_status<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::GetTracingStatusRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::GetTracingStatusResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the ListObservations RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_observations<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::ListObservationsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::ListObservationsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the GetTrace RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_trace<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::GetTraceRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::GetTraceResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the GetSession RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_session<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::GetSessionRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::GetSessionResponse,
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
pub trait TracingServiceExt: TracingService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: TracingService> TracingServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_idempotent(
                TRACING_SERVICE_SERVICE_NAME,
                "GetTracingStatus",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::GetTracingStatusRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::GetTracingStatusRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_tracing_status(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::GetTracingStatusResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(TRACING_SERVICE_GET_TRACING_STATUS_SPEC)
            .route_view_idempotent(
                TRACING_SERVICE_SERVICE_NAME,
                "ListObservations",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::ListObservationsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::ListObservationsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_observations(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::ListObservationsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(TRACING_SERVICE_LIST_OBSERVATIONS_SPEC)
            .route_view_idempotent(
                TRACING_SERVICE_SERVICE_NAME,
                "GetTrace",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::GetTraceRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::GetTraceRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_trace(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::GetTraceResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(TRACING_SERVICE_GET_TRACE_SPEC)
            .route_view_idempotent(
                TRACING_SERVICE_SERVICE_NAME,
                "GetSession",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::GetSessionRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::GetSessionRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_session(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::GetSessionResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(TRACING_SERVICE_GET_SESSION_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct TracingServiceRegisterMarker;
impl<S: TracingService> ::connectrpc::ServiceRegister<TracingServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as TracingServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `TracingService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = TracingServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct TracingServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: TracingService> TracingServiceServer<T> {
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
impl<T> Clone for TracingServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: TracingService> ::connectrpc::Dispatcher for TracingServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("tilde.management.v1.TracingService/")?;
        match method {
            "GetTracingStatus" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(TRACING_SERVICE_GET_TRACING_STATUS_SPEC),
                )
            }
            "ListObservations" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(TRACING_SERVICE_LIST_OBSERVATIONS_SPEC),
                )
            }
            "GetTrace" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(TRACING_SERVICE_GET_TRACE_SPEC),
                )
            }
            "GetSession" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(TRACING_SERVICE_GET_SESSION_SPEC),
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
        let Some(method) = path.strip_prefix("tilde.management.v1.TracingService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "GetTracingStatus" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::GetTracingStatusRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::GetTracingStatusRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::GetTracingStatusRequest,
                    >::from_parts(&req, &body);
                    svc.get_tracing_status(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::GetTracingStatusResponse,
                        >(format)
                })
            }
            "ListObservations" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::ListObservationsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::ListObservationsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::ListObservationsRequest,
                    >::from_parts(&req, &body);
                    svc.list_observations(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::ListObservationsResponse,
                        >(format)
                })
            }
            "GetTrace" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::GetTraceRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::GetTraceRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::GetTraceRequest,
                    >::from_parts(&req, &body);
                    svc.get_trace(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::GetTraceResponse,
                        >(format)
                })
            }
            "GetSession" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::GetSessionRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::GetSessionRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::GetSessionRequest,
                    >::from_parts(&req, &body);
                    svc.get_session(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::GetSessionResponse,
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
        let Some(method) = path.strip_prefix("tilde.management.v1.TracingService/") else {
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
        let Some(method) = path.strip_prefix("tilde.management.v1.TracingService/") else {
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
        let Some(method) = path.strip_prefix("tilde.management.v1.TracingService/") else {
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
/// let client = TracingServiceClient::new(conn, config);
/// let response = client.get_tracing_status(request).await?;
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
/// let client = TracingServiceClient::new(http, config);
/// let response = client.get_tracing_status(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.get_tracing_status(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.get_tracing_status(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct TracingServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> TracingServiceClient<T>
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
    /// Call the GetTracingStatus RPC. Sends a request to /tilde.management.v1.TracingService/GetTracingStatus.
    pub async fn get_tracing_status(
        &self,
        request: crate::proto::tilde::management::v1::GetTracingStatusRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetTracingStatusResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_tracing_status_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetTracingStatus RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_tracing_status_with_options(
        &self,
        request: crate::proto::tilde::management::v1::GetTracingStatusRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetTracingStatusResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                TRACING_SERVICE_GET_TRACING_STATUS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListObservations RPC. Sends a request to /tilde.management.v1.TracingService/ListObservations.
    pub async fn list_observations(
        &self,
        request: crate::proto::tilde::management::v1::ListObservationsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListObservationsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_observations_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListObservations RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_observations_with_options(
        &self,
        request: crate::proto::tilde::management::v1::ListObservationsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListObservationsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                TRACING_SERVICE_LIST_OBSERVATIONS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetTrace RPC. Sends a request to /tilde.management.v1.TracingService/GetTrace.
    pub async fn get_trace(
        &self,
        request: crate::proto::tilde::management::v1::GetTraceRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetTraceResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_trace_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetTrace RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_trace_with_options(
        &self,
        request: crate::proto::tilde::management::v1::GetTraceRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetTraceResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                TRACING_SERVICE_GET_TRACE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetSession RPC. Sends a request to /tilde.management.v1.TracingService/GetSession.
    pub async fn get_session(
        &self,
        request: crate::proto::tilde::management::v1::GetSessionRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetSessionResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_session_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetSession RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_session_with_options(
        &self,
        request: crate::proto::tilde::management::v1::GetSessionRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetSessionResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                TRACING_SERVICE_GET_SESSION_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
