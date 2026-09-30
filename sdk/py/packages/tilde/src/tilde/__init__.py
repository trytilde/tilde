"""Tilde Python SDK: agent hosts, invocation context, channel tools and management clients.

Generated contracts live beside these modules under the proto package names, for example
``tilde.runtime.v1.chat_pb2`` and ``tilde.types.v1.chat_pb2``. Run ``sdk/py/scripts/generate.py``
after changing proto/.
"""

from tilde._cancel import Cancellation, InvocationCancelled, StopLoop
from tilde._inference import Inference, inference
from tilde._invocation import current_invocation
from tilde._tools import (
    BundledOptions,
    BundledTools,
    Tool,
    ToolAnnotations,
    ToolCatalog,
    define_tools,
    tool_id,
)
from tilde.channels import Channels, ChannelTool, ChannelTools
from tilde.clients import (
    ManagementClient,
    RuntimeClient,
    create_management_client,
    create_runtime_client,
    create_tilde_chat_client,
)
from tilde.context import AgentContext, DownloadedAttachment, SteeringInput
from tilde.host import (
    ConnectedAgent,
    HostState,
    InvocationOptions,
    Wake,
    connect_agent,
    create_lambda_handler,
    run_connected_agent,
    run_invocation,
)
from tilde.logging import DeploymentLogging, agent_log_processor, configure_deployment_logging
from tilde.messages import (
    ContextMessage,
    ConversationMessage,
    GoalMessage,
    MessageClient,
    MessageHistory,
    ObjectiveMessage,
    TaskMessage,
)
from tilde.prompts import PromptDefinition, define_prompt
from tilde.skills import (
    SkillDefinition,
    SkillFile,
    SkillFileBody,
    SkillsClient,
    SkillsDefinition,
    define_skill,
    define_skills,
)
from tilde.tracing import agent_span_processor

__all__ = [
    "Inference",
    "AgentContext",
    "BundledOptions",
    "BundledTools",
    "Cancellation",
    "ChannelTool",
    "ChannelTools",
    "Channels",
    "ConnectedAgent",
    "ContextMessage",
    "ConversationMessage",
    "DeploymentLogging",
    "DownloadedAttachment",
    "GoalMessage",
    "HostState",
    "InvocationCancelled",
    "InvocationOptions",
    "ManagementClient",
    "MessageClient",
    "MessageHistory",
    "ObjectiveMessage",
    "PromptDefinition",
    "RuntimeClient",
    "SkillDefinition",
    "SkillFile",
    "SkillFileBody",
    "SkillsClient",
    "SkillsDefinition",
    "SteeringInput",
    "StopLoop",
    "TaskMessage",
    "Tool",
    "ToolAnnotations",
    "ToolCatalog",
    "Wake",
    "agent_log_processor",
    "agent_span_processor",
    "configure_deployment_logging",
    "connect_agent",
    "create_lambda_handler",
    "create_management_client",
    "create_runtime_client",
    "create_tilde_chat_client",
    "current_invocation",
    "define_prompt",
    "define_skill",
    "define_skills",
    "define_tools",
    "inference",
    "run_connected_agent",
    "run_invocation",
    "tool_id",
]
