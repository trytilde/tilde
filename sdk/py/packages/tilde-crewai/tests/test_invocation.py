"""A module-level CrewAI LLM on `tilde.inference`: every model request reaches the running
invocation's gateway with its token and prompt stamps, and steered input arrives through the
global before_llm_call hook as a user message."""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest
from crewai import LLM, Agent

import tilde
from tilde import SteeringInput
from tilde._invocation import current
from tilde_crewai import inference_interceptor

COMPLETION = {
    "id": "chatcmpl-1",
    "object": "chat.completion",
    "created": 0,
    "model": "gpt-4o-mini",
    "choices": [
        {
            "index": 0,
            "message": {"role": "assistant", "content": "Done."},
            "finish_reason": "stop",
        }
    ],
    "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
}


@pytest.fixture
def gateway():
    requests: list[dict] = []

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            body = self.rfile.read(int(self.headers["content-length"]))
            requests.append({"path": self.path, "headers": dict(self.headers), "body": body})
            payload = json.dumps(COMPLETION).encode()
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


class Invocation:
    """The parts of AgentContext that inference and steering read."""

    def __init__(self, callback_url: str) -> None:
        self.callback_url = callback_url
        self.steering = [SteeringInput(id="in-1", text="Actually, answer in French.")]
        self.system = tilde.define_prompt("system", template="Be brief.")

    def trace_authorization(self) -> str:
        return "Bearer invocation-token"

    def prompt_stamps(self) -> str:
        return self.system.stamp

    def take_inputs(self) -> list[SteeringInput]:
        inputs, self.steering = self.steering, []
        return inputs


INFERENCE = tilde.inference("default")
llm = LLM(
    model="openai/gpt-4o-mini",
    base_url=INFERENCE.base_url,
    api_key=INFERENCE.api_key,
    interceptor=inference_interceptor(INFERENCE),
    max_retries=0,
)


async def test_module_level_llm_serves_the_running_invocation(gateway):
    url, requests = gateway
    agent = Agent(role="Helper", goal="Help", backstory="Helpful", llm=llm, max_iter=2)
    invocation = Invocation(url)
    token = current.set(invocation)  # type: ignore[arg-type]
    try:
        await agent.kickoff_async([{"role": "user", "content": "Hello"}])
    finally:
        current.reset(token)

    [request] = requests
    assert request["path"] == "/gateway/inference/default/chat/completions"
    assert request["headers"]["authorization"] == "Bearer invocation-token"
    assert request["headers"]["x-tilde-prompt"] == invocation.system.stamp
    messages = json.loads(request["body"])["messages"]
    assert messages[-1] == {"role": "user", "content": "Actually, answer in French."}
    assert invocation.steering == []
