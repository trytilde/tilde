import { createFileRoute } from "@tanstack/react-router";
import { parseSearch } from "@/features/tracing/search";
export const Route = createFileRoute("/_app/registry/agent/$agentId/tracing")({
  validateSearch: parseSearch,
});
