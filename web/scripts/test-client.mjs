import { startOidc, loginManagement } from "../../scripts/test-oidc.mjs";
// Real generated-client integration against the packaged Rust server and isolated Postgres.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomBytes, randomUUID } from "node:crypto";
import { resolve, join } from "node:path";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { once } from "node:events";
import { createClient, Code, ConnectError } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { toJsonString } from "@bufbuild/protobuf";
import {
  AgentService,
  GetAgentResponseSchema,
} from "@trytilde/contracts/tilde/management/v1/agents_pb.js";

const database = process.env.TEST_DATABASE_URL;
assert(database, "TEST_DATABASE_URL is required; use task test");
const binary = resolve(process.env.ENGINE_TEST_BINARY ?? "target/debug/tilde");
const seed = randomBytes(32).toString("base64");
let logs = "";
const oidc = await startOidc();
const tokens = new Map();
const scratch = await mkdtemp(join(tmpdir(), "tilde-binary-"));
async function start(key = seed) {
  const env = {
    ...process.env,
    ...oidc.env,
    DATABASE_URL: database,
    ENGINE_ENCRYPTION_BACKEND: "seed",
    ENGINE_ENCRYPTION_KEY: key,
    RUST_LOG: "tilde=info",
  };
  delete env.ENGINE_PUBLIC_URL;
  delete env.ENGINE_RUNTIME_PUBLIC_URL;
  delete env.ENGINE_KMS_KEY_ID;
  delete env.ENGINE_LISTEN;
  delete env.ENGINE_ALLOW_NETWORK;
  env.ENGINE_WEB_ORIGINS = "";
  const child = spawn(binary, ["--listen", "127.0.0.1:0"], {
    env,
    cwd: scratch,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const ready = new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error("Engine startup timed out"));
    }, 20000);
    const data = (chunk) => {
      const text = chunk.toString();
      output += text;
      logs += text;
      const match = output.match(/ address=(127\.0\.0\.1:\d+)/);
      if (match) {
        clearTimeout(timer);
        resolve(`http://${match[1]}`);
      }
    };
    child.stdout.on("data", data);
    child.stderr.on("data", data);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`Engine exited ${code}: ${output}`));
    });
  });
  const url = await ready;
  tokens.set(url, await loginManagement(url));
  return {
    url,
    async stop() {
      const exited = once(child, "exit");
      child.kill("SIGTERM");
      const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
      try {
        await exited;
      } finally {
        clearTimeout(timer);
      }
    },
  };
}
function client(url, binary) {
  return createClient(
    AgentService,
    createConnectTransport({
      baseUrl: url,
      useBinaryFormat: binary,
      useHttpGet: !binary,
      interceptors: [
        (next) => async (request) => {
          request.header.set("Authorization", `Bearer ${tokens.get(url)}`);
          return next(request);
        },
      ],
    }),
  );
}
let server;
const ids = [];
const value = randomBytes(32).toString("hex");
try {
  server = await start();
  assert.equal((await fetch(server.url + "/healthz")).status, 200);
  assert.equal((await fetch(server.url + "/readyz")).status, 200);
  const html = await fetch(server.url + "/");
  assert.equal(html.status, 200);
  assert.match(html.headers.get("content-type"), /text\/html/);
  const body = await html.text();
  assert.match(body, /Tilde/);
  const script = body.match(/src="([^"]+\.js)"/)[1];
  const asset = await fetch(server.url + script);
  assert.equal(asset.status, 200);
  assert.match(asset.headers.get("cache-control"), /immutable/);
  assert.equal(
    (await fetch(server.url + "/agents/example", { headers: { accept: "text/html" } })).status,
    200,
  );
  assert.equal(
    (await fetch(server.url + "/assets/missing.js", { headers: { accept: "text/html" } })).status,
    404,
  );
  assert.equal(
    (
      await fetch(server.url + "/tilde.management.v1.AgentService/Unknown", {
        headers: { accept: "text/html" },
      })
    ).status,
    404,
  );
  assert.equal(
    (await fetch(server.url + "/healthz", { headers: { origin: "https://attacker.example" } }))
      .status,
    403,
  );
  await assert.rejects(
    client(server.url, false).createAgent({ name: "Missing key" }),
    (error) => error instanceof ConnectError && error.code === Code.InvalidArgument,
  );
  const minimal = await client(server.url, false).createAgent({
    name: "Minimal agent",
    endpointUrl: "http://127.0.0.1:3000/agent",
    webhookSigningKey: value,
  });
  assert.match(minimal.agent.id, /^[0-9a-f-]{36}$/);
  await client(server.url, false).pauseAgent({ id: minimal.agent.id });
  await client(server.url, false).deleteAgent({ id: minimal.agent.id });
  for (const binary of [false, true]) {
    const rpc = client(server.url, binary);
    const id = randomUUID();
    ids.push(id);
    const request = {
      id,
      name: binary ? "Protobuf agent" : "JSON agent",
      webhookSigningKey: value,
      endpointUrl: "http://127.0.0.1:3000/agent",
    };
    const created = await rpc.createAgent(request);
    assert.equal(created.agent.id, id);
    assert.equal((await rpc.createAgent(request)).agent.id, id);
    await assert.rejects(
      rpc.createAgent({ ...request, name: "Different" }),
      (error) => error instanceof ConnectError && error.code === Code.AlreadyExists,
    );
    await assert.rejects(
      rpc.createAgent({ ...request, webhookSigningKey: randomBytes(32).toString("hex") }),
      (error) => error instanceof ConnectError && error.code === Code.AlreadyExists,
    );
    await assert.rejects(
      rpc.updateAgent({ id, endpointUrl: "" }),
      (error) => error instanceof ConnectError && error.code === Code.InvalidArgument,
    );
    const updated = await rpc.updateAgent({ id, name: "Renamed" });
    assert.equal(updated.agent.endpointUrl, request.endpointUrl);
    const read = await rpc.getAgent({ id });
    const json = toJsonString(GetAgentResponseSchema, read);
    assert(!json.includes(value));
    assert(!json.includes("webhookSigningKey"));
    assert(!json.includes("secretNames"));
    assert(!json.includes("description"));
    assert((await rpc.listAgents({ pageSize: 100 })).agents.some((agent) => agent.id === id));
    await assert.rejects(
      rpc.getAgent({ id: "not-a-uuid" }),
      (error) => error instanceof ConnectError && error.code === Code.InvalidArgument,
    );
  }
  await server.stop();
  server = undefined;
  await assert.rejects(start(randomBytes(32).toString("base64")), /Encryption|encryption/);
  server = await start();
  const rpc = client(server.url, true);
  for (const id of ids) {
    assert.equal((await rpc.getAgent({ id })).agent.name, "Renamed");
    await rpc.createAgent({
      id,
      name: "Renamed",
      endpointUrl: "http://127.0.0.1:3000/agent",
      webhookSigningKey: value,
    });
    await rpc.pauseAgent({ id });
    await rpc.deleteAgent({ id });
    await rpc.deleteAgent({ id });
    await assert.rejects(
      rpc.getAgent({ id }),
      (error) => error instanceof ConnectError && error.code === Code.NotFound,
    );
  }
  assert(!logs.includes(value));
  assert(!logs.includes(seed));
  console.log(
    "PASS: generated Connect JSON/Protobuf clients, CRUD, required signing key, key redaction, key recovery, restart, embedded UI, and browser-origin guard.",
  );
} finally {
  if (server) await server.stop();
  await rm(scratch, { recursive: true, force: true });
  await oidc.stop();
}
