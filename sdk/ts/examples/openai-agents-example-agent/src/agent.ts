import { logger } from "./logger.js";
import type { AgentContext } from "@trytilde/sdk";
import { Agent, run } from "@openai/agents";
import {
  convertToOpenAIAgentsMessages,
  convertToOpenAIAgentsTools,
} from "@trytilde/sdk-openai-agents-node";

const instructions =
  "You are Example Agent (OpenAI Agents), a helpful local development assistant. Use the current channel tools to respond to the latest message, concisely and helpfully. Model text is private and is not delivered to the user. Choose the appropriate provider tool using its instructions and conversation references. Use the supplied attachments when answering.";

/** Visible responses are explicit provider tool calls; a text-only model result sends nothing. */
export async function respond(ctx: AgentContext, model: string): Promise<void> {
  logger.info("Agent invocation started");
  let stage: "history" | "inference" = "history";
  try {
    const history = await ctx.message.history();
    logger.debug({ messageCount: history.items.length }, "Loaded conversation history");
    const items = await convertToOpenAIAgentsMessages({ messages: history.items, context: ctx });
    stage = "inference";
    const agent = new Agent({
      name: "openai-agents-example-agent",
      instructions,
      model,
      tools: convertToOpenAIAgentsTools(ctx.channel.current),
      modelSettings: { maxTokens: 600, store: false },
    });
    const result = await run(agent, items, {
      signal: AbortSignal.any([ctx.signal, AbortSignal.timeout(60000)]),
      maxTurns: 8,
    });
    logger.info(
      { itemCount: result.newItems.length, responseId: result.lastResponseId },
      "Agent invocation completed",
    );
  } catch (error) {
    // Upstream response bodies can contain sensitive details; expose only the HTTP status.
    const upstream = (error as { status?: unknown } | null)?.status;
    const statusCode = typeof upstream === "number" ? upstream : undefined;
    if (ctx.signal.aborted) logger.warn("Agent invocation cancelled");
    else logger.error({ stage, statusCode }, "Agent invocation failed");
    const status = statusCode ? ` (${statusCode})` : "";
    throw new Error(
      `${stage === "inference" ? "OpenAI inference" : "Conversation history"} failed${status}`,
    );
  }
}
