"""Tilde adapter for CrewAI: typed context to CrewAI messages, channel tools to CrewAI tools,
the inference gateway through a transport interceptor, steering through a global
``before_llm_call`` hook (registered on import) and ``tilde deploy`` discovery of
``@CrewBase`` YAML prompts."""

from tilde_crewai.attachments import (
    AttachmentConversion,
    AttachmentHandler,
    AttachmentPart,
    AttachmentResult,
    convert_attachment,
    normalize_media_type,
)
from tilde_crewai.discover import discover
from tilde_crewai.inference import InferenceInterceptor, inference_interceptor
from tilde_crewai.messages import MessageHandlers, convert_to_crewai_messages
from tilde_crewai.steering import inject_steering
from tilde_crewai.tools import convert_to_crewai_tools, with_tilde_tools

__all__ = [
    "AttachmentConversion",
    "AttachmentHandler",
    "AttachmentPart",
    "AttachmentResult",
    "InferenceInterceptor",
    "MessageHandlers",
    "convert_attachment",
    "convert_to_crewai_messages",
    "convert_to_crewai_tools",
    "discover",
    "inference_interceptor",
    "inject_steering",
    "normalize_media_type",
    "with_tilde_tools",
]
