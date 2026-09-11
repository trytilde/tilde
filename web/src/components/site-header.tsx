import { Link, useParams, useRouterState } from "@tanstack/react-router";
import { Separator } from "@/components/ui/separator";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { useDashboardBreadcrumbs } from "./dashboard-breadcrumbs";

export function SiteHeader() {
  const { agentId } = useParams({ strict: false });
  const creating = useRouterState({ select: (state) => state.location.pathname === "/agent/new" });
  const { agent, setNavigationHost } = useDashboardBreadcrumbs();
  const current = creating
    ? "Create agent"
    : agentId
      ? agent?.id === agentId
        ? agent.name
        : "Loading agent…"
      : undefined;
  return (
    <header className="flex min-h-14 shrink-0 items-center gap-2 border-b">
      <div className="flex min-w-0 w-full flex-wrap items-center gap-x-3 gap-y-2 px-4 py-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator orientation="vertical" className="mx-2 h-4 data-vertical:self-auto" />
        <nav aria-label="Breadcrumb" className="min-w-0 max-w-full lg:max-w-[50%]">
          <ol className="flex min-w-0 items-center gap-2 text-sm">
            <li className="shrink-0">
              {current ? (
                <Link
                  to="/"
                  className="text-muted-foreground transition-colors hover:text-foreground"
                >
                  Agent Registry
                </Link>
              ) : (
                <span aria-current="page" className="font-medium">
                  Agent Registry
                </span>
              )}
            </li>
            {current && (
              <>
                <li aria-hidden="true" className="text-muted-foreground">
                  /
                </li>
                <li className="min-w-0">
                  <span aria-current="page" title={current} className="block truncate font-medium">
                    {current}
                  </span>
                </li>
              </>
            )}
          </ol>
        </nav>
        <div
          ref={setNavigationHost}
          data-slot="dashboard-navigation"
          className="min-w-0 max-w-full overflow-x-auto empty:hidden"
        />
      </div>
    </header>
  );
}
