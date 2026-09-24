import { createFileRoute, Navigate } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/agent/$agentId/")({ component: AgentIndex });

function AgentIndex() {
  const { agentId } = Route.useParams();
  return <Navigate to="/agent/$agentId/capabilities" params={{ agentId }} replace />;
}
