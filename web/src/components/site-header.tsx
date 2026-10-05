import { AgentAvatar } from "./agent-avatar";
import { Link, useParams, useRouterState } from "@tanstack/react-router";
import { Separator } from "@/components/ui/separator";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { useDashboardBreadcrumbs } from "./dashboard-breadcrumbs";
import { Skeleton } from "@/components/ui/skeleton";

/** Top-level sections by path prefix; anything under them gets a second crumb. */
const sections = [
  { prefix: "/skills", label: "Skills", to: "/skills" },
  { prefix: "/tools", label: "Tools", to: "/tools/connections" },
  { prefix: "/sandboxes", label: "Sandboxes", to: "/sandboxes" },
] as const;
export function SiteHeader() {
  const { agentId } = useParams({ strict: false });
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const creating = pathname === "/agent/new";
  const { agent, page, setNavigationHost, setActionsHost } = useDashboardBreadcrumbs();
  const section = sections.find(
    (candidate) => pathname === candidate.prefix || pathname.startsWith(`${candidate.prefix}/`),
  ) ?? { prefix: "/", label: "Agent Registry", to: "/" as const };
  const named = page?.path === pathname ? page : undefined;
  let current: string | null | undefined = creating
    ? "Create agent"
    : pathname === "/tools/remote-servers"
      ? "Remote servers"
      : pathname === "/tools/connections"
        ? "Connections"
        : pathname === "/tools/catalog" || pathname.startsWith("/tools/catalog/")
          ? "Catalog"
          : pathname.startsWith("/skills/") ||
              pathname.startsWith("/tools/") ||
              pathname.startsWith("/sandboxes/")
            ? (named?.label ?? null)
            : agentId
              ? agent?.id === agentId
                ? agent.name
                : null
              : undefined;
  return (
    <header className="flex shrink-0 flex-col border-b">
      <div className="flex min-h-14 min-w-0 w-full items-center gap-x-3 px-4 py-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator orientation="vertical" className="mx-2 h-4 data-vertical:self-auto" />
        <nav aria-label="Breadcrumb" className="min-w-0 max-w-full lg:max-w-[50%]">
          <ol className="flex min-w-0 items-center gap-2 text-sm">
            <li className="shrink-0">
              {current !== undefined ? (
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
            {current !== undefined && named?.parent && (
              <>
                <li aria-hidden="true" className="text-muted-foreground">
                  /
                </li>
                <li className="shrink-0">
                  <Link
                    to={named.parent.to}
                    className="text-muted-foreground transition-colors hover:text-foreground"
                  >
                    {named.parent.label}
                  </Link>
                </li>
              </>
            )}
            {current !== undefined && (
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
                  {current === null ? (
                    <Skeleton className="h-4 w-28" role="status" aria-label="Loading" />
                  ) : (
                    <span
                      aria-current="page"
                      title={current}
                      className="min-w-0 truncate font-medium"
                    >
                      {current}
                    </span>
                  )}
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
