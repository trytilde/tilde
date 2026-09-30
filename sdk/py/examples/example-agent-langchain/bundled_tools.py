"""This agent's bundled tools: ordinary LangChain tools shipped in its code and run in its process.

Passed as ``bundled`` to the per-invocation helper (or to ``with_tilde_tools``), they are
published, so Tilde's ``tools.search`` and ``tools.schemas`` describe
them beside the agent's remote tools and its Tools tab lists them, and records each call with the
summary from the tool's ``metadata["tilde"]``.
"""

import random
from datetime import datetime
from zoneinfo import ZoneInfo

from langchain_core.tools import tool

from tilde import BundledOptions, ToolAnnotations, define_tools

TOOL_GUIDANCE = (
    " Your other tools (the time, dice, a CRM, unit and date helpers, and tools.search when some "
    "tools are only found by searching) are for working out the answer; still reply through "
    "the channel tools."
)


@tool
def local_time(time_zone: str = "UTC") -> dict[str, str]:
    """The current date and time, optionally in an IANA time zone such as Europe/London."""
    now = datetime.now(ZoneInfo(time_zone))
    return {"time": now.strftime("%A %d %B %Y, %H:%M:%S %Z"), "timeZone": time_zone}


@tool
def roll_dice(count: int = 1, sides: int = 6) -> dict[str, int | list[int]]:
    """Roll dice and add them up, for games or picking at random. Up to 10 dice of 2-100 sides."""
    count = min(max(count, 1), 10)
    sides = min(max(sides, 2), 100)
    rolls = [random.randint(1, sides) for _ in range(count)]
    return {"rolls": rolls, "total": sum(rolls)}


local_time.metadata = {
    "tilde": BundledOptions(summary="Checked the time", annotations=ToolAnnotations(read_only=True))
}
roll_dice.metadata = {"tilde": BundledOptions(summary="Rolled dice")}

BUNDLED_TOOLS = [local_time, roll_dice]

# At module level, so `tilde deploy` declares them with the deployment.
TOOLS = define_tools(BUNDLED_TOOLS)
