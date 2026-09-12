import { createFileRoute } from "@tanstack/react-router";
import { parseLogSearch } from "@/features/logs/search";
export const Route = createFileRoute("/_app/agent/$agentId/logs")({
  validateSearch: parseLogSearch,
});
