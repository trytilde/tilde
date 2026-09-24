import { openai } from "@ai-sdk/openai";
import * as ai from "ai";
import { z } from "zod";
import { Client } from "langsmith";
import { traceable } from "langsmith/traceable";
import { wrapAISDK } from "langsmith/experimental/vercel";
import { randomUUID } from "node:crypto";

export const defaultPrompt =
  "Write a vegetarian lasagna recipe for 4 people using the pantry ingredients. Include quantities and clear cooking steps.";

/** Only the synthetic prompt and model/tool results become LangSmith inputs/outputs. */
export async function runRecipe(
  prompt: string,
  consume: (chunks: AsyncIterable<string>) => Promise<unknown>,
  signal?: AbortSignal,
) {
  const client = new Client();
  const { streamText, tool } = wrapAISDK(ai, { client });
  const runId = randomUUID();
  const run = traceable(
    async (request: string) => {
      const result = streamText({
        model: openai(process.env.OPENAI_MODEL ?? "gpt-4o-mini"),
        prompt: request,
        tools: {
          getPantry: tool({
            description:
              "Look up the demo vegetarian pantry and scale its ingredients for the requested servings.",
            inputSchema: z.object({ servings: z.number().int().min(1).max(12) }),
            execute: async ({ servings }) => ({
              servings,
              ingredients: [
                { name: "lasagna sheets", grams: 75 * servings },
                { name: "tomato passata", grams: 200 * servings },
                { name: "spinach", grams: 100 * servings },
                { name: "ricotta", grams: 100 * servings },
                { name: "mozzarella", grams: 50 * servings },
              ],
            }),
          }),
        },
        // Force one deterministic tool step so the trace UI has something to expand.
        prepareStep: ({ stepNumber }) => ({
          toolChoice: stepNumber === 0 ? { type: "tool", toolName: "getPantry" } : "none",
        }),
        stopWhen: ai.stepCountIs(2),
        maxOutputTokens: 900,
        maxRetries: 1,
        abortSignal: signal,
        experimental_telemetry: { isEnabled: true, functionId: "dummy.recipe-agent" },
      });
      await consume(result.textStream);
      return { text: await result.text, usage: await result.totalUsage };
    },
    {
      name: "dummy.recipe-agent",
      id: runId,
      project_name: process.env.LANGSMITH_PROJECT ?? "test",
      client,
    },
  );
  try {
    const result = await run(prompt);
    await client.awaitPendingTraceBatches();
    const url = await client.getRunUrl({ runId });
    console.log(`\nLangSmith trace: ${url}`);
    return { ...result, runId, url };
  } finally {
    await client.awaitPendingTraceBatches();
  }
}
