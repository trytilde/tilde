import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http2";
import { once } from "node:events";
import { setTimeout as delay } from "node:timers/promises";
import { create } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";
import { connectNodeAdapter } from "@connectrpc/connect-node";
import { AgentContext, RuntimeChatService } from "../dist/index.js";
import { InvokeRequestSchema } from "@trytilde/contracts/tilde/agent_host/v1/agent_pb.js";
import {
  InvocationControlService,
  InvocationCommandKind,
} from "@trytilde/contracts/tilde/runtime/v1/controls_pb.js";

test(
  "renewed tokens reach RPCs and replacement control streams; denied renewal stops the invocation",
  { timeout: 10000 },
  async (t) => {
    t.mock.timers.enable({ apis: ["setInterval", "Date"], now: Date.now() });
    const controller = new AbortController();
    const finishFirstStream = Promise.withResolvers();
    const ready = Promise.withResolvers();
    const reconnected = Promise.withResolvers();
    const renewals = [];
    const reads = [];
    const watches = [];
    const sessions = new Set();
    let rejectRenewal = false;
    const server = createServer(
      connectNodeAdapter({
        routes(router) {
          router.service(RuntimeChatService, {
            async listTools(_, ctx) {
              reads.push(ctx.requestHeader.get("authorization"));
              return { tools: [] };
            },
            async renewConnectToken(_, ctx) {
              renewals.push(ctx.requestHeader.get("authorization"));
              if (rejectRenewal) throw new ConnectError("Invocation stopped", Code.Unauthenticated);
              return { token: "renewed-token" };
            },
          });
          router.service(InvocationControlService, {
            async *watchCommands(_, ctx) {
              watches.push(ctx.requestHeader.get("authorization"));
              yield { kind: InvocationCommandKind.READY };
              if (watches.length === 1) await finishFirstStream.promise;
              else {
                reconnected.resolve();
                if (!ctx.signal.aborted) await once(ctx.signal, "abort");
              }
            },
          });
        },
      }),
    );
    server.on("session", (session) => {
      sessions.add(session);
      session.on("close", () => sessions.delete(session));
    });
    t.after(async () => {
      controller.abort();
      finishFirstStream.resolve();
      for (const session of sessions) session.destroy();
      await new Promise((resolve) => server.close(resolve));
    });
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    const ctx = new AgentContext(
      create(InvokeRequestSchema, {
        callbackUrl: `http://127.0.0.1:${server.address().port}`,
        capability: "initial-token",
      }),
      controller,
      () => {},
    );
    const controls = ctx.consumeControls(() => ready.resolve());
    controls.catch(() => {});
    await ready.promise;
    await ctx.refreshTools();
    t.mock.timers.tick(4 * 60 * 1000 - 1);
    assert.deepEqual(renewals, []);
    t.mock.timers.tick(1);
    for (let i = 0; i < 200 && ctx.traceAuthorization() !== "Bearer renewed-token"; i++)
      await delay(5);
    assert.equal(ctx.traceAuthorization(), "Bearer renewed-token");
    await ctx.refreshTools();
    assert.deepEqual(reads, ["Bearer initial-token", "Bearer renewed-token"]);
    finishFirstStream.resolve();
    await reconnected.promise;
    assert.deepEqual(watches, ["Bearer initial-token", "Bearer renewed-token"]);
    rejectRenewal = true;
    t.mock.timers.tick(4 * 60 * 1000);
    if (!controller.signal.aborted) await once(controller.signal, "abort");
    assert.deepEqual(renewals, ["Bearer initial-token", "Bearer renewed-token"]);
    await controls;
  },
);
