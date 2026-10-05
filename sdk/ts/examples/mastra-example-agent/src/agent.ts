import { fileURLToPath } from "node:url";
import { logger } from "./logger.js";
import { bundledTools } from "./bundled-tools.js";
import { inference, type AgentContext } from "@trytilde/sdk";
import { Agent } from "@mastra/core/agent";
import { createOpenAI } from "@ai-sdk/openai";
import { APICallError } from "ai";
import { convertToMastraMessages, tildeMastra, tildeMastraSkills } from "@trytilde/sdk-mastra-node";

// The inference connection this agent was given; the gateway holds its provider key.
const openai = createOpenAI(inference(process.env.TILDE_INFERENCE ?? "default"));

/**
 * A plain Mastra agent at module scope. `tilde deploy` reads its instructions and skills
 * from here; model calls go through the invocation that is running when they are made.
 */
export const agent = new Agent({
  id: "mastra-example-agent",
  name: "Example Agent Mastra",
  instructions:
    "You are Example Agent Mastra, a helpful local development assistant. Use the current channel tools to respond to the latest message, concisely and helpfully. Model text is private and is not delivered to the user. Choose the appropriate provider tool using its instructions and conversation references. Use the supplied attachments when answering. Your other tools (the time, dice, a CRM, unit and date helpers, and tools.search when some tools are only found by searching) are for working out the answer; still reply through the channel tools.",
  model: openai.responses(process.env.OPENAI_MODEL ?? "gpt-5.6-terra"),
  // Absolute, so it does not depend on the directory the process starts in. The wrapper adds
  // the skills assigned to this agent in Tilde at each invocation.
  skills: tildeMastraSkills([fileURLToPath(new URL("../skills", import.meta.url))]),
});

/** Visible responses are explicit provider tool calls; a text-only model result sends nothing. */
export async function respond(ctx: AgentContext): Promise<void> {
  logger.info("Agent invocation started");
  let stage: "history" | "inference" = "history";
  try {
    const history = await ctx.message.history();
    logger.debug({ messageCount: history.items.length }, "Loaded conversation history");
    const messages = await convertToMastraMessages({ messages: history.items, context: ctx });
    stage = "inference";
    const result = await agent.generate(messages, {
      ...(await tildeMastra(ctx, { bundled: bundledTools })),
      abortSignal: AbortSignal.any([ctx.signal, AbortSignal.timeout(60000)]),
      maxSteps: 8,
      modelSettings: { maxOutputTokens: 600, maxRetries: 0 },
      providerOptions: { openai: { store: false } },
    });
    logger.info(
      { stepCount: result.steps.length, finishReason: result.finishReason },
      "Agent invocation completed",
    );
  } catch (error) {
    // Upstream response bodies can contain sensitive details; expose only the HTTP status.
    const statusCode = APICallError.isInstance(error) ? error.statusCode : undefined;
    if (ctx.signal.aborted) logger.warn("Agent invocation cancelled");
    else logger.error({ stage, statusCode }, "Agent invocation failed");
    const status = statusCode ? ` (${statusCode})` : "";
    throw new Error(
      `${stage === "inference" ? "OpenAI inference" : "Conversation history"} failed${status}`,
    );
  }
}
