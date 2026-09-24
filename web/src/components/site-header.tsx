import { AgentAvatar } from "./agent-avatar";
import { useEffect, useState } from "react";
import { Link, useParams, useRouterState } from "@tanstack/react-router";
import { iam } from "@/client";
import { Separator } from "@/components/ui/separator";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { useDashboardBreadcrumbs } from "./dashboard-breadcrumbs";

/** Top-level sections by path prefix; anything under them gets a second crumb. */
const sections: { prefix: string; label: string; to: "/" | "/api-keys" | "/groups" }[] = [
  { prefix: "/api-keys", label: "API keys", to: "/api-keys" },
  { prefix: "/groups", label: "Groups", to: "/groups" },
];
export function SiteHeader() {
  const { agentId, groupId } = useParams({ strict: false });
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const creating = pathname === "/agent/new";
  const { agent, setNavigationHost, setActionsHost } = useDashboardBreadcrumbs();
  const [groupName, setGroupName] = useState<{ id: string; name: string }>();
  useEffect(() => {
    if (!groupId) return;
    const abort = new AbortController();
    void iam
      .getGroup({ id: groupId }, { signal: abort.signal })
      .then((response) => {
        if (!abort.signal.aborted && response.group)
          setGroupName({ id: groupId, name: response.group.name });
      })
      .catch(() => undefined);
    return () => abort.abort();
  }, [groupId]);
  const section = sections.find(
    (candidate) => pathname === candidate.prefix || pathname.startsWith(`${candidate.prefix}/`),
  ) ?? { prefix: "/", label: "Agent Registry", to: "/" as const };
  const current = creating
    ? "Create agent"
    : pathname === "/api-keys/new"
      ? "Create API key"
      : groupId
        ? groupName?.id === groupId
          ? groupName.name
          : "Loading group…"
        : agentId
          ? agent?.id === agentId
            ? agent.name
            : "Loading agent…"
          : undefined;
  return (
    <header className="flex shrink-0 flex-col border-b">
      <div className="flex min-h-14 min-w-0 w-full items-center gap-x-3 px-4 py-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator orientation="vertical" className="mx-2 h-4 data-vertical:self-auto" />
        <nav aria-label="Breadcrumb" className="min-w-0 max-w-full lg:max-w-[50%]">
          <ol className="flex min-w-0 items-center gap-2 text-sm">
            <li className="shrink-0">
              {current ? (
                <Link
                  to={section.to}
                  className="text-muted-foreground transition-colors hover:text-foreground"
                >
                  {section.label}
                </Link>
              ) : (
                <span aria-current="page" className="font-medium">
                  {section.label}
                </span>
              )}
            </li>
            {current && (
              <>
                <li aria-hidden="true" className="text-muted-foreground">
                  /
                </li>
                <li className="flex min-w-0 items-center gap-2">
                  {!creating && agent && agent.id === agentId && (
                    <span aria-hidden="true" className="shrink-0">
                      <AgentAvatar agent={agent} className="size-5!" />
                    </span>
                  )}
                  <span
                    aria-current="page"
                    title={current}
                    className="min-w-0 truncate font-medium"
                  >
                    {current}
                  </span>
                </li>
              </>
            )}
          </ol>
        </nav>
        <div
          ref={setActionsHost}
          data-slot="dashboard-actions"
          className="ml-auto flex shrink-0 items-center gap-2 empty:hidden"
        />
      </div>
      <div
        ref={setNavigationHost}
        data-slot="dashboard-navigation"
        className="min-w-0 w-full overflow-x-auto border-t px-4 py-1.5 empty:hidden lg:px-6"
      />
    </header>
  );
}
