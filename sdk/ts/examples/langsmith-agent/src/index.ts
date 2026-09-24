import { connectAgent } from "@trytilde/sdk";
import { defaultPrompt, runRecipe } from "./recipe.js";

for (const name of ["OPENAI_API_KEY", "LANGSMITH_API_KEY"])
  if (!process.env[name]) throw new Error(`${name} is required`);
process.env.LANGSMITH_TRACING = "true";
process.env.LANGSMITH_ENDPOINT ??= "https://api.smith.langchain.com";
process.env.LANGSMITH_PROJECT ??= "test";

if (process.argv.includes("--serve")) {
  // Dials out with TILDE_GATEWAY_URL and TILDE_DEPLOYMENT_TOKEN.
  const connection = connectAgent({
    onRegistered: (registration) =>
      console.log("LangSmith demo agent connected as deployment", registration.deploymentId),
    async run(ctx) {
      await runRecipe(
        ctx.objective || defaultPrompt,
        (chunks) => ctx.sendNativeMessage(chunks),
        ctx.signal,
      );
    },
  });
  for (const signal of ["SIGINT", "SIGTERM"] as const)
    process.once(signal, () => void connection.close());
} else {
  try {
    await runRecipe(
      process.env.AGENT_PROMPT ?? defaultPrompt,
      async (chunks) => {
        for await (const chunk of chunks) process.stdout.write(chunk);
      },
      AbortSignal.timeout(120_000),
    );
  } catch {
    console.error(
      "Demo failed. Check OpenAI/LangSmith access and the test project; credentials are not logged.",
    );
    process.exitCode = 1;
  }
}
