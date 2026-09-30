"""A module-level create_agent graph with `tilde_middleware()` and a model on `tilde.inference`,
run inside an invocation: requests reach the invocation's gateway stamped with the dynamic
prompt `tilde deploy` registers, carry the channel and skill tools, the skills summary and
steered input (kept for later calls), and the model's channel tool call reaches Tilde."""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import pytest
from langchain.agents import create_agent
from langchain.agents.middleware import dynamic_prompt
from langchain_openai import ChatOpenAI

import tilde
from tilde import SteeringInput, Tool
from tilde._invocation import current
from tilde.discovery import DiscoveryContext
from tilde_langchain import discover, tilde_middleware


def completion(message: dict) -> dict:
    return {
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "created": 0,
        "model": "gpt-4o-mini",
        "choices": [{"index": 0, "message": message, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
    }


SEND = {
    "role": "assistant",
    "content": None,
    "tool_calls": [
        {
            "id": "call_1",
            "type": "function",
            "function": {"name": "sendMessage", "arguments": '{"text": "Bonjour"}'},
        }
    ],
}


@pytest.fixture
def gateway():
    requests: list[dict] = []
    replies = [completion(SEND), completion({"role": "assistant", "content": "Sent."})]

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            body = self.rfile.read(int(self.headers["content-length"]))
            requests.append({"path": self.path, "headers": dict(self.headers), "body": body})
            payload = json.dumps(replies.pop(0)).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, *_args):
            return None

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{server.server_port}/gateway", requests
    server.shutdown()


def tool(description: str, execute) -> Tool:
    schema = {"type": "object", "properties": {"text": {"type": "string"}}}
    return Tool(description=description, input_schema=schema, _execute=execute)


class Skills:
    def tools(self) -> dict[str, Tool]:
        async def read(_input, _call):
            return "Always cite."

        return {"read_skill": tool("Read a skill file.", read)}

    async def summary(self) -> str:
        return "You have these skills.\n- citations: Cite sources."


class Channel:
    def __init__(self) -> None:
        self.sent: list[tuple[dict, str]] = []

        async def send(input, call):
            self.sent.append((input, call))
            return {"ok": True}

        self.current = {"sendMessage": tool("Send a message to the chat.", send)}


class Invocation:
    """The parts of AgentContext the middleware and `tilde.inference` read."""

    def __init__(self, callback_url: str) -> None:
        self.callback_url = callback_url
        self.steering = [SteeringInput(id="in-1", text="Answer in French.")]
        self.stamps: dict[str, str] = {}
        self.channel = Channel()
        self.agent_tools: dict[str, object] = {}
        self.skills = Skills()

    def trace_authorization(self) -> str:
        return "Bearer invocation-token"

    def activate_prompt(self, name: str, hash: str) -> None:
        self.stamps[name] = f"{name}@{hash}"

    def prompt_stamps(self) -> str:
        return ", ".join(self.stamps.values())

    def take_inputs(self) -> list[SteeringInput]:
        inputs, self.steering = self.steering, []
        return inputs

    def check(self) -> None:
        return None

    async def _set_bundled_tools(self, tools: dict[str, object]) -> None:
        return None


@dynamic_prompt
def personalized(request) -> str:
    return f"You help {request.runtime.context or 'everyone'}."


INFERENCE = tilde.inference("default")
agent = create_agent(
    ChatOpenAI(
        model="gpt-4o-mini",
        base_url=INFERENCE.base_url,
        api_key=INFERENCE.api_key,
        http_async_client=INFERENCE.async_client(),
        max_retries=0,
    ),
    system_prompt="Static prompt.",
    middleware=[tilde_middleware(), personalized],
)


async def test_module_level_agent_serves_the_running_invocation(gateway):
    url, requests = gateway
    invocation = Invocation(url)
    token = current.set(invocation)  # type: ignore[arg-type]
    try:
        await agent.ainvoke({"messages": [{"role": "user", "content": "Hello"}]})
    finally:
        current.reset(token)

    [declared] = discover(personalized, DiscoveryContext(Path.cwd(), __name__, "p")).prompts
    first, second = requests
    assert first["path"] == "/gateway/inference/default/chat/completions"
    assert first["headers"]["authorization"] == "Bearer invocation-token"
    assert first["headers"]["x-tilde-prompt"] == f"personalized/system_prompt@{declared.hash}"
    body = json.loads(first["body"])
    assert [t["function"]["name"] for t in body["tools"]] == ["sendMessage", "read_skill"]
    assert [(m["role"], m["content"]) for m in body["messages"]] == [
        ("system", "You help everyone."),
        ("system", "You have these skills.\n- citations: Cite sources."),
        ("user", "Hello"),
        ("user", "Answer in French."),
    ]
    assert invocation.channel.sent == [({"text": "Bonjour"}, "call_1")]
    # The steered message is part of the graph state, so the next call still has it.
    later = [(m["role"], m.get("content")) for m in json.loads(second["body"])["messages"]]
    assert later[3] == ("user", "Answer in French.")
    assert later[5][0] == "tool"
