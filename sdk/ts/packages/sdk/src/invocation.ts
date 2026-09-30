import { AsyncLocalStorage } from "node:async_hooks";
import type { PromptStamp } from "./prompts.js";

/**
 * Marks objects `tilde deploy` collects from an entry module's exports. A registered symbol,
 * so declarations made through another copy of the SDK are still recognised.
 */
export const declaration = Symbol.for("tilde.declaration");

export type InferenceClient = { baseURL: string; apiKey: string; fetch: typeof fetch };
/** What module-level helpers need from the running invocation (an `AgentContext`). */
export type Invocation = {
  activatePrompt(prompt: PromptStamp): void;
  inference(slug: string): InferenceClient;
};
const store = new AsyncLocalStorage<Invocation>();
/** The invocation whose `run(ctx)` is executing, if any. */
export const currentInvocation = (): Invocation | undefined => store.getStore();
/**
 * Run `fn` as part of `context`'s invocation, so module-level `inference()` and prompt renders
 * resolve to it. Hosts do this around `run(ctx)`; tests and custom hosts may call it directly.
 */
export function runWithContext<T>(context: Invocation, fn: () => T): T {
  return store.run(context, fn);
}

export const validInferenceSlug = (slug: string) =>
  /^([a-z0-9_]+\/)?[A-Za-z0-9._@+:-]+$/.test(slug);

/**
 * `ctx.inference(slug)` for code that builds its model client at module scope, such as a
 * framework agent: `createOpenAI(inference("openai/prod"))`. The base URL is a placeholder;
 * each request is sent through the invocation running when it is made, with that
 * invocation's token and prompt stamps. A request made outside an invocation fails.
 */
export function inference(slug: string): InferenceClient {
  if (!validInferenceSlug(slug))
    throw new Error("Inference slug must be provider/account or an alias");
  const baseURL = `https://inference.tilde.invalid/${slug}`;
  return {
    baseURL,
    apiKey: "tilde",
    fetch: async (input, init) => {
      const invocation = currentInvocation();
      if (!invocation)
        throw new Error(
          `inference("${slug}") was used outside a Tilde invocation; call the model from run(ctx)`,
        );
      const target = invocation.inference(slug);
      const url = input instanceof Request ? input.url : String(input);
      const routed = url.startsWith(baseURL) ? target.baseURL + url.slice(baseURL.length) : url;
      return target.fetch(input instanceof Request ? new Request(routed, input) : routed, init);
    },
  };
}
