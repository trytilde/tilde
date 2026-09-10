import { createAgentServer } from "@trytilde/sdk";
import { setTimeout as delay } from "node:timers/promises";

const signingKey = process.env.AGENT_SIGNING_KEY;
if (!signingKey)
  throw new Error("AGENT_SIGNING_KEY must match the key supplied when registering this agent");
const server = createAgentServer({
  signingKey,
  async run(ctx) {
    // Register ctx.tools with your agent framework. Provider descriptions/schemas come from Tilde.
    // Ordinary return values and reasoning never become chat messages.
    await ctx.reason("I will record the work and send a streamed response.");
    const goal = await ctx.goals.create({ objective: ctx.objective });
    const task = await ctx.tasks.create({
      goalId: goal.id,
      title: "Respond to the thread",
    });
    await ctx.tasks.update({ id: task.id, status: "working" });
    await ctx.sendNativeMessage(
      (async function* () {
        for (const word of `Hello from the example agent. Your objective was: ${ctx.objective}`.split(
          " ",
        )) {
          for (const input of ctx.takeInputs())
            await ctx.reason(`Received steering: ${input.text}`);
          await delay(30, undefined, { signal: ctx.signal });
          yield `${word} `;
        }
      })(),
    );
    await ctx.tasks.update({ id: task.id, status: "completed" });
    await ctx.goals.update({ id: goal.id, status: "completed" });
    await ctx.setRunStatus("completed");
    ctx.stop();
  },
});
server.listen(Number(process.env.AGENT_PORT ?? 3001), "127.0.0.1", () =>
  console.log("Example ConnectRPC agent listening", server.address()),
);
for (const signal of ["SIGINT", "SIGTERM"] as const) process.once(signal, () => server.close());
