///Shorthand for `OwnedView<SetConnectionNameRequestView<'static>>`.
pub type OwnedSetConnectionNameRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::SetConnectionNameRequestView<'static>,
>;
///Shorthand for `OwnedView<SetConnectionNameResponseView<'static>>`.
pub type OwnedSetConnectionNameResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::SetConnectionNameResponseView<'static>,
>;
///Shorthand for `OwnedView<GetSetupRequestView<'static>>`.
pub type OwnedGetSetupRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::GetSetupRequestView<'static>,
>;
///Shorthand for `OwnedView<GetSetupResponseView<'static>>`.
pub type OwnedGetSetupResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::GetSetupResponseView<'static>,
>;
///Shorthand for `OwnedView<SaveDraftRequestView<'static>>`.
pub type OwnedSaveDraftRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::SaveDraftRequestView<'static>,
>;
///Shorthand for `OwnedView<SaveDraftResponseView<'static>>`.
pub type OwnedSaveDraftResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::SaveDraftResponseView<'static>,
>;
///Shorthand for `OwnedView<StartOAuthRequestView<'static>>`.
pub type OwnedStartOAuthRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::StartOAuthRequestView<'static>,
>;
///Shorthand for `OwnedView<StartOAuthResponseView<'static>>`.
pub type OwnedStartOAuthResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::StartOAuthResponseView<'static>,
>;
///Shorthand for `OwnedView<SaveCredentialsRequestView<'static>>`.
pub type OwnedSaveCredentialsRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::SaveCredentialsRequestView<'static>,
>;
///Shorthand for `OwnedView<SaveCredentialsResponseView<'static>>`.
pub type OwnedSaveCredentialsResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::SaveCredentialsResponseView<'static>,
>;
///Shorthand for `OwnedView<ExecuteProviderActionRequestView<'static>>`.
pub type OwnedExecuteProviderActionRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::ExecuteProviderActionRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ExecuteProviderActionResponseView<'static>>`.
pub type OwnedExecuteProviderActionResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::ExecuteProviderActionResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<CancelSetupRequestView<'static>>`.
pub type OwnedCancelSetupRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::CancelSetupRequestView<'static>,
>;
///Shorthand for `OwnedView<CancelSetupResponseView<'static>>`.
pub type OwnedCancelSetupResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::CancelSetupResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::SetConnectionNameResponse>
for crate::proto::tilde::setup::v1::__buffa::view::SetConnectionNameResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::SetConnectionNameResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::SetConnectionNameResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::GetSetupResponse>
for crate::proto::tilde::setup::v1::__buffa::view::GetSetupResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::GetSetupResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::GetSetupResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::SaveDraftResponse>
for crate::proto::tilde::setup::v1::__buffa::view::SaveDraftResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::SaveDraftResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::SaveDraftResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::StartOAuthResponse>
for crate::proto::tilde::setup::v1::__buffa::view::StartOAuthResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::StartOAuthResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::StartOAuthResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::SaveCredentialsResponse>
for crate::proto::tilde::setup::v1::__buffa::view::SaveCredentialsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::SaveCredentialsResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::SaveCredentialsResponseView<'static>,
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
    crate::proto::tilde::setup::v1::ExecuteProviderActionResponse,
>
for crate::proto::tilde::setup::v1::__buffa::view::ExecuteProviderActionResponseView<
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
    crate::proto::tilde::setup::v1::ExecuteProviderActionResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::ExecuteProviderActionResponseView<
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
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::CancelSetupResponse>
for crate::proto::tilde::setup::v1::__buffa::view::CancelSetupResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::setup::v1::CancelSetupResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::setup::v1::__buffa::view::CancelSetupResponseView<'static>,
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
pub const CONNECTION_SETUP_SERVICE_SERVICE_NAME: &str = "tilde.setup.v1.ConnectionSetupService";
/// Static [`Spec`](::connectrpc::Spec) for the `SetConnectionName` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const CONNECTION_SETUP_SERVICE_SET_CONNECTION_NAME_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.setup.v1.ConnectionSetupService/SetConnectionName",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `GetSetup` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const CONNECTION_SETUP_SERVICE_GET_SETUP_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.setup.v1.ConnectionSetupService/GetSetup",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `SaveDraft` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const CONNECTION_SETUP_SERVICE_SAVE_DRAFT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.setup.v1.ConnectionSetupService/SaveDraft",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `StartOAuth` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const CONNECTION_SETUP_SERVICE_START_O_AUTH_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.setup.v1.ConnectionSetupService/StartOAuth",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `SaveCredentials` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const CONNECTION_SETUP_SERVICE_SAVE_CREDENTIALS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.setup.v1.ConnectionSetupService/SaveCredentials",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ExecuteProviderAction` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const CONNECTION_SETUP_SERVICE_EXECUTE_PROVIDER_ACTION_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.setup.v1.ConnectionSetupService/ExecuteProviderAction",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `CancelSetup` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const CONNECTION_SETUP_SERVICE_CANCEL_SETUP_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.setup.v1.ConnectionSetupService/CancelSetup",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Server trait for ConnectionSetupService.
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
pub trait ConnectionSetupService: Send + Sync + 'static {
    /// Handle the SetConnectionName RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn set_connection_name<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::setup::v1::SetConnectionNameRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::setup::v1::SetConnectionNameResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the GetSetup RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_setup<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::setup::v1::GetSetupRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::setup::v1::GetSetupResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the SaveDraft RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn save_draft<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::setup::v1::SaveDraftRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::setup::v1::SaveDraftResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the StartOAuth RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn start_o_auth<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::setup::v1::StartOAuthRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::setup::v1::StartOAuthResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the SaveCredentials RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn save_credentials<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::setup::v1::SaveCredentialsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::setup::v1::SaveCredentialsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the ExecuteProviderAction RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn execute_provider_action<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::setup::v1::ExecuteProviderActionRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::setup::v1::ExecuteProviderActionResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the CancelSetup RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn cancel_setup<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::setup::v1::CancelSetupRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::setup::v1::CancelSetupResponse,
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
pub trait ConnectionSetupServiceExt: ConnectionSetupService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: ConnectionSetupService> ConnectionSetupServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                CONNECTION_SETUP_SERVICE_SERVICE_NAME,
                "SetConnectionName",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::setup::v1::__buffa::view::SetConnectionNameRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::setup::v1::SetConnectionNameRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.set_connection_name(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::setup::v1::SetConnectionNameResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(CONNECTION_SETUP_SERVICE_SET_CONNECTION_NAME_SPEC)
            .route_view(
                CONNECTION_SETUP_SERVICE_SERVICE_NAME,
                "GetSetup",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::setup::v1::__buffa::view::GetSetupRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::setup::v1::GetSetupRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_setup(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::setup::v1::GetSetupResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(CONNECTION_SETUP_SERVICE_GET_SETUP_SPEC)
            .route_view(
                CONNECTION_SETUP_SERVICE_SERVICE_NAME,
                "SaveDraft",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::setup::v1::__buffa::view::SaveDraftRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::setup::v1::SaveDraftRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.save_draft(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::setup::v1::SaveDraftResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(CONNECTION_SETUP_SERVICE_SAVE_DRAFT_SPEC)
            .route_view(
                CONNECTION_SETUP_SERVICE_SERVICE_NAME,
                "StartOAuth",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::setup::v1::__buffa::view::StartOAuthRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::setup::v1::StartOAuthRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.start_o_auth(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::setup::v1::StartOAuthResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(CONNECTION_SETUP_SERVICE_START_O_AUTH_SPEC)
            .route_view(
                CONNECTION_SETUP_SERVICE_SERVICE_NAME,
                "SaveCredentials",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::setup::v1::__buffa::view::SaveCredentialsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::setup::v1::SaveCredentialsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.save_credentials(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::setup::v1::SaveCredentialsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(CONNECTION_SETUP_SERVICE_SAVE_CREDENTIALS_SPEC)
            .route_view(
                CONNECTION_SETUP_SERVICE_SERVICE_NAME,
                "ExecuteProviderAction",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::setup::v1::__buffa::view::ExecuteProviderActionRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::setup::v1::ExecuteProviderActionRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.execute_provider_action(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::setup::v1::ExecuteProviderActionResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(CONNECTION_SETUP_SERVICE_EXECUTE_PROVIDER_ACTION_SPEC)
            .route_view(
                CONNECTION_SETUP_SERVICE_SERVICE_NAME,
                "CancelSetup",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::setup::v1::__buffa::view::CancelSetupRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::setup::v1::CancelSetupRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.cancel_setup(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::setup::v1::CancelSetupResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(CONNECTION_SETUP_SERVICE_CANCEL_SETUP_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct ConnectionSetupServiceRegisterMarker;
impl<
    S: ConnectionSetupService,
> ::connectrpc::ServiceRegister<ConnectionSetupServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as ConnectionSetupServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `ConnectionSetupService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = ConnectionSetupServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct ConnectionSetupServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: ConnectionSetupService> ConnectionSetupServiceServer<T> {
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
impl<T> Clone for ConnectionSetupServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: ConnectionSetupService> ::connectrpc::Dispatcher
for ConnectionSetupServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("tilde.setup.v1.ConnectionSetupService/")?;
        match method {
            "SetConnectionName" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(CONNECTION_SETUP_SERVICE_SET_CONNECTION_NAME_SPEC),
                )
            }
            "GetSetup" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(CONNECTION_SETUP_SERVICE_GET_SETUP_SPEC),
                )
            }
            "SaveDraft" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(CONNECTION_SETUP_SERVICE_SAVE_DRAFT_SPEC),
                )
            }
            "StartOAuth" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(CONNECTION_SETUP_SERVICE_START_O_AUTH_SPEC),
                )
            }
            "SaveCredentials" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(CONNECTION_SETUP_SERVICE_SAVE_CREDENTIALS_SPEC),
                )
            }
            "ExecuteProviderAction" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(CONNECTION_SETUP_SERVICE_EXECUTE_PROVIDER_ACTION_SPEC),
                )
            }
            "CancelSetup" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(CONNECTION_SETUP_SERVICE_CANCEL_SETUP_SPEC),
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
        let Some(method) = path.strip_prefix("tilde.setup.v1.ConnectionSetupService/")
        else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "SetConnectionName" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::setup::v1::SetConnectionNameRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::setup::v1::__buffa::view::SetConnectionNameRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::setup::v1::SetConnectionNameRequest,
                    >::from_parts(&req, &body);
                    svc.set_connection_name(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::setup::v1::SetConnectionNameResponse,
                        >(format)
                })
            }
            "GetSetup" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::setup::v1::GetSetupRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::setup::v1::__buffa::view::GetSetupRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::setup::v1::GetSetupRequest,
                    >::from_parts(&req, &body);
                    svc.get_setup(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::setup::v1::GetSetupResponse,
                        >(format)
                })
            }
            "SaveDraft" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::setup::v1::SaveDraftRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::setup::v1::__buffa::view::SaveDraftRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::setup::v1::SaveDraftRequest,
                    >::from_parts(&req, &body);
                    svc.save_draft(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::setup::v1::SaveDraftResponse,
                        >(format)
                })
            }
            "StartOAuth" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::setup::v1::StartOAuthRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::setup::v1::__buffa::view::StartOAuthRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::setup::v1::StartOAuthRequest,
                    >::from_parts(&req, &body);
                    svc.start_o_auth(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::setup::v1::StartOAuthResponse,
                        >(format)
                })
            }
            "SaveCredentials" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::setup::v1::SaveCredentialsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::setup::v1::__buffa::view::SaveCredentialsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::setup::v1::SaveCredentialsRequest,
                    >::from_parts(&req, &body);
                    svc.save_credentials(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::setup::v1::SaveCredentialsResponse,
                        >(format)
                })
            }
            "ExecuteProviderAction" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::setup::v1::ExecuteProviderActionRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::setup::v1::__buffa::view::ExecuteProviderActionRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::setup::v1::ExecuteProviderActionRequest,
                    >::from_parts(&req, &body);
                    svc.execute_provider_action(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::setup::v1::ExecuteProviderActionResponse,
                        >(format)
                })
            }
            "CancelSetup" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::setup::v1::CancelSetupRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::setup::v1::__buffa::view::CancelSetupRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::setup::v1::CancelSetupRequest,
                    >::from_parts(&req, &body);
                    svc.cancel_setup(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::setup::v1::CancelSetupResponse,
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
        let Some(method) = path.strip_prefix("tilde.setup.v1.ConnectionSetupService/")
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
        let Some(method) = path.strip_prefix("tilde.setup.v1.ConnectionSetupService/")
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
        let Some(method) = path.strip_prefix("tilde.setup.v1.ConnectionSetupService/")
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
/// let client = ConnectionSetupServiceClient::new(conn, config);
/// let response = client.set_connection_name(request).await?;
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
/// let client = ConnectionSetupServiceClient::new(http, config);
/// let response = client.set_connection_name(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.set_connection_name(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.set_connection_name(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct ConnectionSetupServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> ConnectionSetupServiceClient<T>
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
    /// Call the SetConnectionName RPC. Sends a request to /tilde.setup.v1.ConnectionSetupService/SetConnectionName.
    pub async fn set_connection_name(
        &self,
        request: crate::proto::tilde::setup::v1::SetConnectionNameRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::SetConnectionNameResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.set_connection_name_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the SetConnectionName RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn set_connection_name_with_options(
        &self,
        request: crate::proto::tilde::setup::v1::SetConnectionNameRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::SetConnectionNameResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                CONNECTION_SETUP_SERVICE_SET_CONNECTION_NAME_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetSetup RPC. Sends a request to /tilde.setup.v1.ConnectionSetupService/GetSetup.
    pub async fn get_setup(
        &self,
        request: crate::proto::tilde::setup::v1::GetSetupRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::GetSetupResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_setup_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetSetup RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_setup_with_options(
        &self,
        request: crate::proto::tilde::setup::v1::GetSetupRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::GetSetupResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                CONNECTION_SETUP_SERVICE_GET_SETUP_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the SaveDraft RPC. Sends a request to /tilde.setup.v1.ConnectionSetupService/SaveDraft.
    pub async fn save_draft(
        &self,
        request: crate::proto::tilde::setup::v1::SaveDraftRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::SaveDraftResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.save_draft_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the SaveDraft RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn save_draft_with_options(
        &self,
        request: crate::proto::tilde::setup::v1::SaveDraftRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::SaveDraftResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                CONNECTION_SETUP_SERVICE_SAVE_DRAFT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the StartOAuth RPC. Sends a request to /tilde.setup.v1.ConnectionSetupService/StartOAuth.
    pub async fn start_o_auth(
        &self,
        request: crate::proto::tilde::setup::v1::StartOAuthRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::StartOAuthResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.start_o_auth_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the StartOAuth RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn start_o_auth_with_options(
        &self,
        request: crate::proto::tilde::setup::v1::StartOAuthRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::StartOAuthResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                CONNECTION_SETUP_SERVICE_START_O_AUTH_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the SaveCredentials RPC. Sends a request to /tilde.setup.v1.ConnectionSetupService/SaveCredentials.
    pub async fn save_credentials(
        &self,
        request: crate::proto::tilde::setup::v1::SaveCredentialsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::SaveCredentialsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.save_credentials_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the SaveCredentials RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn save_credentials_with_options(
        &self,
        request: crate::proto::tilde::setup::v1::SaveCredentialsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::SaveCredentialsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                CONNECTION_SETUP_SERVICE_SAVE_CREDENTIALS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ExecuteProviderAction RPC. Sends a request to /tilde.setup.v1.ConnectionSetupService/ExecuteProviderAction.
    pub async fn execute_provider_action(
        &self,
        request: crate::proto::tilde::setup::v1::ExecuteProviderActionRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::ExecuteProviderActionResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.execute_provider_action_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ExecuteProviderAction RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn execute_provider_action_with_options(
        &self,
        request: crate::proto::tilde::setup::v1::ExecuteProviderActionRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::ExecuteProviderActionResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                CONNECTION_SETUP_SERVICE_EXECUTE_PROVIDER_ACTION_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the CancelSetup RPC. Sends a request to /tilde.setup.v1.ConnectionSetupService/CancelSetup.
    pub async fn cancel_setup(
        &self,
        request: crate::proto::tilde::setup::v1::CancelSetupRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::CancelSetupResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.cancel_setup_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CancelSetup RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn cancel_setup_with_options(
        &self,
        request: crate::proto::tilde::setup::v1::CancelSetupRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::setup::v1::__buffa::view::CancelSetupResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                CONNECTION_SETUP_SERVICE_CANCEL_SETUP_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
