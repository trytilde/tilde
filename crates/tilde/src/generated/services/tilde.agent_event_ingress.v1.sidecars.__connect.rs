///Shorthand for `OwnedView<WatchRequestView<'static>>`.
pub type OwnedWatchRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<WatchResponseView<'static>>`.
pub type OwnedWatchResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<PublishRequestView<'static>>`.
pub type OwnedPublishRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::PublishRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<PublishResponseView<'static>>`.
pub type OwnedPublishResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::PublishResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<HydrateRequestView<'static>>`.
pub type OwnedHydrateRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HydrateRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<HydrateResponseView<'static>>`.
pub type OwnedHydrateResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HydrateResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ForwardRequestView<'static>>`.
pub type OwnedForwardRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ForwardRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ForwardResponseView<'static>>`.
pub type OwnedForwardResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ForwardResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<UploadAttachmentRequestView<'static>>`.
pub type OwnedUploadAttachmentRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::UploadAttachmentRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<UploadAttachmentResponseView<'static>>`.
pub type OwnedUploadAttachmentResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::UploadAttachmentResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<DownloadAttachmentRequestView<'static>>`.
pub type OwnedDownloadAttachmentRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::DownloadAttachmentRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<DownloadAttachmentResponseView<'static>>`.
pub type OwnedDownloadAttachmentResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::DownloadAttachmentResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ResolveParticipantRequestView<'static>>`.
pub type OwnedResolveParticipantRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ResolveParticipantRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ResolveParticipantResponseView<'static>>`.
pub type OwnedResolveParticipantResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ResolveParticipantResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<RelayRequestView<'static>>`.
pub type OwnedRelayRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RelayRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<RelayResponseView<'static>>`.
pub type OwnedRelayResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RelayResponseView<
        'static,
    >,
>;
impl ::connectrpc::Encodable<crate::proto::tilde::agent_event_ingress::v1::WatchResponse>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::agent_event_ingress::v1::WatchResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::PublishResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::PublishResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::PublishResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::PublishResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::HydrateResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HydrateResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::HydrateResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HydrateResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::ForwardResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ForwardResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::ForwardResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ForwardResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::UploadAttachmentResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::UploadAttachmentResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::DownloadAttachmentResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::DownloadAttachmentResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ResolveParticipantResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ResolveParticipantResponseView<
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
impl ::connectrpc::Encodable<crate::proto::tilde::agent_event_ingress::v1::RelayResponse>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RelayResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::agent_event_ingress::v1::RelayResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RelayResponseView<
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
pub const SIDECAR_SERVICE_SERVICE_NAME: &str = "tilde.agent_event_ingress.v1.SidecarService";
/// Static [`Spec`](::connectrpc::Spec) for the `Watch` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_WATCH_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/Watch",
        ::connectrpc::StreamType::ServerStream,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `Publish` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_PUBLISH_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/Publish",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `Hydrate` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_HYDRATE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/Hydrate",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `Forward` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_FORWARD_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/Forward",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `UploadAttachment` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_UPLOAD_ATTACHMENT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/UploadAttachment",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `DownloadAttachment` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_DOWNLOAD_ATTACHMENT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/DownloadAttachment",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ResolveParticipant` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_RESOLVE_PARTICIPANT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/ResolveParticipant",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `Relay` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_RELAY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/Relay",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Sidecars dial the gateway; every call authenticates one agent deployment token.
/// The gateway never opens a connection to a sidecar. Postgres is the record; a
/// replica is a cache plus a write-behind queue plus the executor for the threads
/// it holds a lease on.
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
pub trait SidecarService: Send + Sync + 'static {
    /// Held open for the life of a replica: a snapshot first, then configuration,
    /// lease changes and directives as they change.
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call (until the response stream is returned);
    /// message fields are read directly on it (zero-copy). Data the
    /// returned stream needs must be copied out or converted via
    /// `.to_owned_message()`.
    fn watch(
        &self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::WatchRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            ::connectrpc::ServiceStream<
                impl ::connectrpc::Encodable<
                    crate::proto::tilde::agent_event_ingress::v1::WatchResponse,
                > + Send + use<Self>,
            >,
        >,
    > + Send;
    /// Heartbeats, typed events, directive results, lease releases and telemetry, in order.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn publish<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::PublishRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::PublishResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Conversation state from the projection, taking the thread lease in the same call when asked.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn hydrate<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::HydrateRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::HydrateResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Run one ingress call on a live replica and relay its answer.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn forward<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::ForwardRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::ForwardResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the UploadAttachment RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn upload_attachment<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the DownloadAttachment RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn download_attachment<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the ResolveParticipant RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn resolve_participant<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Registry RPCs from the agent process, re-verified at the gateway.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn relay<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::RelayRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::RelayResponse,
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
pub trait SidecarServiceExt: SidecarService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: SidecarService> SidecarServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_server_stream::<
                _,
                _,
                crate::proto::tilde::agent_event_ingress::v1::WatchResponse,
            >(
                SIDECAR_SERVICE_SERVICE_NAME,
                "Watch",
                ::connectrpc::view_streaming_handler_fn({
                    let svc = ::std::sync::Arc::clone(&self);
                    move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchRequestView<
                                'static,
                            >,
                        >|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::WatchRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.watch(ctx, sreq).await
                        }
                    }
                }),
            )
            .with_spec(SIDECAR_SERVICE_WATCH_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "Publish",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::PublishRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::PublishRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.publish(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::PublishResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_PUBLISH_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "Hydrate",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HydrateRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::HydrateRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.hydrate(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::HydrateResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_HYDRATE_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "Forward",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ForwardRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::ForwardRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.forward(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::ForwardResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_FORWARD_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "UploadAttachment",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::UploadAttachmentRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.upload_attachment(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_UPLOAD_ATTACHMENT_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "DownloadAttachment",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::DownloadAttachmentRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.download_attachment(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_DOWNLOAD_ATTACHMENT_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "ResolveParticipant",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ResolveParticipantRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.resolve_participant(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_RESOLVE_PARTICIPANT_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "Relay",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RelayRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::RelayRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.relay(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::RelayResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_RELAY_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct SidecarServiceRegisterMarker;
impl<S: SidecarService> ::connectrpc::ServiceRegister<SidecarServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as SidecarServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `SidecarService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = SidecarServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct SidecarServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: SidecarService> SidecarServiceServer<T> {
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
impl<T> Clone for SidecarServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: SidecarService> ::connectrpc::Dispatcher for SidecarServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("tilde.agent_event_ingress.v1.SidecarService/")?;
        match method {
            "Watch" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::server_streaming()
                        .with_spec(SIDECAR_SERVICE_WATCH_SPEC),
                )
            }
            "Publish" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_PUBLISH_SPEC),
                )
            }
            "Hydrate" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_HYDRATE_SPEC),
                )
            }
            "Forward" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_FORWARD_SPEC),
                )
            }
            "UploadAttachment" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_UPLOAD_ATTACHMENT_SPEC),
                )
            }
            "DownloadAttachment" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_DOWNLOAD_ATTACHMENT_SPEC),
                )
            }
            "ResolveParticipant" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_RESOLVE_PARTICIPANT_SPEC),
                )
            }
            "Relay" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_RELAY_SPEC),
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
        let Some(method) = path
            .strip_prefix("tilde.agent_event_ingress.v1.SidecarService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "Publish" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::PublishRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::PublishRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::PublishRequest,
                    >::from_parts(&req, &body);
                    svc.publish(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::PublishResponse,
                        >(format)
                })
            }
            "Hydrate" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::HydrateRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HydrateRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::HydrateRequest,
                    >::from_parts(&req, &body);
                    svc.hydrate(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::HydrateResponse,
                        >(format)
                })
            }
            "Forward" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::ForwardRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ForwardRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::ForwardRequest,
                    >::from_parts(&req, &body);
                    svc.forward(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::ForwardResponse,
                        >(format)
                })
            }
            "UploadAttachment" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::UploadAttachmentRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentRequest,
                    >::from_parts(&req, &body);
                    svc.upload_attachment(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentResponse,
                        >(format)
                })
            }
            "DownloadAttachment" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::DownloadAttachmentRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentRequest,
                    >::from_parts(&req, &body);
                    svc.download_attachment(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentResponse,
                        >(format)
                })
            }
            "ResolveParticipant" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ResolveParticipantRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantRequest,
                    >::from_parts(&req, &body);
                    svc.resolve_participant(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantResponse,
                        >(format)
                })
            }
            "Relay" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::RelayRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RelayRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::RelayRequest,
                    >::from_parts(&req, &body);
                    svc.relay(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::RelayResponse,
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
        let Some(method) = path
            .strip_prefix("tilde.agent_event_ingress.v1.SidecarService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_streaming(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "Watch" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::WatchRequest,
                    >(request, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::WatchRequest,
                    >::from_parts(&req, &body);
                    let resp = svc.watch(ctx, req).await?;
                    Ok(
                        resp
                            .map_body(|s| ::connectrpc::dispatcher::codegen::encode_response_stream::<
                                crate::proto::tilde::agent_event_ingress::v1::WatchResponse,
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
        let Some(method) = path
            .strip_prefix("tilde.agent_event_ingress.v1.SidecarService/") else {
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
        let Some(method) = path
            .strip_prefix("tilde.agent_event_ingress.v1.SidecarService/") else {
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
/// let client = SidecarServiceClient::new(conn, config);
/// let response = client.watch(request).await?;
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
/// let client = SidecarServiceClient::new(http, config);
/// let response = client.watch(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.watch(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.watch(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct SidecarServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> SidecarServiceClient<T>
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
    /// Call the Watch RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/Watch.
    pub async fn watch(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::WatchRequest,
    ) -> Result<
        ::connectrpc::client::ServerStream<
            T::ResponseBody,
            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchResponseView<
                'static,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.watch_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the Watch RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn watch_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::WatchRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::ServerStream<
            T::ResponseBody,
            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchResponseView<
                'static,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_server_stream(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_WATCH_SPEC.with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the Publish RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/Publish.
    pub async fn publish(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::PublishRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::PublishResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.publish_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the Publish RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn publish_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::PublishRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::PublishResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_PUBLISH_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the Hydrate RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/Hydrate.
    pub async fn hydrate(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::HydrateRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HydrateResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.hydrate_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the Hydrate RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn hydrate_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::HydrateRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HydrateResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_HYDRATE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the Forward RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/Forward.
    pub async fn forward(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::ForwardRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ForwardResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.forward_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the Forward RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn forward_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::ForwardRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ForwardResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_FORWARD_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UploadAttachment RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/UploadAttachment.
    pub async fn upload_attachment(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::UploadAttachmentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.upload_attachment_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UploadAttachment RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn upload_attachment_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::UploadAttachmentRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::UploadAttachmentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_UPLOAD_ATTACHMENT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the DownloadAttachment RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/DownloadAttachment.
    pub async fn download_attachment(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::DownloadAttachmentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.download_attachment_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the DownloadAttachment RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn download_attachment_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::DownloadAttachmentRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::DownloadAttachmentResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_DOWNLOAD_ATTACHMENT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ResolveParticipant RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/ResolveParticipant.
    pub async fn resolve_participant(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ResolveParticipantResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.resolve_participant_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ResolveParticipant RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn resolve_participant_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::ResolveParticipantRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ResolveParticipantResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_RESOLVE_PARTICIPANT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the Relay RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/Relay.
    pub async fn relay(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::RelayRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RelayResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.relay_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the Relay RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn relay_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::RelayRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RelayResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_RELAY_SPEC.with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
