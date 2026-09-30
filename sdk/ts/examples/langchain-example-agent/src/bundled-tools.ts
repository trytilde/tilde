import { defineTools } from "@trytilde/sdk";
import { tool } from "@langchain/core/tools";
import { z } from "zod";

const readOnly = { readOnly: true, destructive: false, idempotent: false, openWorld: false };

/**
 * This agent's own tools, written as ordinary LangChain tools. Passed as `bundled` to the
 * per-invocation helper, they are published to Tilde, so `tools.search` and `tools.schemas`
 * describe them beside the agent's remote tools, and each call is recorded with the summary in `metadata.tilde`.
 * Exported from the entry module, `tilde deploy` declares them, so the agent's Tools tab lists
 * them before it first runs.
 */
const tools = [
  tool(
    async ({ timeZone }) => {
      const zone = timeZone ?? Intl.DateTimeFormat().resolvedOptions().timeZone;
      const time = new Date().toLocaleString("en-GB", {
        timeZone: zone,
        dateStyle: "full",
        timeStyle: "long",
      });
      return { time, timeZone: zone };
    },
    {
      name: "local_time",
      description:
        "The current date and time where this agent runs, optionally in an IANA time zone such as Europe/London.",
      schema: z.object({ timeZone: z.string().optional().describe("IANA time zone") }),
      metadata: { tilde: { summary: "Checked the time", annotations: readOnly } },
    },
  ),
  tool(
    async ({ count = 1, sides = 6 }) => {
      const rolls = Array.from({ length: count }, () => 1 + Math.floor(Math.random() * sides));
      return { rolls, total: rolls.reduce((sum, roll) => sum + roll, 0) };
    },
    {
      name: "roll_dice",
      description: "Roll dice and add them up, for games or picking at random.",
      schema: z.object({
        count: z.number().int().min(1).max(10).optional().describe("How many dice"),
        sides: z.number().int().min(2).max(100).optional().describe("Sides per die"),
      }),
      metadata: { tilde: { summary: "Rolled dice", annotations: readOnly } },
    },
  ),
];

export const bundledTools = defineTools(tools);
