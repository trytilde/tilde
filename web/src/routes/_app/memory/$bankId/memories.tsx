import { createFileRoute } from "@tanstack/react-router";

// The parent page owns tab state; `memory` opens one memory's panel.
export const Route = createFileRoute("/_app/memory/$bankId/memories")({
  validateSearch: (search: Record<string, unknown>): { memory?: string } => ({
    memory: typeof search.memory === "string" && search.memory ? search.memory : undefined,
  }),
});
