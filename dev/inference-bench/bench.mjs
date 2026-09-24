// Load generator: the same chat completion against several OpenAI-compatible base URLs.
// Reports time to first byte and total latency percentiles per target. Node's http module
// with keep-alive, so the client side is identical for every gateway.
import { request as httpRequest, Agent } from "node:http";
import { request as httpsRequest, Agent as HttpsAgent } from "node:https";

const args = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i === -1 ? fallback : args[i + 1];
};
const requests = Number(flag("requests", "500"));
const concurrency = Number(flag("concurrency", "16"));
const stream = flag("stream", "true") !== "false";
const warmup = Number(flag("warmup", "50"));
// --target name|url|model|authorization   (repeatable)
const targets = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--target") {
    const [name, url, model, authorization] = args[i + 1].split("|");
    targets.push({ name, url, model, authorization });
  }
}
if (!targets.length) {
  console.error(
    "usage: bench.mjs --target name|baseUrl|model|authorization [...] [--requests N] [--concurrency C] [--stream true|false]",
  );
  process.exit(2);
}
const agents = {
  "http:": new Agent({ keepAlive: true, maxSockets: 256 }),
  "https:": new HttpsAgent({ keepAlive: true, maxSockets: 256 }),
};

function once(target) {
  return new Promise((resolve, reject) => {
    const url = new URL(`${target.url.replace(/\/$/, "")}/chat/completions`);
    const body = JSON.stringify({
      model: target.model,
      stream,
      messages: [{ role: "user", content: "Say hello in five words." }],
    });
    const started = process.hrtime.bigint();
    let first = null;
    const req = (url.protocol === "https:" ? httpsRequest : httpRequest)(
      url,
      {
        method: "POST",
        agent: agents[url.protocol],
        headers: {
          "content-type": "application/json",
          "content-length": Buffer.byteLength(body),
          authorization: target.authorization,
        },
      },
      (res) => {
        res.on("data", () => {
          if (first === null) first = process.hrtime.bigint();
        });
        res.on("end", () => {
          const end = process.hrtime.bigint();
          if (res.statusCode !== 200)
            return reject(new Error(`${target.name}: HTTP ${res.statusCode}`));
          resolve({
            ttfb: Number((first ?? end) - started) / 1e6,
            total: Number(end - started) / 1e6,
          });
        });
      },
    );
    req.on("error", reject);
    req.end(body);
  });
}
async function run(target, count) {
  const samples = [];
  let failures = 0;
  let next = 0;
  const started = process.hrtime.bigint();
  await Promise.all(
    Array.from({ length: concurrency }, async () => {
      while (next < count) {
        next++;
        try {
          samples.push(await once(target));
        } catch (error) {
          failures++;
          if (failures === 1) console.error(String(error.message ?? error));
        }
      }
    }),
  );
  const elapsed = Number(process.hrtime.bigint() - started) / 1e9;
  return { samples, failures, elapsed };
}
const percentile = (values, p) => {
  if (!values.length) return NaN;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))];
};
const fmt = (n) => (Number.isNaN(n) ? "-" : n.toFixed(2).padStart(8));

console.log(`requests=${requests} concurrency=${concurrency} stream=${stream}`);
console.log(
  `${"target".padEnd(14)} ${"ttfb p50".padStart(8)} ${"ttfb p95".padStart(8)} ${"ttfb p99".padStart(8)} ${"total p50".padStart(9)} ${"total p95".padStart(9)} ${"total p99".padStart(9)} ${"rps".padStart(8)} fail`,
);
const results = {};
for (const target of targets) {
  await run(target, warmup);
  const { samples, failures, elapsed } = await run(target, requests);
  const ttfb = samples.map((s) => s.ttfb);
  const total = samples.map((s) => s.total);
  results[target.name] = {
    ttfb: { p50: percentile(ttfb, 50), p95: percentile(ttfb, 95), p99: percentile(ttfb, 99) },
    total: { p50: percentile(total, 50), p95: percentile(total, 95), p99: percentile(total, 99) },
    rps: samples.length / elapsed,
    failures,
  };
  const r = results[target.name];
  console.log(
    `${target.name.padEnd(14)} ${fmt(r.ttfb.p50)} ${fmt(r.ttfb.p95)} ${fmt(r.ttfb.p99)} ${fmt(r.total.p50).padStart(9)} ${fmt(r.total.p95).padStart(9)} ${fmt(r.total.p99).padStart(9)} ${fmt(r.rps)} ${failures}`,
  );
}
if (flag("json")) {
  const { writeFileSync } = await import("node:fs");
  writeFileSync(flag("json"), JSON.stringify({ requests, concurrency, stream, results }, null, 2));
}
