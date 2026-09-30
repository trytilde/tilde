import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http2";
import { once } from "node:events";
import { randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { connectNodeAdapter } from "@connectrpc/connect-node";
import {
  auditToolCall,
  connectAgent,
  RuntimeChatService,
  RunService,
  ToolDisplay,
} from "../dist/index.js";
import {
  InvocationControlService,
  InvocationCommandKind as Kind,
} from "@trytilde/contracts/tilde/runtime/v1/controls_pb.js";

void test(
  "connectAgent dials the gateway to receive wakes, report runs and heartbeat readiness",
  { timeout: 15000 },
  async (t) => {
    const deploymentToken = `deployment-${randomUUID()}`;
    const wake = {
      deploymentId: randomUUID(),
      agentId: randomUUID(),
      threadId: randomUUID(),
      invocationId: randomUUID(),
      runId: randomUUID(),
      commandId: randomUUID(),
      capability: `capability-${randomUUID()}`,
      objective: "Say hello",
      assignmentGeneration: 1n,
    };
    const watches = [];
    const heartbeats = [];
    const reports = [];
    const bundledRegistrations = [];
    const toolReports = [];
    const executions = [];
    const serverToolCalls = [];
    const sessions = new Set();
    let port;
    const gateway = createServer(
      connectNodeAdapter({
        routes(router) {
          router.service(RunService, {
            async *watch(request, ctx) {
              watches.push({
                instanceId: request.instanceId,
                authorization: ctx.requestHeader.get("authorization"),
              });
              yield {
                frame: {
                  case: "registered",
                  value: {
                    agentId: wake.agentId,
                    deploymentId: wake.deploymentId,
                    instanceId: request.instanceId,
                  },
                },
              };
              yield { frame: { case: "ping", value: {} } };
              if (watches.length === 1)
                yield {
                  frame: {
                    case: "wake",
                    value: { ...wake, callbackUrl: `http://127.0.0.1:${port}` },
                  },
                };
              await new Promise((resolve) =>
                ctx.signal.addEventListener("abort", resolve, { once: true }),
              );
            },
            async heartbeat(request, ctx) {
              heartbeats.push({
                ...request,
                authorization: ctx.requestHeader.get("authorization"),
              });
              return {};
            },
            async report(request, ctx) {
              reports.push({
                authorization: ctx.requestHeader.get("authorization"),
                invocationId: request.invocationId,
                event: request.event,
              });
              return {};
            },
          });
          router.service(RuntimeChatService, {
            async listTools() {
              return {
                tools: [
                  {
                    name: "tools.execute",
                    providerId: "tilde",
                    description: "Call a tool found with tools.search.",
                    inputSchemaJson: "{}",
                  },
                ],
              };
            },
            async invokeTool(requests) {
              for await (const frame of requests)
                serverToolCalls.push([frame.name, JSON.parse(frame.inputJson)]);
              return { outputJson: JSON.stringify({ routed: true }) };
            },
            async registerBundledTools(request) {
              bundledRegistrations.push(request.tools);
              return {};
            },
            async reportToolCall(request) {
              toolReports.push(request.toolCall);
              return {};
            },
          });
          router.service(InvocationControlService, {
            async *watchCommands(_, ctx) {
              yield { kind: Kind.READY };
              await new Promise((resolve) =>
                ctx.signal.addEventListener("abort", resolve, { once: true }),
              );
            },
          });
        },
      }),
    );
    gateway.on("session", (session) => {
      sessions.add(session);
      session.on("close", () => sessions.delete(session));
    });
    gateway.listen(0, "127.0.0.1");
    await once(gateway, "listening");
    port = gateway.address().port;
    const contexts = [];
    const registrations = [];
    let dependenciesReady = false;
    const host = connectAgent({
      gatewayUrl: `http://127.0.0.1:${port}`,
      deploymentToken,
      tracing: "existing",
      ready: () => {
        if (!dependenciesReady) throw new Error("Dependency unavailable");
        return true;
      },
      onRegistered: (registration) => registrations.push(registration),
      async run(ctx) {
        contexts.push(ctx);
        // The agent's bundled tool, found by search elsewhere and run here through tools.execute.
        // Framework adapters publish it with an executor that runs their audited native tool.
        const readFile = {
          name: "read_file",
          description: "Read a file from the workspace.",
          summary: "Read a file",
          display: "summary",
          inputSchema: { type: "object", properties: { path: { type: "string" } } },
          outputSchema: { type: "object", properties: { text: { type: "string" } } },
          annotations: { readOnly: true, destructive: false, idempotent: true, openWorld: false },
          execute: (input, execution) => {
            executions.push(execution);
            return auditToolCall(
              ctx,
              {
                ...execution,
                name: "read_file",
                input,
                summary: "Read a file",
                display: "summary",
              },
              async () => ({ text: `contents of ${input.path}` }),
            );
          },
        };
        await ctx.setBundledTools([readFile]);
        await assert.rejects(
          ctx.setBundledTools([{ ...readFile, name: "tools.run" }]),
          /conflicts/,
        );
        await assert.rejects(ctx.setBundledTools([{ ...readFile, name: "stop" }]), /conflicts/);
        // The framework already holds its native tool: the bundled tool is not offered again.
        assert(!("read_file" in ctx.agentTools));
        assert(!("read_file" in ctx.tools));
        assert.deepEqual(
          await ctx.tools["tools.execute"].execute(
            { name: "read_file", input: { path: "a.txt" } },
            { toolCallId: "call-1", frameworkContext: "framework state" },
          ),
          { text: "contents of a.txt" },
        );
        // Removing the last bundled tool drops its executor: tools.execute goes to Tilde.
        await ctx.setBundledTools([]);
        assert.deepEqual(
          await ctx.tools["tools.execute"].execute(
            { name: "read_file", input: { path: "b.txt" } },
            { toolCallId: "call-2" },
          ),
          { routed: true },
        );
        await ctx.reason("thinking");
        await ctx.reason(" harder");
      },
    });
    t.after(async () => {
      await host.close();
      for (const session of sessions) session.destroy();
      gateway.close();
    });
    async function eventually(check) {
      for (let i = 0; i < 400; i++) {
        if (check()) return;
        await delay(10);
      }
      throw new Error("Condition not observed");
    }
    await eventually(() => reports.some((r) => r.event.case === "stopped"));
    assert.equal(watches.length, 1);
    assert.ok(watches[0].instanceId);
    assert.equal(watches[0].authorization, `Bearer ${deploymentToken}`);
    assert.equal(registrations.length, 1);
    assert.equal(registrations[0].agentId, wake.agentId);
    assert.equal(registrations[0].deploymentId, wake.deploymentId);
    assert.equal(contexts.length, 1);
    assert.equal(contexts[0].invocationId, wake.invocationId);
    assert.equal(contexts[0].threadId, wake.threadId);
    assert.equal(contexts[0].objective, "Say hello");
    // Registered on every invocation, then again with the agent's bundled tools, metadata
    // included, and again without them. SDK helpers such as stop are not bundled tools.
    assert.equal(bundledRegistrations.length, 3);
    assert.deepEqual(bundledRegistrations[0], []);
    assert.deepEqual(bundledRegistrations[2], []);
    assert.deepEqual(serverToolCalls, [
      ["tools.execute", { name: "read_file", input: { path: "b.txt" } }],
    ]);
    assert.deepEqual(
      bundledRegistrations[1].map((t) => t.name),
      ["read_file"],
    );
    const readFile = bundledRegistrations[1][0];
    assert.equal(readFile.display, ToolDisplay.SUMMARY);
    assert.equal(readFile.summary, "Read a file");
    assert.deepEqual(JSON.parse(readFile.outputSchemaJson).properties.text, { type: "string" });
    assert.equal(readFile.annotations.readOnly, true);
    // The routed call reaches the executor with the framework's state and is audited once, as
    // the bundled tool with its summary, not as Tilde's tools.execute.
    assert.deepEqual(executions, [{ toolCallId: "call-1", frameworkContext: "framework state" }]);
    assert.deepEqual(
      toolReports.map((c) => [c.name, c.status, c.summary, c.display]),
      [
        ["read_file", "running", "Read a file", ToolDisplay.SUMMARY],
        ["read_file", "completed", "Read a file", ToolDisplay.SUMMARY],
      ],
    );
    assert.deepEqual(JSON.parse(toolReports[1].outputJson), { text: "contents of a.txt" });
    assert(reports.every((r) => r.invocationId === wake.invocationId));
    assert(reports.every((r) => r.authorization === `Bearer ${wake.capability}`));
    assert.deepEqual(
      reports.map((r) => r.event.case),
      ["accepted", "reasoningDelta", "reasoningDelta", "stopped"],
    );
    assert.equal(reports[0].event.value.commandId, wake.commandId);
    assert.deepEqual(
      reports.filter((r) => r.event.case === "reasoningDelta").map((r) => r.event.value),
      ["thinking", " harder"],
    );
    assert.deepEqual(reports[3].event.value.pendingInputIds, []);
    // Heartbeats carry the deployment token and the ready hook's result; a throw is not ready.
    assert.equal(heartbeats[0].instanceId, watches[0].instanceId);
    assert.equal(heartbeats[0].ready, false);
    assert.equal(heartbeats[0].authorization, `Bearer ${deploymentToken}`);
    dependenciesReady = true;
    // Ending the stream from the gateway makes the host reconnect after backoff.
    for (const session of sessions) session.destroy();
    await eventually(() => watches.length >= 2);
    assert.equal(watches[1].instanceId, watches[0].instanceId);
    await eventually(() => heartbeats.at(-1).ready === true);
    await host.close();
    const count = watches.length;
    await delay(1200);
    assert.equal(watches.length, count);
  },
);
