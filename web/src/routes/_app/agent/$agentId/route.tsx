import { useDashboardBreadcrumbs } from "@/components/dashboard-breadcrumbs";
import { useEffect, useState, type JSX } from "react";
import { Code, ConnectError } from "@connectrpc/connect";
import { createFileRoute, Outlet, useRouterState } from "@tanstack/react-router";
import { AgentEditor, type AgentTab } from "@/components/agent-editor";
import { Button } from "@/components/ui/button";
import { agents } from "@/client";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";

export const Route = createFileRoute("/_app/agent/$agentId")({ component: AgentPage });

const tabPaths = {
  capabilities: "/agent/$agentId/capabilities",
  "chat-providers": "/agent/$agentId/chat-providers",
  iam: "/agent/$agentId/iam",
  tracing: "/agent/$agentId/tracing",
  logs: "/agent/$agentId/logs",
  deployment: "/agent/$agentId/deployment",
} as const;
function AgentPage(): JSX.Element {
  const { agentId } = Route.useParams();
  const tab: AgentTab = useRouterState({
    select: (state) =>
      state.matches.some((match) => match.routeId === "/_app/agent/$agentId/logs")
        ? "logs"
        : state.matches.some((match) => match.routeId === "/_app/agent/$agentId/tracing")
          ? "tracing"
          : state.matches.some((match) => match.routeId === "/_app/agent/$agentId/deployment")
            ? "deployment"
            : state.matches.some((match) => match.routeId === "/_app/agent/$agentId/iam")
              ? "iam"
              : state.matches.some(
                    (match) => match.routeId === "/_app/agent/$agentId/chat-providers",
                  )
                ? "chat-providers"
                : "capabilities",
  });
  const navigate = Route.useNavigate();
  // The shared route remains mounted while child routes select the editor's tab.
  return (
    <>
      <AgentDetails
        key={agentId}
        agentId={agentId}
        tab={tab}
        onTabChange={(next) => {
          void navigate({ to: tabPaths[next], params: { agentId } });
        }}
        onClose={() => {
          void navigate({ to: "/" });
        }}
      />
      <Outlet />
    </>
  );
}
function AgentDetails({
  agentId,
  tab,
  onTabChange,
  onClose,
}: {
  agentId: string;
  tab: AgentTab;
  onTabChange: (tab: AgentTab) => void;
  onClose: () => void;
}) {
  const [agent, setAgent] = useState<Agent>();
  const { setAgent: setBreadcrumbAgent } = useDashboardBreadcrumbs();
  useEffect(() => {
    if (agent)
      setBreadcrumbAgent((current) =>
        current?.id === agent.id && current.name === agent.name
          ? current
          : { id: agent.id, name: agent.name },
      );
  }, [agent?.id, agent?.name, setBreadcrumbAgent]);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  // Fetch after Auth mounts its children. Router loaders would run before that session check.
  useEffect(() => {
    const abort = new AbortController();
    setError("");
    void agents
      .getAgent({ id: agentId }, { signal: abort.signal })
      .then((response) => {
        if (!abort.signal.aborted) {
          if (!response.agent) setError("Agent not found.");
          else setAgent(response.agent);
        }
      })
      .catch((error) => {
        if (!abort.signal.aborted)
          setError(
            error instanceof ConnectError && error.code === Code.NotFound
              ? "Agent not found."
              : error instanceof Error
                ? error.message
                : "Unable to load agent.",
          );
      });
    return () => abort.abort();
  }, [agentId, attempt]);
  if (error)
    return (
      <section className="space-y-4 p-6">
        <p role="alert">{error}</p>
        <div className="flex gap-2">
          <Button onClick={() => setAttempt((value) => value + 1)}>Retry</Button>
        </div>
      </section>
    );
  if (!agent)
    return (
      <p role="status" className="p-6 text-muted-foreground">
        Loading agent…
      </p>
    );
  return (
    <AgentEditor
      agent={agent}
      tab={tab}
      onTabChange={onTabChange}
      onClose={onClose}
      onSaved={onClose}
      onNameSaved={(name) => setAgent((current) => (current ? { ...current, name } : current))}
    />
  );
}
