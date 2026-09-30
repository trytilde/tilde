import assert from "node:assert/strict";
import test from "node:test";
import { createServer } from "node:http";
import { once } from "node:events";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const agent = "11111111-1111-4111-8111-111111111111";
const deployment = "22222222-2222-4222-8222-222222222222";
const apiToken = "fixture_management_secret";
const deploymentToken = "fixture_deployment_secret";
const main = fileURLToPath(new URL("index.mjs", import.meta.url));

async function fixture(t, respond) {
  const calls = [];
  const server = createServer(async (req, res) => {
    let text = "";
    for await (const chunk of req) text += chunk;
    calls.push({ path: req.url, headers: req.headers, body: text ? JSON.parse(text) : null });
    const reply = respond(calls.at(-1), calls.length);
    res.writeHead(reply.status ?? 200, { "content-type": "application/json", ...reply.headers });
    res.end(
      JSON.stringify(
        reply.body ?? { deployment: { id: deployment }, created: true, token: deploymentToken },
      ),
    );
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => {
    server.closeAllConnections();
    server.close();
  });
  return { calls, url: `http://127.0.0.1:${server.address().port}` };
}
async function run(t, url, overrides = {}) {
  const dir = await mkdtemp(join(tmpdir(), "tilde-action-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const output = join(dir, "output");
  const summary = join(dir, "summary");
  const event = join(dir, "event.json");
  await Promise.all([
    writeFile(output, ""),
    writeFile(summary, ""),
    writeFile(
      event,
      JSON.stringify({
        head_commit: {
          id: "abcdef1234",
          message: "Ship\n::error::untrusted commit text",
          author: { name: "Ada" },
        },
      }),
    ),
  ]);
  const child = spawn(process.execPath, [main], {
    env: {
      PATH: process.env.PATH,
      TILDE_API_KEY: apiToken,
      TILDE_URL: url,
      TILDE_AGENT_ID: agent,
      GITHUB_OUTPUT: output,
      GITHUB_STEP_SUMMARY: summary,
      GITHUB_EVENT_PATH: event,
      GITHUB_REPOSITORY: "example/agent",
      GITHUB_RUN_ID: "42",
      GITHUB_RUN_ATTEMPT: "1",
      GITHUB_SHA: "abcdef1234",
      GITHUB_REF_NAME: "main",
      ...overrides,
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
  const raw = await readFile(output, "utf8");
  const outputs = Object.fromEntries(
    [...raw.matchAll(/^([^\n]+)<<([^\n]+)\n([\s\S]*?)\n\2\n/gm)].map((m) => [m[1], m[3]]),
  );
  const text = await readFile(summary, "utf8");
  // Mask commands are runner protocol, not human-visible output. Nothing else may contain secrets.
  const visible = logs.replace(/^::add-mask::.*\n/gm, "");
  for (const secret of [apiToken, deploymentToken, overrides["INPUT_API-TOKEN"]].filter(Boolean)) {
    assert.ok(!visible.includes(secret), "credential leaked outside the masking protocol");
    assert.ok(!text.includes(secret), "credential leaked into summary");
  }
  return { code, outputs, logs, summary: text };
}

test("registers using env defaults, includes CI metadata, masks token and links manual handoff", async (t) => {
  const server = await fixture(t, () => ({}));
  const result = await run(t, server.url);
  assert.equal(result.code, 0);
  const request = server.calls[0];
  assert.equal(request.headers.authorization, `Bearer ${apiToken}`);
  assert.equal(request.headers["connect-protocol-version"], "1");
  assert.equal(request.body.externalId, "example/agent:42");
  assert.equal(request.body.source, "DEPLOYMENT_SOURCE_CI");
  assert.equal(request.body.target, "DEPLOYMENT_TARGET_GATEWAY");
  assert.equal(request.body.commitAuthor, "Ada");
  assert.equal(request.body.commitSha, "abcdef1234");
  assert.equal(request.body.endpointUrl, undefined);
  assert.equal(result.outputs["deployment-token"], deploymentToken);
  assert.equal(result.outputs["deployment-id"], deployment);
  assert.equal(result.outputs.created, "true");
  assert.ok(
    result.logs.indexOf(`::add-mask::${deploymentToken}`) <
      result.logs.indexOf("Tilde deployment registered"),
  );
  assert.ok(!result.logs.includes("untrusted commit text"));
  assert.ok(result.summary.includes("Rotate token"));
});

test("explicit credentials and fields override environment; Lambda sends only its ARN", async (t) => {
  const server = await fixture(t, () => ({}));
  const result = await run(t, server.url, {
    "INPUT_API-TOKEN": "explicit_key",
    INPUT_TYPE: "lambda",
    "INPUT_FUNCTION-ARN": "arn:aws:lambda:eu-west-1:123456789012:function:agent:7",
    "INPUT_UI-URL": "https://console.example.com",
    "INPUT_COMMIT-SHA": "fedcba",
    "INPUT_EXTERNAL-ID": "production-release-7",
  });
  assert.equal(result.code, 0);
  assert.equal(server.calls[0].headers.authorization, "Bearer explicit_key");
  assert.equal(server.calls[0].body.target, "DEPLOYMENT_TARGET_LAMBDA");
  assert.equal(server.calls[0].body.externalId, "production-release-7");
  assert.equal(server.calls[0].body.commitMessage, undefined);
  assert.ok(server.calls[0].body.targetReference.endsWith(":7"));
  assert.equal(
    result.outputs["deployment-url"],
    `https://console.example.com/agent/${agent}/deployment`,
  );
});

test("retries preserve registration identity and never rotate implicitly", async (t) => {
  const server = await fixture(t, () => ({ body: { deployment: { id: deployment } } }));
  const result = await run(t, server.url, { GITHUB_RUN_ATTEMPT: "2", INPUT_TYPE: "sidecar" });
  assert.equal(result.code, 1);
  assert.equal(server.calls.length, 1);
  assert.equal(server.calls[0].body.externalId, "example/agent:42");
  assert.equal(server.calls[0].body.target, "DEPLOYMENT_TARGET_SIDECAR");
  assert.equal(result.outputs.created, "false");
  assert.equal(result.outputs["deployment-token"], undefined);
  assert.match(result.logs, /already registered/);
});

test("an explicit retry rotation issues a new masked token for the existing deployment", async (t) => {
  const server = await fixture(t, (_, count) => ({
    body: count === 1 ? { deployment: { id: deployment } } : { token: deploymentToken },
  }));
  const result = await run(t, server.url, { "INPUT_ROTATE-EXISTING-TOKEN": "true" });
  assert.equal(result.code, 0);
  assert.equal(server.calls.length, 2);
  assert.ok(server.calls[1].path.endsWith("/IssueDeploymentToken"));
  assert.deepEqual(server.calls[1].body, { agentId: agent, deploymentId: deployment });
  assert.equal(result.outputs["deployment-token"], deploymentToken);
});

test("does not echo sensitive API error bodies or follow redirects", async (t) => {
  for (const status of [401, 500, 302]) {
    const server = await fixture(t, () => ({
      status,
      headers: { location: "/credential-trap" },
      body: { message: `${apiToken} ${deploymentToken}` },
    }));
    const result = await run(t, server.url);
    assert.equal(result.code, 1);
    assert.equal(server.calls.length, 1);
    assert.deepEqual(result.outputs, {});
  }
});

test("invalid type, missing Lambda ARN, and invalid rotation fail before registering", async (t) => {
  const server = await fixture(t, () => ({}));
  for (const overrides of [
    { INPUT_TYPE: "other" },
    { INPUT_TYPE: "lambda" },
    { "INPUT_ROTATE-EXISTING-TOKEN": "yes" },
  ]) {
    assert.equal((await run(t, server.url, overrides)).code, 1);
  }
  assert.equal(server.calls.length, 0);
});

test("without a management key (open-source Tilde) registration sends no authorization", async (t) => {
  const server = await fixture(t, () => ({}));
  const result = await run(t, server.url, { TILDE_API_KEY: "" });
  assert.equal(result.code, 0, result.logs);
  assert.equal(server.calls[0].headers.authorization, undefined);
  assert.equal(result.outputs["deployment-token"], deploymentToken);
});

test("manual setup omits the token output on creation and idempotent retries", async (t) => {
  const server = await fixture(t, (_, count) => ({
    body:
      count === 1
        ? { deployment: { id: deployment }, created: true, token: deploymentToken }
        : { deployment: { id: deployment } },
  }));
  for (const attempt of ["1", "2"]) {
    const result = await run(t, server.url, {
      "INPUT_OUTPUT-TOKEN": "false",
      GITHUB_RUN_ATTEMPT: attempt,
    });
    assert.equal(result.code, 0);
    assert.equal(result.outputs["deployment-token"], undefined);
    assert.equal(result.outputs["deployment-id"], deployment);
    assert.ok(result.outputs["deployment-url"]);
    assert.match(result.summary, /Rotate token/);
  }
  assert.equal(server.calls.length, 2);
  assert.ok(server.calls.every((call) => call.path.endsWith("/RegisterDeployment")));
});

test("manual setup rejects rotation and invalid output-token values before mutation", async (t) => {
  const server = await fixture(t, () => ({}));
  for (const overrides of [
    { "INPUT_OUTPUT-TOKEN": "false", "INPUT_ROTATE-EXISTING-TOKEN": "true" },
    { "INPUT_OUTPUT-TOKEN": "yes" },
  ]) {
    assert.equal((await run(t, server.url, overrides)).code, 1);
  }
  assert.equal(server.calls.length, 0);
});

test("a deploy command registers instead of the action; its JSON result becomes the outputs", async (t) => {
  const server = await fixture(t, () => ({}));
  const dir = await mkdtemp(join(tmpdir(), "tilde-command-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const script = join(dir, "deploy.mjs");
  // Stands in for `npx tilde deploy dist/index.js --json`.
  await writeFile(
    script,
    `const e = process.env;
process.stderr.write("tilde deploy: dist/index.js\\n");
if (e.TILDE_URL !== ${JSON.stringify(server.url)} || e.TILDE_AGENT_ID !== ${JSON.stringify(agent)} || e.TILDE_API_KEY !== ${JSON.stringify(apiToken)} || e.GITHUB_RUN_ID !== "42") process.exit(3);
process.stdout.write(JSON.stringify({ deploymentId: ${JSON.stringify(deployment)}, token: ${JSON.stringify(deploymentToken)}, created: true }) + "\\n");
`,
  );
  const result = await run(t, server.url, {
    INPUT_COMMAND: `${JSON.stringify(process.execPath)} ${JSON.stringify(script)}`,
  });
  assert.equal(result.code, 0, result.logs);
  assert.equal(server.calls.length, 0);
  assert.equal(result.outputs["deployment-id"], deployment);
  assert.equal(result.outputs["deployment-token"], deploymentToken);
  assert.equal(result.outputs.created, "true");
  assert.ok(result.logs.includes("tilde deploy: dist/index.js"));

  const failing = await run(t, server.url, {
    INPUT_COMMAND: `${JSON.stringify(process.execPath)} -e "process.exit(2)"`,
  });
  assert.equal(failing.code, 1);
  assert.ok(failing.logs.includes("command failed (exit 2)"));
});
