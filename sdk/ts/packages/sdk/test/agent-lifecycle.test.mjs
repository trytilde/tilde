import test from "node:test";
import assert from "node:assert/strict";
import { createServer as httpServer } from "node:http";
import { createServer } from "node:http2";
import { EventEmitter, once } from "node:events";
import { randomUUID, createHash, createHmac } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { create, toBinary } from "@bufbuild/protobuf";
import { createClient } from "@connectrpc/connect";
import { connectNodeAdapter, createConnectTransport } from "@connectrpc/connect-node";
import { createAgentHandler, AgentHostService, RuntimeChatService } from "../dist/index.js";
import { InvokeRequestSchema } from "../dist/gen/tilde/agent_host/v1/agent_pb.js";
import {
  InvocationControlService,
  InvocationCommandKind as Kind,
} from "../dist/gen/tilde/runtime/v1/controls_pb.js";

void test(
  "separate stateless hosts consume invocation controls, replay steering, and checkpoint suspension",
  { timeout: 15000 },
  async (t) => {
    const key = "shared-test-key-0123456789abcdef0123456789";
    const events = new EventEmitter();
    const pending = new Map();
    const acknowledged = new Set();
    const contexts = new Map();
    const checkpoints = [];
    const failCheckpoint = new Set();
    const hostRequests = [];
    const sessions = new Set();
    const callback = createServer(
      connectNodeAdapter({
        routes(router) {
          router.service(RuntimeChatService, {
            async listTools() {
              return { tools: [] };
            },
          });
          router.service(InvocationControlService, {
            async *watchCommands(_, ctx) {
              const invocation = ctx.requestHeader.get("authorization").slice(7);
              if (!(pending.get(invocation) ?? []).some((c) => c.kind === Kind.STOP))
                yield { kind: Kind.READY };
              while (!ctx.signal.aborted) {
                for (const command of pending.get(invocation) ?? []) {
                  if (!acknowledged.has(command.id)) yield command;
                }
                await new Promise((resolve) => {
                  const done = () => {
                    events.off(invocation, done);
                    ctx.signal.removeEventListener("abort", done);
                    resolve();
                  };
                  events.once(invocation, done);
                  ctx.signal.addEventListener("abort", done, { once: true });
                });
              }
            },
            async acknowledgeCommand(request, ctx) {
              const invocation = ctx.requestHeader.get("authorization").slice(7);
              assert((pending.get(invocation) ?? []).some((c) => c.id === request.id));
              acknowledged.add(request.id);
              return {};
            },
          });
        },
      }),
    );
    callback.on("session", (session) => {
      sessions.add(session);
      session.on("close", () => sessions.delete(session));
    });
    callback.listen(0, "127.0.0.1");
    await once(callback, "listening");
    const hosts = [0, 1].map((index) => {
      const handler = createAgentHandler({
        signingKey: key,
        tracing: "existing",
        pathPrefix: "/api/agent",
        async run(ctx) {
          contexts.set(ctx.invocationId, { ctx, index });
          await new Promise((resolve) =>
            ctx.signal.addEventListener("abort", resolve, { once: true }),
          );
        },
        async checkpoint(ctx) {
          if (failCheckpoint.has(ctx.invocationId)) throw new Error("Storage unavailable");
          checkpoints.push(ctx.invocationId);
        },
      });
      return httpServer((req, res) => {
        hostRequests.push(req.url);
        return handler(req, res);
      });
    });
    for (const host of hosts) {
      host.listen(0, "127.0.0.1");
      await once(host, "listening");
    }
    t.after(() => {
      for (const session of sessions) session.destroy();
      callback.close();
      for (const host of hosts) host.close();
    });
    async function eventually(check) {
      for (let i = 0; i < 200; i++) {
        const value = check();
        if (value) return value;
        await delay(10);
      }
      throw new Error("Condition not observed");
    }
    function invoke(index, invocation = randomUUID()) {
      const request = {
        agentId: randomUUID(),
        threadId: randomUUID(),
        invocationId: invocation,
        runId: randomUUID(),
        capability: invocation,
        callbackUrl: `http://127.0.0.1:${callback.address().port}`,
        commandId: invocation,
      };
      const timestamp = String(Math.floor(Date.now() / 1000));
      const digest = createHash("sha256")
        .update(toBinary(InvokeRequestSchema, create(InvokeRequestSchema, request)))
        .digest("hex");
      const headers = {
        "x-tilde-timestamp": timestamp,
        "x-tilde-signature": createHmac("sha256", key)
          .update(`Invoke.${timestamp}.${digest}`)
          .digest("hex"),
      };
      const client = createClient(
        AgentHostService,
        createConnectTransport({
          baseUrl: `http://127.0.0.1:${hosts[index].address().port}/api/agent`,
          httpVersion: "1.1",
        }),
      );
      const finished = (async () => {
        for await (const _ of client.invoke(request, { headers })) {
        }
      })();
      return { invocation, finished };
    }
    function command(invocation, kind, extra = {}) {
      const value = { id: randomUUID(), kind, ...extra };
      pending.set(invocation, [...(pending.get(invocation) ?? []), value]);
      events.emit(invocation);
      return value;
    }
    const a = invoke(0),
      b = invoke(1);
    await eventually(() => contexts.size === 2);
    const steer = command(a.invocation, Kind.STEER, {
      inputId: randomUUID(),
      text: "change direction",
    });
    await eventually(() => acknowledged.has(steer.id));
    assert.equal(contexts.get(a.invocation).ctx.takeInputs()[0].text, "change direction");
    acknowledged.delete(steer.id);
    events.emit(a.invocation);
    await eventually(() => acknowledged.has(steer.id));
    assert.deepEqual(contexts.get(a.invocation).ctx.takeInputs(), []);
    const stop = command(a.invocation, Kind.STOP);
    await a.finished;
    assert(contexts.get(a.invocation).ctx.signal.aborted);
    assert(acknowledged.has(stop.id));
    assert(!contexts.get(b.invocation).ctx.signal.aborted);
    const suspend = command(b.invocation, Kind.SUSPEND);
    await b.finished;
    assert.deepEqual(checkpoints, [b.invocation]);
    assert(acknowledged.has(suspend.id));
    const failed = invoke(0);
    await eventually(() => contexts.has(failed.invocation));
    failCheckpoint.add(failed.invocation);
    const failure = assert.rejects(failed.finished, /Suspension checkpoint failed/);
    const failedSuspend = command(failed.invocation, Kind.SUSPEND);
    await failure;
    assert(!acknowledged.has(failedSuspend.id));
    const late = randomUUID();
    command(late, Kind.STOP);
    await assert.rejects(invoke(1, late).finished);
    assert(!contexts.has(late));
    assert(hostRequests.every((path) => path.endsWith("/Invoke")));
    for (const method of ["Stop", "Steer", "Cancel"]) {
      const response = await fetch(
        `http://127.0.0.1:${hosts[0].address().port}/api/agent/tilde.agent_host.v1.AgentService/${method}`,
        { method: "POST" },
      );
      assert.equal(response.status, 404);
    }
  },
);
