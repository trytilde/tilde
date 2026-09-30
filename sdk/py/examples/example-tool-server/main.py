"""Example remote tool server (Python): date and unit helpers that need no credentials.

Its tools are available as soon as they are enabled; pydantic models type each tool's input and
output, and Tilde validates calls against the schemas they produce.
"""

import asyncio
import logging
from datetime import date, timedelta
from typing import Literal

from pydantic import BaseModel, Field

from tilde.tool_hosts import Annotations, ToolContext, create_tool_host, tool

log = logging.getLogger("example-tool-server")

Unit = Literal["km", "mi", "kg", "lb", "c", "f"]
# Each unit's partner and how to convert into it.
CONVERSIONS = {
    "km": ("mi", lambda v: v / 1.609344),
    "mi": ("km", lambda v: v * 1.609344),
    "kg": ("lb", lambda v: v / 0.45359237),
    "lb": ("kg", lambda v: v * 0.45359237),
    "c": ("f", lambda v: v * 9 / 5 + 32),
    "f": ("c", lambda v: (v - 32) * 5 / 9),
}


class ConvertInput(BaseModel):
    value: float
    unit: Unit = Field(description="The unit of value; it is converted to its counterpart")


class Converted(BaseModel):
    value: float
    unit: Unit


class BusinessDaysInput(BaseModel):
    start: date
    end: date


class BusinessDays(BaseModel):
    business_days: int = Field(description="Weekdays from start up to, not including, end")


@tool(
    description="Convert kilometres, miles, kilograms, pounds or degrees Celsius/Fahrenheit.",
    summary="Converted a unit",
    annotations=Annotations(read_only=True, idempotent=True),
)
async def convert_units(input: ConvertInput, ctx: ToolContext) -> Converted:
    unit, convert = CONVERSIONS[input.unit]
    return Converted(value=round(convert(input.value), 4), unit=unit)


@tool(
    description="Count the weekdays between two dates.",
    summary="Counted business days",
    annotations=Annotations(read_only=True, idempotent=True),
)
async def business_days_between(input: BusinessDaysInput, ctx: ToolContext) -> BusinessDays:
    if input.end < input.start:
        raise ValueError("end must not be before start")
    days = (input.end - input.start).days
    weekdays = sum(1 for n in range(days) if (input.start + timedelta(n)).weekday() < 5)
    return BusinessDays(business_days=weekdays)


def main() -> None:
    logging.basicConfig(level=logging.INFO)
    log.info("Example tool server (Python) is dialing in to Tilde")
    asyncio.run(create_tool_host(tools=[convert_units, business_days_between]).run())


if __name__ == "__main__":
    main()
