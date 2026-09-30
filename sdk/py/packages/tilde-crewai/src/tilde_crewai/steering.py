"""Steering for CrewAI: a global ``before_llm_call`` hook, registered when ``tilde_crewai`` is
imported, appends the running invocation's newly steered inputs as user messages before each
model call. Outside an invocation it does nothing, so crews run unchanged elsewhere.
"""

from __future__ import annotations

from crewai.hooks import LLMCallHookContext, register_before_llm_call_hook

from tilde import current_invocation


def inject_steering(context: LLMCallHookContext) -> None:
    ctx = current_invocation()
    if ctx is None:
        return None
    # Mutate in place: the executor keeps its own reference to this list.
    for steered in ctx.take_inputs():
        context.messages.append({"role": "user", "content": steered.text})
    return None


register_before_llm_call_hook(inject_steering)
