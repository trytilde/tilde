import {
  convertToAiSdkMessages,
  type ConvertToAiSdkMessagesOptions,
} from "@trytilde/sdk-vercel-ai-node";
import type { UIMessage } from "ai";

export type ConvertToMastraMessagesOptions = ConvertToAiSdkMessagesOptions;

/**
 * Mastra is built on the Vercel AI SDK: `agent.generate()`/`agent.stream()` accept AI SDK v6
 * UI messages directly (its MessageList normalizes them), so no `convertToModelMessages` step
 * is needed. Conversion, caching and attachment hydration are owned by the Vercel adapter.
 */
export function convertToMastraMessages(
  options: ConvertToMastraMessagesOptions,
): Promise<UIMessage[]> {
  return convertToAiSdkMessages(options);
}
