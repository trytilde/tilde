"""Tool hosts across Tilde's contracts: Lambda JSON events and a connected host over Watch."""

from __future__ import annotations

import asyncio
import json
from typing import Literal

import pytest
from connectrpc.code import Code
from connectrpc.errors import ConnectError
from pydantic import BaseModel, SecretStr
from support import serve_app

from tilde.tool_host.v1 import tool_host_pb2 as pb
from tilde.tool_host.v1.tool_host_connect import ToolHostServiceASGIApplication
from tilde.tool_hosts import (
    Annotations,
    Auth,
    Instance,
    Method,
    ToolContext,
    create_tool_host,
    create_tool_lambda_handler,
    tool,
)
from tilde.types.v1 import connections_pb2


class ApiKey(BaseModel):
    api_key: SecretStr
    region: Literal["us", "eu"] = "us"


class Workspace(BaseModel):
    workspace: str


async def verify(auth: ApiKey | BaseModel, instance: Instance) -> str | None:
    if isinstance(auth, ApiKey):
        if auth.api_key.get_secret_value() == "bad":
            raise ValueError("Unknown key")
        return f"acme-{auth.region} ({instance.connection_id})"
    return None


auth = Auth(
    id="acme-crm",
    name="Acme CRM",
    methods={
        "api_key": Method(name="API key", schema=ApiKey),
        "oauth": Method(
            name="Acme login",
            oauth={
                "authorization_url": "https://acme.test/authorize",
                "token_url": "https://acme.test/token",
            },
            additional_schema=Workspace,
        ),
    },
    verify=verify,
)


class SearchInput(BaseModel):
    q: str


class SearchOutput(BaseModel):
    results: list[str]


@auth.tool(description="Search the CRM.", annotations=Annotations(read_only=True))
async def search(input: SearchInput, ctx: ToolContext[ApiKey]) -> SearchOutput:
    if ctx.auth_method == "oauth":
        secret = f"{ctx.auth.workspace}:{ctx.auth.access_token}"
    else:
        secret = f"{ctx.auth.region}:{ctx.auth.api_key.get_secret_value()}"
    return SearchOutput(results=[input.q, secret, ctx.connection_id, ctx.call_id, ctx.thread_id])


@auth.tool(description="Returns the wrong shape.", output_schema=SearchOutput)
async def broken(input: SearchInput, ctx: ToolContext[ApiKey]):
    return {"results": "not a list"}


def call(name: str, method: str, credentials: dict[str, str], input: dict) -> dict:
    return {
        "call": {
            "callId": "call-1",
            "name": name,
            "inputJson": json.dumps(input),
            "agentId": "agent-1",
            "threadId": "thread-1",
            "connectionId": "conn-1",
            "connectionType": method,
            "credentials": [{"key": k, "value": v} for k, v in credentials.items()],
        }
    }


def test_lambda_host_publishes_provider_and_runs_credentialed_calls():
    handler = create_tool_lambda_handler(auth=auth, tools=[search, broken])

    listed = handler({"listTools": {}}, None)
    assert [t["name"] for t in listed["tools"]] == ["search", "broken"]
    assert json.loads(listed["tools"][0]["inputSchemaJson"])["required"] == ["q"]
    assert listed["tools"][0]["annotations"] == {"readOnly": True}
    assert "results" in json.loads(listed["tools"][0]["outputSchemaJson"])["properties"]
    provider = listed["provider"]
    assert (provider["id"], provider["name"]) == ("acme-crm", "Acme CRM")
    static, oauth = provider["connectionTypes"]
    assert (static["id"], static["name"]) == ("api_key", "API key")
    assert json.loads(static["static"]["schemaJson"]) == {
        "type": "object",
        "properties": {
            "api_key": {"type": "string", "title": "Api Key", "writeOnly": True},
            "region": {"type": "string", "title": "Region", "enum": ["us", "eu"], "default": "us"},
        },
        "required": ["api_key"],
        "additionalProperties": False,
    }
    assert oauth["oauth"]["grant"] == "O_AUTH_GRANT_AUTHORIZATION_CODE"
    assert oauth["oauth"]["configuration"]["tokenUrl"] == "https://acme.test/token"
    assert json.loads(oauth["oauth"]["additionalSchemaJson"])["required"] == ["workspace"]

    refused = {
        "verify": {
            "callId": "v-1",
            "connectionId": "conn-1",
            "connectionType": "api_key",
            "credentials": [{"key": "api_key", "value": "bad"}],
        }
    }
    assert handler(refused, None) == {"error": "Unknown key"}
    accepted = {
        "verify": {
            "callId": "v-2",
            "connectionId": "conn-1",
            "connectionType": "api_key",
            "credentials": [{"key": "api_key", "value": "k"}, {"key": "region", "value": "eu"}],
        }
    }
    assert handler(accepted, None) == {"accountLabel": "acme-eu (conn-1)"}
    missing = {"verify": {"callId": "v-3", "connectionId": "conn-1", "connectionType": "api_key"}}
    assert handler(missing, None)["error"].startswith("Invalid credentials: api_key")

    out = handler(call("search", "api_key", {"api_key": "k1"}, {"q": "deals"}), None)
    assert json.loads(out["outputJson"]) == {
        "results": ["deals", "us:k1", "conn-1", "call-1", "thread-1"]
    }
    out = handler(
        call("search", "oauth", {"access_token": "tok", "workspace": "w1"}, {"q": "x"}), None
    )
    assert json.loads(out["outputJson"])["results"][1] == "w1:tok"

    assert handler(call("broken", "api_key", {"api_key": "k"}, {"q": "x"}), None) == {
        "error": "Invalid output: results: Input should be a valid list"
    }
    assert handler(call("search", "api_key", {"api_key": "k"}, {}), None)["error"].startswith(
        "Invalid input: q"
    )
    no_instance = call("search", "api_key", {}, {"q": "x"})
    del no_instance["call"]["connectionType"]
    assert "only callable through a connection" in handler(no_instance, None)["error"]


class FakeToolHostService:
    def __init__(self) -> None:
        self.watches: list[pb.WatchRequest] = []
        self.frames: asyncio.Queue[pb.WatchResponse | None] = asyncio.Queue()
        self.responses: asyncio.Queue[pb.RespondRequest] = asyncio.Queue()

    async def watch(self, request, ctx):
        if ctx.request_headers.get("authorization") != "Bearer tool-host-token":
            raise ConnectError(Code.UNAUTHENTICATED, "bad token")
        self.watches.append(request)
        yield pb.WatchResponse(registered=pb.HostRegistered(tool_host_id="th-1"))
        while (frame := await self.frames.get()) is not None:
            yield frame

    async def respond(self, request, ctx):
        self.responses.put_nowait(request)
        return pb.RespondResponse()


async def test_connected_host_answers_verify_and_concurrent_calls(served):
    fake = FakeToolHostService()
    upstream = await serve_app(ToolHostServiceASGIApplication(fake))
    served.append(upstream)
    released = asyncio.Event()

    @tool(description="Waits for release.")
    async def slow(input: SearchInput, ctx: ToolContext[None]) -> SearchOutput:
        await released.wait()
        return SearchOutput(results=[input.q, str(ctx.auth)])

    @tool(description="Releases slow.", name="release")
    async def fast(input: SearchInput, ctx: ToolContext[None]) -> SearchOutput:
        released.set()
        return SearchOutput(results=[input.q])

    host = create_tool_host(
        gateway_url=upstream.url, token="tool-host-token", auth=auth, tools=[search, slow, fast]
    )
    running = asyncio.create_task(host.run())
    try:
        credentials = [connections_pb2.InputField(key="api_key", value="k")]
        fake.frames.put_nowait(
            pb.WatchResponse(
                verify=pb.VerifyRequest(
                    call_id="v-1",
                    connection_id="conn-1",
                    connection_type="api_key",
                    credentials=credentials,
                )
            )
        )
        # `slow` only finishes once `release` ran, so the calls must not be serialized.
        for call_id, name in (("c-1", "slow"), ("c-2", "release"), ("c-3", "search")):
            fake.frames.put_nowait(
                pb.WatchResponse(
                    call=pb.ToolCallRequest(
                        call_id=call_id,
                        name=name,
                        input_json='{"q": "hi"}',
                        agent_id="agent-1",
                        thread_id="thread-1",
                        connection_id="conn-1",
                        connection_type="api_key",
                        credentials=credentials,
                    )
                )
            )
        responses = {}
        for _ in range(4):
            response = await asyncio.wait_for(fake.responses.get(), 10)
            responses[response.call_id] = response

        published = fake.watches[0]
        assert [t.name for t in published.tools] == ["search", "slow", "release"]
        assert published.provider.id == "acme-crm"
        assert [t.id for t in published.provider.connection_types] == ["api_key", "oauth"]
        assert responses["v-1"].output_json == "{}"
        assert responses["v-1"].account_label == "acme-us (conn-1)"
        assert json.loads(responses["c-1"].output_json) == {"results": ["hi", "None"]}
        assert json.loads(responses["c-2"].output_json) == {"results": ["hi"]}
        assert json.loads(responses["c-3"].output_json)["results"][:2] == ["hi", "us:k"]
    finally:
        host.close()
        await asyncio.wait_for(running, 10)
        fake.frames.put_nowait(None)

    rejected = create_tool_host(gateway_url=upstream.url, token="wrong", tools=[fast])
    with pytest.raises(ConnectError) as error:
        await asyncio.wait_for(rejected.run(), 10)
    assert error.value.code == Code.UNAUTHENTICATED
