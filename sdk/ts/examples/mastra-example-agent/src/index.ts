import { connectAgent } from "@trytilde/sdk";
import { createOpenAI } from "@ai-sdk/openai";
import { logger, flushLogs } from "./logger.js";
import { respond } from "./agent.js";

const apiKey = process.env.OPENAI_API_KEY;
if (!apiKey) throw new Error("OPENAI_API_KEY is required");

const openai = createOpenAI({ apiKey });
const model = openai.responses(process.env.OPENAI_MODEL ?? "gpt-4o-mini");
// Dials out with TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN; Tilde never calls this process.
const connection = connectAgent({
  logging: "existing",
  run: (ctx) => respond(ctx, model),
  onRegistered: (registration) =>
    logger.info({ deploymentId: registration.deploymentId }, "Example Agent Mastra is ready"),
});
for (const signal of ["SIGINT", "SIGTERM"] as const)
  process.once(signal, () => {
    void connection
      .close()
      .then(flushLogs)
      .catch(() => {});
  });
