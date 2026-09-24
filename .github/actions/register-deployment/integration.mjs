// Real action process -> management API key guard -> deployment service -> PostgreSQL.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomBytes, randomUUID } from "node:crypto";
import { once } from "node:events";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { startOidc, loginManagement } from "../../../scripts/test-oidc.mjs";

assert.ok(process.env.TEST_DATABASE_URL, "Run with scripts/with-postgres.sh");
const scratch = await mkdtemp(join(tmpdir(), "tilde-action-integration-"));
const oidc = await startOidc();
let server;
try {
  server = spawn(
    resolve(process.env.ENGINE_TEST_BINARY ?? "target/debug/tilde"),
    ["--listen", "127.0.0.1:0"],
    {
      env: {
        PATH: process.env.PATH,
        DATABASE_URL: process.env.TEST_DATABASE_URL,
        ENGINE_ENCRYPTION_BACKEND: "seed",
        ENGINE_ENCRYPTION_KEY: randomBytes(32).toString("base64"),
        ENGINE_WEB_ENABLED: "false",
        ENGINE_SERVE: "all",
        LOGS_QUEUE_DIR: join(scratch, "logs"),
        RUST_LOG: "tilde=info",
        ...oidc.env,
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  const url = await new Promise((resolve, reject) => {
    let logs = "";
    const timer = setTimeout(() => reject(new Error("API startup timed out")), 20_000);
    const read = (chunk) => {
      logs += chunk;
      const match = logs.match(/ address=(127\.0\.0\.1:\d+)/);
      if (match) {
        clearTimeout(timer);
        resolve(`http://${match[1]}`);
      }
    };
    server.stdout.on("data", read);
    server.stderr.on("data", read);
    server.once("error", () => {
      clearTimeout(timer);
      reject(new Error("API startup failed"));
    });
    server.once("exit", () => {
      clearTimeout(timer);
      reject(new Error("API exited before ready"));
    });
  });
  const session = await loginManagement(url);
  async function rpc(service, method, body, credential = session) {
    const result = await fetch(`${url}/tilde.management.v1.${service}/${method}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${credential}` },
      body: JSON.stringify(body),
    });
    assert.equal(result.status, 200, `${method} failed`);
    return result.json();
  }
  const { secret } = await rpc("ApiKeysService", "CreateApiKey", { name: "Action fixture" });
  const agentId = randomUUID();
  await rpc("AgentService", "CreateAgent", {
    id: agentId,
    name: "Action fixture",
  });
  async function action(rotate = false, outputToken = true) {
    const outputFile = join(scratch, "output");
    const summaryFile = join(scratch, "summary");
    await Promise.all([writeFile(outputFile, ""), writeFile(summaryFile, "")]);
    const child = spawn(process.execPath, [fileURLToPath(new URL("index.mjs", import.meta.url))], {
      env: {
        PATH: process.env.PATH,
        TILDE_URL: url,
        TILDE_API_KEY: secret,
        TILDE_AGENT_ID: agentId,
        GITHUB_OUTPUT: outputFile,
        GITHUB_STEP_SUMMARY: summaryFile,
        GITHUB_REPOSITORY: "trytilde/action-fixture",
        GITHUB_RUN_ID: "1",
        GITHUB_SHA: "abcdef1234",
        GITHUB_REF_NAME: "main",
        "INPUT_ROTATE-EXISTING-TOKEN": String(rotate),
        "INPUT_OUTPUT-TOKEN": String(outputToken),
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let logs = "";
    child.stdout.on("data", (chunk) => {
      logs += chunk;
    });
    child.stderr.on("data", (chunk) => {
      logs += chunk;
    });
    const [code] = await once(child, "close");
    const raw = await readFile(outputFile, "utf8");
    const outputs = Object.fromEntries(
      [...raw.matchAll(/^([^\n]+)<<([^\n]+)\n([\s\S]*?)\n\2\n/gm)].map((m) => [m[1], m[3]]),
    );
    const summary = await readFile(summaryFile, "utf8");
    const visible = logs.replace(/^::add-mask::.*\n/gm, "");
    for (const credential of [secret, outputs["deployment-token"]].filter(Boolean)) {
      assert.ok(
        !visible.includes(credential) && !summary.includes(credential),
        "Credential escaped the masking protocol",
      );
    }
    return { code, outputs };
  }
  const first = await action();
  assert.equal(first.code, 0, "First registration succeeds");
  assert.ok(first.outputs["deployment-token"], "New registration returns a token");
  const snapshot = await rpc("DeploymentService", "GetDeployment", { agentId }, secret);
  assert.equal(snapshot.deployments.length, 1);
  assert.equal(snapshot.deployments[0].id, first.outputs["deployment-id"]);
  assert.equal(snapshot.deployments[0].source, "DEPLOYMENT_SOURCE_CI");
  assert.equal(snapshot.deployments[0].commitSha, "abcdef1234");
  assert.equal(snapshot.deployments[0].routable ?? false, false);
  const retry = await action();
  assert.equal(retry.code, 1, "Retry never silently rotates credentials");
  assert.equal(retry.outputs["deployment-id"], first.outputs["deployment-id"]);
  const manual = await action(false, false);
  assert.equal(manual.code, 0, "Manual retry succeeds without rotating credentials");
  assert.equal(manual.outputs["deployment-token"], undefined);
  assert.equal(manual.outputs["deployment-id"], first.outputs["deployment-id"]);
  const rotated = await action(true);
  assert.equal(rotated.code, 0);
  assert.equal(rotated.outputs["deployment-id"], first.outputs["deployment-id"]);
  assert.ok(
    rotated.outputs["deployment-token"] &&
      rotated.outputs["deployment-token"] !== first.outputs["deployment-token"],
    "Explicit rotation replaces the credential",
  );
  console.log(
    "PASS: action registration, management API key authentication, idempotent retry, explicit rotation and masked handoff against real Tilde/PostgreSQL.",
  );
} finally {
  if (server && server.exitCode === null) {
    const closed = once(server, "close");
    server.kill("SIGTERM");
    const timer = setTimeout(() => server.kill("SIGKILL"), 5000);
    await closed;
    clearTimeout(timer);
  }
  await oidc.stop();
  await rm(scratch, { recursive: true, force: true });
}
