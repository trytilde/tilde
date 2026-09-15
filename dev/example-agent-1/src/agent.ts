import type { AgentContext } from "@trytilde/sdk";
import {
  APICallError,
  generateText,
  convertToModelMessages,
  stepCountIs,
  type LanguageModel,
} from "ai";
import { convertToAiSdkMessages, convertToAiSdkTools } from "@trytilde/sdk-vercel-ai-node";

const instructions =
  "You are Example Agent 1, a helpful local development assistant. Use the current channel tools to respond to the latest message, concisely and helpfully. Model text is private and is not delivered to the user. Choose the appropriate provider tool using its instructions and conversation references. Use the supplied attachments when answering.";

/** Visible responses are explicit provider tool calls; a text-only model result sends nothing. */
export async function respond(ctx: AgentContext, model: LanguageModel): Promise<void> {
  const history = await ctx.message.history();
  const messages = await convertToAiSdkMessages({ messages: history.items, context: ctx });

  try {
    await generateText({
      model,
      tools: convertToAiSdkTools(ctx.channel.current),
      stopWhen: stepCountIs(8),
      experimental_telemetry: { isEnabled: true, functionId: "example-agent-1" },
      system: instructions,
      messages: await convertToModelMessages(messages),
      maxOutputTokens: 600,
      maxRetries: 0,
      abortSignal: AbortSignal.any([ctx.signal, AbortSignal.timeout(60000)]),
      providerOptions: { openai: { store: false } },
    });
  } catch (error) {
    // Upstream response bodies can contain sensitive details; expose only the HTTP status.
    const status = APICallError.isInstance(error) ? ` (${error.statusCode})` : "";
    throw new Error(`OpenAI inference failed${status}`);
  }
}
