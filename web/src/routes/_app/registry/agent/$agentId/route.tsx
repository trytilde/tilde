import { useDashboardBreadcrumbs } from "@/components/dashboard-breadcrumbs";
import { useEffect, useState, type JSX } from "react";
import { Code, ConnectError } from "@connectrpc/connect";
import { createFileRoute, Link, Outlet, useRouterState } from "@tanstack/react-router";
import { AgentEditor, type AgentTab } from "@/components/agent-editor";
import { Button } from "@/components/ui/button";
import { agents } from "@/client";
import type { Agent } from "@trytilde/contracts/tilde/types/v1/agent_pb.js";
import { TildeLoader } from "@/components/loading-screen";

const NOT_FOUND = "Agent not found.";

export const Route = createFileRoute("/_app/registry/agent/$agentId")({ component: AgentPage });

const tabPaths = {
  capabilities: "/registry/agent/$agentId/capabilities",
  "chat-providers": "/registry/agent/$agentId/chat-providers",
  tools: "/registry/agent/$agentId/tools",
  inference: "/registry/agent/$agentId/inference",
  iam: "/registry/agent/$agentId/iam",
  sessions: "/registry/agent/$agentId/sessions",
  tracing: "/registry/agent/$agentId/tracing",
  logs: "/registry/agent/$agentId/logs",
  deployment: "/registry/agent/$agentId/deployment",
  prompts: "/registry/agent/$agentId/prompts",
  skills: "/registry/agent/$agentId/skills",
  routines: "/registry/agent/$agentId/routines",
} as const;
function AgentPage(): JSX.Element {
  const { agentId } = Route.useParams();
  const tab: AgentTab = useRouterState({
    select: (state) =>
      state.matches.some((match) => match.routeId === "/_app/registry/agent/$agentId/sessions")
        ? "sessions"
        : state.matches.some((match) => match.routeId === "/_app/registry/agent/$agentId/prompts")
          ? "prompts"
          : state.matches.some((match) => match.routeId === "/_app/registry/agent/$agentId/skills")
            ? "skills"
            : state.matches.some(
                  (match) => match.routeId === "/_app/registry/agent/$agentId/routines",
                )
              ? "routines"
              : state.matches.some(
                    (match) => match.routeId === "/_app/registry/agent/$agentId/logs",
                  )
                ? "logs"
                : state.matches.some(
                      (match) => match.routeId === "/_app/registry/agent/$agentId/tracing",
                    )
                  ? "tracing"
                  : state.matches.some(
                        (match) => match.routeId === "/_app/registry/agent/$agentId/deployment",
                      )
                    ? "deployment"
                    : state.matches.some(
                          (match) => match.routeId === "/_app/registry/agent/$agentId/iam",
                        )
                      ? "iam"
                      : state.matches.some(
                            (match) =>
                              match.routeId === "/_app/registry/agent/$agentId/chat-providers",
                          )
                        ? "chat-providers"
                        : state.matches.some(
                              (match) => match.routeId === "/_app/registry/agent/$agentId/tools",
                            )
                          ? "tools"
                          : state.matches.some(
                                (match) =>
                                  match.routeId === "/_app/registry/agent/$agentId/inference",
                              )
                            ? "inference"
                            : "capabilities",
  });
  // An open prompt (`?prompt=`) takes the whole page, like the skill editor.
  const promptOpen = useRouterState({
    select: (state) => !!(state.location.search as { prompt?: string }).prompt,
  });
  const navigate = Route.useNavigate();
  // The shared route remains mounted while child routes select the editor's tab.
  return (
    <>
      <AgentDetails
        key={agentId}
        agentId={agentId}
        tab={tab}
        promptOpen={promptOpen}
        onTabChange={(next) => {
          void navigate({ to: tabPaths[next], params: { agentId } });
        }}
        onClose={() => {
          void navigate({ to: "/registry" });
        }}
      />
      <Outlet />
    </>
  );
}
function AgentDetails({
  agentId,
  tab,
  promptOpen,
  onTabChange,
  onClose,
}: {
  agentId: string;
  tab: AgentTab;
  promptOpen: boolean;
  onTabChange: (tab: AgentTab) => void;
  onClose: () => void;
}) {
  const [agent, setAgent] = useState<Agent>();
  const { setAgent: setBreadcrumbAgent } = useDashboardBreadcrumbs();
  useEffect(() => {
    if (agent)
      setBreadcrumbAgent((current) =>
        current?.id === agent.id &&
        current.name === agent.name &&
        current.avatarSeed === agent.avatarSeed &&
        current.avatarUrl === agent.avatarUrl
          ? current
          : {
              id: agent.id,
              name: agent.name,
              avatarSeed: agent.avatarSeed,
              avatarUrl: agent.avatarUrl,
            },
      );
  }, [agent?.id, agent?.name, agent?.avatarSeed, agent?.avatarUrl, setBreadcrumbAgent]);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const abort = new AbortController();
    setError("");
    void agents
      .getAgent({ id: agentId }, { signal: abort.signal })
      .then((response) => {
        if (!abort.signal.aborted) {
          if (!response.agent) setError(NOT_FOUND);
          else setAgent(response.agent);
        }
      })
      .catch((error) => {
        if (!abort.signal.aborted)
          setError(
            error instanceof ConnectError && error.code === Code.NotFound
              ? NOT_FOUND
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
          {error === NOT_FOUND ? (
            <Button variant="outline" nativeButton={false} render={<Link to="/registry" />}>
              Back to Agent Registry
            </Button>
          ) : (
            <Button onClick={() => setAttempt((value) => value + 1)}>Retry</Button>
          )}
        </div>
      </section>
    );
  if (!agent) return <TildeLoader className="flex-1" />;
  return (
    <AgentEditor
      agent={agent}
      tab={tab}
      promptOpen={promptOpen}
      onTabChange={onTabChange}
      onClose={onClose}
      onSaved={onClose}
      onNameSaved={(name) => setAgent((current) => (current ? { ...current, name } : current))}
      onAvatarSaved={(avatarUrl) =>
        setAgent((current) => (current ? { ...current, avatarUrl } : current))
      }
      onPausedChange={(paused) => {
        void (paused ? agents.pauseAgent({ id: agentId }) : agents.resumeAgent({ id: agentId }))
          .then(() => setAgent((current) => (current ? { ...current, paused } : current)))
          .catch((cause) =>
            setError(cause instanceof Error ? cause.message : "Unable to change agent state."),
          );
      }}
    />
  );
}
