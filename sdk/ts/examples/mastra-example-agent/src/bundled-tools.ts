import { defineTools } from "@trytilde/sdk";
import { createTool } from "@mastra/core/tools";
import { z } from "zod";

const readOnly = {
  readOnlyHint: true,
  destructiveHint: false,
  idempotentHint: false,
  openWorldHint: false,
};

/**
 * This agent's own tools, written as ordinary Mastra tools. Passed as `bundled` to the
 * per-invocation helper, they are published to Tilde, so `tools.search` and `tools.schemas`
 * describe them beside the agent's remote tools, and each call is recorded with the summary in
 * `mcp._meta.tilde`. `mcp.annotations` become Tilde's annotations.
 * Exported from the entry module, `tilde deploy` declares them, so the agent's Tools tab lists
 * them before it first runs.
 */
const tools = {
  local_time: createTool({
    id: "local_time",
    description:
      "The current date and time where this agent runs, optionally in an IANA time zone such as Europe/London.",
    inputSchema: z.object({ timeZone: z.string().optional().describe("IANA time zone") }),
    outputSchema: z.object({ time: z.string(), timeZone: z.string() }),
    mcp: { annotations: readOnly, _meta: { tilde: { summary: "Checked the time" } } },
    execute: async ({ timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone }) => ({
      time: new Date().toLocaleString("en-GB", { timeZone, dateStyle: "full", timeStyle: "long" }),
      timeZone,
    }),
  }),
  roll_dice: createTool({
    id: "roll_dice",
    description: "Roll dice and add them up, for games or picking at random.",
    inputSchema: z.object({
      count: z.number().int().min(1).max(10).optional().describe("How many dice"),
      sides: z.number().int().min(2).max(100).optional().describe("Sides per die"),
    }),
    outputSchema: z.object({ rolls: z.array(z.number().int()), total: z.number().int() }),
    mcp: { annotations: readOnly, _meta: { tilde: { summary: "Rolled dice" } } },
    execute: async ({ count = 1, sides = 6 }) => {
      const rolls = Array.from({ length: count }, () => 1 + Math.floor(Math.random() * sides));
      return { rolls, total: rolls.reduce((sum, roll) => sum + roll, 0) };
    },
  }),
};

export const bundledTools = defineTools(tools);
