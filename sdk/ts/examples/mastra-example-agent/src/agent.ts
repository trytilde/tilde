import { logger } from "./logger.js";
import type { AgentContext } from "@trytilde/sdk";
import { Agent } from "@mastra/core/agent";
import type { MastraModelConfig } from "@mastra/core/llm";
import { APICallError } from "ai";
import { convertToMastraMessages, convertToMastraTools } from "@trytilde/sdk-mastra-node";

const instructions =
  "You are Example Agent Mastra, a helpful local development assistant. Use the current channel tools to respond to the latest message, concisely and helpfully. Model text is private and is not delivered to the user. Choose the appropriate provider tool using its instructions and conversation references. Use the supplied attachments when answering.";

/** Visible responses are explicit provider tool calls; a text-only model result sends nothing. */
export async function respond(ctx: AgentContext, model: MastraModelConfig): Promise<void> {
  logger.info("Agent invocation started");
  let stage: "history" | "inference" = "history";
  try {
    const history = await ctx.message.history();
    logger.debug({ messageCount: history.items.length }, "Loaded conversation history");
    const messages = await convertToMastraMessages({ messages: history.items, context: ctx });
    stage = "inference";
    // Tools are invocation-scoped, so the Mastra agent is built per invocation.
    const agent = new Agent({
      id: "mastra-example-agent",
      name: "Example Agent Mastra",
      instructions,
      model,
      tools: convertToMastraTools(ctx.channel.current),
    });
    const result = await agent.generate(messages, {
      maxSteps: 8,
      abortSignal: AbortSignal.any([ctx.signal, AbortSignal.timeout(60000)]),
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
