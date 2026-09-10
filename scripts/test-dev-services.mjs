// Exercise the actual dev launcher with stub processes, without starting local services.
import assert from "node:assert/strict";
import { mkdtemp, mkdir, copyFile, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { setTimeout as delay } from "node:timers/promises";
const root = await mkdtemp(join(tmpdir(), "tilde-dev-switches-"));
try {
  await mkdir(join(root, "scripts"));
  await mkdir(join(root, "bin"));
  await copyFile(new URL("../Taskfile.yml", import.meta.url), join(root, "Taskfile.yml"));
  const stub = `#!/usr/bin/env node
const fs=require('node:fs');const path=require('node:path');
const command=path.basename(process.argv[1]);const args=process.argv.slice(2);
fs.appendFileSync(process.env.SWITCH_LOG,JSON.stringify({command,args,publicUrl:process.env.ENGINE_MANAGEMENT_PUBLIC_URL,bind:process.env.ENGINE_MANAGEMENT_LISTEN,host:process.env.WEB_HOST,network:process.env.ENGINE_ALLOW_NETWORK,issuer:process.env.ENGINE_OIDC_ISSUER,agent:process.env.ENGINE_AGENT_RUNTIME_LISTEN,connectionUi:process.env.ENGINE_CONNECTION_UI_DEV_URL,setup:process.env.ENGINE_CONNECTION_SETUP_PUBLIC_URL,apiUpstream:process.env.ENGINE_DEV_URL,webCallback:process.env.DEX_DEV_WEB_CALLBACK,apiCallback:process.env.DEX_DEV_API_CALLBACK})+'\\n');
if(command==='docker'||args.includes('check-config')||(command==='pnpm' && !args.includes('vite')))process.exit(0);
if(command==='cargo' && process.env.FAIL_API==='true')setTimeout(()=>{console.error('API fixture failed');process.exit(7)},700);
if(command==='pnpm' && process.env.FAIL_API==='true')setTimeout(()=>{console.log('Vite streaming stdout');console.error('Vite streaming stderr')},25);
process.on('SIGTERM',()=>process.exit(0));setInterval(()=>{},1000);
`;
  for (const command of ["cargo", "pnpm", "docker"])
    await writeFile(join(root, "bin", command), stub, { mode: 0o755 });
  for (const address of [undefined, "100.64.12.34"])
    for (const management of [true, false])
      for (const web of [true, false]) {
        const log = join(root, `calls-${address}-${management}-${web}`);
        await writeFile(
          join(root, ".env"),
          `ENGINE_MANAGEMENT_ENABLED=${management}\nENGINE_WEB_ENABLED=${web}\nAPI_PORT=18080\nWEB_PORT=15173\nDATABASE_URL=postgres://engine:fixture@127.0.0.1:5432/engine\nPOSTGRES_PASSWORD=fixture\n`,
        );
        const env = {
          ...process.env,
          PATH: `${join(root, "bin")}:${process.env.PATH}`,
          SWITCH_LOG: log,
        };
        for (const key of [
          "DATABASE_URL",
          "POSTGRES_PASSWORD",
          "ENGINE_MANAGEMENT_ENABLED",
          "ENGINE_WEB_ENABLED",
          "ENGINE_MANAGEMENT_PUBLIC_URL",
          "ENGINE_OIDC_ISSUER",
          "API_PORT",
          "WEB_PORT",
          "ADDRESS",
          "WEB_HOST",
          "ENGINE_MANAGEMENT_LISTEN",
          "ENGINE_ALLOW_NETWORK",
          "ENGINE_DEV_URL",
          "ENGINE_CONNECTION_SETUP_PUBLIC_URL",
          "ENGINE_CONNECTION_UI_DEV_URL",
          "CONNECTION_UI_PORT",
        ])
          delete env[key];
        const child = spawn(
          "task",
          ["--dir", root, "dev", ...(address ? [`ADDRESS=${address}`] : [])],
          {
            env,
            detached: true,
            stdio: ["ignore", "pipe", "pipe"],
          },
        );
        let output = "";
        child.stderr.on("data", (chunk) => (output += chunk));
        try {
          let calls = [];
          for (let i = 0; i < 100; i++) {
            calls = (await readFile(log, "utf8").catch(() => ""))
              .trim()
              .split("\n")
              .filter(Boolean)
              .map((line) => JSON.parse(line));
            if (
              calls.filter((c) => c.command === "cargo").length === 2 &&
              (!web ||
                calls.some(
                  (c) => c.command === "pnpm" && c.args.join(" ") === "--dir web exec vite",
                ))
            )
              break;
            if (child.exitCode !== null) throw new Error(`dev exited: ${output}`);
            await delay(25);
          }
          assert.equal(calls.filter((c) => c.command === "cargo").length, 2);
          assert.equal(calls.filter((c) => c.command === "docker").length, management ? 2 : 1);
          assert.equal(
            calls.filter((c) => c.command === "pnpm" && c.args.join(" ") === "--dir web exec vite")
              .length,
            web ? 1 : 0,
          );
          assert(
            calls
              .filter(
                (c) =>
                  c.command === "cargo" ||
                  (c.command === "docker" && c.args.at(-1) !== "postgres") ||
                  c.args.join(" ") === "--dir web exec vite",
              )
              .every(
                (c) => c.publicUrl === `http://${address ?? "127.0.0.1"}:${web ? 15173 : 18080}`,
              ),
          );
          const postgres = calls.findIndex(
            (c) => c.command === "docker" && c.args.at(-1) === "postgres",
          );
          const apiIndex = calls.findIndex(
            (c) => c.command === "cargo" && !c.args.includes("check-config"),
          );
          assert(postgres >= 0 && postgres < apiIndex, "Postgres starts before the API");
          assert(calls[postgres].args.includes("--wait"), "Startup waits for Postgres readiness");
          const api = calls.find((c) => c.command === "cargo" && !c.args.includes("check-config"));
          const host = address ?? "127.0.0.1";
          assert.equal(api.bind, `${host}:18080`);
          assert.equal(api.host, host);
          assert.equal(api.network, address ? "true" : "false");
          assert.equal(api.issuer, `http://${host}:5556`);
          assert.equal(api.agent, "127.0.0.1:8081");
          assert.equal(api.connectionUi, `http://${host}:5174`);
          assert.equal(api.setup, `http://${host}:18080`);
          assert.equal(api.apiUpstream, `http://${host}:18080`);
          assert.equal(api.webCallback, `http://${host}:15173/auth/callback`);
          assert.equal(api.apiCallback, `http://${host}:18080/auth/callback`);
        } finally {
          const exited = once(child, "exit");
          process.kill(-requirePid(child.pid), "SIGINT");
          const timer = setTimeout(() => {
            try {
              process.kill(-requirePid(child.pid), "SIGKILL");
            } catch {}
          }, 5000);
          await exited;
          clearTimeout(timer);
        }
      }
  // Output streams while services are running; an API failure stops the sibling services.
  await writeFile(
    join(root, ".env"),
    "ENGINE_MANAGEMENT_ENABLED=false\nENGINE_WEB_ENABLED=true\nDATABASE_URL=postgres://engine:fixture@database.example:5432/engine\n",
  );
  const env = {
    ...process.env,
    PATH: `${join(root, "bin")}:${process.env.PATH}`,
    SWITCH_LOG: join(root, "failure"),
    FAIL_API: "true",
  };
  for (const key of ["DATABASE_URL", "ENGINE_MANAGEMENT_ENABLED", "ENGINE_WEB_ENABLED"])
    delete env[key];
  const failed = spawn("task", ["--dir", root, "dev"], {
    env,
    detached: true,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "",
    stderr = "";
  failed.stdout.on("data", (chunk) => (stdout += chunk));
  failed.stderr.on("data", (chunk) => (stderr += chunk));
  const ended = once(failed, "exit");
  const timer = setTimeout(() => {
    try {
      process.kill(-requirePid(failed.pid), "SIGKILL");
    } catch {}
  }, 5000);
  try {
    for (let i = 0; i < 100; i++) {
      if (stdout.includes("Vite streaming stdout") && stderr.includes("Vite streaming stderr"))
        break;
      if (failed.exitCode !== null) throw new Error(`Dev exited before live output: ${stderr}`);
      await delay(25);
    }
    assert.equal(failed.exitCode, null, "Output arrives before the process exits");
    assert(stdout.includes("Vite streaming stdout"));
    assert(stderr.includes("Vite streaming stderr"));
    const [code, signal] = await ended;
    assert.equal(signal, null, "Task stops itself on service failure");
    assert.notEqual(code, 0);
    assert(stderr.includes("API fixture failed"));
    const failureCalls = (await readFile(env.SWITCH_LOG, "utf8"))
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    assert(
      !failureCalls.some((c) => c.command === "docker"),
      "External databases are not started or changed",
    );
  } finally {
    clearTimeout(timer);
    try {
      process.kill(-requirePid(failed.pid), "SIGKILL");
    } catch {}
  }
  console.log(
    "PASS: .env switches control dev Vite/Dex startup and public URL defaults in all four modes; live output and fail-fast shutdown are preserved.",
  );
} finally {
  await rm(root, { recursive: true, force: true });
}

function requirePid(pid) {
  if (typeof pid !== "number") throw new Error("Child process did not start");
  return pid;
}
