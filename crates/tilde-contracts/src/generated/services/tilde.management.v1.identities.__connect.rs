///Shorthand for `OwnedView<CreateIdentityRequestView<'static>>`.
pub type OwnedCreateIdentityRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateIdentityRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<CreateIdentityResponseView<'static>>`.
pub type OwnedCreateIdentityResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateIdentityResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetIdentityRequestView<'static>>`.
pub type OwnedGetIdentityRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetIdentityRequestView<'static>,
>;
///Shorthand for `OwnedView<GetIdentityResponseView<'static>>`.
pub type OwnedGetIdentityResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetIdentityResponseView<'static>,
>;
///Shorthand for `OwnedView<ListIdentitiesRequestView<'static>>`.
pub type OwnedListIdentitiesRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListIdentitiesRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ListIdentitiesResponseView<'static>>`.
pub type OwnedListIdentitiesResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListIdentitiesResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<CreateRootIdentityRequestView<'static>>`.
pub type OwnedCreateRootIdentityRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateRootIdentityRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<CreateRootIdentityResponseView<'static>>`.
pub type OwnedCreateRootIdentityResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateRootIdentityResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetRootIdentityRequestView<'static>>`.
pub type OwnedGetRootIdentityRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetRootIdentityRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetRootIdentityResponseView<'static>>`.
pub type OwnedGetRootIdentityResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetRootIdentityResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ListRootIdentitiesRequestView<'static>>`.
pub type OwnedListRootIdentitiesRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListRootIdentitiesRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ListRootIdentitiesResponseView<'static>>`.
pub type OwnedListRootIdentitiesResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListRootIdentitiesResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<LinkIdentityRequestView<'static>>`.
pub type OwnedLinkIdentityRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::LinkIdentityRequestView<'static>,
>;
///Shorthand for `OwnedView<LinkIdentityResponseView<'static>>`.
pub type OwnedLinkIdentityResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::LinkIdentityResponseView<'static>,
>;
///Shorthand for `OwnedView<UnlinkIdentityRequestView<'static>>`.
pub type OwnedUnlinkIdentityRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::UnlinkIdentityRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<UnlinkIdentityResponseView<'static>>`.
pub type OwnedUnlinkIdentityResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::UnlinkIdentityResponseView<
        'static,
    >,
>;
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::CreateIdentityResponse>
for crate::proto::tilde::management::v1::__buffa::view::CreateIdentityResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::CreateIdentityResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateIdentityResponseView<
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
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::GetIdentityResponse>
for crate::proto::tilde::management::v1::__buffa::view::GetIdentityResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::GetIdentityResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetIdentityResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::ListIdentitiesResponse>
for crate::proto::tilde::management::v1::__buffa::view::ListIdentitiesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::ListIdentitiesResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListIdentitiesResponseView<
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
    crate::proto::tilde::management::v1::CreateRootIdentityResponse,
>
for crate::proto::tilde::management::v1::__buffa::view::CreateRootIdentityResponseView<
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
    crate::proto::tilde::management::v1::CreateRootIdentityResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::CreateRootIdentityResponseView<
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
    crate::proto::tilde::management::v1::GetRootIdentityResponse,
>
for crate::proto::tilde::management::v1::__buffa::view::GetRootIdentityResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::proto::tilde::management::v1::GetRootIdentityResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::GetRootIdentityResponseView<
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
    crate::proto::tilde::management::v1::ListRootIdentitiesResponse,
>
for crate::proto::tilde::management::v1::__buffa::view::ListRootIdentitiesResponseView<
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
    crate::proto::tilde::management::v1::ListRootIdentitiesResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::ListRootIdentitiesResponseView<
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
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::LinkIdentityResponse>
for crate::proto::tilde::management::v1::__buffa::view::LinkIdentityResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::LinkIdentityResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::LinkIdentityResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::UnlinkIdentityResponse>
for crate::proto::tilde::management::v1::__buffa::view::UnlinkIdentityResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::management::v1::UnlinkIdentityResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::management::v1::__buffa::view::UnlinkIdentityResponseView<
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
pub const IDENTITIES_SERVICE_SERVICE_NAME: &str = "tilde.management.v1.IdentitiesService";
/// Static [`Spec`](::connectrpc::Spec) for the `CreateIdentity` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const IDENTITIES_SERVICE_CREATE_IDENTITY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.IdentitiesService/CreateIdentity",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `GetIdentity` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const IDENTITIES_SERVICE_GET_IDENTITY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.IdentitiesService/GetIdentity",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ListIdentities` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const IDENTITIES_SERVICE_LIST_IDENTITIES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.IdentitiesService/ListIdentities",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `CreateRootIdentity` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const IDENTITIES_SERVICE_CREATE_ROOT_IDENTITY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.IdentitiesService/CreateRootIdentity",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `GetRootIdentity` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const IDENTITIES_SERVICE_GET_ROOT_IDENTITY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.IdentitiesService/GetRootIdentity",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ListRootIdentities` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const IDENTITIES_SERVICE_LIST_ROOT_IDENTITIES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.IdentitiesService/ListRootIdentities",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `LinkIdentity` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const IDENTITIES_SERVICE_LINK_IDENTITY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.IdentitiesService/LinkIdentity",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `UnlinkIdentity` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const IDENTITIES_SERVICE_UNLINK_IDENTITY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.management.v1.IdentitiesService/UnlinkIdentity",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Server trait for IdentitiesService.
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
pub trait IdentitiesService: Send + Sync + 'static {
    /// Handle the CreateIdentity RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn create_identity<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::CreateIdentityRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::CreateIdentityResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the GetIdentity RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_identity<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::GetIdentityRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::GetIdentityResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the ListIdentities RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_identities<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::ListIdentitiesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::ListIdentitiesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the CreateRootIdentity RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn create_root_identity<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::CreateRootIdentityRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::CreateRootIdentityResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the GetRootIdentity RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_root_identity<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::GetRootIdentityRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::GetRootIdentityResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the ListRootIdentities RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_root_identities<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::ListRootIdentitiesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::ListRootIdentitiesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the LinkIdentity RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn link_identity<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::LinkIdentityRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::LinkIdentityResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the UnlinkIdentity RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn unlink_identity<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::management::v1::UnlinkIdentityRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::management::v1::UnlinkIdentityResponse,
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
pub trait IdentitiesServiceExt: IdentitiesService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: IdentitiesService> IdentitiesServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                IDENTITIES_SERVICE_SERVICE_NAME,
                "CreateIdentity",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::CreateIdentityRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::CreateIdentityRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.create_identity(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::CreateIdentityResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(IDENTITIES_SERVICE_CREATE_IDENTITY_SPEC)
            .route_view(
                IDENTITIES_SERVICE_SERVICE_NAME,
                "GetIdentity",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::GetIdentityRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::GetIdentityRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_identity(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::GetIdentityResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(IDENTITIES_SERVICE_GET_IDENTITY_SPEC)
            .route_view(
                IDENTITIES_SERVICE_SERVICE_NAME,
                "ListIdentities",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::ListIdentitiesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::ListIdentitiesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_identities(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::ListIdentitiesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(IDENTITIES_SERVICE_LIST_IDENTITIES_SPEC)
            .route_view(
                IDENTITIES_SERVICE_SERVICE_NAME,
                "CreateRootIdentity",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::CreateRootIdentityRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::CreateRootIdentityRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.create_root_identity(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::CreateRootIdentityResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(IDENTITIES_SERVICE_CREATE_ROOT_IDENTITY_SPEC)
            .route_view(
                IDENTITIES_SERVICE_SERVICE_NAME,
                "GetRootIdentity",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::GetRootIdentityRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::GetRootIdentityRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_root_identity(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::GetRootIdentityResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(IDENTITIES_SERVICE_GET_ROOT_IDENTITY_SPEC)
            .route_view(
                IDENTITIES_SERVICE_SERVICE_NAME,
                "ListRootIdentities",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::ListRootIdentitiesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::ListRootIdentitiesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_root_identities(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::ListRootIdentitiesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(IDENTITIES_SERVICE_LIST_ROOT_IDENTITIES_SPEC)
            .route_view(
                IDENTITIES_SERVICE_SERVICE_NAME,
                "LinkIdentity",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::LinkIdentityRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::LinkIdentityRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.link_identity(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::LinkIdentityResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(IDENTITIES_SERVICE_LINK_IDENTITY_SPEC)
            .route_view(
                IDENTITIES_SERVICE_SERVICE_NAME,
                "UnlinkIdentity",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::management::v1::__buffa::view::UnlinkIdentityRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::management::v1::UnlinkIdentityRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.unlink_identity(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::management::v1::UnlinkIdentityResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(IDENTITIES_SERVICE_UNLINK_IDENTITY_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct IdentitiesServiceRegisterMarker;
impl<S: IdentitiesService> ::connectrpc::ServiceRegister<IdentitiesServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as IdentitiesServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `IdentitiesService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = IdentitiesServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct IdentitiesServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: IdentitiesService> IdentitiesServiceServer<T> {
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
impl<T> Clone for IdentitiesServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: IdentitiesService> ::connectrpc::Dispatcher for IdentitiesServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("tilde.management.v1.IdentitiesService/")?;
        match method {
            "CreateIdentity" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(IDENTITIES_SERVICE_CREATE_IDENTITY_SPEC),
                )
            }
            "GetIdentity" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(IDENTITIES_SERVICE_GET_IDENTITY_SPEC),
                )
            }
            "ListIdentities" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(IDENTITIES_SERVICE_LIST_IDENTITIES_SPEC),
                )
            }
            "CreateRootIdentity" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(IDENTITIES_SERVICE_CREATE_ROOT_IDENTITY_SPEC),
                )
            }
            "GetRootIdentity" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(IDENTITIES_SERVICE_GET_ROOT_IDENTITY_SPEC),
                )
            }
            "ListRootIdentities" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(IDENTITIES_SERVICE_LIST_ROOT_IDENTITIES_SPEC),
                )
            }
            "LinkIdentity" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(IDENTITIES_SERVICE_LINK_IDENTITY_SPEC),
                )
            }
            "UnlinkIdentity" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(IDENTITIES_SERVICE_UNLINK_IDENTITY_SPEC),
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
        let Some(method) = path.strip_prefix("tilde.management.v1.IdentitiesService/")
        else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "CreateIdentity" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::CreateIdentityRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::CreateIdentityRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::CreateIdentityRequest,
                    >::from_parts(&req, &body);
                    svc.create_identity(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::CreateIdentityResponse,
                        >(format)
                })
            }
            "GetIdentity" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::GetIdentityRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::GetIdentityRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::GetIdentityRequest,
                    >::from_parts(&req, &body);
                    svc.get_identity(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::GetIdentityResponse,
                        >(format)
                })
            }
            "ListIdentities" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::ListIdentitiesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::ListIdentitiesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::ListIdentitiesRequest,
                    >::from_parts(&req, &body);
                    svc.list_identities(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::ListIdentitiesResponse,
                        >(format)
                })
            }
            "CreateRootIdentity" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::CreateRootIdentityRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::CreateRootIdentityRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::CreateRootIdentityRequest,
                    >::from_parts(&req, &body);
                    svc.create_root_identity(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::CreateRootIdentityResponse,
                        >(format)
                })
            }
            "GetRootIdentity" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::GetRootIdentityRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::GetRootIdentityRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::GetRootIdentityRequest,
                    >::from_parts(&req, &body);
                    svc.get_root_identity(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::GetRootIdentityResponse,
                        >(format)
                })
            }
            "ListRootIdentities" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::ListRootIdentitiesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::ListRootIdentitiesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::ListRootIdentitiesRequest,
                    >::from_parts(&req, &body);
                    svc.list_root_identities(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::ListRootIdentitiesResponse,
                        >(format)
                })
            }
            "LinkIdentity" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::LinkIdentityRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::LinkIdentityRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::LinkIdentityRequest,
                    >::from_parts(&req, &body);
                    svc.link_identity(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::LinkIdentityResponse,
                        >(format)
                })
            }
            "UnlinkIdentity" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::management::v1::UnlinkIdentityRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::management::v1::__buffa::view::UnlinkIdentityRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::management::v1::UnlinkIdentityRequest,
                    >::from_parts(&req, &body);
                    svc.unlink_identity(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::management::v1::UnlinkIdentityResponse,
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
        let Some(method) = path.strip_prefix("tilde.management.v1.IdentitiesService/")
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
        let Some(method) = path.strip_prefix("tilde.management.v1.IdentitiesService/")
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
        let Some(method) = path.strip_prefix("tilde.management.v1.IdentitiesService/")
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
/// let client = IdentitiesServiceClient::new(conn, config);
/// let response = client.create_identity(request).await?;
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
/// let client = IdentitiesServiceClient::new(http, config);
/// let response = client.create_identity(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.create_identity(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.create_identity(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct IdentitiesServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> IdentitiesServiceClient<T>
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
    /// Call the CreateIdentity RPC. Sends a request to /tilde.management.v1.IdentitiesService/CreateIdentity.
    pub async fn create_identity(
        &self,
        request: crate::proto::tilde::management::v1::CreateIdentityRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::CreateIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.create_identity_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CreateIdentity RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn create_identity_with_options(
        &self,
        request: crate::proto::tilde::management::v1::CreateIdentityRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::CreateIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                IDENTITIES_SERVICE_CREATE_IDENTITY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetIdentity RPC. Sends a request to /tilde.management.v1.IdentitiesService/GetIdentity.
    pub async fn get_identity(
        &self,
        request: crate::proto::tilde::management::v1::GetIdentityRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_identity_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetIdentity RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_identity_with_options(
        &self,
        request: crate::proto::tilde::management::v1::GetIdentityRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                IDENTITIES_SERVICE_GET_IDENTITY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListIdentities RPC. Sends a request to /tilde.management.v1.IdentitiesService/ListIdentities.
    pub async fn list_identities(
        &self,
        request: crate::proto::tilde::management::v1::ListIdentitiesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListIdentitiesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_identities_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListIdentities RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_identities_with_options(
        &self,
        request: crate::proto::tilde::management::v1::ListIdentitiesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListIdentitiesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                IDENTITIES_SERVICE_LIST_IDENTITIES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the CreateRootIdentity RPC. Sends a request to /tilde.management.v1.IdentitiesService/CreateRootIdentity.
    pub async fn create_root_identity(
        &self,
        request: crate::proto::tilde::management::v1::CreateRootIdentityRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::CreateRootIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.create_root_identity_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CreateRootIdentity RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn create_root_identity_with_options(
        &self,
        request: crate::proto::tilde::management::v1::CreateRootIdentityRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::CreateRootIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                IDENTITIES_SERVICE_CREATE_ROOT_IDENTITY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetRootIdentity RPC. Sends a request to /tilde.management.v1.IdentitiesService/GetRootIdentity.
    pub async fn get_root_identity(
        &self,
        request: crate::proto::tilde::management::v1::GetRootIdentityRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetRootIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_root_identity_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetRootIdentity RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_root_identity_with_options(
        &self,
        request: crate::proto::tilde::management::v1::GetRootIdentityRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::GetRootIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                IDENTITIES_SERVICE_GET_ROOT_IDENTITY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListRootIdentities RPC. Sends a request to /tilde.management.v1.IdentitiesService/ListRootIdentities.
    pub async fn list_root_identities(
        &self,
        request: crate::proto::tilde::management::v1::ListRootIdentitiesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListRootIdentitiesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_root_identities_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListRootIdentities RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_root_identities_with_options(
        &self,
        request: crate::proto::tilde::management::v1::ListRootIdentitiesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::ListRootIdentitiesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                IDENTITIES_SERVICE_LIST_ROOT_IDENTITIES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the LinkIdentity RPC. Sends a request to /tilde.management.v1.IdentitiesService/LinkIdentity.
    pub async fn link_identity(
        &self,
        request: crate::proto::tilde::management::v1::LinkIdentityRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::LinkIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.link_identity_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the LinkIdentity RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn link_identity_with_options(
        &self,
        request: crate::proto::tilde::management::v1::LinkIdentityRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::LinkIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                IDENTITIES_SERVICE_LINK_IDENTITY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UnlinkIdentity RPC. Sends a request to /tilde.management.v1.IdentitiesService/UnlinkIdentity.
    pub async fn unlink_identity(
        &self,
        request: crate::proto::tilde::management::v1::UnlinkIdentityRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::UnlinkIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.unlink_identity_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UnlinkIdentity RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn unlink_identity_with_options(
        &self,
        request: crate::proto::tilde::management::v1::UnlinkIdentityRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::management::v1::__buffa::view::UnlinkIdentityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                IDENTITIES_SERVICE_UNLINK_IDENTITY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
