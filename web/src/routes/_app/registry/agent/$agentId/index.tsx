import { createFileRoute, Navigate } from "@tanstack/react-router";

export const Route = createFileRoute("/_app/registry/agent/$agentId/")({ component: AgentIndex });

function AgentIndex() {
  const { agentId } = Route.useParams();
  return <Navigate to="/registry/agent/$agentId/capabilities" params={{ agentId }} replace />;
}
