import { connectAgent } from "@trytilde/sdk";
import { ChatOpenAI } from "@langchain/openai";
import { logger, flushLogs } from "./logger.js";
import { respond } from "./agent.js";

const apiKey = process.env.OPENAI_API_KEY;
if (!apiKey) throw new Error("OPENAI_API_KEY is required");

const model = new ChatOpenAI({
  apiKey,
  model: process.env.OPENAI_MODEL ?? "gpt-4o-mini",
  maxTokens: 600,
  maxRetries: 0,
  timeout: 60000,
  modelKwargs: { store: false },
});
// Dials out with TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN; Tilde never calls this process.
const connection = connectAgent({
  logging: "existing",
  run: (ctx) => respond(ctx, model),
  onRegistered: (registration) =>
    logger.info({ deploymentId: registration.deploymentId }, "Example LangChain agent is ready"),
});
for (const signal of ["SIGINT", "SIGTERM"] as const)
  process.once(signal, () => {
    void connection
      .close()
      .then(flushLogs)
      .catch(() => {});
  });
