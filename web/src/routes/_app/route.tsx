import { DashboardBreadcrumbProvider } from "@/components/dashboard-breadcrumbs";
import { createFileRoute, Outlet, useRouterState } from "@tanstack/react-router";
import { AppSidebar } from "@/components/app-sidebar";
import { SiteHeader } from "@/components/site-header";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";

export const Route = createFileRoute("/_app")({ component: AppLayout });

function AppLayout() {
  const fullHeight = useRouterState({
    select: (state) =>
      /^\/agent\/[^/]+\/(?:tracing|logs|deployment)$/.test(state.location.pathname) ||
      // A skill page lays out its own full-height file tree and scrolling content.
      /^\/skills\/(?!catalog$)[^/]+$/.test(state.location.pathname),
  });
  return (
      <DashboardBreadcrumbProvider>
        <TooltipProvider>
          <SidebarProvider>
            <AppSidebar />
            <SidebarInset
              className={
                fullHeight ? "h-svh min-h-0 overflow-hidden md:h-[calc(100svh-1rem)]" : undefined
              }
            >
              <SiteHeader />
              <main className="flex min-h-0 flex-1 flex-col">
                <Outlet />
              </main>
            </SidebarInset>
          </SidebarProvider>
        </TooltipProvider>
      </DashboardBreadcrumbProvider>
  );
}
