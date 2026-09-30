"""Tool hosts: processes that serve tools to agents, connected over Watch or invoked as a Lambda.

A connected host dials ``ToolHostService.Watch`` with its tool host token, publishes its tools
(and a provider, when they need credentials) and answers each call or verify frame through
``Respond``. A Lambda host answers Protobuf-JSON ``LambdaRequest`` events synchronously.
Schemas come from pydantic models: tool input and output from the decorated function's
annotations, credential forms from each auth method's model.
"""

from __future__ import annotations

import asyncio
import inspect
import logging
import os
import typing
from collections.abc import Awaitable, Callable
from dataclasses import dataclass, field
from typing import Any, Generic, Literal, TypeVar

import pydantic_core
from connectrpc.code import Code
from connectrpc.errors import ConnectError
from google.protobuf.json_format import MessageToDict, ParseDict
from pydantic import BaseModel, ValidationError, create_model

from tilde._cancel import Cancellation
from tilde._transport import http_client
from tilde.tool_host.v1 import tool_host_pb2 as pb
from tilde.tool_host.v1.tool_host_connect import ToolHostServiceClient
from tilde.types.v1 import chat_pb2, connections_pb2

log = logging.getLogger("tilde")

A = TypeVar("A")

Grant = Literal["authorization_code", "client_credentials", "jwt_bearer"]
_GRANTS = {
    "authorization_code": connections_pb2.O_AUTH_GRANT_AUTHORIZATION_CODE,
    "client_credentials": connections_pb2.O_AUTH_GRANT_CLIENT_CREDENTIALS,
    "jwt_bearer": connections_pb2.O_AUTH_GRANT_JWT_BEARER,
}
# The flat JSON Schema subset Tilde accepts for credential forms (connections/schema.rs).
_PROPERTY_KEYS = {
    "type",
    "title",
    "description",
    "writeOnly",
    "enum",
    "default",
    "minLength",
    "maxLength",
    "pattern",
    "format",
    "minimum",
    "maximum",
    "contentMediaType",
}


@dataclass(slots=True)
class Method:
    """One way to set up an instance: a static form (``schema``) or an OAuth grant.

    Credential values always arrive as strings, so keep method models to ``str``, ``SecretStr``
    (rendered as a secret field) and enum/``Literal`` string fields. Mark other secret fields
    with ``Field(json_schema_extra={"writeOnly": True})``. For OAuth, ``oauth`` is a
    ``tilde.types.v1.OAuthConfiguration`` or a dict of its fields; the instance's credentials
    carry ``access_token`` (refreshed by Tilde) plus the fields of ``additional_schema``.
    """

    name: str
    schema: type[BaseModel] | None = None
    oauth: connections_pb2.OAuthConfiguration | dict[str, Any] | None = None
    grant: Grant = "authorization_code"
    additional_schema: type[BaseModel] | None = None
    # The model ``ctx.auth`` is parsed into for this method.
    credentials: type[BaseModel] = field(init=False)

    def __post_init__(self) -> None:
        if (self.schema is None) == (self.oauth is None):
            raise ValueError(f"Method {self.name!r} needs exactly one of schema or oauth")
        self.credentials = self.schema or create_model(
            "OAuthCredentials",
            __base__=self.additional_schema or BaseModel,
            access_token=(str, ...),
        )


@dataclass(slots=True)
class Instance:
    """The connection being verified, and the method it was set up with."""

    connection_id: str
    method: str


VerifyFn = Callable[[Any, Instance], Awaitable[str | None]]


@dataclass(slots=True)
class ToolContext(Generic[A]):
    """One call. ``call_id`` is the agent's tool call ID: use it for upstream idempotency.

    ``auth`` is the instance's credentials parsed into its method's model (``None`` for tools
    without auth); ``cancellation`` aborts when the host closes.
    """

    call_id: str
    agent_id: str
    thread_id: str
    connection_id: str | None
    auth_method: str | None
    auth: A | None
    cancellation: Cancellation


@dataclass(slots=True, frozen=True)
class Annotations:
    """Hints, not guarantees: frameworks may use them to order, confirm or parallelise calls."""

    read_only: bool = False
    destructive: bool = False
    idempotent: bool = False
    open_world: bool = False


@dataclass(slots=True)
class HostedTool:
    name: str
    description: str
    summary: str
    annotations: Annotations | None
    input_model: type[BaseModel]
    output_model: type[BaseModel] | None
    run: Callable[[Any, ToolContext[Any]], Awaitable[Any]]
    auth: Auth | None


def tool(
    *,
    description: str,
    name: str | None = None,
    summary: str = "",
    annotations: Annotations | None = None,
    input_schema: type[BaseModel] | None = None,
    output_schema: type[BaseModel] | None = None,
) -> Callable[[Callable[..., Awaitable[Any]]], HostedTool]:
    """Declare an async tool that needs no credentials (``ctx.auth`` is ``None``).

    ``async def run(input: InputModel, ctx: ToolContext) -> OutputModel``: the input model comes
    from the first parameter's annotation and the output model from the return annotation unless
    ``input_schema``/``output_schema`` are given. Without an output model the result is
    serialized as is. A raised exception's message is returned to the agent as the tool error.
    """
    return _decorator(None, description, name, summary, annotations, input_schema, output_schema)


def _decorator(auth, description, name, summary, annotations, input_schema, output_schema):
    def decorate(fn: Callable[..., Awaitable[Any]]) -> HostedTool:
        if not inspect.iscoroutinefunction(fn):
            raise TypeError(f"Tool {fn.__name__} must be an async function")
        hints = typing.get_type_hints(fn)
        params = list(inspect.signature(fn).parameters)
        input_model = input_schema or (hints.get(params[0]) if params else None)
        if not (isinstance(input_model, type) and issubclass(input_model, BaseModel)):
            raise TypeError(f"Tool {fn.__name__} needs a pydantic input model")
        output_model = output_schema or hints.get("return")
        if not (isinstance(output_model, type) and issubclass(output_model, BaseModel)):
            output_model = None
        return HostedTool(
            name=name or fn.__name__,
            description=description,
            summary=summary,
            annotations=annotations,
            input_model=input_model,
            output_model=output_model,
            run=fn,
            auth=auth,
        )

    return decorate


@dataclass(slots=True)
class Auth:
    """The provider a host publishes when its tools need credentials.

    Every connection of this provider is an instance of the host, set up on Tilde's setup page
    with one of ``methods`` (keyed by method ID). ``verify`` runs at the end of setup: raise to
    refuse the credentials (the message is shown on the setup form) or return an account label.
    """

    id: str
    name: str
    methods: dict[str, Method]
    verify: VerifyFn | None = None
    icon_url: str | None = None
    instructions: str | None = None
    account_name_label: str | None = None

    def tool(
        self,
        *,
        description: str,
        name: str | None = None,
        summary: str = "",
        annotations: Annotations | None = None,
        input_schema: type[BaseModel] | None = None,
        output_schema: type[BaseModel] | None = None,
    ) -> Callable[[Callable[..., Awaitable[Any]]], HostedTool]:
        """Like :func:`tool`, but ``ctx.auth`` holds the calling instance's credentials."""
        return _decorator(
            self, description, name, summary, annotations, input_schema, output_schema
        )

    def provider(self) -> connections_pb2.Provider:
        types = []
        for method_id, method in self.methods.items():
            if method.schema is not None:
                source = {
                    "static": connections_pb2.StaticCredentialSource(
                        schema_json=_json(flat_schema(method.schema))
                    )
                }
            else:
                configuration = method.oauth
                if isinstance(configuration, dict):
                    configuration = connections_pb2.OAuthConfiguration(**configuration)
                source = {
                    "oauth": connections_pb2.OAuthCredentialSource(
                        configuration=configuration,
                        grant=_GRANTS[method.grant],
                        additional_schema_json=_json(flat_schema(method.additional_schema))
                        if method.additional_schema
                        else None,
                    )
                }
            types.append(connections_pb2.ConnectionType(id=method_id, name=method.name, **source))
        return connections_pb2.Provider(
            id=self.id,
            name=self.name,
            icon_url=self.icon_url,
            instructions=self.instructions,
            account_name_label=self.account_name_label,
            connection_types=types,
        )


def flat_schema(model: type[BaseModel]) -> dict[str, Any]:
    """The credential-form subset Tilde accepts: one level of primitive properties."""
    schema = model.model_json_schema()
    defs = schema.get("$defs", {})
    properties = {}
    for key, prop in schema.get("properties", {}).items():
        if "$ref" in prop:  # enums; the definition's title is the class name
            definition = defs[prop["$ref"].rsplit("/", 1)[-1]]
            prop = {**{k: v for k, v in definition.items() if k != "title"}, **prop}
        if "anyOf" in prop:  # Optional[X]
            options = [option for option in prop["anyOf"] if option.get("type") != "null"]
            if len(options) == 1:
                prop = {**options[0], **prop}
        if "default" in prop and prop["default"] is None:
            del prop["default"]
        if prop.get("format") not in (None, "uri", "email"):
            del prop["format"]  # SecretStr's "password"; writeOnly already marks it secret
        if prop.get("type") not in ("string", "boolean", "integer", "number"):
            raise ValueError(f"Credential field {key!r} must be a string, boolean or number")
        prop.setdefault("title", model.model_fields[key].title or key.replace("_", " ").title())
        properties[key] = {k: v for k, v in prop.items() if k in _PROPERTY_KEYS}
    flat: dict[str, Any] = {"type": "object", "properties": properties}
    if schema.get("required"):
        flat["required"] = schema["required"]
    if schema.get("description"):
        flat["description"] = schema["description"]
    flat["additionalProperties"] = False
    return flat


def _json(value: Any) -> str:
    return pydantic_core.to_json(value).decode()


def _describe(error: ValidationError) -> str:
    # Never include input values: they may be credentials.
    return "; ".join(
        f"{'.'.join(str(part) for part in item['loc']) or 'value'}: {item['msg']}"
        for item in error.errors()
    )


class _Tools:
    """What both host kinds share: definitions, the provider and running calls and verifies."""

    def __init__(self, tools: list[HostedTool], auth: Auth | None) -> None:
        self.auth = auth
        self.tools: dict[str, HostedTool] = {}
        for hosted in tools:
            if hosted.auth is not None and hosted.auth is not auth:
                raise ValueError(f"Tool {hosted.name} uses an auth the host was not given")
            if hosted.name in self.tools:
                raise ValueError(f"Duplicate tool name {hosted.name}")
            self.tools[hosted.name] = hosted

    def definitions(self) -> list[chat_pb2.ToolDefinition]:
        return [
            chat_pb2.ToolDefinition(
                name=hosted.name,
                description=hosted.description,
                summary=hosted.summary,
                annotations=chat_pb2.ToolAnnotations(
                    read_only=hosted.annotations.read_only,
                    destructive=hosted.annotations.destructive,
                    idempotent=hosted.annotations.idempotent,
                    open_world=hosted.annotations.open_world,
                )
                if hosted.annotations
                else None,
                input_schema_json=_json(hosted.input_model.model_json_schema()),
                output_schema_json=_json(hosted.output_model.model_json_schema())
                if hosted.output_model
                else "",
            )
            for hosted in self.tools.values()
        ]

    def provider(self) -> connections_pb2.Provider | None:
        return self.auth.provider() if self.auth else None

    def _credentials(self, method_id: str, fields) -> Any:
        assert self.auth is not None
        method = self.auth.methods.get(method_id)
        if method is None:
            raise ValueError(f"Unknown auth method {method_id!r}")
        values = {item.key: item.value for item in fields}
        try:
            return method.credentials.model_validate(values)
        except ValidationError as error:
            raise ValueError(f"Invalid credentials: {_describe(error)}") from None

    async def call(
        self, request: pb.ToolCallRequest, cancellation: Cancellation
    ) -> pb.RespondRequest:
        hosted = self.tools.get(request.name)
        if hosted is None:
            return pb.RespondRequest(call_id=request.call_id, error=f"Unknown tool {request.name}")
        method = request.connection_type if request.HasField("connection_type") else None
        try:
            auth = None
            if hosted.auth is not None:
                if method is None:
                    raise ValueError("This tool is only callable through a connection")
                auth = self._credentials(method, request.credentials)
            try:
                data = hosted.input_model.model_validate_json(request.input_json or "{}")
            except ValidationError as error:
                raise ValueError(f"Invalid input: {_describe(error)}") from None
            context = ToolContext(
                call_id=request.call_id,
                agent_id=request.agent_id,
                thread_id=request.thread_id,
                connection_id=request.connection_id if request.HasField("connection_id") else None,
                auth_method=method,
                auth=auth,
                cancellation=cancellation,
            )
            result = await hosted.run(data, context)
            if hosted.output_model is None:
                return pb.RespondRequest(call_id=request.call_id, output_json=_json(result))
            try:
                output = hosted.output_model.model_validate(result)
            except ValidationError as error:
                raise ValueError(f"Invalid output: {_describe(error)}") from None
            return pb.RespondRequest(call_id=request.call_id, output_json=output.model_dump_json())
        except Exception as error:  # noqa: BLE001 - the message reaches the agent
            log.warning("tool %s failed", request.name, exc_info=error)
            return pb.RespondRequest(
                call_id=request.call_id, error=str(error) or type(error).__name__
            )

    async def verify(self, request: pb.VerifyRequest) -> pb.RespondRequest:
        if self.auth is None:
            return pb.RespondRequest(
                call_id=request.call_id, error="This host takes no credentials"
            )
        try:
            credentials = self._credentials(request.connection_type, request.credentials)
            label = None
            if self.auth.verify is not None:
                label = await self.auth.verify(
                    credentials, Instance(request.connection_id, request.connection_type)
                )
            return pb.RespondRequest(call_id=request.call_id, output_json="{}", account_label=label)
        except Exception as error:  # noqa: BLE001 - shown on the setup form
            return pb.RespondRequest(
                call_id=request.call_id,
                error=str(error) or "The credentials were refused",
            )


class ToolHost:
    """A connected tool host; ``run()`` watches until ``close()``."""

    def __init__(self, tools: _Tools, gateway_url: str, token: str) -> None:
        self._tools = tools
        self._client = ToolHostServiceClient(gateway_url.rstrip("/"), http_client=http_client())
        self._headers = {"authorization": f"Bearer {token}"}
        self._closed = Cancellation()
        self._active: set[asyncio.Task[None]] = set()

    async def run(self) -> None:
        """Publish the tools and answer frames, reconnecting with backoff until ``close()``.

        Raises ``ConnectError`` when Tilde rejects the token.
        """
        watching = asyncio.ensure_future(self._watch())
        self._closed.on_abort(watching.cancel)
        try:
            await watching
        except asyncio.CancelledError:
            if not self._closed.aborted:
                raise
        finally:
            watching.cancel()
            for task in self._active:
                task.cancel()
            await asyncio.gather(watching, *self._active, return_exceptions=True)

    def close(self) -> None:
        self._closed.abort()

    async def _watch(self) -> None:
        request = pb.WatchRequest(tools=self._tools.definitions(), provider=self._tools.provider())
        delay = 0.5
        while not self._closed.aborted:
            try:
                stream = self._client.watch(request, headers=self._headers)
                try:
                    async for frame in stream:
                        delay = 0.5
                        kind = frame.WhichOneof("frame")
                        # Calls run concurrently; one slow tool must not hold up the stream.
                        if kind == "call":
                            self._spawn(self._tools.call(frame.call, self._closed))
                        elif kind == "verify":
                            self._spawn(self._tools.verify(frame.verify))
                finally:
                    await stream.aclose()
            except ConnectError as error:
                if error.code == Code.UNAUTHENTICATED:
                    raise
                # Cancelling a stream surfaces as a CANCELED error, not CancelledError.
                if self._closed.aborted:
                    return
                log.warning("tool host watch failed: %s", error.message)
            except Exception as error:  # noqa: BLE001 - reconnect on any transport failure
                log.warning("tool host watch failed: %s", error)
            await asyncio.sleep(delay)
            delay = min(delay * 2, 15)

    def _spawn(self, outcome: Awaitable[pb.RespondRequest]) -> None:
        async def answer() -> None:
            response = await outcome
            try:
                await self._client.respond(response, headers=self._headers, timeout_ms=10_000)
            except ConnectError as error:
                log.warning("responding to %s failed: %s", response.call_id, error.message)

        task = asyncio.ensure_future(answer())
        self._active.add(task)
        task.add_done_callback(self._active.discard)


def create_tool_host(
    *,
    tools: list[HostedTool],
    auth: Auth | None = None,
    gateway_url: str | None = None,
    token: str | None = None,
) -> ToolHost:
    """A connected tool host; ``gateway_url`` and ``token`` default to ``TILDE_GATEWAY_URL`` and
    ``TILDE_TOOL_HOST_TOKEN``. The host is never dialed, so it needs no public address."""
    gateway_url = gateway_url or os.environ.get("TILDE_GATEWAY_URL")
    token = token or os.environ.get("TILDE_TOOL_HOST_TOKEN")
    if not gateway_url or not token:
        raise ValueError(
            "create_tool_host needs gateway_url and token "
            "(or TILDE_GATEWAY_URL and TILDE_TOOL_HOST_TOKEN)"
        )
    return ToolHost(_Tools(tools, auth), gateway_url, token)


def create_tool_lambda_handler(
    *, tools: list[HostedTool], auth: Auth | None = None
) -> Callable[[dict[str, Any], Any], dict[str, Any]]:
    """An AWS Lambda handler answering Tilde's Protobuf-JSON ``LambdaRequest`` events."""
    hosted = _Tools(tools, auth)

    def handler(event: dict[str, Any], _context: Any = None) -> dict[str, Any]:
        request = ParseDict(event, pb.LambdaRequest(), ignore_unknown_fields=True)
        kind = request.WhichOneof("request")
        if kind == "call":
            answer = asyncio.run(hosted.call(request.call, Cancellation()))
            response = pb.LambdaResponse(output_json=answer.output_json, error=answer.error)
        elif kind == "verify":
            answer = asyncio.run(hosted.verify(request.verify))
            response = pb.LambdaResponse(
                error=answer.error,
                account_label=answer.account_label if answer.HasField("account_label") else None,
            )
        else:
            response = pb.LambdaResponse(tools=hosted.definitions(), provider=hosted.provider())
        return MessageToDict(response)

    return handler
