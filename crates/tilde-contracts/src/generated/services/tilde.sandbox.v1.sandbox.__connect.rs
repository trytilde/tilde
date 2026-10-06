///Shorthand for `OwnedView<ConnectRequestView<'static>>`.
pub type OwnedConnectRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::ConnectRequestView<'static>,
>;
///Shorthand for `OwnedView<ConnectResponseView<'static>>`.
pub type OwnedConnectResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::ConnectResponseView<'static>,
>;
///Shorthand for `OwnedView<RespondRequestView<'static>>`.
pub type OwnedRespondRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::RespondRequestView<'static>,
>;
///Shorthand for `OwnedView<RespondResponseView<'static>>`.
pub type OwnedRespondResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::RespondResponseView<'static>,
>;
///Shorthand for `OwnedView<ListToolsRequestView<'static>>`.
pub type OwnedListToolsRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::ListToolsRequestView<'static>,
>;
///Shorthand for `OwnedView<ListToolsResponseView<'static>>`.
pub type OwnedListToolsResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::ListToolsResponseView<'static>,
>;
///Shorthand for `OwnedView<InvokeToolRequestView<'static>>`.
pub type OwnedInvokeToolRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::InvokeToolRequestView<'static>,
>;
///Shorthand for `OwnedView<InvokeToolResponseView<'static>>`.
pub type OwnedInvokeToolResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::InvokeToolResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::proto::tilde::sandbox::v1::ConnectResponse>
for crate::proto::tilde::sandbox::v1::__buffa::view::ConnectResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::sandbox::v1::ConnectResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::ConnectResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::sandbox::v1::RespondResponse>
for crate::proto::tilde::sandbox::v1::__buffa::view::RespondResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::sandbox::v1::RespondResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::RespondResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::sandbox::v1::ListToolsResponse>
for crate::proto::tilde::sandbox::v1::__buffa::view::ListToolsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::sandbox::v1::ListToolsResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::ListToolsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::sandbox::v1::InvokeToolResponse>
for crate::proto::tilde::sandbox::v1::__buffa::view::InvokeToolResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::sandbox::v1::InvokeToolResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::sandbox::v1::__buffa::view::InvokeToolResponseView<'static>,
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
pub const SANDBOX_SERVICE_SERVICE_NAME: &str = "tilde.sandbox.v1.SandboxService";
/// Static [`Spec`](::connectrpc::Spec) for the `Connect` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SANDBOX_SERVICE_CONNECT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.sandbox.v1.SandboxService/Connect",
        ::connectrpc::StreamType::ServerStream,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `Respond` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SANDBOX_SERVICE_RESPOND_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.sandbox.v1.SandboxService/Respond",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ListTools` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SANDBOX_SERVICE_LIST_TOOLS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.sandbox.v1.SandboxService/ListTools",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `InvokeTool` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SANDBOX_SERVICE_INVOKE_TOOL_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.sandbox.v1.SandboxService/InvokeTool",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// The `tilde sandbox connect` process inside a sandbox VM. Like a tool host it is never dialed:
/// Tilde launches the VM with a single-use enrollment token, which the process's first Connect
/// exchanges for a session token that authenticates every later request (including reconnecting
/// Connects, for example after the VM was paused and resumed). Over the Connect stream it receives
/// the agent's sandbox operations as frames and answers each through Respond. Processes inside the
/// VM reach the blueprint's tools through it with ListTools and InvokeTool.
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
pub trait SandboxService: Send + Sync + 'static {
    /// Handle the Connect RPC.
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call (until the response stream is returned);
    /// message fields are read directly on it (zero-copy). Data the
    /// returned stream needs must be copied out or converted via
    /// `.to_owned_message()`.
    fn connect(
        &self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::sandbox::v1::ConnectRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            ::connectrpc::ServiceStream<
                impl ::connectrpc::Encodable<
                    crate::proto::tilde::sandbox::v1::ConnectResponse,
                > + Send + use<Self>,
            >,
        >,
    > + Send;
    /// Handle the Respond RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn respond<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::sandbox::v1::RespondRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::sandbox::v1::RespondResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the ListTools RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_tools<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::sandbox::v1::ListToolsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::sandbox::v1::ListToolsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the InvokeTool RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn invoke_tool<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::sandbox::v1::InvokeToolRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::sandbox::v1::InvokeToolResponse,
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
pub trait SandboxServiceExt: SandboxService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: SandboxService> SandboxServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_server_stream::<
                _,
                _,
                crate::proto::tilde::sandbox::v1::ConnectResponse,
            >(
                SANDBOX_SERVICE_SERVICE_NAME,
                "Connect",
                ::connectrpc::view_streaming_handler_fn({
                    let svc = ::std::sync::Arc::clone(&self);
                    move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::sandbox::v1::__buffa::view::ConnectRequestView<
                                'static,
                            >,
                        >|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::sandbox::v1::ConnectRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.connect(ctx, sreq).await
                        }
                    }
                }),
            )
            .with_spec(SANDBOX_SERVICE_CONNECT_SPEC)
            .route_view(
                SANDBOX_SERVICE_SERVICE_NAME,
                "Respond",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::sandbox::v1::__buffa::view::RespondRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::sandbox::v1::RespondRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.respond(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::sandbox::v1::RespondResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SANDBOX_SERVICE_RESPOND_SPEC)
            .route_view(
                SANDBOX_SERVICE_SERVICE_NAME,
                "ListTools",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::sandbox::v1::__buffa::view::ListToolsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::sandbox::v1::ListToolsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_tools(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::sandbox::v1::ListToolsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SANDBOX_SERVICE_LIST_TOOLS_SPEC)
            .route_view(
                SANDBOX_SERVICE_SERVICE_NAME,
                "InvokeTool",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::sandbox::v1::__buffa::view::InvokeToolRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::sandbox::v1::InvokeToolRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.invoke_tool(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::sandbox::v1::InvokeToolResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SANDBOX_SERVICE_INVOKE_TOOL_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct SandboxServiceRegisterMarker;
impl<S: SandboxService> ::connectrpc::ServiceRegister<SandboxServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as SandboxServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `SandboxService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = SandboxServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct SandboxServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: SandboxService> SandboxServiceServer<T> {
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
impl<T> Clone for SandboxServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: SandboxService> ::connectrpc::Dispatcher for SandboxServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("tilde.sandbox.v1.SandboxService/")?;
        match method {
            "Connect" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::server_streaming()
                        .with_spec(SANDBOX_SERVICE_CONNECT_SPEC),
                )
            }
            "Respond" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SANDBOX_SERVICE_RESPOND_SPEC),
                )
            }
            "ListTools" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SANDBOX_SERVICE_LIST_TOOLS_SPEC),
                )
            }
            "InvokeTool" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SANDBOX_SERVICE_INVOKE_TOOL_SPEC),
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
        let Some(method) = path.strip_prefix("tilde.sandbox.v1.SandboxService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "Respond" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::sandbox::v1::RespondRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::sandbox::v1::__buffa::view::RespondRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::sandbox::v1::RespondRequest,
                    >::from_parts(&req, &body);
                    svc.respond(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::sandbox::v1::RespondResponse,
                        >(format)
                })
            }
            "ListTools" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::sandbox::v1::ListToolsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::sandbox::v1::__buffa::view::ListToolsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::sandbox::v1::ListToolsRequest,
                    >::from_parts(&req, &body);
                    svc.list_tools(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::sandbox::v1::ListToolsResponse,
                        >(format)
                })
            }
            "InvokeTool" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::sandbox::v1::InvokeToolRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::sandbox::v1::__buffa::view::InvokeToolRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::sandbox::v1::InvokeToolRequest,
                    >::from_parts(&req, &body);
                    svc.invoke_tool(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::sandbox::v1::InvokeToolResponse,
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
        let Some(method) = path.strip_prefix("tilde.sandbox.v1.SandboxService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_streaming(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "Connect" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::sandbox::v1::ConnectRequest,
                    >(request, format)?;
                    let req: crate::proto::tilde::sandbox::v1::__buffa::view::ConnectRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::sandbox::v1::ConnectRequest,
                    >::from_parts(&req, &body);
                    let resp = svc.connect(ctx, req).await?;
                    Ok(
                        resp
                            .map_body(|s| ::connectrpc::dispatcher::codegen::encode_response_stream::<
                                crate::proto::tilde::sandbox::v1::ConnectResponse,
                                _,
                                _,
                            >(s, format)),
                    )
                })
            }
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
        let Some(method) = path.strip_prefix("tilde.sandbox.v1.SandboxService/") else {
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
        let Some(method) = path.strip_prefix("tilde.sandbox.v1.SandboxService/") else {
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
/// let client = SandboxServiceClient::new(conn, config);
/// let response = client.connect(request).await?;
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
/// let client = SandboxServiceClient::new(http, config);
/// let response = client.connect(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.connect(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.connect(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct SandboxServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> SandboxServiceClient<T>
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
    /// Call the Connect RPC. Sends a request to /tilde.sandbox.v1.SandboxService/Connect.
    pub async fn connect(
        &self,
        request: crate::proto::tilde::sandbox::v1::ConnectRequest,
    ) -> Result<
        ::connectrpc::client::ServerStream<
            T::ResponseBody,
            crate::proto::tilde::sandbox::v1::__buffa::view::ConnectResponseView<'static>,
        >,
        ::connectrpc::ConnectError,
    > {
        self.connect_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the Connect RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn connect_with_options(
        &self,
        request: crate::proto::tilde::sandbox::v1::ConnectRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::ServerStream<
            T::ResponseBody,
            crate::proto::tilde::sandbox::v1::__buffa::view::ConnectResponseView<'static>,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_server_stream(
                &self.transport,
                &self.config,
                SANDBOX_SERVICE_CONNECT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the Respond RPC. Sends a request to /tilde.sandbox.v1.SandboxService/Respond.
    pub async fn respond(
        &self,
        request: crate::proto::tilde::sandbox::v1::RespondRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::sandbox::v1::__buffa::view::RespondResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.respond_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the Respond RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn respond_with_options(
        &self,
        request: crate::proto::tilde::sandbox::v1::RespondRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::sandbox::v1::__buffa::view::RespondResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SANDBOX_SERVICE_RESPOND_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListTools RPC. Sends a request to /tilde.sandbox.v1.SandboxService/ListTools.
    pub async fn list_tools(
        &self,
        request: crate::proto::tilde::sandbox::v1::ListToolsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::sandbox::v1::__buffa::view::ListToolsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_tools_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListTools RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_tools_with_options(
        &self,
        request: crate::proto::tilde::sandbox::v1::ListToolsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::sandbox::v1::__buffa::view::ListToolsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SANDBOX_SERVICE_LIST_TOOLS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the InvokeTool RPC. Sends a request to /tilde.sandbox.v1.SandboxService/InvokeTool.
    pub async fn invoke_tool(
        &self,
        request: crate::proto::tilde::sandbox::v1::InvokeToolRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::sandbox::v1::__buffa::view::InvokeToolResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.invoke_tool_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the InvokeTool RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn invoke_tool_with_options(
        &self,
        request: crate::proto::tilde::sandbox::v1::InvokeToolRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::sandbox::v1::__buffa::view::InvokeToolResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SANDBOX_SERVICE_INVOKE_TOOL_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
