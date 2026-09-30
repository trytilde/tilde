"""Route CrewAI's native OpenAI client through Tilde's inference gateway.

CrewAI builds its own OpenAI clients, so the invocation token is stamped by a transport
interceptor rather than a supplied ``httpx`` client. With ``tilde.inference(alias)`` the LLM is
built once at module scope and every request resolves the invocation running it (token,
callback URL and prompt stamps)::

    INFERENCE = tilde.inference("default")
    llm = LLM(model="openai/gpt-4o-mini", base_url=INFERENCE.base_url,
              api_key=INFERENCE.api_key, interceptor=inference_interceptor(INFERENCE))
"""

from __future__ import annotations

import httpx
from crewai.llms.hooks.base import BaseInterceptor

from tilde import Inference


class InferenceInterceptor(BaseInterceptor[httpx.Request, httpx.Response]):
    def __init__(self, inference: Inference) -> None:
        self._inference = inference

    def on_outbound(self, message: httpx.Request) -> httpx.Request:
        return self._inference.prepare(message)

    def on_inbound(self, message: httpx.Response) -> httpx.Response:
        return message

    async def aon_outbound(self, message: httpx.Request) -> httpx.Request:
        return self._inference.prepare(message)

    async def aon_inbound(self, message: httpx.Response) -> httpx.Response:
        return message


def inference_interceptor(inference: Inference) -> InferenceInterceptor:
    return InferenceInterceptor(inference)
