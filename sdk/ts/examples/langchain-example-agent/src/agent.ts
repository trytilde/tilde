import { logger } from "./logger.js";
import type { AgentContext } from "@trytilde/sdk";
import type { BaseChatModel } from "@langchain/core/language_models/chat_models";
import { createAgent } from "langchain";
import { convertToLangChainMessages, convertToLangChainTools } from "@trytilde/sdk-langchain-node";

const instructions =
  "You are the example LangChain agent, a helpful local development assistant. Use the current channel tools to respond to the latest message, concisely and helpfully. Model text is private and is not delivered to the user. Choose the appropriate provider tool using its instructions and conversation references. Use the supplied attachments when answering.";

/** Visible responses are explicit provider tool calls; a text-only model result sends nothing. */
export async function respond(ctx: AgentContext, model: BaseChatModel): Promise<void> {
  logger.info("Agent invocation started");
  let stage: "history" | "inference" = "history";
  try {
    const history = await ctx.message.history();
    logger.debug({ messageCount: history.items.length }, "Loaded conversation history");
    const messages = await convertToLangChainMessages({ messages: history.items, context: ctx });
    stage = "inference";
    const agent = createAgent({
      model,
      tools: convertToLangChainTools(ctx.channel.current),
      systemPrompt: instructions,
    });
    // Each model call and each tool batch is one graph step: at most eight model rounds.
    const result = await agent.invoke(
      { messages },
      { signal: ctx.signal, recursionLimit: 16, runName: "langchain-example-agent" },
    );
    logger.info({ messageCount: result.messages.length }, "Agent invocation completed");
  } catch (error) {
    // Upstream response bodies can contain sensitive details; expose only the HTTP status.
    const statusCode =
      error instanceof Error && "status" in error && typeof error.status === "number"
        ? error.status
        : undefined;
    if (ctx.signal.aborted) logger.warn("Agent invocation cancelled");
    else logger.error({ stage, statusCode }, "Agent invocation failed");
    const status = statusCode ? ` (${statusCode})` : "";
    throw new Error(
      `${stage === "inference" ? "OpenAI inference" : "Conversation history"} failed${status}`,
    );
  }
}
