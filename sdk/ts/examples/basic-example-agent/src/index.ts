import { connectAgent } from "@trytilde/sdk";
import { setTimeout as delay } from "node:timers/promises";

// Dials out with TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN; Tilde never calls this process.
const connection = connectAgent({
  onRegistered: (registration) =>
    console.log("Example agent connected as deployment", registration.deploymentId),
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
for (const signal of ["SIGINT", "SIGTERM"] as const)
  process.once(signal, () => void connection.close());
