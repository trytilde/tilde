import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import { once } from "node:events";
import { trace, context } from "@opentelemetry/api";
import {
  initializeTracing,
  invocationTracing,
  tracingInterceptor,
  agentSpanProcessor,
} from "../dist/tracing.js";

test("concurrent invocations preserve context, rotate credentials, and export streaming RPCs only when ended", async (t) => {
  const received = [];
  const server = createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    received.push({ token: request.headers.authorization, body: Buffer.concat(chunks) });
    assert.equal(request.url, "/v1/traces");
    response.writeHead(200, { "content-type": "application/x-protobuf" });
    response.end();
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => server.close());
  initializeTracing(false);
  const callbackUrl = `http://127.0.0.1:${server.address().port}`;
  const traceId = "12345678901234567890123456789012";
  const headers = new Headers({ traceparent: `00-${traceId}-1234567890123456-01` });
  let tokenA = "Bearer old-a";
  const a = invocationTracing(
    {
      callbackUrl,
      invocationId: "invocation-a",
      runId: "run-a",
      threadId: "thread",
      agentId: "agent",
    },
    headers,
    () => tokenA,
  );
  const b = invocationTracing(
    {
      callbackUrl,
      invocationId: "invocation-b",
      runId: "run-b",
      threadId: "thread",
      agentId: "agent",
    },
    headers,
    () => "Bearer b",
  );
  await Promise.all([
    a.run(async () => {
      await Promise.resolve();
      assert.equal(trace.getSpan(context.active()).spanContext().traceId, traceId);
      trace.getTracer("framework").startSpan("only-a").end();
    }),
    b.run(async () => {
      await Promise.resolve();
      trace.getTracer("framework").startSpan("only-b").end();
    }),
  ]);
  tokenA = "Bearer renewed-a";
  const response = await a.run(() =>
    tracingInterceptor(async (request) => {
      assert.ok(request.header.get("traceparent").startsWith(`00-${traceId}-`));
      return {
        stream: true,
        message: (async function* () {
          yield "first";
          yield "second";
        })(),
      };
    })({
      service: { typeName: "StreamingService" },
      method: { name: "InvokeTool" },
      header: new Headers(),
      signal: new AbortController().signal,
    }),
  );
  await agentSpanProcessor.forceFlush();
  assert.ok(received.every(({ body }) => !body.includes("StreamingService/InvokeTool")));
  assert.deepEqual(await Array.fromAsync(response.message), ["first", "second"]);
  await Promise.all([a.end(false), b.end(false)]);
  const aBatches = received.filter((r) => r.token === "Bearer renewed-a");
  const bBatches = received.filter((r) => r.token === "Bearer b");
  assert.ok(aBatches.some(({ body }) => body.includes("only-a")));
  assert.ok(aBatches.some(({ body }) => body.includes("StreamingService/InvokeTool")));
  assert.ok(bBatches.some(({ body }) => body.includes("only-b")));
  assert.ok(
    aBatches.every(({ body }) => !body.includes("only-b") && !body.includes("invocation-b")),
  );
  assert.ok(
    bBatches.every(({ body }) => !body.includes("only-a") && !body.includes("invocation-a")),
  );
  assert.ok(received.every((r) => r.token !== "Bearer old-a"));
});

test("trace exports keep the runtime callback path prefix", async (t) => {
  const received = [];
  const server = createServer(async (request, response) => {
    for await (const _ of request) {
      /* Drain the OTLP upload before acknowledging it. */
    }
    received.push({ path: request.url, authorization: request.headers.authorization });
    response.writeHead(200, { "content-type": "application/x-protobuf" });
    response.end();
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => server.close());
  initializeTracing(false);
  const origin = `http://127.0.0.1:${server.address().port}`;
  for (const suffix of ["/runtime", "/runtime/"]) {
    const invocation = invocationTracing(
      {
        callbackUrl: origin + suffix,
        invocationId: "prefixed-invocation",
        agentId: "agent",
        runId: "run",
        threadId: "thread",
      },
      new Headers({ traceparent: "00-12345678901234567890123456789012-1234567890123456-01" }),
      () => "Bearer runtime-token",
    );
    await invocation.end(false);
  }
  assert.deepEqual(received, [
    { path: "/runtime/v1/traces", authorization: "Bearer runtime-token" },
    { path: "/runtime/v1/traces", authorization: "Bearer runtime-token" },
  ]);
});
