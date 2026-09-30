"""A module-level Agent on `tilde.inference`, run with `tilde_openai_agents(ctx, agent)` inside
an invocation: requests reach the invocation's gateway stamped with the dynamic instructions
`tilde deploy` registers, carry the channel and skill tools and the skills summary, steered
input stays in place on later calls, and the model's channel tool call reaches Tilde."""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import pytest
from agents import Agent, OpenAIChatCompletionsModel, Runner, set_tracing_disabled
from openai import AsyncOpenAI

import tilde
from tilde import SteeringInput, Tool
from tilde._invocation import current
from tilde.discovery import DiscoveryContext
from tilde_openai_agents import discover, tilde_openai_agents


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
    set_tracing_disabled(True)
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
    """The parts of AgentContext the helper and `tilde.inference` read."""

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


def instructions(context, agent) -> str:
    return "Be brief."


INFERENCE = tilde.inference("default")
client = AsyncOpenAI(
    base_url=INFERENCE.base_url,
    api_key=INFERENCE.api_key,
    http_client=INFERENCE.async_client(),
    max_retries=0,
)
agent = Agent(
    name="helper",
    instructions=instructions,
    model=OpenAIChatCompletionsModel(model="gpt-4o-mini", openai_client=client),
)


async def test_module_level_agent_serves_the_running_invocation(gateway):
    url, requests = gateway
    invocation = Invocation(url)
    token = current.set(invocation)  # type: ignore[arg-type]
    try:
        run_agent, run_config = await tilde_openai_agents(invocation, agent)  # type: ignore[arg-type]
        await Runner.run(run_agent, "Hello", run_config=run_config, max_turns=4)
    finally:
        current.reset(token)

    [declared] = discover(agent, DiscoveryContext(Path.cwd(), __name__, "agent")).prompts
    first, second = requests
    assert first["path"] == "/gateway/inference/default/chat/completions"
    assert first["headers"]["authorization"] == "Bearer invocation-token"
    assert first["headers"]["x-tilde-prompt"] == f"helper/instructions@{declared.hash}"
    body = json.loads(first["body"])
    assert [t["function"]["name"] for t in body["tools"]] == ["sendMessage", "read_skill"]
    assert [(m["role"], m["content"]) for m in body["messages"]] == [
        ("system", "Be brief.\n\nYou have these skills.\n- citations: Cite sources."),
        ("user", "Hello"),
        ("user", "Answer in French."),
    ]
    assert invocation.channel.sent == [({"text": "Bonjour"}, "call_1")]
    # The SDK resends its own history; the steered message is put back where it arrived.
    later = [m["role"] for m in json.loads(second["body"])["messages"]]
    assert later == ["system", "user", "user", "assistant", "tool"]
    assert json.loads(second["body"])["messages"][2]["content"] == "Answer in French."
    # The module-level agent is untouched.
    assert agent.tools == [] and agent.instructions is instructions
