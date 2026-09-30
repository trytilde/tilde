import { createFileRoute } from "@tanstack/react-router";
import { AgentRegistry, parseRegistrySearch } from "@/components/agent-registry";

export const Route = createFileRoute("/_app/")({
  validateSearch: parseRegistrySearch,
  component: RegistryPage,
});

function RegistryPage() {
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return (
    <AgentRegistry
      search={search}
      onSearch={(patch) => {
        void navigate({ search: (previous) => ({ ...previous, ...patch }), replace: true });
      }}
      onOpen={(agent) => {
        void navigate({ to: "/agent/$agentId/capabilities", params: { agentId: agent.id } });
      }}
      onCreate={() => {
        void navigate({ to: "/agent/new" });
      }}
    />
  );
}
