import { logger } from "./logger.js";
import { bundledTools } from "./bundled-tools.js";
import { inference, type AgentContext } from "@trytilde/sdk";
import { ChatOpenAI } from "@langchain/openai";
import { createAgent } from "langchain";
import {
  convertToLangChainMessages,
  tildeLangChain,
  tildeMiddleware,
} from "@trytilde/sdk-langchain-node";

// The inference connection this agent was given; the gateway holds its provider key.
const { baseURL, apiKey, fetch } = inference(process.env.TILDE_INFERENCE ?? "default");

/**
 * A plain LangChain agent at module scope. `tilde deploy` registers its system prompt as the
 * prompt `langchain-example-agent/system_prompt`; model calls go through the invocation that is
 * running when they are made. `tildeMiddleware({ bundled })` brings each invocation's channel tools,
 * the agent's Tilde and bundled tools, steering and skills.
 */
export const agent = createAgent({
  name: "langchain-example-agent",
  model: new ChatOpenAI({
    apiKey,
    configuration: { baseURL, fetch },
    model: process.env.OPENAI_MODEL ?? "gpt-4o-mini",
    maxTokens: 600,
    maxRetries: 0,
    timeout: 60000,
    modelKwargs: { store: false },
  }),
  systemPrompt:
    "You are the example LangChain agent, a helpful local development assistant. Use the current channel tools to respond to the latest message, concisely and helpfully. Model text is private and is not delivered to the user. Choose the appropriate provider tool using its instructions and conversation references. Use the supplied attachments when answering. Your other tools (the time, dice, a CRM, unit and date helpers, and tools.search when some tools are only found by searching) are for working out the answer; still reply through the channel tools.",
  middleware: [tildeMiddleware({ bundled: bundledTools })],
});

/** Visible responses are explicit provider tool calls; a text-only model result sends nothing. */
export async function respond(ctx: AgentContext): Promise<void> {
  logger.info("Agent invocation started");
  let stage: "history" | "inference" = "history";
  try {
    const history = await ctx.message.history();
    logger.debug({ messageCount: history.items.length }, "Loaded conversation history");
    const messages = await convertToLangChainMessages({ messages: history.items, context: ctx });
    stage = "inference";
    // Each model call and each tool batch is one graph step: at most eight model rounds.
    const result = await agent.invoke(
      { messages },
      { ...tildeLangChain(ctx), recursionLimit: 16, runName: "langchain-example-agent" },
    );
    logger.info({ messageCount: result.messages.length }, "Agent invocation completed");
  } catch (error) {
    // Upstream response bodies can contain sensitive details; expose only the HTTP status.
    // Model errors raised under middleware arrive as a MiddlewareError with the provider's as `cause`.
    const status = (value: unknown) => (value as { status?: unknown } | null)?.status;
    const upstream = status(error) ?? status((error as { cause?: unknown } | null)?.cause);
    const statusCode = typeof upstream === "number" ? upstream : undefined;
    if (ctx.signal.aborted) logger.warn("Agent invocation cancelled");
    else logger.error({ stage, statusCode }, "Agent invocation failed");
    const suffix = statusCode ? ` (${statusCode})` : "";
    throw new Error(
      `${stage === "inference" ? "OpenAI inference" : "Conversation history"} failed${suffix}`,
    );
  }
}
