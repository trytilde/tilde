import { tool } from "@openai/agents";
import { defineTools, type BundledToolOptions } from "@trytilde/sdk";
import { z } from "zod";

/**
 * This agent's own tools, written as ordinary OpenAI Agents function tools. Passed as `bundled` to the
 * per-invocation helper, they are published to Tilde, so `tools.search` and `tools.schemas`
 * describe them beside the agent's remote tools, and each call is recorded with its summary. Function tools carry
 * no metadata, so Tilde's summaries and annotations are the options.
 * Exported from the entry module, `tilde deploy` declares them, so the agent's Tools tab lists
 * them before it first runs.
 */
const tools = [
  tool({
    name: "local_time",
    description:
      "The current date and time where this agent runs, optionally in an IANA time zone such as Europe/London.",
    // Strict mode requires every field; null means "where this agent runs".
    parameters: z.object({ timeZone: z.string().nullable().describe("IANA time zone") }),
    execute: async ({ timeZone }) => {
      const zone = timeZone ?? Intl.DateTimeFormat().resolvedOptions().timeZone;
      const time = new Date().toLocaleString("en-GB", {
        timeZone: zone,
        dateStyle: "full",
        timeStyle: "long",
      });
      return { time, timeZone: zone };
    },
  }),
  tool({
    name: "roll_dice",
    description: "Roll dice and add them up, for games or picking at random.",
    parameters: z.object({
      count: z.number().int().min(1).max(10).nullable().describe("How many dice"),
      sides: z.number().int().min(2).max(100).nullable().describe("Sides per die"),
    }),
    execute: async ({ count, sides }) => {
      const rolls = Array.from(
        { length: count ?? 1 },
        () => 1 + Math.floor(Math.random() * (sides ?? 6)),
      );
      return { rolls, total: rolls.reduce((sum, roll) => sum + roll, 0) };
    },
  }),
];

const readOnly = { readOnly: true, destructive: false, idempotent: false, openWorld: false };
const options: BundledToolOptions = {
  local_time: { summary: "Checked the time", annotations: readOnly },
  roll_dice: { summary: "Rolled dice", annotations: readOnly },
};

export const bundledTools = defineTools(tools, options);
