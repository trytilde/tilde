// Exercise the actual dev launcher with stub processes, without starting local services.
import assert from "node:assert/strict";
import { mkdtemp, mkdir, copyFile, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { setTimeout as delay } from "node:timers/promises";
const root = await mkdtemp(join(tmpdir(), "tilde-dev-switches-"));
try {
  await mkdir(join(root, "scripts"));
  await mkdir(join(root, "bin"));
  await copyFile(new URL("../Taskfile.yml", import.meta.url), join(root, "Taskfile.yml"));
  await copyFile(new URL("./dev-ngrok.sh", import.meta.url), join(root, "scripts/dev-ngrok.sh"));
  await copyFile(new URL("./dev-api.sh", import.meta.url), join(root, "scripts/dev-api.sh"));
  const stub = `#!/usr/bin/env node
const fs=require('node:fs');const path=require('node:path');
const command=path.basename(process.argv[1]);const args=process.argv.slice(2);
fs.appendFileSync(process.env.SWITCH_LOG,JSON.stringify({command,args,publicUrl:process.env.ENGINE_MANAGEMENT_PUBLIC_URL,bind:process.env.ENGINE_MANAGEMENT_LISTEN,host:process.env.WEB_HOST,network:process.env.ENGINE_ALLOW_NETWORK,issuer:process.env.ENGINE_OIDC_ISSUER,agent:process.env.ENGINE_AGENT_RUNTIME_LISTEN,ingress:process.env.ENGINE_EVENT_INGRESS_LISTEN,ingressUrl:process.env.ENGINE_EVENT_INGRESS_PUBLIC_URL,connectionUi:process.env.ENGINE_CONNECTION_UI_DEV_URL,setup:process.env.ENGINE_CONNECTION_SETUP_PUBLIC_URL,apiUpstream:process.env.ENGINE_DEV_URL,webCallback:process.env.DEX_DEV_WEB_CALLBACK,apiCallback:process.env.DEX_DEV_API_CALLBACK})+'\\n');
if(command==='docker'||args.includes('check-config')||(command==='pnpm' && !args.includes('vite')))process.exit(0);
if(command==='ngrok' && process.env.FAIL_NGROK==='true')setTimeout(()=>{console.error('ngrok fixture failed');process.exit(8)},700);
if(command==='cargo' && process.env.FAIL_API==='true')setTimeout(()=>{console.error('API fixture failed');process.exit(7)},700);
if(command==='pnpm' && (process.env.FAIL_API==='true'||process.env.FAIL_NGROK==='true'))setTimeout(()=>{console.log('Vite streaming stdout');console.error('Vite streaming stderr')},25);
process.on('SIGTERM',()=>process.exit(0));setInterval(()=>{},1000);
`;
  for (const command of ["cargo", "pnpm", "docker", "ngrok"])
    await writeFile(join(root, "bin", command), stub, { mode: 0o755 });
  for (const ngrok of [false, true])
    for (const address of [undefined, "100.64.12.34"])
      for (const management of [true, false])
        for (const web of [true, false]) {
          const log = join(root, `calls-${ngrok}-${address}-${management}-${web}`);
          await writeFile(
            join(root, ".env"),
            `ENGINE_EVENT_INGRESS_PUBLIC_URL=https://explicit-ingress.example\nNGROK_ENABLED=false\nNGROK_DOMAIN=fixture.ngrok.app\nNGROK_AUTHTOKEN=fixture-token\nINGRESS_PORT=18082\nENGINE_MANAGEMENT_ENABLED=${management}\nENGINE_WEB_ENABLED=${web}\nAPI_PORT=18080\nWEB_PORT=15173\nDATABASE_URL=postgres://engine:fixture@127.0.0.1:5432/engine\nPOSTGRES_PASSWORD=fixture\n`,
          );
          const env = {
            ...process.env,
            PATH: `${join(root, "bin")}:${process.env.PATH}`,
            SWITCH_LOG: log,
          };
          for (const key of [
            "NGROK_ENABLED",
            "NGROK_DOMAIN",
            "NGROK_AUTHTOKEN",
            "INGRESS_PORT",
            "ENGINE_EVENT_INGRESS_LISTEN",
            "ENGINE_EVENT_INGRESS_PUBLIC_URL",
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
          // Explicit process environment must also yield to the enabled tunnel URL.
          if (address) env.ENGINE_EVENT_INGRESS_PUBLIC_URL = "https://explicit-ingress.example";
          const child = spawn(
            "task",
            [
              "--dir",
              root,
              "dev",
              ...(ngrok ? ["NGROK_ENABLED=true"] : []),
              ...(address ? [`ADDRESS=${address}`] : []),
            ],
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
                (!ngrok || calls.some((c) => c.command === "ngrok")) &&
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
              calls.filter(
                (c) => c.command === "pnpm" && c.args.join(" ") === "--dir web exec vite",
              ).length,
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
            const api = calls.find(
              (c) => c.command === "cargo" && !c.args.includes("check-config"),
            );
            const host = address ?? "127.0.0.1";
            assert.equal(api.ingress, `${host}:18082`);
            assert.equal(
              api.ingressUrl,
              ngrok ? "https://fixture.ngrok.app" : "https://explicit-ingress.example",
            );
            const tunnel = calls.filter((c) => c.command === "ngrok");
            assert.equal(tunnel.length, ngrok ? 1 : 0);
            if (ngrok)
              assert.deepEqual(tunnel[0].args, [
                "http",
                `http://${host}:18082`,
                "--url",
                "https://fixture.ngrok.app",
                "--log",
                "stdout",
              ]);
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
            if (child.exitCode === null) {
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
        }
  // Invalid tunnel configuration stops before any service or build is launched.
  for (const overrides of [
    { NGROK_AUTHTOKEN: "" },
    { NGROK_DOMAIN: "https://invalid.example/path" },
    { NGROK_ENABLED: "invalid" },
  ]) {
    const check = spawnSync("bash", ["scripts/dev-ngrok.sh", "--check"], {
      cwd: root,
      env: {
        ...process.env,
        PATH: `${join(root, "bin")}:${process.env.PATH}`,
        NGROK_ENABLED: "true",
        NGROK_DOMAIN: "fixture.ngrok.app",
        NGROK_AUTHTOKEN: "never-print-this-token",
        ENGINE_EVENT_INGRESS_PUBLIC_URL: "https://fixture.ngrok.app",
        ...overrides,
      },
      encoding: "utf8",
    });
    assert.notEqual(check.status, 0);
    assert(!check.stderr.includes("never-print-this-token"));
  }
  for (const failureSource of ["api", "ngrok"]) {
    // Output streams while services are running; an API failure stops the sibling services.
    await writeFile(
      join(root, ".env"),
      "ENGINE_MANAGEMENT_ENABLED=false\nENGINE_WEB_ENABLED=true\nDATABASE_URL=postgres://engine:fixture@database.example:5432/engine\n",
    );
    const env = {
      ...process.env,
      PATH: `${join(root, "bin")}:${process.env.PATH}`,
      SWITCH_LOG: join(root, `failure-${failureSource}`),
      FAIL_API: String(failureSource === "api"),
      FAIL_NGROK: String(failureSource === "ngrok"),
      NGROK_ENABLED: "true",
      NGROK_DOMAIN: "fixture.ngrok.app",
      NGROK_AUTHTOKEN: "fixture-token",
      ENGINE_EVENT_INGRESS_PUBLIC_URL: "https://fixture.ngrok.app",
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
      assert(
        stderr.includes(failureSource === "api" ? "API fixture failed" : "ngrok fixture failed"),
      );
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
  }
  console.log(
    "PASS: dev switches and ngrok route only to ingress in all modes; validation, live output and fail-fast shutdown pass.",
  );
} finally {
  await rm(root, { recursive: true, force: true });
}

function requirePid(pid) {
  if (typeof pid !== "number") throw new Error("Child process did not start");
  return pid;
}
