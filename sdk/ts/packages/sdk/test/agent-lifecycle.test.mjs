import assert from "node:assert/strict";
import { test } from "node:test";
import { createHash, createHmac, randomUUID } from "node:crypto";
import { createServer } from "node:http2";
import { once } from "node:events";
import { setTimeout as delay } from "node:timers/promises";
import { create, toBinary } from "@bufbuild/protobuf";
import { createClient, Code } from "@connectrpc/connect";
import { connectNodeAdapter, createConnectTransport } from "@connectrpc/connect-node";
import { createAgentServer, AgentHostService, RuntimeChatService } from "../dist/index.js";
import {
  InvokeRequestSchema,
  StopRequestSchema,
} from "../dist/gen/tilde/agent_host/v1/agent_pb.js";

// Exercise signed HTTP/2 calls and cooperative cancellation without a model/provider service.
test(
  "Stop fences late invokes, leaves resumed work running, and keeps Healthz available",
  { timeout: 15000 },
  async () => {
    const key = "shared-test-key-0123456789abcdef0123456789";
    const sessions = new Set();
    const contexts = [];
    const callback = createServer(
      connectNodeAdapter({
        routes: (router) =>
          router.service(RuntimeChatService, {
            async listTools() {
              return { tools: [] };
            },
          }),
      }),
    );
    const host = createAgentServer({
      signingKey: key,
      tracing: "existing",
      async run(ctx) {
        contexts.push(ctx);
        await new Promise((resolve) =>
          ctx.signal.addEventListener("abort", resolve, { once: true }),
        );
      },
    });
    for (const server of [callback, host]) {
      server.on("session", (session) => {
        sessions.add(session);
        session.on("close", () => sessions.delete(session));
      });
      server.listen(0, "127.0.0.1");
      await once(server, "listening");
    }
    const rpc = createClient(
      AgentHostService,
      createConnectTransport({
        baseUrl: `http://127.0.0.1:${host.address().port}`,
        httpVersion: "2",
      }),
    );
    function signed(schema, value, method) {
      const timestamp = String(Math.floor(Date.now() / 1000));
      const digest = createHash("sha256")
        .update(toBinary(schema, create(schema, value)))
        .digest("hex");
      return {
        headers: {
          "x-tilde-timestamp": timestamp,
          "x-tilde-signature": createHmac("sha256", key)
            .update(`${method}.${timestamp}.${digest}`)
            .digest("hex"),
        },
      };
    }
    const agent = randomUUID();
    const other = randomUUID();
    function invoke(agentId, generation) {
      const request = {
        agentId,
        agentGeneration: generation,
        invocationId: randomUUID(),
        runId: randomUUID(),
        threadId: randomUUID(),
        callbackUrl: `http://127.0.0.1:${callback.address().port}`,
        capability: "test-token",
      };
      return (async () => {
        for await (const _ of rpc.invoke(request, signed(InvokeRequestSchema, request, "Invoke"))) {
          /* drain */
        }
      })();
    }
    async function stop(agentId, throughGeneration) {
      const request = { agentId, throughGeneration };
      return rpc.stop(request, signed(StopRequestSchema, request, "Stop"));
    }
    async function entered(count) {
      for (let i = 0; i < 100 && contexts.length < count; i++) await delay(10);
      assert.equal(contexts.length, count);
    }
    try {
      await assert.rejects(
        rpc.stop({ agentId: agent, throughGeneration: 1n }),
        (e) => e.code === Code.Unauthenticated,
      );
      const first = invoke(agent, 0n);
      const unrelated = invoke(other, 0n);
      await entered(2);
      await stop(agent, 1n);
      await first;
      assert(contexts.find((c) => c.agentId === agent).signal.aborted);
      assert(!contexts.find((c) => c.agentId === other).signal.aborted);
      assert((await rpc.healthz({})).ready);
      await assert.rejects(invoke(agent, 0n), (e) => e.code === Code.FailedPrecondition);
      const resumed = invoke(agent, 2n);
      await entered(3);
      await stop(agent, 1n); // Delayed retry from before resume.
      assert(!contexts[2].signal.aborted);
      await stop(agent, 3n);
      await resumed;
      await stop(other, 1n);
      await unrelated;
      assert((await rpc.healthz({})).ready);
    } finally {
      for (const session of sessions) session.destroy();
      host.close();
      callback.close();
    }
  },
);
