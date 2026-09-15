import { createAgentServer } from "@trytilde/sdk";
import { createOpenAI } from "@ai-sdk/openai";
import { respond } from "./agent.js";

const apiKey = process.env.OPENAI_API_KEY;
const signingKey = process.env.AGENT_SIGNING_KEY;
if (!apiKey || !signingKey) throw new Error("OpenAI and agent signing keys are required");

const openai = createOpenAI({ apiKey });
const model = openai.responses(process.env.OPENAI_MODEL ?? "gpt-4o-mini");
const server = createAgentServer({
  signingKey,
  run: (ctx) => respond(ctx, model),
});

server.listen(Number(process.env.DEV_AGENT_PORT ?? 3001), "127.0.0.1", () => {
  console.log("Example Agent 1 is ready on", server.address());
});
for (const signal of ["SIGINT", "SIGTERM"] as const) process.once(signal, () => server.close());
