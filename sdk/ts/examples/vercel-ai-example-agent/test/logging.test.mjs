import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import { once } from "node:events";
import { initializeTracing, invocationTracing } from "../../../packages/sdk/dist/tracing.js";
import {
  invocationLogging,
  configureDeploymentLogging,
  agentLogProcessor,
} from "../../../packages/sdk/dist/logging.js";
import { logger } from "../dist/logger.js";
import { respond } from "../dist/agent.js";

test("the example's Pino bridge sends correlated OTel logs through the generic SDK adapter", async (t) => {
  const batches = [];
  const server = createServer(async (req, res) => {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    batches.push({ path: req.url, token: req.headers.authorization, body: Buffer.concat(chunks) });
    res.writeHead(200, { "content-type": "application/x-protobuf" });
    res.end();
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => server.close());
  initializeTracing(false);
  const request = {
    callbackUrl: `http://127.0.0.1:${server.address().port}`,
    agentId: "example-agent",
    invocationId: "pino-invocation",
    threadId: "pino-session",
    runId: "pino-run",
  };
  const tracing = invocationTracing(
    request,
    new Headers({ traceparent: `00-${"c".repeat(32)}-${"d".repeat(16)}-00` }),
    () => "Bearer pino-token",
  );
  configureDeploymentLogging({
    gatewayUrl: request.callbackUrl,
    deploymentToken: "pino-deployment",
  });
  t.after(() => agentLogProcessor.shutdown());
  logger.info("Pino startup log");
  const active = invocationLogging(request, tracing.context, () => "Bearer pino-token");
  active.run(() => {
    logger.info({ operation: "pino-bridge-test" }, "Pino invocation log");
    logger.error({ statusCode: 503 }, "Pino failure log");
  });
  await active.run(() =>
    assert.rejects(
      respond(
        {
          message: {
            history: async () => {
              throw new Error("sensitive response sentinel");
            },
          },
          signal: new AbortController().signal,
        },
        null,
      ),
      /Conversation history failed/,
    ),
  );
  await active.end();
  await agentLogProcessor.forceFlush();
  assert.equal(batches.length, 2);
  const batch = batches.find((batch) => batch.token === "Bearer pino-token");
  const startup = batches.find((batch) => batch.token === "Bearer pino-deployment");
  assert.ok(startup.body.includes("Pino startup log"));
  assert.ok(!startup.body.includes("pino-invocation"));
  assert.ok(!batch.body.includes("Pino startup log"));
  assert.ok(batch.body.includes("Agent invocation started"));
  assert.ok(batch.body.includes("Agent invocation failed"));
  assert.ok(!batch.body.includes("sensitive response sentinel"));
  assert.equal(batch.path, "/v1/logs");
  assert.equal(batch.token, "Bearer pino-token");
  for (const value of [
    "Pino invocation log",
    "Pino failure log",
    "pino-bridge-test",
    "vercel-ai-example-agent",
    "pino-run",
    "pino-session",
    "pino-invocation",
  ])
    assert.ok(batch.body.includes(value), value);
  assert.ok(batch.body.includes(Buffer.from("c".repeat(32), "hex")));
});
