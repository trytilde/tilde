import { createFileRoute } from "@tanstack/react-router";
import { AgentRegistry } from "@/components/agent-registry";

export const Route = createFileRoute("/_app/")({ component: RegistryPage });

function RegistryPage() {
  const navigate = Route.useNavigate();
  return (
    <AgentRegistry
      onOpen={(agent) => {
        void navigate({ to: "/agent/$agentId/capabilities", params: { agentId: agent.id } });
      }}
      onCreate={() => {
        void navigate({ to: "/agent/new" });
      }}
    />
  );
}
