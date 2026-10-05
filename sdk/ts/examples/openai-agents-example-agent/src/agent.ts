import { logger } from "./logger.js";
import { bundledTools } from "./bundled-tools.js";
import { inference, type AgentContext } from "@trytilde/sdk";
import { Agent, OpenAIResponsesModel, run, setTracingDisabled } from "@openai/agents";
import OpenAI from "openai";
import { convertToOpenAIAgentsMessages, tildeOpenAIAgents } from "@trytilde/sdk-openai-agents-node";

// The SDK's default trace exporter uploads prompts and tool data to OpenAI; Tilde collects OTel spans itself.
setTracingDisabled(true);
// The inference connection this agent was given; the gateway holds its provider key.
const openai = new OpenAI({
  ...inference(process.env.TILDE_INFERENCE ?? "default"),
  maxRetries: 0,
});

/**
 * A plain OpenAI Agents SDK agent at module scope. `tilde deploy` registers its instructions as
 * the prompt `openai-agents-example-agent/instructions`; model calls go through the invocation
 * that is running when they are made.
 */
export const agent = new Agent({
  name: "openai-agents-example-agent",
  instructions:
    "You are Example Agent (OpenAI Agents), a helpful local development assistant. Use the current channel tools to respond to the latest message, concisely and helpfully. Model text is private and is not delivered to the user. Choose the appropriate provider tool using its instructions and conversation references. Use the supplied attachments when answering. Your other tools (the time, dice, a CRM, unit and date helpers, and tools.search when some tools are only found by searching) are for working out the answer; still reply through the channel tools.",
  model: new OpenAIResponsesModel(openai, process.env.OPENAI_MODEL ?? "gpt-5.6-terra"),
  modelSettings: { store: false },
});

/** Visible responses are explicit provider tool calls; a text-only model result sends nothing. */
export async function respond(ctx: AgentContext): Promise<void> {
  logger.info("Agent invocation started");
  let stage: "history" | "inference" = "history";
  try {
    const history = await ctx.message.history();
    logger.debug({ messageCount: history.items.length }, "Loaded conversation history");
    const items = await convertToOpenAIAgentsMessages({ messages: history.items, context: ctx });
    stage = "inference";
    // A copy of the agent with the channel tools and skills, plus steering and cancellation.
    const tilde = await tildeOpenAIAgents(ctx, agent, { bundled: bundledTools });
    const result = await run(tilde.agent, items, {
      ...tilde.options,
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
