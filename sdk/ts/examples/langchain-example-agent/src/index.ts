import { connectAgent } from "@trytilde/sdk";
import { logger, flushLogs } from "./logger.js";
import { respond } from "./agent.js";

// `tilde deploy` discovers the agent's system prompt and bundled tools from the entry's exports.
export { agent } from "./agent.js";
export { bundledTools } from "./bundled-tools.js";

// Dials out with TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN; Tilde never calls this process.
// Under `tilde deploy` (TILDE_DISCOVERY=1) this starts nothing.
const connection = connectAgent({
  logging: "existing",
  run: respond,
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
