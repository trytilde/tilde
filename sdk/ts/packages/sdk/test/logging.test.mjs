import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import { once } from "node:events";
import { logs, SeverityNumber } from "@opentelemetry/api-logs";
import { LoggerProvider } from "@opentelemetry/sdk-logs";
import { resourceFromAttributes } from "@opentelemetry/resources";
import { initializeTracing, invocationTracing } from "../dist/tracing.js";
import {
  initializeLogging,
  invocationLogging,
  agentLogProcessor,
  configureDeploymentLogging,
} from "../dist/logging.js";

async function receiver(t, status = 200) {
  const received = [];
  const server = createServer(async (req, res) => {
    const parts = [];
    for await (const chunk of req) parts.push(chunk);
    received.push({ token: req.headers.authorization, path: req.url, body: Buffer.concat(parts) });
    res.writeHead(status, { "content-type": "application/x-protobuf" });
    res.end();
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => server.close());
  return { received, url: `http://127.0.0.1:${server.address().port}` };
}
function invocation(url, name, token) {
  const request = {
    callbackUrl: url + "/runtime/?ignored=yes",
    agentId: `agent-${name}`,
    invocationId: `invocation-${name}`,
    runId: `run-${name}`,
    threadId: `session-${name}`,
  };
  const trace = invocationTracing(
    request,
    new Headers({ traceparent: `00-${"a".repeat(32)}-${"b".repeat(16)}-00` }),
    token,
  );
  return invocationLogging(request, trace.context, token);
}
test("ordinary OTel logs isolate concurrent invocations, preserve unsampled correlation and renew export credentials", async (t) => {
  const { received, url } = await receiver(t);
  initializeTracing(false);
  initializeLogging(false);
  const logger = logs.getLogger("application");
  let tokenA = "Bearer expired-a";
  const a = invocation(url, "a", () => tokenA),
    b = invocation(url, "b", () => "Bearer b");
  logger.emit({ body: "unscoped-startup" });
  await Promise.all([
    a.run(async () => {
      await Promise.resolve();
      logger.emit({
        body: "only-a",
        severityNumber: SeverityNumber.INFO,
        attributes: { "tilde.run.id": "spoofed", "application.key": "custom-a" },
      });
    }),
    b.run(async () => {
      await Promise.resolve();
      logger.emit({
        body: "only-b",
        severityNumber: SeverityNumber.ERROR,
        attributes: { "application.key": "custom-b" },
      });
    }),
  ]);
  tokenA = "Bearer renewed-a";
  await Promise.all([a.end(), a.end(), b.end()]);
  assert.equal(received.length, 2);
  for (const batch of received) {
    assert.equal(batch.path, "/runtime/v1/logs");
    assert.ok(batch.body.includes(Buffer.from("a".repeat(32), "hex")));
    assert.ok(batch.body.includes(Buffer.from("b".repeat(16), "hex")));
    for (const key of ["tilde.agent.id", "tilde.thread.id", "tilde.run.id", "tilde.invocation.id"])
      assert.ok(batch.body.includes(key));
    assert.ok(!batch.body.includes("spoofed"));
    assert.ok(!batch.body.includes("unscoped-startup"));
    const own = batch.token === "Bearer renewed-a" ? "a" : "b",
      other = own === "a" ? "b" : "a";
    assert.ok(["Bearer renewed-a", "Bearer b"].includes(batch.token));
    for (const prefix of ["only", "agent", "invocation", "run", "session", "custom"]) {
      assert.ok(batch.body.includes(`${prefix}-${own}`));
      assert.ok(!batch.body.includes(`${prefix}-${other}`));
    }
  }
  a.run(() => logger.emit({ body: "late-after-end" }));
  await agentLogProcessor.forceFlush();
  assert.equal(received.length, 2);
});
test("existing LoggerProviders preserve their resource and custom attributes", async (t) => {
  const { received, url } = await receiver(t);
  const provider = new LoggerProvider({
    resource: resourceFromAttributes({ "service.name": "custom-service" }),
    processors: [agentLogProcessor],
  });
  initializeLogging(true);
  const active = invocation(url, "existing", () => "Bearer custom");
  active.run(() =>
    provider
      .getLogger("own-provider")
      .emit({ body: "own-logger", attributes: { purpose: "existing-provider-test" } }),
  );
  await active.end();
  await provider.shutdown();
  assert.equal(received.length, 1);
  assert.ok(received[0].body.includes("custom-service"));
  assert.ok(received[0].body.includes("existing-provider-test"));
});
test("export failure is bounded and does not reject invocation completion", async (t) => {
  const { received, url } = await receiver(t, 400);
  const active = invocation(url, "failure", () => "Bearer failed");
  active.run(() => logs.getLogger("application").emit({ body: "failed-export" }));
  await assert.doesNotReject(active.end());
  assert.equal(received.length, 1);
});

test("deployment logs use their own queue and never receive late invocation logs", async (t) => {
  const { received, url } = await receiver(t);
  configureDeploymentLogging({
    gatewayUrl: url + "/runtime",
    deploymentToken: "deployment-credential",
  });
  t.after(() => agentLogProcessor.shutdown());
  assert.throws(
    () => configureDeploymentLogging({ gatewayUrl: url, deploymentToken: "another-agent" }),
    /one deployment/,
  );
  const logger = logs.getLogger("application");
  logger.emit({ body: "startup-before-invocation" });
  const active = invocation(url, "scoped", () => "Bearer invocation-credential");
  active.run(() => logger.emit({ body: "inside-invocation" }));
  await active.end();
  active.run(() => logger.emit({ body: "late-must-not-fallback" }));
  logger.emit({ body: "background-after-invocation" });
  await agentLogProcessor.forceFlush();
  assert.equal(received.length, 2);
  const deployment = received.find((batch) => batch.token === "Bearer deployment-credential");
  const execution = received.find((batch) => batch.token === "Bearer invocation-credential");
  assert.equal(deployment.path, "/runtime/v1/logs");
  assert.ok(deployment.body.includes("startup-before-invocation"));
  assert.ok(deployment.body.includes("background-after-invocation"));
  assert.ok(!deployment.body.includes("inside-invocation"));
  assert.ok(!deployment.body.includes("tilde.run.id"));
  assert.ok(execution.body.includes("inside-invocation"));
  for (const batch of received) assert.ok(!batch.body.includes("late-must-not-fallback"));
});
