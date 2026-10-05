///Shorthand for `OwnedView<ListRoutinesRequestView<'static>>`.
pub type OwnedListRoutinesRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListRoutinesRequestView<'static>,
>;
///Shorthand for `OwnedView<ListRoutinesResponseView<'static>>`.
pub type OwnedListRoutinesResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListRoutinesResponseView<'static>,
>;
///Shorthand for `OwnedView<CreateRoutineRequestView<'static>>`.
pub type OwnedCreateRoutineRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateRoutineRequestView<'static>,
>;
///Shorthand for `OwnedView<CreateRoutineResponseView<'static>>`.
pub type OwnedCreateRoutineResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateRoutineResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<UpdateRoutineRequestView<'static>>`.
pub type OwnedUpdateRoutineRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::UpdateRoutineRequestView<'static>,
>;
///Shorthand for `OwnedView<UpdateRoutineResponseView<'static>>`.
pub type OwnedUpdateRoutineResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::UpdateRoutineResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<DeleteRoutineRequestView<'static>>`.
pub type OwnedDeleteRoutineRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::DeleteRoutineRequestView<'static>,
>;
///Shorthand for `OwnedView<DeleteRoutineResponseView<'static>>`.
pub type OwnedDeleteRoutineResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::DeleteRoutineResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ListSignalTypesRequestView<'static>>`.
pub type OwnedListSignalTypesRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListSignalTypesRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ListSignalTypesResponseView<'static>>`.
pub type OwnedListSignalTypesResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListSignalTypesResponseView<
        'static,
    >,
>;
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::ListRoutinesResponse>
for crate::proto::tilde::management::v1::__buffa::view::ListRoutinesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::ListRoutinesResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListRoutinesResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::CreateRoutineResponse>
for crate::proto::tilde::management::v1::__buffa::view::CreateRoutineResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::CreateRoutineResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateRoutineResponseView<
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
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::UpdateRoutineResponse>
for crate::proto::tilde::management::v1::__buffa::view::UpdateRoutineResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::UpdateRoutineResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::UpdateRoutineResponseView<
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
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::DeleteRoutineResponse>
for crate::proto::tilde::management::v1::__buffa::view::DeleteRoutineResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::DeleteRoutineResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::DeleteRoutineResponseView<
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
    crate::proto::tilde::management::v1::ListSignalTypesResponse,
>
for crate::proto::tilde::management::v1::__buffa::view::ListSignalTypesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::proto::tilde::management::v1::ListSignalTypesResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListSignalTypesResponseView<
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
pub const ROUTINE_SERVICE_SERVICE_NAME: &str = "tilde.management.v1.RoutineService";
/// Static [`Spec`](::connectrpc::Spec) for the `ListRoutines` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const ROUTINE_SERVICE_LIST_ROUTINES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.RoutineService/ListRoutines",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `CreateRoutine` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const ROUTINE_SERVICE_CREATE_ROUTINE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.RoutineService/CreateRoutine",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `UpdateRoutine` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const ROUTINE_SERVICE_UPDATE_ROUTINE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.RoutineService/UpdateRoutine",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `DeleteRoutine` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const ROUTINE_SERVICE_DELETE_ROUTINE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.RoutineService/DeleteRoutine",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ListSignalTypes` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const ROUTINE_SERVICE_LIST_SIGNAL_TYPES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.RoutineService/ListSignalTypes",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Routines prompt an agent when a cron schedule comes due or a connection emits a signal.
/// Each firing starts a run in a new thread. The prompt and thread title are templates: each
/// `{{ key }}` is replaced with that path of the signal's context (see ListSignalTypes) or, for a
/// cron routine, `scheduled_at`.
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
pub trait RoutineService: Send + Sync + 'static {
    /// Handle the ListRoutines RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_routines<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::ListRoutinesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::ListRoutinesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the CreateRoutine RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn create_routine<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::CreateRoutineRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::CreateRoutineResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the UpdateRoutine RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn update_routine<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::UpdateRoutineRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::UpdateRoutineResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the DeleteRoutine RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn delete_routine<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::DeleteRoutineRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::DeleteRoutineResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// The signal types a signal-capable connection emits.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_signal_types<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::ListSignalTypesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::ListSignalTypesResponse,
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
pub trait RoutineServiceExt: RoutineService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: RoutineService> RoutineServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_idempotent(
                ROUTINE_SERVICE_SERVICE_NAME,
                "ListRoutines",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::ListRoutinesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::ListRoutinesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_routines(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::ListRoutinesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(ROUTINE_SERVICE_LIST_ROUTINES_SPEC)
            .route_view(
                ROUTINE_SERVICE_SERVICE_NAME,
                "CreateRoutine",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::CreateRoutineRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::CreateRoutineRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.create_routine(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::CreateRoutineResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(ROUTINE_SERVICE_CREATE_ROUTINE_SPEC)
            .route_view(
                ROUTINE_SERVICE_SERVICE_NAME,
                "UpdateRoutine",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::UpdateRoutineRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::UpdateRoutineRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.update_routine(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::UpdateRoutineResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(ROUTINE_SERVICE_UPDATE_ROUTINE_SPEC)
            .route_view(
                ROUTINE_SERVICE_SERVICE_NAME,
                "DeleteRoutine",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::DeleteRoutineRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::DeleteRoutineRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.delete_routine(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::DeleteRoutineResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(ROUTINE_SERVICE_DELETE_ROUTINE_SPEC)
            .route_view_idempotent(
                ROUTINE_SERVICE_SERVICE_NAME,
                "ListSignalTypes",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::ListSignalTypesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::ListSignalTypesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_signal_types(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::ListSignalTypesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(ROUTINE_SERVICE_LIST_SIGNAL_TYPES_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct RoutineServiceRegisterMarker;
impl<S: RoutineService> ::connectrpc::ServiceRegister<RoutineServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as RoutineServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `RoutineService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = RoutineServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct RoutineServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: RoutineService> RoutineServiceServer<T> {
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
impl<T> Clone for RoutineServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: RoutineService> ::connectrpc::Dispatcher for RoutineServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("tilde.management.v1.RoutineService/")?;
        match method {
            "ListRoutines" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(ROUTINE_SERVICE_LIST_ROUTINES_SPEC),
                )
            }
            "CreateRoutine" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(ROUTINE_SERVICE_CREATE_ROUTINE_SPEC),
                )
            }
            "UpdateRoutine" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(ROUTINE_SERVICE_UPDATE_ROUTINE_SPEC),
                )
            }
            "DeleteRoutine" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(ROUTINE_SERVICE_DELETE_ROUTINE_SPEC),
                )
            }
            "ListSignalTypes" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(ROUTINE_SERVICE_LIST_SIGNAL_TYPES_SPEC),
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
        let Some(method) = path.strip_prefix("tilde.management.v1.RoutineService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "ListRoutines" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::ListRoutinesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::ListRoutinesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::ListRoutinesRequest,
                    >::from_parts(&req, &body);
                    svc.list_routines(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::ListRoutinesResponse,
                        >(format)
                })
            }
            "CreateRoutine" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::CreateRoutineRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::CreateRoutineRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::CreateRoutineRequest,
                    >::from_parts(&req, &body);
                    svc.create_routine(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::CreateRoutineResponse,
                        >(format)
                })
            }
            "UpdateRoutine" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::UpdateRoutineRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::UpdateRoutineRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::UpdateRoutineRequest,
                    >::from_parts(&req, &body);
                    svc.update_routine(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::UpdateRoutineResponse,
                        >(format)
                })
            }
            "DeleteRoutine" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::DeleteRoutineRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::DeleteRoutineRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::DeleteRoutineRequest,
                    >::from_parts(&req, &body);
                    svc.delete_routine(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::DeleteRoutineResponse,
                        >(format)
                })
            }
            "ListSignalTypes" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::ListSignalTypesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::ListSignalTypesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::ListSignalTypesRequest,
                    >::from_parts(&req, &body);
                    svc.list_signal_types(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::ListSignalTypesResponse,
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
        let Some(method) = path.strip_prefix("tilde.management.v1.RoutineService/") else {
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
        let Some(method) = path.strip_prefix("tilde.management.v1.RoutineService/") else {
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
        let Some(method) = path.strip_prefix("tilde.management.v1.RoutineService/") else {
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
/// let client = RoutineServiceClient::new(conn, config);
/// let response = client.list_routines(request).await?;
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
/// let client = RoutineServiceClient::new(http, config);
/// let response = client.list_routines(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.list_routines(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.list_routines(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct RoutineServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> RoutineServiceClient<T>
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
    /// Call the ListRoutines RPC. Sends a request to /tilde.management.v1.RoutineService/ListRoutines.
    pub async fn list_routines(
        &self,
        request: crate::proto::tilde::management::v1::ListRoutinesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListRoutinesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_routines_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListRoutines RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_routines_with_options(
        &self,
        request: crate::proto::tilde::management::v1::ListRoutinesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListRoutinesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                ROUTINE_SERVICE_LIST_ROUTINES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the CreateRoutine RPC. Sends a request to /tilde.management.v1.RoutineService/CreateRoutine.
    pub async fn create_routine(
        &self,
        request: crate::proto::tilde::management::v1::CreateRoutineRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::CreateRoutineResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.create_routine_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CreateRoutine RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn create_routine_with_options(
        &self,
        request: crate::proto::tilde::management::v1::CreateRoutineRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::CreateRoutineResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                ROUTINE_SERVICE_CREATE_ROUTINE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UpdateRoutine RPC. Sends a request to /tilde.management.v1.RoutineService/UpdateRoutine.
    pub async fn update_routine(
        &self,
        request: crate::proto::tilde::management::v1::UpdateRoutineRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::UpdateRoutineResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.update_routine_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UpdateRoutine RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn update_routine_with_options(
        &self,
        request: crate::proto::tilde::management::v1::UpdateRoutineRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::UpdateRoutineResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                ROUTINE_SERVICE_UPDATE_ROUTINE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the DeleteRoutine RPC. Sends a request to /tilde.management.v1.RoutineService/DeleteRoutine.
    pub async fn delete_routine(
        &self,
        request: crate::proto::tilde::management::v1::DeleteRoutineRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::DeleteRoutineResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.delete_routine_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the DeleteRoutine RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn delete_routine_with_options(
        &self,
        request: crate::proto::tilde::management::v1::DeleteRoutineRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::DeleteRoutineResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                ROUTINE_SERVICE_DELETE_ROUTINE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListSignalTypes RPC. Sends a request to /tilde.management.v1.RoutineService/ListSignalTypes.
    pub async fn list_signal_types(
        &self,
        request: crate::proto::tilde::management::v1::ListSignalTypesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListSignalTypesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_signal_types_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListSignalTypes RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_signal_types_with_options(
        &self,
        request: crate::proto::tilde::management::v1::ListSignalTypesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListSignalTypesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                ROUTINE_SERVICE_LIST_SIGNAL_TYPES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
