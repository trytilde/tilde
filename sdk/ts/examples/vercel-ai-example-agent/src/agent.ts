import { logger } from "./logger.js";
import { bundledTools } from "./bundled-tools.js";
import { inference, type AgentContext } from "@trytilde/sdk";
import { createOpenAI } from "@ai-sdk/openai";
import { APICallError, ToolLoopAgent, convertToModelMessages, stepCountIs } from "ai";
import { convertToAiSdkMessages, tildeAiSdk, tildeCallOptions } from "@trytilde/sdk-vercel-ai-node";

// The inference connection this agent was given; the gateway holds its provider key.
const openai = createOpenAI(inference(process.env.TILDE_INFERENCE ?? "default"));

/**
 * A plain AI SDK agent at module scope. `tilde deploy` registers its instructions as the
 * prompt `vercel-ai-example-agent/instructions`; model calls go through the invocation that is
 * running when they are made. `tildeCallOptions` lets each call carry Tilde's channel tools,
 * the agent's Tilde and bundled tools, skills and steering (`tildeAiSdk(ctx, { bundled })`).
 */
export const agent = new ToolLoopAgent({
  id: "vercel-ai-example-agent",
  instructions:
    "You are Example Agent 1, a helpful local development assistant. Use the current channel tools to respond to the latest message, concisely and helpfully. Model text is private and is not delivered to the user. Choose the appropriate provider tool using its instructions and conversation references. Use the supplied attachments when answering. Your other tools (the time, dice, a CRM, unit and date helpers, and tools.search when some tools are only found by searching) are for working out the answer; still reply through the channel tools.",
  model: openai.responses(process.env.OPENAI_MODEL ?? "gpt-5.6-terra"),
  stopWhen: stepCountIs(8),
  maxOutputTokens: 600,
  maxRetries: 0,
  providerOptions: { openai: { store: false } },
  experimental_telemetry: { isEnabled: true, functionId: "vercel-ai-example-agent" },
  ...tildeCallOptions,
});

/** Visible responses are explicit provider tool calls; a text-only model result sends nothing. */
export async function respond(ctx: AgentContext): Promise<void> {
  logger.info("Agent invocation started");
  let stage: "history" | "inference" = "history";
  try {
    const history = await ctx.message.history();
    logger.debug({ messageCount: history.items.length }, "Loaded conversation history");
    const messages = await convertToAiSdkMessages({ messages: history.items, context: ctx });
    stage = "inference";
    const result = await agent.generate({
      messages: await convertToModelMessages(messages),
      ...tildeAiSdk(ctx, { bundled: bundledTools }),
      timeout: 60_000,
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
