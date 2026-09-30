///Shorthand for `OwnedView<ListSkillsRequestView<'static>>`.
pub type OwnedListSkillsRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::ListSkillsRequestView<'static>,
>;
///Shorthand for `OwnedView<ListSkillsResponseView<'static>>`.
pub type OwnedListSkillsResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::ListSkillsResponseView<'static>,
>;
///Shorthand for `OwnedView<ReadSkillFileRequestView<'static>>`.
pub type OwnedReadSkillFileRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::ReadSkillFileRequestView<'static>,
>;
///Shorthand for `OwnedView<ReadSkillFileResponseView<'static>>`.
pub type OwnedReadSkillFileResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::ReadSkillFileResponseView<'static>,
>;
///Shorthand for `OwnedView<ListSkillSourcesRequestView<'static>>`.
pub type OwnedListSkillSourcesRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::ListSkillSourcesRequestView<'static>,
>;
///Shorthand for `OwnedView<ListSkillSourcesResponseView<'static>>`.
pub type OwnedListSkillSourcesResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::ListSkillSourcesResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<AssignSkillRequestView<'static>>`.
pub type OwnedAssignSkillRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::AssignSkillRequestView<'static>,
>;
///Shorthand for `OwnedView<AssignSkillResponseView<'static>>`.
pub type OwnedAssignSkillResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::AssignSkillResponseView<'static>,
>;
///Shorthand for `OwnedView<UnassignSkillRequestView<'static>>`.
pub type OwnedUnassignSkillRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::UnassignSkillRequestView<'static>,
>;
///Shorthand for `OwnedView<UnassignSkillResponseView<'static>>`.
pub type OwnedUnassignSkillResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::UnassignSkillResponseView<'static>,
>;
///Shorthand for `OwnedView<WriteSkillRequestView<'static>>`.
pub type OwnedWriteSkillRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::WriteSkillRequestView<'static>,
>;
///Shorthand for `OwnedView<WriteSkillResponseView<'static>>`.
pub type OwnedWriteSkillResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::WriteSkillResponseView<'static>,
>;
///Shorthand for `OwnedView<SyncSkillSourceRequestView<'static>>`.
pub type OwnedSyncSkillSourceRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::SyncSkillSourceRequestView<'static>,
>;
///Shorthand for `OwnedView<SyncSkillSourceResponseView<'static>>`.
pub type OwnedSyncSkillSourceResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::SyncSkillSourceResponseView<'static>,
>;
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::ListSkillsResponse>
for crate::proto::tilde::runtime::v1::__buffa::view::ListSkillsResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::ListSkillsResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::ListSkillsResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::ReadSkillFileResponse>
for crate::proto::tilde::runtime::v1::__buffa::view::ReadSkillFileResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::ReadSkillFileResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::ReadSkillFileResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::ListSkillSourcesResponse>
for crate::proto::tilde::runtime::v1::__buffa::view::ListSkillSourcesResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::ListSkillSourcesResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::ListSkillSourcesResponseView<
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
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::AssignSkillResponse>
for crate::proto::tilde::runtime::v1::__buffa::view::AssignSkillResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::AssignSkillResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::AssignSkillResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::UnassignSkillResponse>
for crate::proto::tilde::runtime::v1::__buffa::view::UnassignSkillResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::UnassignSkillResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::UnassignSkillResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::WriteSkillResponse>
for crate::proto::tilde::runtime::v1::__buffa::view::WriteSkillResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::WriteSkillResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::WriteSkillResponseView<'static>,
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
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::SyncSkillSourceResponse>
for crate::proto::tilde::runtime::v1::__buffa::view::SyncSkillSourceResponseView<'_> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<crate::proto::tilde::runtime::v1::SyncSkillSourceResponse>
for ::buffa::view::OwnedView<
    crate::proto::tilde::runtime::v1::__buffa::view::SyncSkillSourceResponseView<'static>,
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
pub const SKILL_SERVICE_SERVICE_NAME: &str = "tilde.runtime.v1.SkillService";
/// Static [`Spec`](::connectrpc::Spec) for the `ListSkills` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SKILL_SERVICE_LIST_SKILLS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.runtime.v1.SkillService/ListSkills",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ReadSkillFile` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SKILL_SERVICE_READ_SKILL_FILE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.runtime.v1.SkillService/ReadSkillFile",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `ListSkillSources` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SKILL_SERVICE_LIST_SKILL_SOURCES_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.runtime.v1.SkillService/ListSkillSources",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::NoSideEffects);
/// Static [`Spec`](::connectrpc::Spec) for the `AssignSkill` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SKILL_SERVICE_ASSIGN_SKILL_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.runtime.v1.SkillService/AssignSkill",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `UnassignSkill` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SKILL_SERVICE_UNASSIGN_SKILL_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.runtime.v1.SkillService/UnassignSkill",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `WriteSkill` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SKILL_SERVICE_WRITE_SKILL_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.runtime.v1.SkillService/WriteSkill",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `SyncSkillSource` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SKILL_SERVICE_SYNC_SKILL_SOURCE_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.runtime.v1.SkillService/SyncSkillSource",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// The skills an agent is given: every skill of its assigned sources plus single skills, at
/// their latest version. Listing returns names, descriptions and paths so an agent loads a
/// skill's files only when it needs them. A skill is addressed as `name`, or `source/name`
/// when two assigned sources share a name. Another agent's skills need agents.read on it;
/// assigning needs agents.edit_skills on the target (itself included) and skills.read (or
/// skills.edit) on the skill's source; writing skills into a source or syncing it needs
/// skills.edit on the source.
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
pub trait SkillService: Send + Sync + 'static {
    /// Handle the ListSkills RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_skills<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::runtime::v1::ListSkillsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::runtime::v1::ListSkillsResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the ReadSkillFile RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn read_skill_file<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::runtime::v1::ReadSkillFileRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::runtime::v1::ReadSkillFileResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// The skill sources the agent holds skills.read or skills.edit on, with their skills.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn list_skill_sources<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::runtime::v1::ListSkillSourcesRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::runtime::v1::ListSkillSourcesResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the AssignSkill RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn assign_skill<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::runtime::v1::AssignSkillRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::runtime::v1::AssignSkillResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the UnassignSkill RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn unassign_skill<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::runtime::v1::UnassignSkillRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::runtime::v1::UnassignSkillResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the WriteSkill RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn write_skill<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::runtime::v1::WriteSkillRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::runtime::v1::WriteSkillResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the SyncSkillSource RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn sync_skill_source<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::runtime::v1::SyncSkillSourceRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::runtime::v1::SyncSkillSourceResponse,
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
pub trait SkillServiceExt: SkillService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router;
}
impl<S: SkillService> SkillServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view_idempotent(
                SKILL_SERVICE_SERVICE_NAME,
                "ListSkills",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::runtime::v1::__buffa::view::ListSkillsRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::runtime::v1::ListSkillsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_skills(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::runtime::v1::ListSkillsResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SKILL_SERVICE_LIST_SKILLS_SPEC)
            .route_view_idempotent(
                SKILL_SERVICE_SERVICE_NAME,
                "ReadSkillFile",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::runtime::v1::__buffa::view::ReadSkillFileRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::runtime::v1::ReadSkillFileRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.read_skill_file(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::runtime::v1::ReadSkillFileResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SKILL_SERVICE_READ_SKILL_FILE_SPEC)
            .route_view_idempotent(
                SKILL_SERVICE_SERVICE_NAME,
                "ListSkillSources",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::runtime::v1::__buffa::view::ListSkillSourcesRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::runtime::v1::ListSkillSourcesRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.list_skill_sources(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::runtime::v1::ListSkillSourcesResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SKILL_SERVICE_LIST_SKILL_SOURCES_SPEC)
            .route_view(
                SKILL_SERVICE_SERVICE_NAME,
                "AssignSkill",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::runtime::v1::__buffa::view::AssignSkillRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::runtime::v1::AssignSkillRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.assign_skill(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::runtime::v1::AssignSkillResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SKILL_SERVICE_ASSIGN_SKILL_SPEC)
            .route_view(
                SKILL_SERVICE_SERVICE_NAME,
                "UnassignSkill",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::runtime::v1::__buffa::view::UnassignSkillRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::runtime::v1::UnassignSkillRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.unassign_skill(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::runtime::v1::UnassignSkillResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SKILL_SERVICE_UNASSIGN_SKILL_SPEC)
            .route_view(
                SKILL_SERVICE_SERVICE_NAME,
                "WriteSkill",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::runtime::v1::__buffa::view::WriteSkillRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::runtime::v1::WriteSkillRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.write_skill(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::runtime::v1::WriteSkillResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SKILL_SERVICE_WRITE_SKILL_SPEC)
            .route_view(
                SKILL_SERVICE_SERVICE_NAME,
                "SyncSkillSource",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::runtime::v1::__buffa::view::SyncSkillSourceRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::runtime::v1::SyncSkillSourceRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.sync_skill_source(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::runtime::v1::SyncSkillSourceResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SKILL_SERVICE_SYNC_SKILL_SOURCE_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct SkillServiceRegisterMarker;
impl<S: SkillService> ::connectrpc::ServiceRegister<SkillServiceRegisterMarker>
for ::std::sync::Arc<S> {
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as SkillServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `SkillService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = SkillServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct SkillServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: SkillService> SkillServiceServer<T> {
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
impl<T> Clone for SkillServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: SkillService> ::connectrpc::Dispatcher for SkillServiceServer<T> {
    #[inline]
    fn lookup(
        &self,
        path: &str,
    ) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("tilde.runtime.v1.SkillService/")?;
        match method {
            "ListSkills" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(SKILL_SERVICE_LIST_SKILLS_SPEC),
                )
            }
            "ReadSkillFile" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(SKILL_SERVICE_READ_SKILL_FILE_SPEC),
                )
            }
            "ListSkillSources" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(true)
                        .with_spec(SKILL_SERVICE_LIST_SKILL_SOURCES_SPEC),
                )
            }
            "AssignSkill" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SKILL_SERVICE_ASSIGN_SKILL_SPEC),
                )
            }
            "UnassignSkill" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SKILL_SERVICE_UNASSIGN_SKILL_SPEC),
                )
            }
            "WriteSkill" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SKILL_SERVICE_WRITE_SKILL_SPEC),
                )
            }
            "SyncSkillSource" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SKILL_SERVICE_SYNC_SKILL_SOURCE_SPEC),
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
        let Some(method) = path.strip_prefix("tilde.runtime.v1.SkillService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "ListSkills" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::runtime::v1::ListSkillsRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::runtime::v1::__buffa::view::ListSkillsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::runtime::v1::ListSkillsRequest,
                    >::from_parts(&req, &body);
                    svc.list_skills(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::runtime::v1::ListSkillsResponse,
                        >(format)
                })
            }
            "ReadSkillFile" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::runtime::v1::ReadSkillFileRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::runtime::v1::__buffa::view::ReadSkillFileRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::runtime::v1::ReadSkillFileRequest,
                    >::from_parts(&req, &body);
                    svc.read_skill_file(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::runtime::v1::ReadSkillFileResponse,
                        >(format)
                })
            }
            "ListSkillSources" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::runtime::v1::ListSkillSourcesRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::runtime::v1::__buffa::view::ListSkillSourcesRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::runtime::v1::ListSkillSourcesRequest,
                    >::from_parts(&req, &body);
                    svc.list_skill_sources(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::runtime::v1::ListSkillSourcesResponse,
                        >(format)
                })
            }
            "AssignSkill" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::runtime::v1::AssignSkillRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::runtime::v1::__buffa::view::AssignSkillRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::runtime::v1::AssignSkillRequest,
                    >::from_parts(&req, &body);
                    svc.assign_skill(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::runtime::v1::AssignSkillResponse,
                        >(format)
                })
            }
            "UnassignSkill" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::runtime::v1::UnassignSkillRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::runtime::v1::__buffa::view::UnassignSkillRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::runtime::v1::UnassignSkillRequest,
                    >::from_parts(&req, &body);
                    svc.unassign_skill(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::runtime::v1::UnassignSkillResponse,
                        >(format)
                })
            }
            "WriteSkill" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::runtime::v1::WriteSkillRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::runtime::v1::__buffa::view::WriteSkillRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::runtime::v1::WriteSkillRequest,
                    >::from_parts(&req, &body);
                    svc.write_skill(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::runtime::v1::WriteSkillResponse,
                        >(format)
                })
            }
            "SyncSkillSource" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::runtime::v1::SyncSkillSourceRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::runtime::v1::__buffa::view::SyncSkillSourceRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::runtime::v1::SyncSkillSourceRequest,
                    >::from_parts(&req, &body);
                    svc.sync_skill_source(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::runtime::v1::SyncSkillSourceResponse,
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
        let Some(method) = path.strip_prefix("tilde.runtime.v1.SkillService/") else {
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
        let Some(method) = path.strip_prefix("tilde.runtime.v1.SkillService/") else {
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
        let Some(method) = path.strip_prefix("tilde.runtime.v1.SkillService/") else {
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
/// let client = SkillServiceClient::new(conn, config);
/// let response = client.list_skills(request).await?;
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
/// let client = SkillServiceClient::new(http, config);
/// let response = client.list_skills(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.list_skills(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.list_skills(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct SkillServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> SkillServiceClient<T>
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
    /// Call the ListSkills RPC. Sends a request to /tilde.runtime.v1.SkillService/ListSkills.
    pub async fn list_skills(
        &self,
        request: crate::proto::tilde::runtime::v1::ListSkillsRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::ListSkillsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_skills_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListSkills RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_skills_with_options(
        &self,
        request: crate::proto::tilde::runtime::v1::ListSkillsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::ListSkillsResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SKILL_SERVICE_LIST_SKILLS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ReadSkillFile RPC. Sends a request to /tilde.runtime.v1.SkillService/ReadSkillFile.
    pub async fn read_skill_file(
        &self,
        request: crate::proto::tilde::runtime::v1::ReadSkillFileRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::ReadSkillFileResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.read_skill_file_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ReadSkillFile RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn read_skill_file_with_options(
        &self,
        request: crate::proto::tilde::runtime::v1::ReadSkillFileRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::ReadSkillFileResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SKILL_SERVICE_READ_SKILL_FILE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ListSkillSources RPC. Sends a request to /tilde.runtime.v1.SkillService/ListSkillSources.
    pub async fn list_skill_sources(
        &self,
        request: crate::proto::tilde::runtime::v1::ListSkillSourcesRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::ListSkillSourcesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.list_skill_sources_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ListSkillSources RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn list_skill_sources_with_options(
        &self,
        request: crate::proto::tilde::runtime::v1::ListSkillSourcesRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::ListSkillSourcesResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SKILL_SERVICE_LIST_SKILL_SOURCES_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the AssignSkill RPC. Sends a request to /tilde.runtime.v1.SkillService/AssignSkill.
    pub async fn assign_skill(
        &self,
        request: crate::proto::tilde::runtime::v1::AssignSkillRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::AssignSkillResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.assign_skill_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the AssignSkill RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn assign_skill_with_options(
        &self,
        request: crate::proto::tilde::runtime::v1::AssignSkillRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::AssignSkillResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SKILL_SERVICE_ASSIGN_SKILL_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the UnassignSkill RPC. Sends a request to /tilde.runtime.v1.SkillService/UnassignSkill.
    pub async fn unassign_skill(
        &self,
        request: crate::proto::tilde::runtime::v1::UnassignSkillRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::UnassignSkillResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.unassign_skill_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the UnassignSkill RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn unassign_skill_with_options(
        &self,
        request: crate::proto::tilde::runtime::v1::UnassignSkillRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::UnassignSkillResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SKILL_SERVICE_UNASSIGN_SKILL_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the WriteSkill RPC. Sends a request to /tilde.runtime.v1.SkillService/WriteSkill.
    pub async fn write_skill(
        &self,
        request: crate::proto::tilde::runtime::v1::WriteSkillRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::WriteSkillResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.write_skill_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the WriteSkill RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn write_skill_with_options(
        &self,
        request: crate::proto::tilde::runtime::v1::WriteSkillRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::WriteSkillResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SKILL_SERVICE_WRITE_SKILL_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the SyncSkillSource RPC. Sends a request to /tilde.runtime.v1.SkillService/SyncSkillSource.
    pub async fn sync_skill_source(
        &self,
        request: crate::proto::tilde::runtime::v1::SyncSkillSourceRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::SyncSkillSourceResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.sync_skill_source_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the SyncSkillSource RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn sync_skill_source_with_options(
        &self,
        request: crate::proto::tilde::runtime::v1::SyncSkillSourceRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::runtime::v1::__buffa::view::SyncSkillSourceResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SKILL_SERVICE_SYNC_SKILL_SOURCE_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
