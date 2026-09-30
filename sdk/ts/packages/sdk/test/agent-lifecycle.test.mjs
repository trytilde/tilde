import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http2";
import { EventEmitter, once } from "node:events";
import { randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { connectNodeAdapter } from "@connectrpc/connect-node";
import { connectAgent, RuntimeChatService, RunService } from "../dist/index.js";
import {
  InvocationControlService,
  InvocationCommandKind as Kind,
} from "@trytilde/contracts/tilde/runtime/v1/controls_pb.js";

void test(
  "separate stateless hosts consume invocation controls, replay steering, and checkpoint suspension",
  { timeout: 15000 },
  async (t) => {
    const events = new EventEmitter();
    const pending = new Map();
    const acknowledged = new Set();
    const contexts = new Map();
    const checkpoints = [];
    const failCheckpoint = new Set();
    const wakes = new Map();
    const reports = [];
    const sessions = new Set();
    const callback = createServer(
      connectNodeAdapter({
        routes(router) {
          router.service(RuntimeChatService, {
            async listTools() {
              return { tools: [] };
            },
            async registerBundledTools() {
              return {};
            },
          });
          router.service(RunService, {
            async *watch(request, ctx) {
              yield { frame: { case: "registered", value: { instanceId: request.instanceId } } };
              while (!ctx.signal.aborted) {
                for (const wake of wakes.get(request.instanceId)?.splice(0) ?? [])
                  yield { frame: { case: "wake", value: wake } };
                await new Promise((resolve) => {
                  const done = () => {
                    events.off(request.instanceId, done);
                    ctx.signal.removeEventListener("abort", done);
                    resolve();
                  };
                  events.once(request.instanceId, done);
                  ctx.signal.addEventListener("abort", done, { once: true });
                });
              }
            },
            async heartbeat() {
              return {};
            },
            async report(request, ctx) {
              reports.push({
                capability: ctx.requestHeader.get("authorization").slice(7),
                invocationId: request.invocationId,
                event: request.event,
              });
              return {};
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
    let registered = 0;
    const hosts = [0, 1].map((index) =>
      connectAgent({
        gatewayUrl: `http://127.0.0.1:${callback.address().port}`,
        deploymentToken: "deployment-token",
        tracing: "existing",
        onRegistered: () => registered++,
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
      }),
    );
    t.after(async () => {
      await Promise.all(hosts.map((host) => host.close()));
      for (const session of sessions) session.destroy();
      callback.close();
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
      const instanceId = hosts[index].instanceId;
      wakes.set(instanceId, [...(wakes.get(instanceId) ?? []), request]);
      events.emit(instanceId);
      const finished = eventually(() =>
        reports.some((r) => r.invocationId === invocation && r.event.case === "stopped"),
      );
      return { invocation, finished };
    }
    function command(invocation, kind, extra = {}) {
      const value = { id: randomUUID(), kind, ...extra };
      pending.set(invocation, [...(pending.get(invocation) ?? []), value]);
      events.emit(invocation);
      return value;
    }
    await eventually(() => registered === 2);
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
    const failedSuspend = command(failed.invocation, Kind.SUSPEND);
    await failed.finished;
    assert(!acknowledged.has(failedSuspend.id));
    const late = randomUUID();
    command(late, Kind.STOP);
    await invoke(1, late).finished;
    assert(!contexts.has(late));
    // Every host reports acceptance and the end of each invocation through RunService with
    // the invocation capability.
    for (const invocation of [a.invocation, b.invocation, failed.invocation]) {
      const own = reports.filter((r) => r.invocationId === invocation);
      assert(own.every((r) => r.capability === invocation));
      assert.deepEqual(
        own.map((r) => r.event.case),
        ["accepted", "stopped"],
      );
      assert.equal(own[0].event.value.commandId, invocation);
    }
    assert.equal(reports.filter((r) => r.invocationId === late).length, 1);
    assert.equal(reports.find((r) => r.invocationId === late).event.case, "stopped");
  },
);
