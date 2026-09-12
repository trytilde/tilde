///Shorthand for `OwnedView<LocateConversationRequestView<'static>>`.
pub type OwnedLocateConversationRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::LocateConversationRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<LocateConversationResponseView<'static>>`.
pub type OwnedLocateConversationResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::LocateConversationResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<IngestProviderEventRequestView<'static>>`.
pub type OwnedIngestProviderEventRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::IngestProviderEventRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<IngestProviderEventResponseView<'static>>`.
pub type OwnedIngestProviderEventResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::IngestProviderEventResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<WatchCommandsRequestView<'static>>`.
pub type OwnedWatchCommandsRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchCommandsRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<WatchCommandsResponseView<'static>>`.
pub type OwnedWatchCommandsResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchCommandsResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetInvocationRequestView<'static>>`.
pub type OwnedGetInvocationRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetInvocationRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetInvocationResponseView<'static>>`.
pub type OwnedGetInvocationResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetInvocationResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<AcknowledgeCommandRequestView<'static>>`.
pub type OwnedAcknowledgeCommandRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::AcknowledgeCommandRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<AcknowledgeCommandResponseView<'static>>`.
pub type OwnedAcknowledgeCommandResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::AcknowledgeCommandResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<CompleteCommandRequestView<'static>>`.
pub type OwnedCompleteCommandRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::CompleteCommandRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<CompleteCommandResponseView<'static>>`.
pub type OwnedCompleteCommandResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::CompleteCommandResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ReportActivityRequestView<'static>>`.
pub type OwnedReportActivityRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ReportActivityRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ReportActivityResponseView<'static>>`.
pub type OwnedReportActivityResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ReportActivityResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<RegisterSidecarRequestView<'static>>`.
pub type OwnedRegisterSidecarRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RegisterSidecarRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<RegisterSidecarResponseView<'static>>`.
pub type OwnedRegisterSidecarResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RegisterSidecarResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<HeartbeatRequestView<'static>>`.
pub type OwnedHeartbeatRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HeartbeatRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<HeartbeatResponseView<'static>>`.
pub type OwnedHeartbeatResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HeartbeatResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetConfigurationRequestView<'static>>`.
pub type OwnedGetConfigurationRequestView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetConfigurationRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetConfigurationResponseView<'static>>`.
pub type OwnedGetConfigurationResponseView = ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetConfigurationResponseView<
        'static,
    >,
>;
impl ::connectrpc::Encodable<
    crate::proto::tilde::agent_event_ingress::v1::LocateConversationResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::LocateConversationResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::LocateConversationResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::LocateConversationResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::IngestProviderEventResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::IngestProviderEventResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::WatchCommandsResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchCommandsResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::WatchCommandsResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchCommandsResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::GetInvocationResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetInvocationResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::GetInvocationResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetInvocationResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::AcknowledgeCommandResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::AcknowledgeCommandResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::CompleteCommandResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::CompleteCommandResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::CompleteCommandResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::CompleteCommandResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::ReportActivityResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ReportActivityResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::ReportActivityResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ReportActivityResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RegisterSidecarResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RegisterSidecarResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::HeartbeatResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HeartbeatResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::HeartbeatResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HeartbeatResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::GetConfigurationResponse,
>
for crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetConfigurationResponseView<
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
    crate::proto::tilde::agent_event_ingress::v1::GetConfigurationResponse,
>
for ::buffa::view::OwnedView<
    crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetConfigurationResponseView<
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
/// Static [`Spec`](::connectrpc::Spec) for the `LocateConversation` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_LOCATE_CONVERSATION_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/LocateConversation",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `IngestProviderEvent` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_INGEST_PROVIDER_EVENT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/IngestProviderEvent",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `WatchCommands` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_WATCH_COMMANDS_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/WatchCommands",
        ::connectrpc::StreamType::ServerStream,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `GetInvocation` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_GET_INVOCATION_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/GetInvocation",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `AcknowledgeCommand` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_ACKNOWLEDGE_COMMAND_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/AcknowledgeCommand",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `CompleteCommand` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_COMPLETE_COMMAND_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/CompleteCommand",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ReportActivity` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_REPORT_ACTIVITY_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/ReportActivity",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `RegisterSidecar` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_REGISTER_SIDECAR_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/RegisterSidecar",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `Heartbeat` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_HEARTBEAT_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/Heartbeat",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `GetConfiguration` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const SIDECAR_SERVICE_GET_CONFIGURATION_SPEC: ::connectrpc::Spec = ::connectrpc::Spec::server(
        "/tilde.agent_event_ingress.v1.SidecarService/GetConfiguration",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// This complete service mounts only on agent-event-ingress. Every call authenticates
/// an agent deployment token, never a browser session or agent invocation token.
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
    /// Handle the LocateConversation RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn locate_conversation<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::LocateConversationRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::LocateConversationResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the IngestProviderEvent RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn ingest_provider_event<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the WatchCommands RPC.
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call (until the response stream is returned);
    /// message fields are read directly on it (zero-copy). Data the
    /// returned stream needs must be copied out or converted via
    /// `.to_owned_message()`.
    fn watch_commands(
        &self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::WatchCommandsRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            ::connectrpc::ServiceStream<
                impl ::connectrpc::Encodable<
                    crate::proto::tilde::agent_event_ingress::v1::WatchCommandsResponse,
                > + Send + use<Self>,
            >,
        >,
    > + Send;
    /// Handle the GetInvocation RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_invocation<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::GetInvocationRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::GetInvocationResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the AcknowledgeCommand RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn acknowledge_command<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the CompleteCommand RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn complete_command<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::CompleteCommandRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::CompleteCommandResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the ReportActivity RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn report_activity<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::ReportActivityRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::ReportActivityResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the RegisterSidecar RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn register_sidecar<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the Heartbeat RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn heartbeat<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::HeartbeatRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::HeartbeatResponse,
            > + Send + use<'a, Self>,
        >,
    > + Send;
    /// Handle the GetConfiguration RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_configuration<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::proto::tilde::agent_event_ingress::v1::GetConfigurationRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::proto::tilde::agent_event_ingress::v1::GetConfigurationResponse,
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
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "LocateConversation",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::LocateConversationRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::LocateConversationRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.locate_conversation(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::LocateConversationResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_LOCATE_CONVERSATION_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "IngestProviderEvent",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::IngestProviderEventRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.ingest_provider_event(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_INGEST_PROVIDER_EVENT_SPEC)
            .route_view_server_stream::<
                _,
                _,
                crate::proto::tilde::agent_event_ingress::v1::WatchCommandsResponse,
            >(
                SIDECAR_SERVICE_SERVICE_NAME,
                "WatchCommands",
                ::connectrpc::view_streaming_handler_fn({
                    let svc = ::std::sync::Arc::clone(&self);
                    move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchCommandsRequestView<
                                'static,
                            >,
                        >|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::WatchCommandsRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.watch_commands(ctx, sreq).await
                        }
                    }
                }),
            )
            .with_spec(SIDECAR_SERVICE_WATCH_COMMANDS_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "GetInvocation",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetInvocationRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::GetInvocationRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_invocation(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::GetInvocationResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_GET_INVOCATION_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "AcknowledgeCommand",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::AcknowledgeCommandRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.acknowledge_command(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_ACKNOWLEDGE_COMMAND_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "CompleteCommand",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::CompleteCommandRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::CompleteCommandRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.complete_command(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::CompleteCommandResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_COMPLETE_COMMAND_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "ReportActivity",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ReportActivityRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::ReportActivityRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.report_activity(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::ReportActivityResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_REPORT_ACTIVITY_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "RegisterSidecar",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RegisterSidecarRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.register_sidecar(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_REGISTER_SIDECAR_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "Heartbeat",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HeartbeatRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::HeartbeatRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.heartbeat(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::HeartbeatResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_HEARTBEAT_SPEC)
            .route_view(
                SIDECAR_SERVICE_SERVICE_NAME,
                "GetConfiguration",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetConfigurationRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::proto::tilde::agent_event_ingress::v1::GetConfigurationRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_configuration(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::proto::tilde::agent_event_ingress::v1::GetConfigurationResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(SIDECAR_SERVICE_GET_CONFIGURATION_SPEC)
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
            "LocateConversation" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_LOCATE_CONVERSATION_SPEC),
                )
            }
            "IngestProviderEvent" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_INGEST_PROVIDER_EVENT_SPEC),
                )
            }
            "WatchCommands" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::server_streaming()
                        .with_spec(SIDECAR_SERVICE_WATCH_COMMANDS_SPEC),
                )
            }
            "GetInvocation" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_GET_INVOCATION_SPEC),
                )
            }
            "AcknowledgeCommand" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_ACKNOWLEDGE_COMMAND_SPEC),
                )
            }
            "CompleteCommand" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_COMPLETE_COMMAND_SPEC),
                )
            }
            "ReportActivity" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_REPORT_ACTIVITY_SPEC),
                )
            }
            "RegisterSidecar" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_REGISTER_SIDECAR_SPEC),
                )
            }
            "Heartbeat" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_HEARTBEAT_SPEC),
                )
            }
            "GetConfiguration" => {
                Some(
                    ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                        .with_spec(SIDECAR_SERVICE_GET_CONFIGURATION_SPEC),
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
            "LocateConversation" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::LocateConversationRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::LocateConversationRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::LocateConversationRequest,
                    >::from_parts(&req, &body);
                    svc.locate_conversation(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::LocateConversationResponse,
                        >(format)
                })
            }
            "IngestProviderEvent" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::IngestProviderEventRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventRequest,
                    >::from_parts(&req, &body);
                    svc.ingest_provider_event(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventResponse,
                        >(format)
                })
            }
            "GetInvocation" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::GetInvocationRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetInvocationRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::GetInvocationRequest,
                    >::from_parts(&req, &body);
                    svc.get_invocation(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::GetInvocationResponse,
                        >(format)
                })
            }
            "AcknowledgeCommand" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::AcknowledgeCommandRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandRequest,
                    >::from_parts(&req, &body);
                    svc.acknowledge_command(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandResponse,
                        >(format)
                })
            }
            "CompleteCommand" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::CompleteCommandRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::CompleteCommandRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::CompleteCommandRequest,
                    >::from_parts(&req, &body);
                    svc.complete_command(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::CompleteCommandResponse,
                        >(format)
                })
            }
            "ReportActivity" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::ReportActivityRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ReportActivityRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::ReportActivityRequest,
                    >::from_parts(&req, &body);
                    svc.report_activity(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::ReportActivityResponse,
                        >(format)
                })
            }
            "RegisterSidecar" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RegisterSidecarRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarRequest,
                    >::from_parts(&req, &body);
                    svc.register_sidecar(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarResponse,
                        >(format)
                })
            }
            "Heartbeat" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::HeartbeatRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HeartbeatRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::HeartbeatRequest,
                    >::from_parts(&req, &body);
                    svc.heartbeat(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::HeartbeatResponse,
                        >(format)
                })
            }
            "GetConfiguration" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::GetConfigurationRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetConfigurationRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::GetConfigurationRequest,
                    >::from_parts(&req, &body);
                    svc.get_configuration(ctx, req)
                        .await?
                        .encode::<
                            crate::proto::tilde::agent_event_ingress::v1::GetConfigurationResponse,
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
            "WatchCommands" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::proto::tilde::agent_event_ingress::v1::WatchCommandsRequest,
                    >(request, format)?;
                    let req: crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchCommandsRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::proto::tilde::agent_event_ingress::v1::WatchCommandsRequest,
                    >::from_parts(&req, &body);
                    let resp = svc.watch_commands(ctx, req).await?;
                    Ok(
                        resp
                            .map_body(|s| ::connectrpc::dispatcher::codegen::encode_response_stream::<
                                crate::proto::tilde::agent_event_ingress::v1::WatchCommandsResponse,
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
/// let response = client.locate_conversation(request).await?;
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
/// let response = client.locate_conversation(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.locate_conversation(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.locate_conversation(request).await?.into_owned();
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
    /// Call the LocateConversation RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/LocateConversation.
    pub async fn locate_conversation(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::LocateConversationRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::LocateConversationResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.locate_conversation_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the LocateConversation RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn locate_conversation_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::LocateConversationRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::LocateConversationResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_LOCATE_CONVERSATION_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the IngestProviderEvent RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/IngestProviderEvent.
    pub async fn ingest_provider_event(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::IngestProviderEventResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.ingest_provider_event_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the IngestProviderEvent RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn ingest_provider_event_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::IngestProviderEventRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::IngestProviderEventResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_INGEST_PROVIDER_EVENT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the WatchCommands RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/WatchCommands.
    pub async fn watch_commands(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::WatchCommandsRequest,
    ) -> Result<
        ::connectrpc::client::ServerStream<
            T::ResponseBody,
            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchCommandsResponseView<
                'static,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.watch_commands_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the WatchCommands RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn watch_commands_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::WatchCommandsRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::ServerStream<
            T::ResponseBody,
            crate::proto::tilde::agent_event_ingress::v1::__buffa::view::WatchCommandsResponseView<
                'static,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_server_stream(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_WATCH_COMMANDS_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetInvocation RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/GetInvocation.
    pub async fn get_invocation(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::GetInvocationRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetInvocationResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_invocation_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetInvocation RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_invocation_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::GetInvocationRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetInvocationResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_GET_INVOCATION_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the AcknowledgeCommand RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/AcknowledgeCommand.
    pub async fn acknowledge_command(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::AcknowledgeCommandResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.acknowledge_command_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the AcknowledgeCommand RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn acknowledge_command_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::AcknowledgeCommandRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::AcknowledgeCommandResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_ACKNOWLEDGE_COMMAND_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the CompleteCommand RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/CompleteCommand.
    pub async fn complete_command(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::CompleteCommandRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::CompleteCommandResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.complete_command_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the CompleteCommand RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn complete_command_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::CompleteCommandRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::CompleteCommandResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_COMPLETE_COMMAND_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the ReportActivity RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/ReportActivity.
    pub async fn report_activity(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::ReportActivityRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ReportActivityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.report_activity_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the ReportActivity RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn report_activity_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::ReportActivityRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::ReportActivityResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_REPORT_ACTIVITY_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the RegisterSidecar RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/RegisterSidecar.
    pub async fn register_sidecar(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RegisterSidecarResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.register_sidecar_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the RegisterSidecar RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn register_sidecar_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::RegisterSidecarRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::RegisterSidecarResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_REGISTER_SIDECAR_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the Heartbeat RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/Heartbeat.
    pub async fn heartbeat(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::HeartbeatRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HeartbeatResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.heartbeat_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the Heartbeat RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn heartbeat_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::HeartbeatRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::HeartbeatResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_HEARTBEAT_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
    /// Call the GetConfiguration RPC. Sends a request to /tilde.agent_event_ingress.v1.SidecarService/GetConfiguration.
    pub async fn get_configuration(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::GetConfigurationRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetConfigurationResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        self.get_configuration_with_options(
                request,
                ::connectrpc::client::CallOptions::default(),
            )
            .await
    }
    /// Call the GetConfiguration RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_configuration_with_options(
        &self,
        request: crate::proto::tilde::agent_event_ingress::v1::GetConfigurationRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::proto::tilde::agent_event_ingress::v1::__buffa::view::GetConfigurationResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    > {
        ::connectrpc::client::call_unary(
                &self.transport,
                &self.config,
                SIDECAR_SERVICE_GET_CONFIGURATION_SPEC
                    .with_origin(::connectrpc::SpecOrigin::Client),
                request,
                options,
            )
            .await
    }
}
