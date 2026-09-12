// Packaged startup matrix: route groups and embedded React serving are independent on one port.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { once } from "node:events";
import { resolve } from "node:path";
import { startOidc, loginManagement } from "./test-oidc.mjs";
assert(process.env.TEST_DATABASE_URL, "TEST_DATABASE_URL required");
const oidc = await startOidc();
const seed = randomBytes(32).toString("base64");
try {
  for (const management of [true, false])
    for (const web of [true, false]) {
      const env = {
        ...process.env,
        DATABASE_URL: process.env.TEST_DATABASE_URL,
        ENGINE_ENCRYPTION_BACKEND: "seed",
        ENGINE_ENCRYPTION_KEY: seed,
        ENGINE_KMS_KEY_ID: "",
        ENGINE_SERVE: management ? "all" : "ingress,runtime,sidecar",
        ENGINE_WEB_ENABLED: String(web),
        ENGINE_INGRESS_PUBLIC_URL: "https://events.example.com",
        RUST_LOG: "tilde=info",
      };
      for (const name of [
        "ENGINE_PUBLIC_URL",
        "ENGINE_RUNTIME_PUBLIC_URL",
        "ENGINE_OIDC_ISSUER",
        "ENGINE_OIDC_CLIENT_ID",
        "ENGINE_OIDC_CLIENT_SECRET",
      ])
        delete env[name];
      if (management) Object.assign(env, oidc.env);
      const child = spawn(
        resolve(process.env.ENGINE_TEST_BINARY ?? "target/debug/tilde"),
        ["--listen", "127.0.0.1:0"],
        { env, stdio: ["ignore", "pipe", "pipe"] },
      );
      let logs = "";
      try {
        await new Promise((resolve, reject) => {
          const timer = setTimeout(() => reject(new Error("startup timeout")), 20000);
          const read = (chunk) => {
            logs += chunk;
            if (logs.includes("tilde listening")) {
              clearTimeout(timer);
              resolve();
            }
          };
          child.stdout.on("data", read);
          child.stderr.on("data", read);
          child.once("exit", () => {
            clearTimeout(timer);
            reject(new Error(logs));
          });
        });
        assert(
          logs.includes(
            `serve=${management ? "management,ingress,runtime,sidecar" : "ingress,runtime,sidecar"}`,
          ),
        );
        const origin = `http://${logs.match(/ address=(127\.0\.0\.1:\d+)/)[1]}`;
        // Agent runtime routes require an invocation token; management routes exist only when served.
        assert.equal(
          (
            await fetch(`${origin}/tilde.runtime.v1.ChatService/ListGoals`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: "{}",
            })
          ).status,
          401,
        );
        assert.equal(
          (
            await fetch(`${origin}/tilde.management.v1.AgentService/ListAgents`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: "{}",
            })
          ).status,
          management ? 401 : 404,
        );
        assert.equal(
          (
            await fetch(`${origin}/connections/webhooks/00000000-0000-0000-0000-000000000001`, {
              method: "POST",
              headers: { Host: "events.example.com" },
              body: "{}",
            })
          ).status,
          400,
          "Webhook reaches signature/connection validation on the shared port",
        );
        assert.equal(
          (await fetch(`${origin}/`, { headers: { Accept: "text/html" } })).status,
          web ? 200 : 404,
        );
        if (management) await loginManagement(origin);
        else
          assert.equal(
            (
              await fetch(`${origin}/auth/login`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: "{}",
              })
            ).status,
            404,
          );
      } finally {
        if (child.exitCode === null) {
          const ended = once(child, "exit");
          child.kill("SIGTERM");
          const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
          await ended;
          clearTimeout(timer);
        }
      }
    }
  console.log(
    "PASS: all four management/web modes on one listener; ingress and runtime stay active, management routes exist only when served.",
  );
} finally {
  await oidc.stop();
}
