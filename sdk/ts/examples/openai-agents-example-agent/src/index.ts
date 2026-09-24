import { connectAgent } from "@trytilde/sdk";
import { setDefaultOpenAIKey, setTracingDisabled } from "@openai/agents";
import { logger, flushLogs } from "./logger.js";
import { respond } from "./agent.js";

const apiKey = process.env.OPENAI_API_KEY;
if (!apiKey) throw new Error("OPENAI_API_KEY is required");

setDefaultOpenAIKey(apiKey);
// The SDK's default trace exporter uploads prompts and tool data to OpenAI; Tilde collects OTel spans itself.
setTracingDisabled(true);
const model = process.env.OPENAI_MODEL ?? "gpt-4o-mini";
// Dials out with TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN; Tilde never calls this process.
const connection = connectAgent({
  logging: "existing",
  run: (ctx) => respond(ctx, model),
  onRegistered: (registration) =>
    logger.info(
      { deploymentId: registration.deploymentId },
      "Example Agent (OpenAI Agents) is ready",
    ),
});
for (const signal of ["SIGINT", "SIGTERM"] as const)
  process.once(signal, () => {
    void connection
      .close()
      .then(flushLogs)
      .catch(() => {});
  });
