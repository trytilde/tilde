"""``tilde deploy`` discovery for LangChain (entry point ``tilde.discover``).

- A module-level ``create_agent`` graph: its static ``system_prompt`` becomes the plain prompt
  ``<agent>/system_prompt`` (``<agent>`` is the ``name=`` given to ``create_agent``, else the
  variable name), and every ``@dynamic_prompt`` middleware in it a dynamic prompt.
- A module-level ``@dynamic_prompt`` middleware: ``<function name>/system_prompt``, dynamic,
  with the function's source.
- A module-level ``PromptTemplate`` / ``ChatPromptTemplate``: ``<variable>`` (chat templates:
  ``<variable>/<index>.<role>`` per message), f-string templates in braces format, mustache in
  mustache format. Jinja2 templates and LangSmith hub prompts are reported, not registered.

Tools are declared as ``with_tilde_tools`` publishes them: those of a ``create_agent`` graph's
``ToolNode`` (``tools=`` and middleware tools) and a ``define_tools`` value of LangChain tools.

``create_agent`` keeps the system prompt and ``wrap_model_call`` middleware only in the model
node's closures (langchain 1.4); they are read from there and reported when unreadable.
"""

from __future__ import annotations

import functools
import inspect
import sys
import types
from collections.abc import Callable
from pathlib import Path
from typing import Any

from langchain.agents.middleware import AgentMiddleware
from langchain_core.messages import BaseMessage
from langchain_core.prompts import (
    ChatPromptTemplate,
    MessagesPlaceholder,
    PromptTemplate,
)
from langchain_core.tools import BaseTool
from langgraph.graph.state import CompiledStateGraph
from langgraph.prebuilt import ToolNode

from tilde import BundledTools
from tilde.discovery import (
    PROMPT_FORMAT_BRACES,
    PROMPT_FORMAT_DYNAMIC,
    PROMPT_FORMAT_MUSTACHE,
    PROMPT_FORMAT_PLAIN,
    Discovered,
    DiscoveryContext,
    declared_prompt,
    declared_tool,
)
from tilde.prompts import canonical_json, prompt_hash, valid_prompt_name
from tilde_langchain.tools import describe_tool

_FORMATS = {"f-string": PROMPT_FORMAT_BRACES, "mustache": PROMPT_FORMAT_MUSTACHE}
_DEFAULT_GRAPH_NAME = "LangGraph"


def is_create_agent(value: Any) -> bool:
    return isinstance(value, CompiledStateGraph) and (
        (value.config or {}).get("metadata", {}).get("ls_integration") == "langchain_create_agent"
    )


def reachable_middleware(root: Any) -> list[AgentMiddleware[Any, Any]]:
    """Middleware instances reachable from ``root`` through bound methods, wrappers, partials
    and closures: from a ``create_agent`` model node, or from a ``wrap_model_call`` handler."""
    found: list[AgentMiddleware[Any, Any]] = []
    seen: set[int] = set()
    pending = [root]
    while pending:
        value = pending.pop()
        if id(value) in seen:
            continue
        seen.add(id(value))
        if isinstance(value, AgentMiddleware):
            found.append(value)
        elif inspect.ismethod(value):
            pending += [value.__self__, value.__func__]
        elif isinstance(value, functools.partial):
            pending += [value.func, *value.args]
        elif isinstance(value, types.FunctionType):
            for cell in value.__closure__ or ():
                try:
                    pending.append(cell.cell_contents)
                except ValueError:  # an empty cell
                    continue
        elif isinstance(value, list | tuple):
            pending += list(value)
        wrapped = getattr(value, "__wrapped__", None)
        if wrapped is not None:
            pending.append(wrapped)
    return found


def dynamic_prompt_function(middleware: AgentMiddleware[Any, Any]) -> Callable[..., Any] | None:
    """The function behind a ``@dynamic_prompt`` middleware (the class ``dynamic_prompt``
    builds keeps it in its hook's closure)."""
    for hook in ("wrap_model_call", "awrap_model_call"):
        method = type(middleware).__dict__.get(hook)
        if not isinstance(method, types.FunctionType):
            continue
        if not method.__qualname__.startswith("dynamic_prompt."):
            return None
        for name, cell in zip(method.__code__.co_freevars, method.__closure__ or (), strict=True):
            if name == "func":
                return cell.cell_contents
    return None


def dynamic_prompt(middleware: AgentMiddleware[Any, Any]) -> tuple[str, str] | None:
    """``(name, source)`` of a ``@dynamic_prompt`` middleware, or None for other middleware."""
    func = dynamic_prompt_function(middleware)
    if func is None:
        return None
    try:
        return f"{middleware.name}/system_prompt", inspect.getsource(func)
    except (OSError, TypeError):
        return None


def dynamic_stamp(middleware: AgentMiddleware[Any, Any]) -> tuple[str, str] | None:
    prompt = dynamic_prompt(middleware)
    if prompt is None:
        return None
    return prompt[0], prompt_hash(prompt[1], {}, canonical_json({}))


def discover(value: Any, context: DiscoveryContext) -> Discovered | None:
    module = sys.modules.get(context.module)
    file = getattr(module, "__file__", None)
    origin = f"{context.relative(Path(file)) if file else context.module}#{context.name}"
    if isinstance(value, BundledTools):
        if not value.tools or not all(isinstance(tool, BaseTool) for tool in value.tools):
            return None
        return Discovered(
            tools=[
                declared_tool(describe_tool(tool, value.options), f"{origin}.tools.{tool.name}")
                for tool in value.tools
            ]
        )
    if is_create_agent(value):
        return _agent(value, origin, context)
    if isinstance(value, AgentMiddleware):
        found = Discovered()
        _dynamic(value, origin, found, context)
        return found
    if isinstance(value, PromptTemplate | ChatPromptTemplate):
        if (value.metadata or {}).get("lc_hub_repo"):
            context.warn(f"{origin}: LangSmith hub prompt is versioned by the hub, not by Tilde")
            return None
        found = Discovered()
        if isinstance(value, PromptTemplate):
            _template(context.name, value, origin, found, context)
        elif isinstance(value, ChatPromptTemplate):
            _chat(value, origin, found, context)
        return found
    return None


def _agent(graph: CompiledStateGraph, origin: str, context: DiscoveryContext) -> Discovered:
    found = Discovered()
    name = graph.name if graph.name != _DEFAULT_GRAPH_NAME else context.name
    model = graph.nodes.get("model")
    bound = getattr(model, "bound", None)
    func = getattr(bound, "func", None)
    try:
        nonlocals = inspect.getclosurevars(func).nonlocals
        system = nonlocals["system_message"]
    except (KeyError, TypeError):
        context.warn(f"{origin}: create_agent's system prompt could not be read")
        system = None
    if isinstance(system, BaseMessage):
        if isinstance(system.content, str) and valid_prompt_name(f"{name}/system_prompt"):
            found.prompts.append(
                declared_prompt(
                    f"{name}/system_prompt",
                    system.content,
                    PROMPT_FORMAT_PLAIN,
                    f"{origin}.system_prompt",
                )
            )
        else:
            context.warn(f"{origin}: system prompt {name!r} is not registered")
    # Hook nodes (`<middleware>.before_model`, ...) and the model node's composed wrappers.
    roots = [
        getattr(getattr(node, "bound", None), attribute, None)
        for node in graph.nodes.values()
        for attribute in ("func", "afunc")
    ]
    for middleware in reachable_middleware(roots):
        _dynamic(middleware, f"{origin}.middleware.{middleware.name}", found, context)
    node = getattr(graph.nodes.get("tools"), "bound", None)
    if isinstance(node, ToolNode):
        found.tools += [
            declared_tool(describe_tool(tool), f"{origin}.tools.{name}")
            for name, tool in node.tools_by_name.items()
        ]
    return found


def _dynamic(
    middleware: AgentMiddleware[Any, Any], origin: str, found: Discovered, context: DiscoveryContext
) -> None:
    if dynamic_prompt_function(middleware) is None:
        return
    prompt = dynamic_prompt(middleware)
    if prompt is None or not valid_prompt_name(prompt[0]):
        context.warn(f"{origin}: dynamic prompt {middleware.name!r} could not be read")
        return
    found.prompts.append(declared_prompt(prompt[0], prompt[1], PROMPT_FORMAT_DYNAMIC, origin))


def _template(
    name: str, template: PromptTemplate, origin: str, found: Discovered, context: DiscoveryContext
) -> None:
    format = _FORMATS.get(template.template_format)
    if format is None:
        context.warn(f"{origin}: {template.template_format} templates are not registered")
        return
    if "{" not in template.template:
        format = PROMPT_FORMAT_PLAIN
    if not valid_prompt_name(name):
        context.warn(f"{origin}: {name!r} cannot name a prompt")
        return
    found.prompts.append(declared_prompt(name, template.template, format, origin))


def _chat(
    chat: ChatPromptTemplate, origin: str, found: Discovered, context: DiscoveryContext
) -> None:
    for index, message in enumerate(chat.messages):
        if isinstance(message, MessagesPlaceholder):
            continue
        if isinstance(message, BaseMessage):
            if isinstance(message.content, str):
                name = f"{context.name}/{index}.{message.type}"
                found.prompts.append(
                    declared_prompt(
                        name, message.content, PROMPT_FORMAT_PLAIN, f"{origin}.messages[{index}]"
                    )
                )
            continue
        prompt = getattr(message, "prompt", None)
        if isinstance(prompt, PromptTemplate):
            # ChatMessagePromptTemplate has a role; the others are named for theirs.
            role = (
                getattr(message, "role", None)
                or type(message).__name__.removesuffix("MessagePromptTemplate").lower()
            )
            name = f"{context.name}/{index}.{role}"
            _template(name, prompt, f"{origin}.messages[{index}]", found, context)
        else:
            context.warn(f"{origin}.messages[{index}]: only text message templates are registered")
