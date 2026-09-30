---
name: tilde-inference
description: Route model calls through the Tilde inference gateway with ctx.inference and declare prompts with ctx.prompt so every call is accounted and traceable to a prompt version.
---

# Inference through Tilde

Agents never hold provider API keys. A connection assigned to the agent is addressed by its
slug (`provider/account`, for example `openai/prod`) or an alias the operator gave it.

```ts
import { createOpenAI } from "@ai-sdk/openai";
const openai = createOpenAI(ctx.inference("openai/prod"));
```

`ctx.inference(slug)` returns `{ baseURL, apiKey, fetch }` for any provider SDK; the gateway
swaps in the real credential and records tokens, latency and cost per call.

## Prompts

Declare prompts in code so their versions are tracked:

```ts
const triage = ctx.prompt("triage", {
  template: "You triage support requests for {{product}}.\n{{> tone}}",
  sections: { tone: "Be direct and warm." },
  config: { model: "gpt-5", temperature: 0.2 },
});
const { text } = await generateText({
  model: openai(triage.config.model),
  system: triage.render({ product: "Tilde" }),
  ...
});
```

The SDK hashes the declared content and registers it; the engine keeps one version per
distinct content, so unchanged prompts never create a version. Calls made through `ctx.inference` after `render()` carry the prompt name and hash,
so the Prompts tab shows requests, tokens and cost per version. Use sections for text shared
by several prompts so a change to it is visible on each of them.
