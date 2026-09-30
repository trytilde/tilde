import { createFileRoute } from "@tanstack/react-router";
export const Route = createFileRoute("/_app/agent/$agentId/prompts")({
  // `prompt` opens one prompt's history, so deployments can link to it.
  validateSearch: (raw: Record<string, unknown>): { prompt?: string } =>
    typeof raw.prompt === "string" && raw.prompt ? { prompt: raw.prompt } : {},
});
