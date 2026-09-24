import { createRequire } from "node:module";
import { logs } from "@opentelemetry/api-logs";
import { LoggerProvider } from "@opentelemetry/sdk-logs";
import { resourceFromAttributes } from "@opentelemetry/resources";
import { registerInstrumentations } from "@opentelemetry/instrumentation";
import { PinoInstrumentation } from "@opentelemetry/instrumentation-pino";
import { agentLogProcessor, configureDeploymentLogging } from "@trytilde/sdk";

// Configure before importing Pino or emitting startup logs. Invocation routing takes precedence.
if (process.env.TILDE_GATEWAY_URL && process.env.TILDE_DEPLOYMENT_TOKEN) {
  configureDeploymentLogging({
    gatewayUrl: process.env.TILDE_GATEWAY_URL,
    deploymentToken: process.env.TILDE_DEPLOYMENT_TOKEN,
  });
}

const provider = new LoggerProvider({
  resource: resourceFromAttributes({ "service.name": "openai-agents-example-agent" }),
  processors: [agentLogProcessor],
});
logs.setGlobalLoggerProvider(provider);
// Pino's OTel bridge runs in this thread, retaining the active invocation context.
registerInstrumentations({
  loggerProvider: provider,
  instrumentations: [new PinoInstrumentation()],
});
// Load Pino after its instrumentation is registered. CJS loading avoids an ESM loader hook.
const pino = createRequire(import.meta.url)("pino") as typeof import("pino");
export const logger = pino({ name: "openai-agents-example-agent" });
export const flushLogs = () => provider.forceFlush();
