import { createFileRoute } from "@tanstack/react-router";
import { AgentEditor } from "@/components/agent-editor";

export const Route = createFileRoute("/_app/registry/agent/new")({ component: NewAgentPage });

function NewAgentPage() {
  const navigate = Route.useNavigate();
  const back = () => {
    void navigate({ to: "/registry" });
  };
  return <AgentEditor agent={null} onClose={back} onSaved={back} />;
}
