// Exercise the seeded Compose Dex through its real authorization-code flow.
// Run: scripts/with-postgres.sh node scripts/test-dex.mjs (Dex must be running).
import { spawn } from "node:child_process";
import { randomBytes, createHash } from "node:crypto";
import { once } from "node:events";
import { resolve } from "node:path";
import assert from "node:assert/strict";
const address = process.env.TEST_ADDRESS ?? "127.0.0.1";
const child = spawn(
  resolve(process.env.ENGINE_TEST_BINARY ?? "target/debug/tilde"),
  ["--management-listen", `${address}:0`, "--agent-runtime-listen", "127.0.0.1:0"],
  {
    env: {
      ...process.env,
      DATABASE_URL: process.env.TEST_DATABASE_URL,
      ENGINE_ENCRYPTION_BACKEND: "seed",
      ENGINE_ENCRYPTION_KEY: randomBytes(32).toString("base64"),
      ENGINE_KMS_KEY_ID: "",
      ENGINE_MANAGEMENT_PUBLIC_URL:
        process.env.TEST_MANAGEMENT_PUBLIC_URL ?? `http://${address}:8080`,
      ENGINE_OIDC_ISSUER: `http://${address}:5556`,
      ENGINE_ALLOW_NETWORK: "true",
      ENGINE_OIDC_CLIENT_ID: "tilde-dev",
      ENGINE_OIDC_CLIENT_SECRET: "tilde-local-dev-secret",
      ENGINE_OIDC_ALLOW_HTTP: "true",
    },
    stdio: ["ignore", "pipe", "pipe"],
  },
);
let logs = "";
try {
  const url = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("startup timeout")), 30000);
    const read = (chunk) => {
      logs += chunk.toString();
      const match = logs.match(/management_address=([0-9.]+:\d+)/);
      if (match) {
        clearTimeout(timer);
        resolve(`http://${match[1]}`);
      }
    };
    child.stdout.on("data", read);
    child.stderr.on("data", read);
    child.on("exit", () => {
      clearTimeout(timer);
      reject(new Error(logs));
    });
  });
  const verifier = randomBytes(32).toString("base64url");
  const challenge = createHash("sha256").update(verifier).digest("base64url");
  const start = await fetch(`${url}/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ challenge }),
  });
  assert.equal(start.status, 200);
  const { authorization_url, state } = await start.json();
  let login = await fetch(authorization_url);
  let html = await login.text();
  const action = html.match(/<form[^>]+action="([^"]+)"/);
  assert(action, "Dex login form");
  const target = new URL(action[1].replaceAll("&amp;", "&"), login.url);
  let response = await fetch(target, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ login: "dev@tilde.local", password: "password" }),
    redirect: "manual",
  });
  for (let n = 0; n < 6; n++) {
    const location = response.headers.get("location");
    assert(location, `Dex redirect, status ${response.status}`);
    const next = new URL(location, target);
    if (next.pathname === "/auth/callback") {
      assert.equal(next.searchParams.get("state"), state);
      const exchange = await fetch(`${url}/auth/exchange`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ state, code: next.searchParams.get("code"), verifier }),
      });
      assert.equal(exchange.status, 200);
      assert.equal(exchange.headers.get("set-cookie"), null);
      const { access_token } = await exchange.json();
      const headers = { Authorization: `Bearer ${access_token}` };
      assert.equal((await fetch(`${url}/auth/session`, { headers })).status, 200);
      const runtime = `http://${logs.match(/agent_runtime_address=(127\.0\.0\.1:\d+)/)[1]}`;
      assert.equal(
        (
          await fetch(`${runtime}/tilde.management.v1.AgentService/ListAgents`, {
            method: "POST",
            headers: { ...headers, "Content-Type": "application/json" },
            body: "{}",
          })
        ).status,
        401,
      );
      assert.equal((await fetch(`${runtime}/auth/login`)).status, 401);
      assert.equal((await fetch(`${url}/auth/logout`, { method: "POST", headers })).status, 204);
      assert.equal((await fetch(`${url}/auth/session`, { headers })).status, 401);
      console.log(
        "PASS: seeded Dex login, PKCE exchange, bearer session, no Tilde cookies, logout revocation.",
      );
      break;
    }
    response = await fetch(next, { redirect: "manual" });
    if (n === 5) throw new Error("Too many Dex redirects");
  }
} finally {
  const exited = once(child, "exit");
  child.kill("SIGTERM");
  const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
  await exited;
  clearTimeout(timer);
}
