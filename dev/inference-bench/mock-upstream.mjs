// OpenAI-compatible stand-in provider shared by every gateway under test, so measured
// differences are proxy overhead only. Streams `--chunks` SSE frames with `--delay-ms`
// between them; without `stream` it answers one JSON body. Usage is always present.
import { createServer } from "node:http";

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  return i === -1 ? fallback : process.argv[i + 1];
};
const port = Number(arg("port", "18300"));
const chunks = Number(arg("chunks", "20"));
const delay = Number(arg("delay-ms", "0"));
const sleep = (ms) => (ms > 0 ? new Promise((r) => setTimeout(r, ms)) : Promise.resolve());
const usage = { prompt_tokens: 42, completion_tokens: chunks, total_tokens: 42 + chunks };

const server = createServer((req, res) => {
  let body = "";
  req.on("data", (d) => (body += d));
  req.on("end", async () => {
    if (!req.url.endsWith("/chat/completions") || req.method !== "POST") {
      res.writeHead(404, { "content-type": "application/json" });
      return res.end('{"error":{"message":"not found"}}');
    }
    let request = {};
    try {
      request = JSON.parse(body);
    } catch {}
    const model = request.model ?? "mock";
    if (request.stream) {
      res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache" });
      for (let i = 0; i < chunks; i++) {
        res.write(
          `data: ${JSON.stringify({ id: "mock", object: "chat.completion.chunk", model, choices: [{ index: 0, delta: { content: "tok " } }] })}\n\n`,
        );
        await sleep(delay);
      }
      res.write(
        `data: ${JSON.stringify({ id: "mock", object: "chat.completion.chunk", model, choices: [], usage })}\n\n`,
      );
      res.end("data: [DONE]\n\n");
    } else {
      await sleep(delay);
      res.writeHead(200, { "content-type": "application/json" });
      res.end(
        JSON.stringify({
          id: "mock",
          object: "chat.completion",
          model,
          choices: [
            {
              index: 0,
              message: { role: "assistant", content: "tok ".repeat(chunks) },
              finish_reason: "stop",
            },
          ],
          usage,
        }),
      );
    }
  });
});
server.keepAliveTimeout = 60_000;
server.listen(port, "0.0.0.0", () =>
  console.log(`mock upstream on :${port} chunks=${chunks} delay=${delay}ms`),
);
