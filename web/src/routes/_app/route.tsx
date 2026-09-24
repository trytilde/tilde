import { DashboardBreadcrumbProvider } from "@/components/dashboard-breadcrumbs";
import { createFileRoute, Outlet, useRouterState } from "@tanstack/react-router";
import { Auth } from "@/components/auth";
import { CallerProvider } from "@/hooks/use-caller";
import { AppSidebar } from "@/components/app-sidebar";
import { SiteHeader } from "@/components/site-header";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";

export const Route = createFileRoute("/_app")({ component: AppLayout });

function AppLayout() {
  const fullHeight = useRouterState({
    select: (state) =>
      /^\/agent\/[^/]+\/(?:tracing|logs|deployment)$/.test(state.location.pathname),
  });
  return (
    <Auth>
      <CallerProvider>
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
      </CallerProvider>
    </Auth>
  );
}
